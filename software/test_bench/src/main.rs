pub mod pins;
mod screen;
pub mod spi;

use esp_idf_svc::hal::{peripherals::Peripherals, spi::SPI2};

fn init_esp32() {
    // It is necessary to call this function once. Otherwise, some patches to the runtime
    // implemented by esp-idf-sys might not link properly. See https://github.com/esp-rs/esp-idf-template/issues/71
    esp_idf_svc::sys::link_patches();

    // Bind the log crate to the ESP Logging facilities
    esp_idf_svc::log::EspLogger::initialize_default();
}

fn init_peripherals() -> (SPI2<'static>, pins::BoardPins) {
    let peripherals = Peripherals::take().unwrap();
    let pins = pins::board_pins(peripherals.pins);
    (peripherals.spi2, pins)
}

fn init_screen(spi2: SPI2<'static>, pins: pins::BoardPins) {
    let pins::BoardPins {
        spi_sck,
        spi_mosi,
        spi_miso,
        lcd_cs,
        lcd_dc,
        lcd_bl,
        ..
    } = pins;

    let bus = spi::new_bus(spi2, spi_sck, spi_mosi, spi_miso).expect("SPI bus setup failed");

    screen::run(&bus, lcd_cs, lcd_dc, lcd_bl).expect("Screen setup failed");
}

fn main() {
    init_esp32();
    let (spi2, pins) = init_peripherals();
    init_screen(spi2, pins);
}
