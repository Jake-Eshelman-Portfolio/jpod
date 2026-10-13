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
    hal::{delay::BLOCK, i2c::I2cDriver, i2s::I2sDriver, task::current}, sys::EspError,
};

#[derive(Clone, Copy)]
struct Reg {
    page: Page,
    addr: u8,
}
const DAC_ADDR: u8 = 0x18;
const MAX_PAGE_SIZE: u8 = 0x80;
const SOFTWARE_RESET: Reg = Reg { page: Page::Control, addr: 0x01 };
const LR_CHANNEL: Reg = Reg { page: Page::Control, addr: 0x40 };
const SET_VOL: Reg = Reg { page: Page::Control, addr: 0x41 };
const DATA_SLOT_OFFSET: Reg = Reg { page: Page::Control, addr: 28 };



#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Control = 0,
    Analog = 1,
    Timer = 3,
    CoefA0 = 8,
    CoefA1 = 9,
    CoefB0 = 12,
    CoefB1 = 13,
}


pub enum DacMessage {
    TestI2c,
    PlaySong(usize),
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
    i2c_bus: I2cDriver<'env>,
    i2s_bus: I2sDriver<'env, Direction>,
    current_page: Option<Page>, // None = unknown, e.g., right after reset
}

impl<Direction> Dac<'_, Direction> {
    // IMPORTANT: Only access pages thru the page enum, read/write to any other pages can brick the DAC
    fn select_page(&mut self, page: Page) -> Result<(), EspError> {
        if self.current_page != Some(page) {
            self.i2c_bus.write(DAC_ADDR, &[Page::Control as u8, page as u8], BLOCK)?;
            self.current_page = Some(page);
        }
        Ok(())
    }

    fn write(&mut self, reg: Reg, value: u8) -> Result<(), EspError> {
        // Select page: First write we use byte array 0x00 to get page select, then send the reg.page as next byte to select
        // Second write we pass a register address (0-127 in hex) and the value we want to set it to
        self.select_page(reg.page)?;
        self.i2c_bus.write(DAC_ADDR, &[reg.addr, value], BLOCK)
    }
    // TODO: update to return a result later when less tired
    fn read(&mut self, reg: Reg, len: usize) -> () {
        self.select_page(reg.page).unwrap(); //yolo
        let mut buf = [0u8; 128];
        match self.i2c_bus.write_read(DAC_ADDR, &[reg.addr], &mut buf[..len], BLOCK) {
            Ok(()) => { log::info!("Bytes Read: {:?} ", buf); }
            Err(e) => { log::error!("{e}; restarting"); }
        }
    }

    // AI code (cringe)
    fn read_u8(&mut self, reg: Reg) -> Result<u8, EspError> {
        self.select_page(reg.page)?;
        let mut value = [0u8; 1];
        self.i2c_bus
            .write_read(DAC_ADDR, &[reg.addr], &mut value, BLOCK)?;
        Ok(value[0])
    }

    fn test_i2c(&mut self) -> Result<(), EspError> {
        for &value in &[0xDE, 0xAD, 0xBE, 0xEF] {
            self.write(DATA_SLOT_OFFSET, value)?;
            let readback = self.read_u8(DATA_SLOT_OFFSET)?;
            assert_eq!(readback, value, "DAC I2C readback mismatch");
            log::info!("dac: I2C test wrote {value:#04X}, read {readback:#04X}");
        }

        self.write(DATA_SLOT_OFFSET, 0x00)?;
        let readback = self.read_u8(DATA_SLOT_OFFSET)?;
        assert_eq!(readback, 0x00, "DAC I2C restore readback mismatch");
        log::info!("dac: I2C test restored DATA_SLOT_OFFSET to {readback:#04X}");
        Ok(())
    }

    fn reset_dac(&mut self) -> Result<(), EspError> {
        let reset_byte_mask: u8 = 0x01;
        self.write(SOFTWARE_RESET, reset_byte_mask)
    }

    fn config_volume_channels(&mut self, mute: bool) -> Result<(), EspError> {
        // configures (un)mute and sets right volume control to be same as left's
        let mute_channel_mask: u8 = 0x0E;
        let unmute_channel_mask: u8 = 0x02;
        if mute {
            self.write(LR_CHANNEL, mute_channel_mask)
        } else {
            self.write(LR_CHANNEL, unmute_channel_mask)
        }
    }

    fn change_volume(&mut self, turn_up: bool) -> Result<(), EspError> {
        const MAX_VOLUME: i8 = 47;
        const MIN_VOLUME: i8 = -127;

        let current_vol: u8 = self.read_u8(SET_VOL)?;
        let mut altered_vol: i8 = current_vol as i8;
        altered_vol = if turn_up {
            altered_vol.saturating_add(2).max(MAX_VOLUME)
        } else {
            altered_vol.saturating_sub(2).min(MIN_VOLUME)
        };

        log::info!("dac: Changed volume to {}", altered_vol);
        self.write(SET_VOL, altered_vol as u8)
    }

    fn handle_message(&mut self, message: DacMessage) -> ControlFlow<()> {
        match message {
            DacMessage::TestI2c => {
                if let Err(error) = self.test_i2c() {
                    log::error!("dac: I2C init check failed: {error}");
                    return ControlFlow::Break(());
                }
            }
            DacMessage::PlaySong(_song_position) => {}
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
    current_page: Option<Page>
) -> DacWorker<'scope> {
    ActiveObject::new("dac", NonZeroUsize::new(8 * 1024).unwrap())
        .spawn_scoped(
            scope,
            mailbox,
            move || initialize_dac(i2c_bus, i2s_bus, current_page),
            Dac::handle_message,
        )
        .unwrap_or_else(|_| crate::restart_on_failure("Failed to start DAC worker"))
}

fn initialize_dac<'env, Direction>(
    i2c_bus: I2cDriver<'env>,
    i2s_bus: I2sDriver<'env, Direction>,
    current_page: Option<Page>
) -> Result<Dac<'env, Direction>, DacError> {
    Ok(Dac {
        i2c_bus,
        i2s_bus,
        current_page
    })
}