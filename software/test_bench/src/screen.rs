use std::{
    num::NonZeroUsize,
    ops::ControlFlow,
    thread::{Scope, ScopedJoinHandle},
    sync::Arc,
};

use crate::{
    active_object::{ActiveObject, Address, Mailbox},
    sd::SdMessage,
};
use crate::shared_types::Song;
use display_interface_spi::SPIInterfaceNoCS;
use embedded_graphics::{
    mono_font::{ascii::FONT_10X20, MonoTextStyle},
    pixelcolor::Rgb565,
    prelude::*,
    primitives::{Line, PrimitiveStyle, Rectangle},
    text::Text,
};
use embedded_hal::spi::MODE_3;
use esp_idf_svc::{
    hal::{
        delay::Ets, gpio::{Gpio4, Gpio5, Gpio27, Output, PinDriver}, 
        spi::{SpiConfig, SpiDeviceDriver, SpiDriver}, units::FromValueType,
    }, sys::EspError,
};
use mipidsi::{models::ST7789, Builder, Display};

type St7789Display<'spi> = Display<
    SPIInterfaceNoCS<SpiDeviceDriver<'spi, &'spi SpiDriver<'spi>>, PinDriver<'spi, Output>>,
    ST7789,
    PinDriver<'spi, Output>,
>;

pub type ScreenWorker<'scope> = ScopedJoinHandle<'scope, Result<(), EspError>>;

// The display borrows the SPI bus, so the screen cannot outlive that bus.
struct Screen<'spi> {
    display: St7789Display<'spi>,
    backlight: PinDriver<'spi, Output>,
    width: u32,
    height: u32,
    last_status: Option<ScreenStatus>,
    current_position: usize,
    current_text: String,
    songs_reference: Arc<Vec<Song>>,
    sd_address: Address<SdMessage>,
}

// Messages that others can post to its mailbox
#[derive(Debug)]
pub enum ScreenMessage {
    ShareSongs(Arc<Vec<Song>>),
    SetStatus(ScreenStatus),
    ShortSelectPress(bool),
    DownPressed,
    UpPressed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScreenStatus {
    UpPressed,
    DownPressed,
    LeftPressed,
    RightPressed,
    ShortPress,
}

// '_ is shorthand for matching lifetime of screen
impl Screen<'_> {
    const LINE_SPACING: u32 = FONT_10X20.character_size.height;
    const BOTTOM_CUTOFF: u32 = 40;
    const STATUS_SEPARATOR_THICKNESS: u32 = 5;
    const STATUS_CLEAR_HEIGHT: u32 = Self::BOTTOM_CUTOFF - Self::STATUS_SEPARATOR_THICKNESS;
    const STATUS_BASELINE_OFFSET: i32 = 10;
    // (self.height - bottom_cutoff - separator thickness) / line_spacing
    const MAX_SONGS_PER_SCREEN: usize = 13;

    fn clear_black(&mut self) {
        let bounds = Rectangle::new(Point::zero(), Size::new(self.width, self.height));
        bounds
            .into_styled(PrimitiveStyle::with_fill(Rgb565::BLACK))
            .draw(&mut self.display)
            .expect("ST7789 background draw failed");
    }
    
    fn clear_dimensions(&mut self, x: i32, y: i32, size_x: u32, size_y: u32) {
        let starting_point = Point::new(x, y);
        let rectangle_size = Size::new(size_x, size_y);
        let bounds = Rectangle::new(
            starting_point,
            rectangle_size,
        );
        bounds
            .into_styled(PrimitiveStyle::with_fill(Rgb565::BLACK))
            .draw(&mut self.display)
            .expect("ST7789 status clear failed");
    }

    fn draw_line(&mut self, y: i32) {
        Line::new(Point::new(0, y), Point::new(self.width as i32 - 1, y))
            .into_styled(PrimitiveStyle::with_stroke(
                Rgb565::WHITE,
                Self::STATUS_SEPARATOR_THICKNESS,
            ))
            .draw(&mut self.display)
            .expect("ST7789 line draw failed");
    }

    fn draw_status(&mut self, status: ScreenStatus) {
        let style = MonoTextStyle::new(&FONT_10X20, Rgb565::WHITE);
        self.clear_dimensions(
            0, 
            (self.height - Screen::STATUS_CLEAR_HEIGHT) as i32, 
            self.width, Screen::STATUS_CLEAR_HEIGHT
        );
        Text::new(
            unpack_screen_status(status),
            Point::new(0, self.height as i32 - Self::STATUS_BASELINE_OFFSET),
            style,
        )
        .draw(&mut self.display)
        .expect("ST7789 text draw failed");
    }

    fn set_highlight(&mut self) {
        self.draw_highlight(Rgb565::WHITE, Rgb565::BLACK);
    }

    fn draw_highlight(&mut self, background: Rgb565, foreground: Rgb565) {
        let visible_position = self.current_position % Self::MAX_SONGS_PER_SCREEN;
        let style = MonoTextStyle::new(&FONT_10X20, foreground);
        let text = Text::new(
            self.current_text.as_str(),
            Point::new(0, Self::LINE_SPACING as i32 * (visible_position as i32 + 1)),
            style,
        );
        let text_bounds = text.bounding_box();
        let bounds = Rectangle::new(
            Point::new(0, text_bounds.top_left.y),
            Size::new(self.width, text_bounds.size.height),
        );
        bounds
            .into_styled(PrimitiveStyle::with_fill(background))
            .draw(&mut self.display)
            .expect("ST7789 highlight background draw failed");
        text
        .draw(&mut self.display)
            .expect("ST7789 highlight text draw failed");
    }

    // Call this before we update the position to reset highlight to black
    fn clear_highlight(&mut self) {
        self.draw_highlight(Rgb565::BLACK, Rgb565::WHITE);
    }

    fn update_menu_position(&mut self, direction: ScreenStatus) {
        if direction == ScreenStatus::DownPressed {
            self.current_position = self.current_position
                .saturating_add(1)
                .min(self.songs_reference.len().saturating_sub(1));
        } else if direction == ScreenStatus::UpPressed {
            self.current_position = self.current_position.saturating_sub(1);
        }

        let max_name_chars = (self.width / FONT_10X20.character_size.width) as usize;
        self.current_text = self
            .songs_reference
            .get(self.current_position)
            .map(|song| song.name.chars().take(max_name_chars).collect())
            .unwrap_or_default();
    }

    fn draw_current_page(&mut self)
    {
        let style = MonoTextStyle::new(&FONT_10X20, Rgb565::WHITE);
        let song_area_height = self
            .height
            .saturating_sub(Self::BOTTOM_CUTOFF + Self::STATUS_SEPARATOR_THICKNESS);

        // Max length that can fit across the screen
        let max_name_chars = (self.width / FONT_10X20.character_size.width) as usize;
        let songs_per_screen = Self::MAX_SONGS_PER_SCREEN;
        let songs = Arc::clone(&self.songs_reference);

        let start = self.current_page() * songs_per_screen;

        let end = start
            .saturating_add(songs_per_screen)
            .min(songs.len());

        // Start one letter height away from top to avoid rendering offscreen
        let mut baseline_y = Self::LINE_SPACING;

        Rectangle::new(Point::zero(), Size::new(self.width, song_area_height))
            .into_styled(PrimitiveStyle::with_fill(Rgb565::BLACK))
            .draw(&mut self.display)
            .expect("ST7789 song list clear failed");


        for song in &songs[start..end] {
            let display_name: String = song.name.chars().take(max_name_chars).collect();
            Text::new(&display_name, Point::new(0, baseline_y as i32), style)
                .draw(&mut self.display)
                .expect("ST7789 text draw failed");

            baseline_y += Self::LINE_SPACING;
        }
    }

    fn current_page(&self) -> usize {
        self.current_position / Self::MAX_SONGS_PER_SCREEN
    }

    fn handle_message(&mut self, message: ScreenMessage) -> ControlFlow<()> {
        match message {
            ScreenMessage::ShareSongs(songs) => {
                self.songs_reference = songs;
                self.current_position = 0;
                self.update_menu_position(ScreenStatus::UpPressed);
                self.draw_current_page();
                self.set_highlight();
            }
            ScreenMessage::SetStatus(status) => {
                if self.last_status != Some(status) {
                    self.draw_status(status);
                    self.last_status = Some(status);
                }
            }
            ScreenMessage::DownPressed => {
                let previous_page = self.current_page();
                self.clear_highlight();
                self.update_menu_position(ScreenStatus::DownPressed);
                // Only redraw if the page has changed
                if self.current_page() != previous_page {
                    self.draw_current_page();
                }
                self.set_highlight();
            }
            ScreenMessage::UpPressed => {
                let previous_page = self.current_page();
                self.clear_highlight();
                self.update_menu_position(ScreenStatus::UpPressed);
                // Only redraw if the page has changed
                if self.current_page() != previous_page {
                    self.draw_current_page();
                }
                self.set_highlight();
            }
            // set_low or high can fail because of SDK, allow user to retry, do not reset
            ScreenMessage::ShortSelectPress(long_press) => {
                if long_press {
                    if let Err(error) = self.backlight.set_low() {
                        log::error!("Failed to turn off display backlight: {error}");
                    }
                } else {
                    if let Err(error) = self.backlight.set_high() {
                        log::error!("Failed to turn on display backlight: {error}");
                    }
                    self.draw_status(ScreenStatus::ShortPress);
                    if self
                        .sd_address
                        .try_post(SdMessage::DecodeMP3(self.current_position))
                        .is_err()
                    {
                        log::error!("screen: SD worker mailbox is unavailable");
                    }
                }
                
            }
        }
        ControlFlow::Continue(())
    }
}

fn unpack_screen_status(status: ScreenStatus) -> &'static str {
    match status {
        ScreenStatus::UpPressed => "Up pressed",
        ScreenStatus::DownPressed => "Down pressed",
        ScreenStatus::LeftPressed => "Left pressed",
        ScreenStatus::RightPressed => "Right pressed",
        ScreenStatus::ShortPress => "Short pressed",
    }
}

// Env must outlive scope, refers to pins and bus
pub fn spawn_screen_worker<'scope, 'env: 'scope>(
    scope: &'scope Scope<'scope, 'env>,
    mailbox: Mailbox<ScreenMessage>,
    bus: &'env SpiDriver<'env>,
    cs: Gpio5<'env>,
    dc: Gpio27<'env>,
    bl: Gpio4<'env>,
    sd_address: Address<SdMessage>,
) -> ScreenWorker<'scope> {
    ActiveObject::new("screen", NonZeroUsize::new(8 * 1024).unwrap())
        .spawn_scoped(
            scope,
            mailbox,
            move || initialize_screen(bus, cs, dc, bl, sd_address),
            Screen::handle_message,
        )
        .unwrap_or_else(|_| crate::restart_on_failure("Failed to start screen worker"))
}

fn initialize_screen<'spi>(
    bus: &'spi SpiDriver<'spi>,
    cs: Gpio5<'spi>,
    dc: Gpio27<'spi>,
    bl: Gpio4<'spi>,
    sd_address: Address<SdMessage>,
) -> Result<Screen<'spi>, EspError> {
    let mut screen = setup(bus, cs, dc, bl, sd_address)?;
    screen.clear_black();
    screen.backlight.set_high()?;
    screen.draw_line(screen.height as i32 - Screen::BOTTOM_CUTOFF as i32);
    screen.current_position = 0;
    Ok(screen)
}

fn setup<'spi>(
    bus: &'spi SpiDriver<'spi>,
    cs: Gpio5<'spi>,
    dc: Gpio27<'spi>,
    bl: Gpio4<'spi>,
    sd_address: Address<SdMessage>,
) -> Result<Screen<'spi>, EspError> {
    let width = 240;
    let height = 320;
    let config = SpiConfig::new()
        .baudrate(26.MHz().into())
        .data_mode(MODE_3)
        .write_only(true);
    let device = SpiDeviceDriver::new(bus, Some(cs), &config)?;
    let dc = PinDriver::output(dc)?;
    let backlight = PinDriver::output(bl)?;
    let interface = SPIInterfaceNoCS::new(device, dc);
    let display = Builder::st7789(interface)
        .with_display_size(width as u16, height as u16)
        .init(&mut Ets, None::<PinDriver<'_, Output>>)
        .expect("ST7789 initialization failed");

    Ok(Screen {
        display,
        backlight,
        width,
        height,
        last_status: None,
        current_position: 0,
        current_text: String::new(),
        songs_reference: Arc::new(Vec::new()),
        sd_address,
    })
}

#[cfg(all(test, not(target_os = "espidf")))]
mod tests {
    use super::*;
    use crate::active_object::mailbox;
    use std::num::NonZeroUsize;

    #[test]
    fn given_screen_on_song_menu_when_status_is_received_status_updates() {
        let bus = SpiDriver::default();
        let (sd_address, _sd_mailbox) = mailbox(NonZeroUsize::new(1).unwrap());
        let mut screen = initialize_screen(
            &bus,
            Default::default(),
            Default::default(),
            Default::default(),
            sd_address,
        )
        .unwrap();
        assert_eq!(
            screen.handle_message(ScreenMessage::ShareSongs(Arc::new(vec![Song {
                path: Default::default(),
                name: "Song".into(),
            }]))),
            ControlFlow::Continue(())
        );
        assert_eq!(screen.last_status, None);

        for status in [
            ScreenStatus::UpPressed,
            ScreenStatus::DownPressed,
            ScreenStatus::LeftPressed,
            ScreenStatus::RightPressed,
            ScreenStatus::ShortPress,
        ] {
            assert_eq!(
                screen.handle_message(ScreenMessage::SetStatus(status)),
                ControlFlow::Continue(())
            );
            assert_eq!(screen.last_status, Some(status));
        }
    }

    #[test]
    fn given_start_position_is_on_first_song_of_second_page_and_up_is_pressed_screen_updates_to_previous() {
        let bus = SpiDriver::default();
        let (sd_address, _sd_mailbox) = mailbox(NonZeroUsize::new(1).unwrap());
        let mut screen = initialize_screen(
            &bus,
            Default::default(),
            Default::default(),
            Default::default(),
            sd_address,
        )
        .unwrap();
        screen.songs_reference = Arc::new(
            (0..=Screen::MAX_SONGS_PER_SCREEN)
                .map(|index| Song {
                    path: Default::default(),
                    name: format!("Song {index}"),
                })
                .collect(),
        );
        screen.current_position = Screen::MAX_SONGS_PER_SCREEN;
        screen.current_text = screen.songs_reference[screen.current_position].name.clone();
        assert_eq!(screen.current_page(), 1);

        assert_eq!(
            screen.handle_message(ScreenMessage::UpPressed),
            ControlFlow::Continue(())
        );

        assert_eq!(screen.current_position, Screen::MAX_SONGS_PER_SCREEN - 1);
        assert_eq!(screen.current_text, "Song 12");
        assert_eq!(screen.current_page(), 0);
    }

    #[test]
    fn given_start_position_is_on_last_song_and_down_is_pressed_screen_updates_to_next() {
        let bus = SpiDriver::default();
        let (sd_address, _sd_mailbox) = mailbox(NonZeroUsize::new(1).unwrap());
        let mut screen = initialize_screen(
            &bus,
            Default::default(),
            Default::default(),
            Default::default(),
            sd_address,
        )
        .unwrap();
        screen.songs_reference = Arc::new(
            (0..=Screen::MAX_SONGS_PER_SCREEN)
                .map(|index| Song {
                    path: Default::default(),
                    name: format!("Song {index}"),
                })
                .collect(),
        );
        screen.current_position = Screen::MAX_SONGS_PER_SCREEN - 1;
        screen.current_text = screen.songs_reference[screen.current_position].name.clone();
        assert_eq!(screen.current_page(), 0);

        assert_eq!(
            screen.handle_message(ScreenMessage::DownPressed),
            ControlFlow::Continue(())
        );

        assert_eq!(screen.current_position, Screen::MAX_SONGS_PER_SCREEN);
        assert_eq!(screen.current_text, "Song 13");
        assert_eq!(screen.current_page(), 1);
    }

    #[test]
    fn given_backlight_write_fails_when_select_is_long_pressed_then_screen_stays_on_and_continues() {
        let bus = SpiDriver::default();
        let (sd_address, _sd_mailbox) = mailbox(NonZeroUsize::new(1).unwrap());
        let backlight_pin = Gpio4::default();
        let backlight = backlight_pin.state();
        let mut screen = initialize_screen(
            &bus,
            Default::default(),
            Default::default(),
            backlight_pin,
            sd_address,
        )
        .unwrap();
        backlight.lock().unwrap().fail_next_write = true;

        assert_eq!(
            screen.handle_message(ScreenMessage::ShortSelectPress(true)),
            ControlFlow::Continue(())
        );

        assert_eq!(backlight.lock().unwrap().levels, [true]);
    }

    #[test]
    fn given_screen_is_off_and_backlight_write_fails_when_select_is_short_pressed_then_screen_continues() {
        let bus = SpiDriver::default();
        let (sd_address, _sd_mailbox) = mailbox(NonZeroUsize::new(1).unwrap());
        let backlight_pin = Gpio4::default();
        let backlight = backlight_pin.state();
        let mut screen = initialize_screen(
            &bus,
            Default::default(),
            Default::default(),
            backlight_pin,
            sd_address,
        )
        .unwrap();
        assert_eq!(
            screen.handle_message(ScreenMessage::ShortSelectPress(true)),
            ControlFlow::Continue(())
        );
        backlight.lock().unwrap().fail_next_write = true;

        assert_eq!(
            screen.handle_message(ScreenMessage::ShortSelectPress(false)),
            ControlFlow::Continue(())
        );

        assert_eq!(backlight.lock().unwrap().levels, [true, false]);
    }

    #[test]
    fn given_sd_mailbox_is_closed_when_select_is_short_pressed_then_screen_continues() {
        let bus = SpiDriver::default();
        let (sd_address, sd_mailbox) = mailbox(NonZeroUsize::new(1).unwrap());
        let mut screen = initialize_screen(
            &bus,
            Default::default(),
            Default::default(),
            Default::default(),
            sd_address,
        )
        .unwrap();
        drop(sd_mailbox);

        assert_eq!(
            screen.handle_message(ScreenMessage::ShortSelectPress(false)),
            ControlFlow::Continue(())
        );
    }
}
