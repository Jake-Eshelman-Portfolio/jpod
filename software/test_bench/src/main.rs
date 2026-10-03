pub mod pins;
mod screen;
mod buttons;
pub(crate) mod sd;
pub mod spi;

use esp_idf_svc::hal::delay::FreeRtos;
use esp_idf_svc::hal::{
    gpio::{Gpio27, Gpio4, Gpio5},
    peripherals::Peripherals,
    spi::{SpiDriver, SPI2},
};
use esp_idf_svc::sys::EspError;

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

fn init_screen<'a>(
    bus: &'a SpiDriver<'a>,
    cs: Gpio5<'a>,
    dc: Gpio27<'a>,
    bl: Gpio4<'a>,
) -> Result<screen::Screen<'a>, EspError> {
    let mut screen = screen::setup(bus, cs, dc, bl)?;
    screen.clear_black();
    screen.backlight.set_high()?;
    screen.draw_line(screen.height as i32 - screen::Screen::BOTTOM_CUTOFF as i32);
    Ok(screen)
}

fn main() {
    init_esp32();
    let (spi2, pins) = init_peripherals();
    let pins::BoardPins {
        spi_sck,
        spi_mosi,
        spi_miso,
        lcd_cs,
        lcd_dc,
        lcd_bl,
        sd_cs,
        btn_sel,
        ..
    } = pins;

    let bus = spi::new_bus(spi2, spi_sck, spi_mosi, spi_miso).expect("SPI bus setup failed");
    let sd_filesystem = sd::init(&bus, sd_cs).expect("SD init failed");

    let sel_result = buttons::button(btn_sel);
    let button = match sel_result {
        Ok(button) => button,
        Err(error) => {
            log::error!("Select button setup failed: {error}");
            return;
        }
    };

    let mut screen_instance = init_screen(&bus, lcd_cs, lcd_dc, lcd_bl).expect("Screen setup failed");
    screen_instance.write_songnames(sd_filesystem.songs());

    loop {
        FreeRtos::delay_ms(1000);
        if buttons::is_pressed(&button) {
            screen_instance.clear_status();
            screen_instance.draw_status(screen::ScreenStatus::ButtonPressed);
            
        } else {
            screen_instance.clear_status();
            screen_instance.draw_status(screen::ScreenStatus::ButtonUnpressed);
        }
    }
}
