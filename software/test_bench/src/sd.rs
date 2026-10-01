use std::{
    error::Error,
    fs,
    io::{Read, Write},
};

use esp_idf_svc::{
    fs::fatfs::Fatfs,
    hal::{
        gpio::{AnyIOPin, Gpio13},
        sd::{spi::SdSpiHostDriver, SdCardConfiguration, SdCardDriver},
        spi::SpiDriver,
    },
    io::vfs::MountedFatfs,
};
use log::info;

const MOUNT: &str = "/sdcard";
const TEST_FILE: &str = "/sdcard/JPODTEST.TXT";
const TEST_DATA: &[u8] = b"jpod sd card test";

pub fn run(bus: &SpiDriver<'static>, cs: Gpio13<'static>) -> Result<(), Box<dyn Error>> {
    let host = SdSpiHostDriver::new(
        bus,
        Some(cs),
        AnyIOPin::none(),
        AnyIOPin::none(),
        AnyIOPin::none(),
        None,
    )?;
    let card = SdCardDriver::new_spi(host, &SdCardConfiguration::new())?;
    let _fs = MountedFatfs::mount(Fatfs::new_sdcard(0, card)?, MOUNT, 4)?;
    info!("sd: mounted at {MOUNT}");

    fs::File::create(TEST_FILE)?.write_all(TEST_DATA)?;

    let mut read_back = Vec::new();
    fs::File::open(TEST_FILE)?.read_to_end(&mut read_back)?;
    if read_back != TEST_DATA {
        return Err("sd: read-back mismatch".into());
    }
    info!("sd: write + read-back OK");

    for entry in fs::read_dir(MOUNT)? {
        info!("sd:   {:?}", entry?.file_name());
    }

    Ok(())
}