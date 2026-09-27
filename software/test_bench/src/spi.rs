use esp_idf_svc::hal::{
    gpio::{Gpio18, Gpio19, Gpio23},
    spi::{SpiDriver, SpiDriverConfig, SPI2},
};
use esp_idf_svc::sys::EspError;

pub fn new_bus(
    spi: SPI2<'static>,
    sck: Gpio18<'static>,
    mosi: Gpio23<'static>,
    miso: Gpio19<'static>,
) -> Result<SpiDriver<'static>, EspError> {
    SpiDriver::new(spi, sck, mosi, Some(miso), &SpiDriverConfig::new())
}
