use std::{
    error::Error,
    fmt,
    num::NonZeroUsize,
    ops::ControlFlow,
    thread::{Scope, ScopedJoinHandle},
};

use crate::active_object::{ActiveObject, Mailbox};
use esp_idf_svc::{
    hal::{delay::{BLOCK, FreeRtos}, i2c::I2cDriver, i2s::{I2sDriver, I2sTx}}, sys::EspError,
};

#[derive(Clone, Copy)]
struct Reg {
    page: Page,
    addr: u8,
}
const DAC_ADDR: u8 = 0x18;
const SOFTWARE_RESET: Reg = Reg { page: Page::Control, addr: 1 };
const LR_CHANNEL: Reg = Reg { page: Page::Control, addr: 64 };
const LEFT_DAC_VOLUME: Reg = Reg { page: Page::Control, addr: 65 };
const RIGHT_DAC_VOLUME: Reg = Reg { page: Page::Control, addr: 66 };
const DATA_SLOT_OFFSET: Reg = Reg { page: Page::Control, addr: 28 };
const PLL_CLOCK_SET: Reg = Reg { page: Page::Control, addr: 4 };
const PLL_PR: Reg = Reg { page: Page::Control, addr: 5 };
const PLL_J: Reg = Reg { page: Page::Control, addr: 6 };
const PLL_D_MSB: Reg = Reg { page: Page::Control, addr: 7 };
const PLL_D_LSB: Reg = Reg { page: Page::Control, addr: 8 };
const NDAC: Reg = Reg { page: Page::Control, addr: 11 };
const MDAC: Reg = Reg { page: Page::Control, addr: 12 };
const DOSR_MSB: Reg = Reg { page: Page::Control, addr: 13 };
const DOSR_LSB: Reg = Reg { page: Page::Control, addr: 14 };
const TIMER_CLOCK: Reg = Reg { page: Page::Timer, addr: 16 };
const POP_REMOVAL: Reg = Reg { page: Page::Analog, addr: 33 };
const MIXER_ROUTING: Reg = Reg { page: Page::Analog, addr: 35 }; 
const LEFT_ANALOG_VOL: Reg = Reg { page: Page::Analog, addr: 36 }; 
const RIGHT_ANALOG_VOL: Reg = Reg { page: Page::Analog, addr: 37 }; 
const HPL_DRIVER: Reg = Reg { page: Page::Analog, addr: 40 }; 
const HPR_DRIVER: Reg = Reg { page: Page::Analog, addr: 41 }; 
const OUTPUT_DRIVER: Reg = Reg { page: Page::Analog, addr: 31 }; 
const DAC_POWER: Reg = Reg { page: Page::Control, addr: 63 };


#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Eq)]
enum Page {
    Control = 0,
    Analog = 1,
    Timer = 3,
    _CoefA0 = 8,
    _CoefA1 = 9,
    _CoefB0 = 12,
    _CoefB1 = 13,
}
enum Delays {
    ResetDelay = 10,
    PLLDelay = 20,
    DePopAndSoftStepDelay = 1500, //Can potentially tune with POP_REMOVAL to shorten.
}

pub enum DacMessage {
    TestI2c,
    PlayChunk(Vec<i16>),
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

struct Dac<'env> {
    i2c_bus: I2cDriver<'env>,
    i2s_bus: I2sDriver<'env, I2sTx>,
    current_page: Option<Page>, // None = unknown, e.g., right after reset
}

impl Dac<'_> {
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

    fn init_clocking(&mut self) -> Result<(), EspError> {
        self.write(TIMER_CLOCK, 0x00)?; // 
        self.write(PLL_CLOCK_SET, 0x07)?;
        self.write(PLL_J, 20)?;
        self.write(PLL_D_MSB, 0)?;
        self.write(PLL_D_LSB, 0)?;
        self.write(PLL_PR, 0x93)?;
        FreeRtos::delay_ms(Delays::PLLDelay as u32);
        self.write(NDAC, 0x85)?;
        self.write(MDAC, 0x83)?;
        self.write(DOSR_MSB, 0x00)?;
        self.write(DOSR_LSB, 0x80)?;
        Ok(())
    }

    fn init_analog(&mut self) -> Result<(), EspError> {
        self.write(POP_REMOVAL, 0x4e)?;
        self.write(MIXER_ROUTING, 0x44)?;
        self.write(LEFT_ANALOG_VOL, 0x92)?; // check 92 val
        self.write(RIGHT_ANALOG_VOL, 0x92)?;
        self.write(HPL_DRIVER, 0x06)?;
        self.write(HPR_DRIVER, 0x06)?;
        self.write(OUTPUT_DRIVER, 0xC4)?;
        FreeRtos::delay_ms(Delays::DePopAndSoftStepDelay as u32);
        Ok(())
    }

    fn init_power_up(&mut self) -> Result<(), EspError> {
        self.write(DAC_POWER, 0xD4)?;
        self.write(LEFT_DAC_VOLUME, 0xD8)?;
        self.write(RIGHT_DAC_VOLUME, 0xD8)?;
        self.write(LR_CHANNEL, 0x00)?;
        Ok(())
    }

    // Planned for mute button in future
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
        const MAX_VOLUME: i8 = 0; // goes up to 47
        const MIN_VOLUME: i8 = -127;

        let current_vol: u8 = self.read_u8(LEFT_DAC_VOLUME)?;
        let mut altered_vol: i8 = current_vol as i8;
        altered_vol = if turn_up {
            altered_vol.saturating_add(2).min(MAX_VOLUME)
        } else {
            altered_vol.saturating_sub(2).max(MIN_VOLUME)
        };

        log::info!("dac: Changed volume to {}", altered_vol);
        self.write(LEFT_DAC_VOLUME, altered_vol as u8)?;
        self.write(RIGHT_DAC_VOLUME, altered_vol as u8)
    }

    fn handle_message(&mut self, message: DacMessage) -> ControlFlow<()> {
        match message {
            DacMessage::TestI2c => {
                if let Err(error) = self.test_i2c() {
                    log::error!("dac: I2C init check failed: {error}");
                    return ControlFlow::Break(());
                }
            }
            DacMessage::PlayChunk(chunk) => {
                let bytes: Vec<u8> = chunk
                    .iter()
                    .flat_map(|sample| sample.to_le_bytes())
                    .collect();
                if let Err(error) = self.i2s_bus.write_all(&bytes, BLOCK) {
                    log::error!("dac: {error}; fix ur shit"); // lgtm
                }
            }
        }
        ControlFlow::Continue(())
    }
}

pub fn spawn_dac_worker<'scope, 'env: 'scope>(
    scope: &'scope Scope<'scope, 'env>,
    mailbox: Mailbox<DacMessage>,
    i2c_bus: I2cDriver<'env>,
    i2s_bus: I2sDriver<'env, I2sTx>,
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

fn initialize_dac<'env>(
    i2c_bus: I2cDriver<'env>,
    i2s_bus: I2sDriver<'env, I2sTx>,
) -> Result<Dac<'env>, DacError> {
    let current_page = None;
    let mut dac = Dac {
        i2c_bus,
        i2s_bus,
        current_page
    };
    dac.reset_dac()?;
    FreeRtos::delay_ms(Delays::ResetDelay as u32);
    dac.i2s_bus.tx_enable()?;
    dac.init_clocking()?;
    dac.init_analog()?;
    dac.init_power_up()?;
    Ok(dac)
}