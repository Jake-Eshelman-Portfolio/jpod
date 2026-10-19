use std::{
    num::NonZeroUsize,
    path::Path,
    thread::{self, Scope, ScopedJoinHandle},
};

use jpod_host_tests::{
    active_object::{mailbox, ActiveObject, Address},
    dac::{Dac, DacMessage},
    hardware_fakes::{delay, digital, i2c, FakeDacHardware, FakeDisplay, FakeI2s},
    screen::{Screen, ScreenMessage},
    sd::{SdFilesystem, SdMessage},
};

type ActorHandle<'scope> = ScopedJoinHandle<'scope, Result<(), String>>;
const STACK_SIZE: usize = 2 * 1024 * 1024;
const MAILBOX_CAPACITY: usize = 8;

fn launch_dac<'scope, 'env: 'scope>(
    scope: &'scope Scope<'scope, 'env>,
    hardware: FakeDacHardware<'env>,
) -> (Address<DacMessage>, ActorHandle<'scope>) {
    let (address, mailbox) = mailbox(NonZeroUsize::new(MAILBOX_CAPACITY).unwrap());
    let dac = ActiveObject::new("dac", NonZeroUsize::new(STACK_SIZE).unwrap())
        .spawn_scoped(
            scope,
            mailbox,
            move || Dac::new(hardware).map_err(|error| error.to_string()),
            Dac::handle_message,
        )
        .unwrap();

    (address, dac)
}

fn launch_sd<'scope, 'env: 'scope>(
    scope: &'scope Scope<'scope, 'env>,
    directory: &'env Path,
    dac_address: Address<DacMessage>,
) -> (Address<SdMessage>, ActorHandle<'scope>) {
    let (address, mailbox) = mailbox(NonZeroUsize::new(MAILBOX_CAPACITY).unwrap());
    let sd = ActiveObject::new("sd", NonZeroUsize::new(STACK_SIZE).unwrap())
        .spawn_scoped(
            scope,
            mailbox,
            move || {
                SdFilesystem::new((), directory, dac_address).map_err(|error| error.to_string())
            },
            SdFilesystem::handle_message,
        )
        .unwrap();

    (address, sd)
}

fn launch_screen<'scope, 'env: 'scope>(
    scope: &'scope Scope<'scope, 'env>,
    sd_address: Address<SdMessage>,
    backlight: digital::Mock,
) -> (Address<ScreenMessage>, ActorHandle<'scope>) {
    let (address, mailbox) = mailbox(NonZeroUsize::new(MAILBOX_CAPACITY).unwrap());
    let screen = ActiveObject::new("screen", NonZeroUsize::new(STACK_SIZE).unwrap())
        .spawn_scoped(
            scope,
            mailbox,
            move || {
                Screen::new(FakeDisplay::new(240, 320), backlight, sd_address)
                    .map_err(|error| format!("{error:?}"))
            },
            Screen::handle_message,
        )
        .unwrap();

    (address, screen)
}

#[test]
fn given_screen_when_select_pressed_then_play_is_posted_to_dac() {
    let directory = tempfile::tempdir().unwrap();
    let mut silent_frame = [0u8; 417];
    silent_frame[..4].copy_from_slice(&[0xff, 0xfb, 0x90, 0x00]);
    std::fs::write(directory.path().join("silence.mp3"), silent_frame.repeat(3)).unwrap();

    let writes = [
        [0, 0],
        [1, 1],
        [0, 3],
        [16, 0],
        [0, 0],
        [4, 0x07],
        [6, 20],
        [7, 0],
        [8, 0],
        [5, 0x93],
        [11, 0x85],
        [12, 0x83],
        [13, 0],
        [14, 0x80],
        [0, 1],
        [33, 0x4e],
        [35, 0x44],
        [36, 0x92],
        [37, 0x92],
        [40, 0x06],
        [41, 0x06],
        [31, 0xc4],
        [0, 0],
        [63, 0xd4],
        [65, 0xd8],
        [66, 0xd8],
        [64, 0],
    ];
    let expectations: Vec<_> = writes
        .iter()
        .map(|bytes| i2c::Transaction::write(0x18, bytes.to_vec()))
        .collect();
    let mut bus = i2c::Mock::new(&expectations);
    let mut delays = delay::CheckedDelay::new(&[
        delay::Transaction::delay_ms(10),
        delay::Transaction::delay_ms(20),
        delay::Transaction::delay_ms(1500),
    ]);
    let mut backlight = digital::Mock::new(&[
        digital::Transaction::set(digital::State::High),
        digital::Transaction::set(digital::State::High),
    ]);
    let mut output = FakeI2s::default();
    let hardware = FakeDacHardware {
        i2c: bus.clone(),
        i2s: &mut output,
        delay: delays.clone(),
    };

    thread::scope(|scope| {
        let (dac_address, dac) = launch_dac(scope, hardware);
        let (sd_address, sd) = launch_sd(scope, directory.path(), dac_address);
        let (screen_address, screen) = launch_screen(scope, sd_address.clone(), backlight.clone());

        sd_address
            .post(SdMessage::FetchSongs(screen_address.clone()))
            .unwrap();
        screen_address
            .post(ScreenMessage::ShortSelectPress(false))
            .unwrap();

        drop(screen_address);
        drop(sd_address);
        screen.join().unwrap().unwrap();
        sd.join().unwrap().unwrap();
        dac.join().unwrap().unwrap();
    });

    bus.done();
    delays.done();
    backlight.done();
    assert!(output.enabled);
    assert_eq!(output.bytes.len(), 3 * 1152 * 2 * 2);
    assert!(output.bytes.iter().all(|byte| *byte == 0));
}
