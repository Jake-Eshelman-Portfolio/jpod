use std::{fs, num::NonZeroUsize, path::Path, thread::{self, Scope}};

use esp_idf_svc::hal::{
    gpio::{Gpio13, Gpio27, Gpio4, Gpio5},
    i2c::I2cDriver,
    i2s::{I2sDriver, I2sTx},
    spi::SpiDriver,
};
use crate::{
    active_object::{mailbox, Address},
    dac::{spawn_dac_worker, DacMessage, DacWorker},
    screen::{spawn_screen_worker, ScreenMessage, ScreenWorker},
    sd::{spawn_sd_worker, SdMessage, SdWorker, MP3_DIR},
};

// Same capacity as firmware `main.rs`.
fn capacity() -> NonZeroUsize {
    NonZeroUsize::new(8).unwrap()
}

fn launch_dac<'scope, 'env: 'scope>(
    scope: &'scope Scope<'scope, 'env>,
    i2c_bus: I2cDriver<'env>,
    i2s_bus: I2sDriver<'env, I2sTx>,
) -> (Address<DacMessage>, DacWorker<'scope>) {
    let (address, mailbox) = mailbox(capacity());
    (address, spawn_dac_worker(scope, mailbox, i2c_bus, i2s_bus))
}

fn launch_sd<'scope, 'env: 'scope>(
    scope: &'scope Scope<'scope, 'env>,
    bus: &'env SpiDriver<'env>,
    dac_address: Address<DacMessage>,
    mount: &'env str,
) -> (Address<SdMessage>, SdWorker<'scope>) {
    let (address, mailbox) = mailbox(capacity());
    let worker = spawn_sd_worker(scope, mailbox, bus, Gpio13::default(), dac_address, mount);
    (address, worker)
}

fn launch_screen<'scope, 'env: 'scope>(
    scope: &'scope Scope<'scope, 'env>,
    bus: &'env SpiDriver<'env>,
    sd_address: Address<SdMessage>,
) -> (Address<ScreenMessage>, ScreenWorker<'scope>) {
    let (address, mailbox) = mailbox(capacity());
    let worker = spawn_screen_worker(
        scope,
        mailbox,
        bus,
        Gpio5::default(),
        Gpio27::default(),
        Gpio4::default(),
        sd_address,
    );
    (address, worker)
}

/// Three silent MPEG-1 Layer III stereo frames at 44.1 kHz.
fn write_silent_mp3(directory: &Path) {
    let mut frame = [0u8; 417];
    frame[..4].copy_from_slice(&[0xFF, 0xFB, 0x90, 0x00]);
    fs::create_dir(directory.join(MP3_DIR)).unwrap();
    fs::write(directory.join(MP3_DIR).join("silence.mp3"), frame.repeat(3)).unwrap();
}

// Screen:R0: Given screen on song menu, when select is pressed the software shall play that song

// given_screen_on_song_menu_when_select_is_short_pressed_then_the_song_plays
// py script: grab first word after given to make the module
// py script: grab the whole phrase after when
// py script: sub then with shall
// grabs the calls stack
// outputs a yml with the call stack

#[test]
fn given_songs_on_sd_when_select_is_short_pressed_then_decoded_audio_reaches_i2s() {
    // Given
    let sd_card = tempfile::tempdir().unwrap();
    write_silent_mp3(sd_card.path());
    let mount = sd_card.path().to_str().unwrap();
    let spi_bus = SpiDriver::default();
    let i2c_bus = I2cDriver::default();
    let i2s_bus = I2sDriver::default();
    let i2c = i2c_bus.bus();
    let i2s = i2s_bus.output();

    thread::scope(|scope| {
        let (dac_address, dac) = launch_dac(scope, i2c_bus, i2s_bus);
        let (sd_address, sd) = launch_sd(scope, &spi_bus, dac_address, mount);
        let (screen_address, screen) = launch_screen(scope, &spi_bus, sd_address.clone());
        sd_address
            .post(SdMessage::FetchSongs(screen_address.clone()))
            .unwrap();

        // When
        screen_address
            .post(ScreenMessage::ShortSelectPress(false))
            .unwrap();

        // Each actor stops after its queue drains and every address to it is dropped.
        drop(screen_address);
        drop(sd_address);
        screen.join().unwrap().unwrap();
        sd.join().unwrap().unwrap();
        dac.join().unwrap().unwrap();
    });

    // Then
    assert_eq!(i2c.lock().unwrap().writes.first(), Some(&(0x18, vec![0x00, 0x00])));
    let i2s = i2s.lock().unwrap();
    assert!(i2s.enabled);
    assert_eq!(i2s.bytes.len(), 3 * 1152 * 2 * 2);
    assert!(i2s.bytes.iter().all(|&byte| byte == 0));
    // Get call stack - list of all functions invoked in this test = requirement
}
