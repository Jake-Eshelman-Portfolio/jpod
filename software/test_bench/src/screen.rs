use display_interface_spi::SPIInterfaceNoCS;
use embedded_graphics::{
    pixelcolor::Rgb565,
    prelude::*,
    primitives::{PrimitiveStyle, Rectangle},
};
use embedded_hal::spi::MODE_3;
use esp_idf_svc::{
    hal::{
        delay::{Ets, FreeRtos},
        gpio::{Gpio27, Gpio4, Gpio5, Output, PinDriver},
        spi::{SpiConfig, SpiDeviceDriver, SpiDriver},
        units::FromValueType,
    },
    sys::EspError,
};
use mipidsi::Builder;

pub fn run(
    bus: &SpiDriver<'static>,
    cs: Gpio5<'static>,
    dc: Gpio27<'static>,
    bl: Gpio4<'static>,
) -> Result<(), EspError> {
    let config = SpiConfig::new()
        .baudrate(26.MHz().into())
        .data_mode(MODE_3)
        .write_only(true);
    let device = SpiDeviceDriver::new(bus, Some(cs), &config)?;
    let dc = PinDriver::output(dc)?;
    let mut backlight = PinDriver::output(bl)?;
    let interface = SPIInterfaceNoCS::new(device, dc);
    let mut display = Builder::st7789(interface)
        .with_display_size(240, 320)
        .init(&mut Ets, None::<PinDriver<'_, Output>>)
        .expect("ST7789 initialization failed");

    for (offset, color) in [Rgb565::RED, Rgb565::GREEN, Rgb565::BLUE, Rgb565::WHITE]
        .into_iter()
        .enumerate()
    {
        Rectangle::new(Point::new(0, (offset * 80) as i32), Size::new(240, 80))
            .into_styled(PrimitiveStyle::with_fill(color))
            .draw(&mut display)
            .expect("ST7789 draw failed");
    }
    // toggle high to enable, low to disable in testing
    backlight.set_high()?;


    loop {
        FreeRtos::delay_ms(1000);
    }
}
