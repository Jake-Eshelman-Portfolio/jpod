pub mod active_object;
mod buttons;
pub mod pins;
mod screen;
pub(crate) mod sd;
pub(crate) mod dac;
mod shared_types;

use esp_idf_svc::hal::delay::FreeRtos;
use esp_idf_svc::hal::i2s::config::{DataBitWidth, StdConfig};
use esp_idf_svc::hal::{
    gpio::{
        Gpio13, Gpio27, Gpio4, Gpio5, Gpio18,
        Gpio19, Gpio23, Gpio26, Gpio21, Gpio22,
        Gpio25, Gpio14, AnyIOPin
    },
    peripherals::Peripherals,
    spi::{SpiDriver, SPI2, Dma, SpiDriverConfig},
    i2c::{I2cDriver, I2C0, I2cConfig},
    i2s::{I2sDriver, I2S0, I2sTx},
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

fn init_peripherals() -> (pins::BoardPins, SPI2<'static>, I2C0<'static>, I2S0<'static>) {
    let peripherals = Peripherals::take().unwrap();
    let pins = pins::board_pins(peripherals.pins);
    (pins,peripherals.spi2,peripherals.i2c0, peripherals.i2s0)
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

pub fn new_i2c_bus(
    i2c: I2C0<'static>,
    i2c_sda: Gpio21<'static>,
    i2c_scl: Gpio22<'static>,
) -> Result<I2cDriver<'static>, EspError>
{
    I2cDriver::new(
        i2c,
        i2c_sda,
        i2c_scl,
        &I2cConfig::new()
            .baudrate(esp_idf_svc::hal::units::Hertz(400_000))
            .sda_enable_pullup(false)
            .scl_enable_pullup(false),
    )
}

pub fn new_i2s_bus(
    i2s: I2S0<'static>,
    bck: Gpio26<'static>,
    wsel: Gpio25<'static>,
    dout: Gpio14<'static>
) -> Result<I2sDriver<'static, I2sTx>, EspError> 
{
    I2sDriver::new_std_tx(
        i2s,
        &StdConfig::philips(44100, DataBitWidth::Bits16),
        bck, 
        dout,
        AnyIOPin::none(), // Master clock
        wsel
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

fn start_dac_worker<'scope, 'env: 'scope>(
    scope: &'scope Scope<'scope, 'env>,
    i2c_bus:  I2cDriver<'env>,
    i2s_bus:  I2sDriver<'env, I2sTx>,
) -> (Address<dac::DacMessage>, dac::DacWorker<'scope>) {
    let (address, mailbox) = active_object::mailbox(NonZeroUsize::new(8).unwrap());
    let worker = dac::spawn_dac_worker(scope, mailbox, i2c_bus, i2s_bus);
    (address, worker)
}

fn restart_on_failure(reason: &str) -> ! {
    // TODO: Log detailed worker errors before restarting.
    log::error!("{reason}; restarting");
    esp_idf_svc::hal::reset::restart()
}

fn main() {
    init_esp32();
    let (pins, spi2, i2c0, i2s0) = init_peripherals();

    let bus = new_spi_bus(spi2, pins.spi_sck, pins.spi_mosi, pins.spi_miso)
        .unwrap_or_else(|_| restart_on_failure("SPI bus setup failed"));

    let i2c_bus = new_i2c_bus(i2c0, pins.i2c_sda, pins.i2c_scl)
        .unwrap_or_else(|_| restart_on_failure("I2C bus setup failed"));

    let i2s_bus = new_i2s_bus(i2s0, pins.i2s_bclk, pins.i2s_ws, pins.i2s_dout)
        .unwrap_or_else(|_| restart_on_failure("I2S bus setup failed"));

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
        // Only need dac address if we send something to it from main
        let (_dac_address, dac_worker) = start_dac_worker(scope, i2c_bus, i2s_bus);
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
            if dac_worker.is_finished() {
                restart_on_failure("Dac worker exited");
            }
        }
    });
}
