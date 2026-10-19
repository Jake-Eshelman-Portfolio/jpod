use std::{
    num::NonZeroUsize,
    ops::ControlFlow,
    sync::mpsc,
    thread::{self, Scope, ScopedJoinHandle},
    time::Duration,
};

use jpod_host_tests::active_object::{mailbox, ActiveObject, Address};

type ActorHandle<'scope> = ScopedJoinHandle<'scope, Result<(), ()>>;
const STACK_SIZE: usize = 64 * 1024;
const MAILBOX_CAPACITY: usize = 1;

fn launch_dac<'scope, 'env>(
    scope: &'scope Scope<'scope, 'env>,
) -> (Address<usize>, ActorHandle<'scope>, mpsc::Receiver<usize>) {
    let (dac_address, dac_mailbox) = mailbox::<usize>(NonZeroUsize::new(MAILBOX_CAPACITY).unwrap());
    let (played_sender, played_receiver) = mpsc::channel();
    let dac = ActiveObject::new("dac-stub", NonZeroUsize::new(STACK_SIZE).unwrap())
        .spawn_scoped(
            scope,
            dac_mailbox,
            move || Ok::<_, ()>(played_sender),
            |played, song| {
                played.send(song).unwrap();
                ControlFlow::Continue(())
            },
        )
        .unwrap();

    (dac_address, dac, played_receiver)
}

fn launch_screen<'scope, 'env>(
    scope: &'scope Scope<'scope, 'env>,
    dac_address: Address<usize>,
    selected_song: usize,
) -> (Address<()>, ActorHandle<'scope>) {
    let (screen_address, screen_mailbox) =
        mailbox::<()>(NonZeroUsize::new(MAILBOX_CAPACITY).unwrap());
    let screen = ActiveObject::new("screen-stub", NonZeroUsize::new(STACK_SIZE).unwrap())
        .spawn_scoped(
            scope,
            screen_mailbox,
            move || Ok::<_, ()>(dac_address),
            move |dac, ()| {
                dac.post(selected_song).unwrap();
                ControlFlow::Continue(())
            },
        )
        .unwrap();

    (screen_address, screen)
}

#[test]
fn given_screen_when_select_pressed_then_play_is_posted_to_dac() {
    let selected_song = 2;

    thread::scope(|scope| {
        let (dac_address, dac, played_receiver) = launch_dac(scope);
        let (screen_address, screen) = launch_screen(scope, dac_address, selected_song);

        screen_address.post(()).unwrap();
        assert_eq!(
            played_receiver
                .recv_timeout(Duration::from_secs(1))
                .unwrap(),
            selected_song,
        );

        drop(screen_address);
        screen.join().unwrap().unwrap();
        dac.join().unwrap().unwrap();
    });
}
