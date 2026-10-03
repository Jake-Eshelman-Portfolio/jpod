use std::{
    io,
    num::NonZeroUsize,
    ops::ControlFlow,
    sync::mpsc::{self, Receiver, SendError, SyncSender, TrySendError},
    thread::{self, Scope, ScopedJoinHandle},
};

/// A clonable, typed address for posting to one worker.
pub struct Address<Message> {
    sender: SyncSender<Message>,
}

impl<Message> Clone for Address<Message> {
    fn clone(&self) -> Self {
        Self {
            sender: self.sender.clone(),
        }
    }
}

impl<Message> Address<Message> {
    /// Posts without blocking, returning the message if full or disconnected.
    pub fn try_post(&self, message: Message) -> Result<(), TrySendError<Message>> {
        self.sender.try_send(message)
    }

    /// Blocks for capacity. Avoid this when posting between mutually dependent workers.
    pub fn post(&self, message: Message) -> Result<(), SendError<Message>> {
        self.sender.send(message)
    }
}

/// The single-consumer half of a mailbox, moved into its worker at launch.
pub struct Mailbox<Message> {
    receiver: Receiver<Message>,
}

/// Creates addresses before launching workers so their dependencies can be wired first.
pub fn mailbox<Message>(capacity: NonZeroUsize) -> (Address<Message>, Mailbox<Message>) {
    let (sender, receiver) = mpsc::sync_channel(capacity.get());
    (Address { sender }, Mailbox { receiver })
}

pub struct ActiveObject {
    name: String,
    stack_size: usize,
}

impl ActiveObject {
    /// Stack size is in bytes and must be sized for the worker's hardware/library calls.
    pub fn new(name: impl Into<String>, stack_size: NonZeroUsize) -> Self {
        Self {
            name: name.into(),
            stack_size: stack_size.get(),
        }
    }

    /// Initializes private state on the worker, then handles messages sequentially.
    ///
    /// The worker blocks while idle. It exits on `Break(())` or after all addresses
    /// are dropped and queued messages are drained. Initialization errors are
    /// returned by `join`; panics are reported by the outer thread result.
    /// State is created inside the thread and does not itself need to be `Send`.
    /// Scoped workers can borrow resources but must exit before their scope ends.
    pub fn spawn_scoped<'scope, 'env, Message, State, Error, Initialize, Handle>(
        self,
        scope: &'scope Scope<'scope, 'env>,
        mailbox: Mailbox<Message>,
        initialize: Initialize,
        mut handle: Handle,
    ) -> io::Result<ScopedJoinHandle<'scope, Result<(), Error>>>
    where
        Message: Send + 'scope,
        Error: Send + 'scope,
        Initialize: FnOnce() -> Result<State, Error> + Send + 'scope,
        Handle: FnMut(&mut State, Message) -> ControlFlow<()> + Send + 'scope,
    {
        thread::Builder::new()
            .name(self.name)
            .stack_size(self.stack_size)
            .spawn_scoped(scope, move || {
                let mut state = initialize()?;
                while let Ok(message) = mailbox.receiver.recv() {
                    if handle(&mut state, message).is_break() {
                        break;
                    }
                }
                Ok(())
            })
    }
}

#[cfg(all(test, not(target_os = "espidf")))]
mod tests {
    use super::*;
    use std::{cell::RefCell, convert::Infallible, rc::Rc, sync::mpsc::TryRecvError};

    fn worker() -> ActiveObject {
        ActiveObject::new("test-worker", NonZeroUsize::new(64 * 1024).unwrap())
    }

    fn capacity(value: usize) -> NonZeroUsize {
        NonZeroUsize::new(value).unwrap()
    }

    #[test]
    fn full_and_disconnected_mailboxes_return_the_message() {
        let (address, mailbox) = mailbox(capacity(1));
        address.try_post(10).unwrap();
        assert!(matches!(address.try_post(20), Err(TrySendError::Full(20))));
        drop(mailbox);
        assert!(matches!(
            address.try_post(30),
            Err(TrySendError::Disconnected(30))
        ));
        assert_eq!(address.post(40).unwrap_err().0, 40);
    }

    #[test]
    fn cloned_address_does_not_require_cloneable_messages() {
        struct Message;
        let (address, mailbox) = mailbox::<Message>(capacity(1));
        let other = address.clone();
        other.try_post(Message).unwrap();
        assert!(mailbox.receiver.recv().is_ok());
    }

    #[test]
    fn processes_messages_in_order_and_drains_on_disconnect() {
        let mut output = Vec::new();
        let (address, mailbox) = mailbox(capacity(3));
        address.post(1).unwrap();
        address.post(2).unwrap();
        address.post(3).unwrap();
        drop(address);
        thread::scope(|scope| {
            let handle = worker()
                .spawn_scoped(
                    scope,
                    mailbox,
                    || Ok::<_, Infallible>(&mut output),
                    |state, message| {
                        state.push(message);
                        ControlFlow::Continue(())
                    },
                )
                .unwrap();
            assert_eq!(handle.join().unwrap(), Ok(()));
        });
        assert_eq!(output, vec![1, 2, 3]);
    }

    #[test]
    fn wakes_on_post_and_explicit_stop_disconnects_remaining_addresses() {
        let (address, mailbox) = mailbox(capacity(2));
        let (ready_sender, ready_receiver) = mpsc::channel();
        let (reply_sender, reply_receiver) = mpsc::channel();
        thread::scope(|scope| {
            let handle = worker()
                .spawn_scoped(
                    scope,
                    mailbox,
                    move || {
                        ready_sender.send(()).unwrap();
                        Ok::<_, Infallible>(Rc::new(RefCell::new(0)))
                    },
                    move |state, message| {
                        *state.borrow_mut() += message;
                        reply_sender.send(*state.borrow()).unwrap();
                        ControlFlow::Break(())
                    },
                )
                .unwrap();
            ready_receiver.recv().unwrap();
            assert_eq!(reply_receiver.try_recv(), Err(TryRecvError::Empty));
            address.try_post(7).unwrap();
            assert_eq!(reply_receiver.recv().unwrap(), 7);
            assert_eq!(handle.join().unwrap(), Ok(()));
            assert!(matches!(
                address.try_post(8),
                Err(TrySendError::Disconnected(8))
            ));
        });
    }

    #[test]
    fn workers_forward_owned_messages_and_release_destination_addresses() {
        let (screen_address, screen_mailbox) = mailbox::<Vec<String>>(capacity(1));
        let (sd_address, sd_mailbox) = mailbox::<Address<Vec<String>>>(capacity(1));
        let mut displayed_songs = Vec::new();

        thread::scope(|scope| {
            let screen_worker = worker()
                .spawn_scoped(
                    scope,
                    screen_mailbox,
                    || Ok::<_, Infallible>(&mut displayed_songs),
                    |state, songs| {
                        state.extend(songs);
                        ControlFlow::Continue(())
                    },
                )
                .unwrap();
            let sd_worker = worker()
                .spawn_scoped(
                    scope,
                    sd_mailbox,
                    || Ok::<_, Infallible>(vec![String::from("song")]),
                    |songs, destination| {
                        destination.post(songs.clone()).unwrap();
                        ControlFlow::Continue(())
                    },
                )
                .unwrap();

            assert!(sd_address.post(screen_address.clone()).is_ok());
            drop(sd_address);
            assert_eq!(sd_worker.join().unwrap(), Ok(()));
            drop(screen_address);
            assert_eq!(screen_worker.join().unwrap(), Ok(()));
        });

        assert_eq!(displayed_songs, vec![String::from("song")]);
    }

    #[test]
    fn initialization_error_is_returned_without_handling_messages() {
        let (address, mailbox) = mailbox::<()>(capacity(1));
        address.post(()).unwrap();
        thread::scope(|scope| {
            let handle = worker()
                .spawn_scoped(
                    scope,
                    mailbox,
                    || Err::<(), _>("initialization failed"),
                    |_, _| panic!("handler must not run after initialization failure"),
                )
                .unwrap();
            assert_eq!(handle.join().unwrap(), Err("initialization failed"));
            assert!(matches!(
                address.try_post(()),
                Err(TrySendError::Disconnected(()))
            ));
        });
    }
}
