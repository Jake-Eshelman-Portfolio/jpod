use std::{
    error::Error,
    fmt,
    num::NonZeroUsize,
    ops::ControlFlow,
    thread::{Scope, ScopedJoinHandle},
};

use crate::active_object::{ActiveObject, Address, Mailbox};
use crate::screen::ScreenMessage;
use esp_idf_svc::{
    hal::{i2c::I2cDriver, i2s::I2sDriver},
    sys::EspError,
};

pub enum DacMessage {
    SongFinished(Address<ScreenMessage>),
}

#[derive(Debug)]
pub enum DacError {
    Esp(EspError),
}

impl From<EspError> for DacError {
    fn from(error: EspError) -> Self {
        Self::Esp(error)
    }
}

impl fmt::Display for DacError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Esp(error) => write!(formatter, "ESP-IDF error: {error}"),
        }
    }
}

impl Error for DacError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Esp(error) => Some(error),
        }
    }
}

pub type DacWorker<'scope> = ScopedJoinHandle<'scope, Result<(), DacError>>;

struct Dac<'env, Direction> {
    _i2c_bus: I2cDriver<'env>,
    _i2s_bus: I2sDriver<'env, Direction>,
}

impl<Direction> Dac<'_, Direction> {
    fn handle_message(&mut self, message: DacMessage) -> ControlFlow<()> {
        match message {
            DacMessage::SongFinished(_screen_address) => {}
        }
        ControlFlow::Continue(())
    }
}

pub fn spawn_dac_worker<'scope, 'env: 'scope, Direction: 'scope>(
    scope: &'scope Scope<'scope, 'env>,
    mailbox: Mailbox<DacMessage>,
    i2c_bus:  I2cDriver<'env>,
    i2s_bus:  I2sDriver<'env, Direction>,
) -> DacWorker<'scope> {
    ActiveObject::new("dac", NonZeroUsize::new(8 * 1024).unwrap())
        .spawn_scoped(
            scope,
            mailbox,
            move || initialize_dac(i2c_bus, i2s_bus),
            Dac::handle_message,
        )
        .unwrap_or_else(|_| crate::restart_on_failure("Failed to start DAC worker"))
}

fn initialize_dac<'env, Direction>(
    i2c_bus: I2cDriver<'env>,
    i2s_bus: I2sDriver<'env, Direction>,
) -> Result<Dac<'env, Direction>, DacError> {
    Ok(Dac {
        _i2c_bus: i2c_bus,
        _i2s_bus: i2s_bus,
    })
}