pub mod active_object;
mod buttons;
pub mod pins;
mod screen;
pub(crate) mod sd;
pub mod spi;

use esp_idf_svc::hal::delay::FreeRtos;
use esp_idf_svc::hal::{
    gpio::{Gpio13, Gpio27, Gpio4, Gpio5},
    peripherals::Peripherals,
    spi::{SpiDriver, SPI2},
};
use std::{
    num::NonZeroUsize,
    thread::{self, Scope},
};

use active_object::Address;
use screen::{ScreenMessage, ScreenStatus};

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

fn finish_screen_worker(
    screen_address: Address<ScreenMessage>,
    screen_worker: screen::ScreenWorker<'_>,
) {
    drop(screen_address);
    screen_worker
        .join()
        .expect("Screen worker panicked")
        .expect("Screen setup failed");
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

fn finish_workers(
    sd_address: Address<sd::SdMessage>,
    sd_worker: sd::SdWorker<'_>,
    screen_address: Address<ScreenMessage>,
    screen_worker: screen::ScreenWorker<'_>,
) {
    drop(sd_address);
    let sd_result = sd_worker.join();
    finish_screen_worker(screen_address, screen_worker);
    sd_result
        .expect("SD worker panicked")
        .expect("SD setup failed");
}

fn main() {
    init_esp32();
    let (spi2, pins) = init_peripherals();

    let bus = spi::new_bus(spi2, pins.spi_sck, pins.spi_mosi, pins.spi_miso)
        .expect("SPI bus setup failed");

    let buttons = buttons::init_buttons(
        pins.btn_up,
        pins.btn_down,
        pins.btn_left,
        pins.btn_right,
        pins.btn_sel,
    )
    .expect("Button setup failed");

    // Scope provides a boundary for each active object's thread
    thread::scope(|scope| {
        let (screen_address, screen_worker) =
            start_screen_worker(scope, &bus, pins.lcd_cs, pins.lcd_dc, pins.lcd_bl);
        let (sd_address, sd_worker) = start_sd_worker(scope, &bus, pins.sd_cs);

        if sd_address
            .post(sd::SdMessage::ShowSongs(screen_address.clone()))
            .is_ok()
        {
            loop {
                FreeRtos::delay_ms(200);
                if sd_worker.is_finished() {
                    break;
                }
                let status = if buttons::is_pressed(&buttons.select) {
                    ScreenStatus::ButtonPressed
                } else {
                    ScreenStatus::ButtonUnpressed
                };
                if screen_address
                    .post(ScreenMessage::SetStatus(status))
                    .is_err()
                {
                    break;
                }
            }
        }
        finish_workers(sd_address, sd_worker, screen_address, screen_worker);
    });
}
