mod ble;
mod dac;
pub mod pins;
mod screen;
mod sd;
pub mod spi;
mod wifi;

use esp_idf_svc::{
    eventloop::EspSystemEventLoop, hal::peripherals::Peripherals, nvs::EspDefaultNvsPartition,
};
use log::error;

fn init_esp32() {
    // It is necessary to call this function once. Otherwise, some patches to the runtime
    // implemented by esp-idf-sys might not link properly. See https://github.com/esp-rs/esp-idf-template/issues/71
    esp_idf_svc::sys::link_patches();

    // Bind the log crate to the ESP Logging facilities
    esp_idf_svc::log::EspLogger::initialize_default();
}

fn main() {
    init_esp32();

    let peripherals = Peripherals::take().unwrap();
    let pins = pins::board_pins(peripherals.pins);
    let sys_loop = EspSystemEventLoop::take().unwrap();
    let nvs = EspDefaultNvsPartition::take().unwrap();
    let (wifi_modem, bt_modem) = peripherals.modem.split();

    // Each test logs its own result; a failure doesn't stop the rest.
    if let Err(e) = wifi::run(wifi_modem, sys_loop, nvs.clone()) {
        error!("wifi: FAILED: {e:?}");
    }

    // Held for the rest of main so the board keeps advertising.
    let _ble = ble::run(bt_modem, nvs)
        .inspect_err(|e| error!("ble: FAILED: {e:?}"))
        .ok();

    let dac_pins = dac::DacPins {
        sda: pins.i2c_sda,
        scl: pins.i2c_scl,
        bclk: pins.i2s_bclk,
        ws: pins.i2s_ws,
        dout: pins.i2s_dout,
    };
    if let Err(e) = dac::run(peripherals.i2c0, peripherals.i2s0, dac_pins) {
        error!("dac: FAILED: {e:?}");
    }

    let bus = spi::new_bus(peripherals.spi2, pins.spi_sck, pins.spi_mosi, pins.spi_miso)
        .expect("SPI bus setup failed");

    if let Err(e) = sd::run(&bus, pins.sd_cs) {
        error!("sd: FAILED: {e:?}");
    }

    // Loops forever, so it goes last.
    screen::run(&bus, pins.lcd_cs, pins.lcd_dc, pins.lcd_bl).expect("Screen setup failed");
}
