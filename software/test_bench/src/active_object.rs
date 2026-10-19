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