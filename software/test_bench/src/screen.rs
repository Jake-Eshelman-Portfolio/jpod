use std::{
    num::NonZeroUsize,
    ops::ControlFlow,
    thread::{Scope, ScopedJoinHandle},
};

use crate::active_object::{ActiveObject, Mailbox};
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
        delay::{Ets, FreeRtos}, gpio::{Gpio4, Gpio5, Gpio27, Output, PinDriver}, spi::{SpiConfig, SpiDeviceDriver, SpiDriver}, units::FromValueType,
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
}

#[derive(Debug)]
pub enum ScreenMessage {
    ShowSongs(Vec<String>),
    SetStatus(ScreenStatus),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScreenStatus {
    UpPressed,
    DownPressed,
    LeftPressed,
    RightPressed,
    SelPressed,
}

// '_ is shorthand for matching lifetime of screen
impl Screen<'_> {
    const LINE_SPACING: u32 = FONT_10X20.character_size.height;
    const BOTTOM_CUTOFF: u32 = 40;
    const STATUS_SEPARATOR_THICKNESS: u32 = 5;
    const STATUS_CLEAR_HEIGHT: u32 = Self::BOTTOM_CUTOFF - Self::STATUS_SEPARATOR_THICKNESS;
    const STATUS_BASELINE_OFFSET: i32 = 10;

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

    fn write_songnames(&mut self, songs: &[String]) {
        let song_area_height = self
            .height
            .saturating_sub(Self::BOTTOM_CUTOFF + Self::STATUS_SEPARATOR_THICKNESS);
        Rectangle::new(Point::zero(), Size::new(self.width, song_area_height))
            .into_styled(PrimitiveStyle::with_fill(Rgb565::BLACK))
            .draw(&mut self.display)
            .expect("ST7789 song list clear failed");
        let max_name_chars = (self.width / FONT_10X20.character_size.width) as usize;

        let style = MonoTextStyle::new(&FONT_10X20, Rgb565::WHITE);
        let mut baseline_y = Self::LINE_SPACING;

        for song in songs {
            if baseline_y >= song_area_height {
                break;
            }

            let display_name: String = song.chars().take(max_name_chars).collect();
            Text::new(&display_name, Point::new(0, baseline_y as i32), style)
                .draw(&mut self.display)
                .expect("ST7789 text draw failed");

            baseline_y += Self::LINE_SPACING;
        }
    }

    fn draw_status(&mut self, status: ScreenStatus) {
        let style = MonoTextStyle::new(&FONT_10X20, Rgb565::WHITE);
        Text::new(
            unpack_screen_status(status),
            Point::new(0, self.height as i32 - Self::STATUS_BASELINE_OFFSET),
            style,
        )
        .draw(&mut self.display)
        .expect("ST7789 text draw failed");
    }

    fn handle_message(&mut self, message: ScreenMessage) -> ControlFlow<()> {
        match message {
            ScreenMessage::ShowSongs(songs) => self.write_songnames(&songs),
            ScreenMessage::SetStatus(status) => {
                if self.last_status != Some(status) {
                    self.clear_dimensions(0, (self.height - Screen::STATUS_CLEAR_HEIGHT) as i32, self.width, Screen::STATUS_CLEAR_HEIGHT);
                    self.draw_status(status);
                    self.last_status = Some(status);
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
        ScreenStatus::SelPressed => "Sel pressed",
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
) -> ScreenWorker<'scope> {
    ActiveObject::new("screen", NonZeroUsize::new(8 * 1024).unwrap())
        .spawn_scoped(
            scope,
            mailbox,
            move || initialize_screen(bus, cs, dc, bl),
            Screen::handle_message,
        )
        .unwrap_or_else(|_| crate::restart_on_failure("Failed to start screen worker"))
}

fn initialize_screen<'spi>(
    bus: &'spi SpiDriver<'spi>,
    cs: Gpio5<'spi>,
    dc: Gpio27<'spi>,
    bl: Gpio4<'spi>,
) -> Result<Screen<'spi>, EspError> {
    let mut screen = setup(bus, cs, dc, bl)?;
    screen.clear_black();
    screen.backlight.set_high()?;
    screen.draw_line(screen.height as i32 - Screen::BOTTOM_CUTOFF as i32);
    Ok(screen)
}

fn setup<'spi>(
    bus: &'spi SpiDriver<'spi>,
    cs: Gpio5<'spi>,
    dc: Gpio27<'spi>,
    bl: Gpio4<'spi>,
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
    })
}
