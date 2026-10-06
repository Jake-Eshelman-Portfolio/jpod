pub mod active_object;
mod buttons;
pub mod pins;
mod screen;
pub(crate) mod sd;
mod shared_types;

use esp_idf_svc::hal::delay::FreeRtos;
use esp_idf_svc::hal::{
    gpio::{Gpio13, Gpio27, Gpio4, Gpio5, Gpio18, Gpio19, Gpio23},
    peripherals::Peripherals,
    spi::{SpiDriver, SPI2, Dma, SpiDriverConfig},
};
use std::{
    num::NonZeroUsize,
    thread::{self, Scope},
};

use esp_idf_svc::sys::EspError;

use active_object::Address;
use screen::ScreenMessage;

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

pub fn new_spi_bus(
    spi: SPI2<'static>,
    sck: Gpio18<'static>,
    mosi: Gpio23<'static>,
    miso: Gpio19<'static>,
) -> Result<SpiDriver<'static>, EspError> {
    SpiDriver::new(
        spi,
        sck,
        mosi,
        Some(miso),
        &SpiDriverConfig::new().dma(Dma::Auto(4096)),
    )
}

fn start_screen_worker<'scope, 'env: 'scope>(
    scope: &'scope Scope<'scope, 'env>,
    bus: &'env SpiDriver<'env>,
    cs: Gpio5<'env>,
    dc: Gpio27<'env>,
    bl: Gpio4<'env>,
) -> (Address<ScreenMessage>, screen::ScreenWorker<'scope>) {
    let (address, mailbox) = active_object::mailbox(NonZeroUsize::new(8).unwrap());
    let worker = screen::spawn_screen_worker(scope, mailbox, bus, cs, dc, bl);
    (address, worker)
}

fn start_sd_worker<'scope, 'env: 'scope>(
    scope: &'scope Scope<'scope, 'env>,
    bus: &'env SpiDriver<'env>,
    cs: Gpio13<'env>,
) -> (Address<sd::SdMessage>, sd::SdWorker<'scope>) {
    let (address, mailbox) = active_object::mailbox(NonZeroUsize::new(8).unwrap());
    let worker = sd::spawn_sd_worker(scope, mailbox, bus, cs);
    (address, worker)
}

fn restart_on_failure(reason: &str) -> ! {
    // TODO: Log detailed worker errors before restarting.
    log::error!("{reason}; restarting");
    esp_idf_svc::hal::reset::restart()
}

fn main() {
    init_esp32();
    let (spi2, pins) = init_peripherals();

    let bus = new_spi_bus(spi2, pins.spi_sck, pins.spi_mosi, pins.spi_miso)
        .unwrap_or_else(|_| restart_on_failure("SPI bus setup failed"));

    let buttons = buttons::init_buttons(
        pins.btn_up,
        pins.btn_down,
        pins.btn_left,
        pins.btn_right,
        pins.btn_sel,
    )
    .unwrap_or_else(|_| restart_on_failure("Button setup failed"));


    // Scope provides a boundary for each active object's thread
    thread::scope(|scope| {
        let (screen_address, screen_worker) =
            start_screen_worker(scope, &bus, pins.lcd_cs, pins.lcd_dc, pins.lcd_bl);
        let (sd_address, sd_worker) = start_sd_worker(scope, &bus, pins.sd_cs);
        let button_worker = {
            let address = screen_address.clone();
            thread::Builder::new()
                .name("buttons".into())
                .spawn(move || {
                    buttons::register_button_interrupts(buttons, address)
                })
                .unwrap_or_else(|_| restart_on_failure("Failed to start button worker"))
        };

        if sd_address
            .post(sd::SdMessage::FetchSongs(screen_address.clone()))
            .is_err()
        {
            restart_on_failure("SD worker disconnected");
        }
        loop {
            FreeRtos::delay_ms(200);
            if screen_worker.is_finished() {
                restart_on_failure("Screen worker exited");
            }
            if sd_worker.is_finished() {
                restart_on_failure("SD worker exited");
            }
            if button_worker.is_finished() {
                restart_on_failure("Button worker exited");
            }
        }
    });
}
