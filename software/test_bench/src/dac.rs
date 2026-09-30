use core::f32::consts::PI;

use esp_idf_svc::{
    hal::{
        delay::{FreeRtos, BLOCK},
        gpio::{AnyIOPin, Gpio14, Gpio21, Gpio22, Gpio25, Gpio26},
        i2c::{I2cConfig, I2cDriver, I2C0},
        i2s::{
            config::{
                Config, DataBitWidth, SlotMode, StdClkConfig, StdConfig, StdGpioConfig,
                StdSlotConfig,
            },
            I2sDriver, I2sTx, I2S0,
        },
        units::FromValueType,
    },
    sys::EspError,
};
use log::{info, warn};

// TLV320DAC3100: fixed 7-bit I2C address.
const DAC_ADDR: u8 = 0x18;

// BCLK = 32 * fs (16-bit stereo). The DAC PLL settings below depend on that ratio, not on fs.
const SAMPLE_RATE: u32 = 48_000;
const TONE_HZ: u32 = 1_000;
const TONE_SECONDS: u32 = 3;
// Must hold a whole number of tone cycles so it loops without a click.
const BUFFER_MS: u32 = 10;
// ~ -20 dBFS so it's not painful in headphones.
const TONE_AMPLITUDE: f32 = 0.1;

// (register, value) pairs. Register 0x00 selects the page.
// MCLK pin is tied to GND on our board, so the DAC clocks itself from BCLK via its PLL:
// PLL_CLK = BCLK * R * J / P = 1.536 MHz * 2 * 32 / 1 = 98.304 MHz
// fs = PLL_CLK / (NDAC * MDAC * DOSR) = 98.304 MHz / (8 * 2 * 128) = 48 kHz
const CLOCK_SETUP: &[(u8, u8)] = &[
    (0x00, 0x00), // page 0
    (0x04, 0x07), // PLL_CLKIN = BCLK, CODEC_CLKIN = PLL
    (0x06, 0x20), // PLL J = 32
    (0x07, 0x00), // PLL D MSB = 0
    (0x08, 0x00), // PLL D LSB = 0
    (0x05, 0x92), // PLL power up, P = 1, R = 2
];

const DAC_SETUP: &[(u8, u8)] = &[
    (0x0B, 0x88), // NDAC power up, = 8
    (0x0C, 0x82), // MDAC power up, = 2
    (0x0D, 0x00), // DOSR MSB
    (0x0E, 0x80), // DOSR LSB = 128
    (0x1B, 0x00), // I2S, 16-bit, BCLK + WCLK are inputs (ESP32 is master)
    // page 1: analog / headphone out
    (0x00, 0x01),
    (0x23, 0x44), // DAC_L -> HPL, DAC_R -> HPR
    (0x24, 0x92), // HPL analog volume enabled, -9 dB
    (0x25, 0x92), // HPR analog volume enabled, -9 dB
    (0x28, 0x06), // HPL driver 0 dB, unmuted
    (0x29, 0x06), // HPR driver 0 dB, unmuted
    (0x1F, 0xC4), // HPL + HPR power up, common mode 1.35 V
    // page 0: DAC digital
    (0x00, 0x00),
    (0x3F, 0xD4), // L + R DAC power up, left data -> L, right data -> R
    (0x40, 0x00), // unmute L + R DAC
];

const REG_DAC_FLAGS: u8 = 0x25; // page 0
const DAC_FLAGS_ALL_ON: u8 = 0xAA; // L DAC, HPL, R DAC, HPR powered

const REG_DAC_MUTE: u8 = 0x40;
const DAC_MUTE_BOTH: u8 = 0x0C;

pub struct DacPins {
    pub sda: Gpio21<'static>,
    pub scl: Gpio22<'static>,
    pub bclk: Gpio26<'static>,
    pub ws: Gpio25<'static>,
    pub dout: Gpio14<'static>,
}

/// Configures the DAC over I2C and plays a short 1 kHz tone on the headphone jack.
pub fn run(i2c: I2C0<'static>, i2s: I2S0<'static>, pins: DacPins) -> Result<(), EspError> {
    let mut i2c = I2cDriver::new(
        i2c,
        pins.sda,
        pins.scl,
        &I2cConfig::new().baudrate(100.kHz().into()),
    )?;

    // auto_clear: send silence (not stale data) whenever we stop feeding the DMA.
    let config = StdConfig::new(
        Config::default().auto_clear(true),
        StdClkConfig::from_sample_rate_hz(SAMPLE_RATE),
        StdSlotConfig::philips_slot_default(DataBitWidth::Bits16, SlotMode::Stereo),
        StdGpioConfig::default(),
    );
    let mut i2s = I2sDriver::<I2sTx>::new_std_tx(
        i2s,
        &config,
        pins.bclk,
        pins.dout,
        AnyIOPin::none(),
        pins.ws,
    )?;
    // The DAC PLL needs BCLK running before it's powered up.
    i2s.tx_enable()?;

    // First write doubles as the "is the DAC there" check: it errors on NACK.
    write_reg(&mut i2c, 0x00, 0x00)?;
    write_reg(&mut i2c, 0x01, 0x01)?; // software reset (hardware RESET is tied to EN)
    FreeRtos::delay_ms(2);
    info!("dac: found on I2C at {DAC_ADDR:#04x}");

    write_regs(&mut i2c, CLOCK_SETUP)?;
    FreeRtos::delay_ms(10); // PLL lock
    write_regs(&mut i2c, DAC_SETUP)?;
    FreeRtos::delay_ms(50); // headphone driver ramp

    let flags = read_reg(&mut i2c, REG_DAC_FLAGS)?;
    if flags == DAC_FLAGS_ALL_ON {
        info!("dac: powered up (flags {flags:#04x})");
    } else {
        warn!("dac: unexpected power flags {flags:#04x}, expected {DAC_FLAGS_ALL_ON:#04x}");
    }

    let buf = tone_buffer();
    let loops = TONE_SECONDS * 1000 / BUFFER_MS;
    info!("dac: playing {TONE_HZ} Hz tone for {TONE_SECONDS} s");
    for _ in 0..loops {
        i2s.write_all(&buf, BLOCK)?;
    }

    write_reg(&mut i2c, REG_DAC_MUTE, DAC_MUTE_BOTH)?;
    info!("dac: done");

    Ok(())
}

/// 10 ms of interleaved 16-bit little-endian stereo samples (L, R, L, R, ...).
fn tone_buffer() -> Vec<u8> {
    let frames = SAMPLE_RATE * BUFFER_MS / 1000;
    let mut buf = Vec::with_capacity(frames as usize * 4);
    for n in 0..frames {
        let t = n as f32 / SAMPLE_RATE as f32;
        let sample = ((2.0 * PI * TONE_HZ as f32 * t).sin() * TONE_AMPLITUDE * i16::MAX as f32) as i16;
        buf.extend_from_slice(&sample.to_le_bytes()); // L
        buf.extend_from_slice(&sample.to_le_bytes()); // R
    }
    buf
}

fn write_reg(i2c: &mut I2cDriver, reg: u8, val: u8) -> Result<(), EspError> {
    i2c.write(DAC_ADDR, &[reg, val], BLOCK)
}

fn write_regs(i2c: &mut I2cDriver, regs: &[(u8, u8)]) -> Result<(), EspError> {
    for &(reg, val) in regs {
        write_reg(i2c, reg, val)?;
    }
    Ok(())
}

fn read_reg(i2c: &mut I2cDriver, reg: u8) -> Result<u8, EspError> {
    let mut val = [0u8];
    i2c.write_read(DAC_ADDR, &[reg], &mut val, BLOCK)?;
    Ok(val[0])
}
