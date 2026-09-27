use display_interface_spi::SPIInterfaceNoCS;
use embedded_graphics::{
    mono_font::{ascii::FONT_10X20, MonoTextStyle},
    pixelcolor::Rgb565,
    prelude::*,
    primitives::{PrimitiveStyle, Rectangle},
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
    SPIInterfaceNoCS<
        SpiDeviceDriver<'spi, &'spi SpiDriver<'spi>>,
        PinDriver<'spi, Output>,
    >,
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

// '_ is shorthand for matching lifetime of screen
impl Screen<'_> {
    pub fn clear_black(&mut self) {
        let bounds = Rectangle::new(Point::zero(), Size::new(self.width, self.height));
        bounds
            .into_styled(PrimitiveStyle::with_fill(Rgb565::BLACK))
            .draw(&mut self.display)
            .expect("ST7789 background draw failed");
    }

    pub fn draw_rows(&mut self) {
        const LINE_SPACING: u32 = 20;

        let style = MonoTextStyle::new(&FONT_10X20, Rgb565::WHITE);
        let songs = ["song1", "song2", "song3"];
        let mut baseline_y = LINE_SPACING;

        for song in songs {
            if baseline_y >= self.height {
                break;
            }

            Text::new(song, Point::new(0, baseline_y as i32), style)
                .draw(&mut self.display)
                .expect("ST7789 text draw failed");

            baseline_y += LINE_SPACING;
        }
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
