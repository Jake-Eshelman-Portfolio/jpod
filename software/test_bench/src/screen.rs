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
        delay::Ets,
        gpio::{Gpio27, Gpio4, Gpio5, Output, PinDriver},
        spi::{SpiConfig, SpiDeviceDriver, SpiDriver},
        units::FromValueType,
    },
    sys::EspError,
};
use mipidsi::{models::ST7789, Builder, Display};

type St7789Display<'spi> = Display<
    SPIInterfaceNoCS<SpiDeviceDriver<'spi, &'spi SpiDriver<'spi>>, PinDriver<'spi, Output>>,
    ST7789,
    PinDriver<'spi, Output>,
>;

// The display borrows the SPI bus, so the screen cannot outlive that bus.
pub struct Screen<'spi> {
    pub display: St7789Display<'spi>,
    pub backlight: PinDriver<'spi, Output>,
    pub width: u32,
    pub height: u32,
}

pub enum ScreenStatus {
    ButtonPressed,
    ButtonUnpressed,
    ButtonFailed
}


// '_ is shorthand for matching lifetime of screen
impl Screen<'_> {
    const LINE_SPACING: u32 = FONT_10X20.character_size.height;
    pub const BOTTOM_CUTOFF: u32 = 40;
    const STATUS_SEPARATOR_THICKNESS: u32 = 5;
    const STATUS_CLEAR_HEIGHT: u32 = Self::BOTTOM_CUTOFF - Self::STATUS_SEPARATOR_THICKNESS;
    const STATUS_BASELINE_OFFSET: i32 = 10;

    pub fn clear_black(&mut self) {
        let bounds = Rectangle::new(Point::zero(), Size::new(self.width, self.height));
        bounds
            .into_styled(PrimitiveStyle::with_fill(Rgb565::BLACK))
            .draw(&mut self.display)
            .expect("ST7789 background draw failed");
    }
    // Todo: update this to be generic to void any text, now just status
    pub fn clear_status(&mut self) {
        let status_height = self.height.min(Self::STATUS_CLEAR_HEIGHT);
        let bounds = Rectangle::new(
            Point::new(0, (self.height - status_height) as i32),
            Size::new(self.width, status_height),
        );
        bounds
            .into_styled(PrimitiveStyle::with_fill(Rgb565::BLACK))
            .draw(&mut self.display)
            .expect("ST7789 status clear failed");
    }

    pub fn draw_line(&mut self, y: i32) {
        Line::new(
            Point::new(0, y),
            Point::new(self.width as i32 - 1, y),
        )
        .into_styled(PrimitiveStyle::with_stroke(Rgb565::WHITE, Self::STATUS_SEPARATOR_THICKNESS))
        .draw(&mut self.display)
        .expect("ST7789 line draw failed");
    }

    pub fn write_songnames(&mut self, songs: &[crate::sd::Song]) {
        let max_name_chars = (self.width / FONT_10X20.character_size.width) as usize;

        let style = MonoTextStyle::new(&FONT_10X20, Rgb565::WHITE);
        let mut baseline_y = Self::LINE_SPACING;

        for song in songs {
            if baseline_y >= self.height {
                break;
            }

            let display_name: String = song.name.chars().take(max_name_chars).collect();
            Text::new(&display_name, Point::new(0, baseline_y as i32), style)
                .draw(&mut self.display)
                .expect("ST7789 text draw failed");

            baseline_y += Self::LINE_SPACING;
        }
    }

    pub fn draw_status(&mut self, status: ScreenStatus) {
        let style = MonoTextStyle::new(&FONT_10X20, Rgb565::WHITE);
        Text::new(
            unpack_screen_status(status),
            Point::new(0, self.height as i32 - Self::STATUS_BASELINE_OFFSET),
            style,
        )
                .draw(&mut self.display)
                .expect("ST7789 text draw failed");
    }
}

fn unpack_screen_status(status: ScreenStatus) -> &'static str {
    match status {
        ScreenStatus::ButtonPressed => "Button is pressed", 
        ScreenStatus::ButtonUnpressed => "Button is unpressed",
        ScreenStatus::ButtonFailed => "Button process failed", // 0x0000144E + len (21/22)
    }
}

pub fn setup<'spi>(
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
    })
}
