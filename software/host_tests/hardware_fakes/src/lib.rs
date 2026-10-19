//! Host replacements for the `esp_idf_svc` operations used by the firmware actors.
//!
//! Only the imported items and called methods exist, with the SDK's signatures.
//! Device operations succeed, record outgoing data where tests observe it, and
//! skip physical waits. This is not an SDK emulator.

pub mod sys {
    use std::fmt;

    #[allow(non_camel_case_types)]
    pub type esp_err_t = i32;

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct EspError(esp_err_t);

    impl fmt::Display for EspError {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(formatter, "ESP-IDF error code {}", self.0)
        }
    }

    impl std::error::Error for EspError {}
}

pub mod hal {
    pub mod delay {
        #[allow(non_camel_case_types)]
        pub type TickType_t = u32;
        pub const BLOCK: TickType_t = TickType_t::MAX;

        pub struct FreeRtos;

        impl FreeRtos {
            pub fn delay_ms(_ms: u32) {}
        }

        pub struct Ets;

        impl embedded_hal_0_2::blocking::delay::DelayUs<u32> for Ets {
            fn delay_us(&mut self, _us: u32) {}
        }
    }

    pub mod gpio {
        use std::marker::PhantomData;

        use crate::sys::EspError;

        pub trait InputPin {}
        pub trait OutputPin {}

        macro_rules! pins {
            ($($name:ident),*) => {$(
                #[derive(Default)]
                pub struct $name<'d>(PhantomData<&'d mut ()>);

                impl InputPin for $name<'_> {}
                impl OutputPin for $name<'_> {}
            )*};
        }

        pins!(Gpio4, Gpio5, Gpio13, Gpio27, AnyIOPin);

        impl AnyIOPin<'static> {
            pub const fn none() -> Option<Self> {
                None
            }
        }

        pub struct Output;

        pub struct PinDriver<'d, MODE> {
            _p: PhantomData<(&'d mut (), MODE)>,
        }

        impl<'d> PinDriver<'d, Output> {
            pub fn output<T: OutputPin + 'd>(_pin: T) -> Result<Self, EspError> {
                Ok(Self { _p: PhantomData })
            }
        }

        impl<MODE> PinDriver<'_, MODE> {
            pub fn set_high(&mut self) -> Result<(), EspError> {
                Ok(())
            }

            pub fn set_low(&mut self) -> Result<(), EspError> {
                Ok(())
            }
        }

        impl<MODE> embedded_hal_0_2::digital::v2::OutputPin for PinDriver<'_, MODE> {
            type Error = EspError;

            fn set_high(&mut self) -> Result<(), EspError> {
                PinDriver::set_high(self)
            }

            fn set_low(&mut self) -> Result<(), EspError> {
                PinDriver::set_low(self)
            }
        }
    }

    pub mod i2c {
        use std::{
            collections::VecDeque,
            marker::PhantomData,
            sync::{Arc, Mutex},
        };

        use super::delay::TickType_t;
        use crate::sys::EspError;

        /// Outgoing I2C bytes per address, and bytes returned by later reads.
        #[derive(Debug, Default)]
        pub struct I2cBus {
            pub writes: Vec<(u8, Vec<u8>)>,
            pub readbacks: VecDeque<u8>,
        }

        #[derive(Default)]
        pub struct I2cDriver<'d> {
            bus: Arc<Mutex<I2cBus>>,
            _p: PhantomData<&'d mut ()>,
        }

        impl I2cDriver<'_> {
            pub fn bus(&self) -> Arc<Mutex<I2cBus>> {
                Arc::clone(&self.bus)
            }

            pub fn write(&mut self, addr: u8, bytes: &[u8], _timeout: TickType_t) -> Result<(), EspError> {
                self.bus.lock().unwrap().writes.push((addr, bytes.to_vec()));
                Ok(())
            }

            pub fn write_read(
                &mut self,
                addr: u8,
                bytes: &[u8],
                buffer: &mut [u8],
                _timeout: TickType_t,
            ) -> Result<(), EspError> {
                let mut bus = self.bus.lock().unwrap();
                bus.writes.push((addr, bytes.to_vec()));
                for byte in buffer {
                    *byte = bus.readbacks.pop_front().unwrap_or_default();
                }
                Ok(())
            }
        }
    }

    pub mod i2s {
        use std::{
            marker::PhantomData,
            sync::{Arc, Mutex},
        };

        use super::delay::TickType_t;
        use crate::sys::EspError;

        pub struct I2sTx {}

        /// Audio bytes written after TX was enabled.
        #[derive(Debug, Default)]
        pub struct I2sOutput {
            pub enabled: bool,
            pub bytes: Vec<u8>,
        }

        pub struct I2sDriver<'d, Dir> {
            output: Arc<Mutex<I2sOutput>>,
            _p: PhantomData<(&'d mut (), Dir)>,
        }

        impl<Dir> Default for I2sDriver<'_, Dir> {
            fn default() -> Self {
                Self {
                    output: Arc::default(),
                    _p: PhantomData,
                }
            }
        }

        impl<Dir> I2sDriver<'_, Dir> {
            pub fn output(&self) -> Arc<Mutex<I2sOutput>> {
                Arc::clone(&self.output)
            }
        }

        impl I2sDriver<'_, I2sTx> {
            pub fn tx_enable(&mut self) -> Result<(), EspError> {
                self.output.lock().unwrap().enabled = true;
                Ok(())
            }

            pub fn write_all(&mut self, data: &[u8], _timeout: TickType_t) -> Result<(), EspError> {
                self.output.lock().unwrap().bytes.extend_from_slice(data);
                Ok(())
            }
        }
    }

    pub mod spi {
        use std::{borrow::Borrow, marker::PhantomData};

        use super::{gpio::OutputPin, units::Hertz};
        use crate::sys::EspError;
        use embedded_hal::spi::Mode;

        #[derive(Default)]
        pub struct SpiDriver<'d>(PhantomData<&'d mut ()>);

        #[derive(Default)]
        pub struct SpiConfig;

        impl SpiConfig {
            pub fn new() -> Self {
                Self
            }

            pub fn baudrate(self, _baudrate: Hertz) -> Self {
                self
            }

            pub fn data_mode(self, _data_mode: Mode) -> Self {
                self
            }

            pub fn write_only(self, _write_only: bool) -> Self {
                self
            }
        }

        pub struct SpiDeviceDriver<'d, T> {
            _driver: T,
            _p: PhantomData<&'d ()>,
        }

        impl<'d, T: Borrow<SpiDriver<'d>> + 'd> SpiDeviceDriver<'d, T> {
            pub fn new(
                driver: T,
                _cs: Option<impl OutputPin + 'd>,
                _config: &SpiConfig,
            ) -> Result<Self, EspError> {
                Ok(Self {
                    _driver: driver,
                    _p: PhantomData,
                })
            }
        }

        impl<T> embedded_hal_0_2::blocking::spi::Write<u8> for SpiDeviceDriver<'_, T> {
            type Error = EspError;

            fn write(&mut self, _words: &[u8]) -> Result<(), EspError> {
                Ok(())
            }
        }
    }

    pub mod sd {
        use crate::sys::EspError;

        #[derive(Default)]
        pub struct SdCardConfiguration;

        impl SdCardConfiguration {
            pub fn new() -> Self {
                Self
            }
        }

        pub struct SdCardDriver<T> {
            _host: T,
        }

        impl<'d, T> SdCardDriver<spi::SdSpiHostDriver<'d, T>> {
            pub fn new_spi(
                host: spi::SdSpiHostDriver<'d, T>,
                _configuration: &SdCardConfiguration,
            ) -> Result<Self, EspError> {
                Ok(Self { _host: host })
            }
        }

        pub mod spi {
            use std::{borrow::Borrow, marker::PhantomData};

            use crate::hal::{
                gpio::{InputPin, OutputPin},
                spi::SpiDriver,
            };
            use crate::sys::EspError;

            pub struct SdSpiHostDriver<'d, T> {
                _spi_driver: T,
                _p: PhantomData<&'d mut ()>,
            }

            impl<'d, T: Borrow<SpiDriver<'d>>> SdSpiHostDriver<'d, T> {
                pub fn new(
                    spi_driver: T,
                    _cs: Option<impl OutputPin + 'd>,
                    _cd: Option<impl InputPin + 'd>,
                    _wp: Option<impl InputPin + 'd>,
                    _int: Option<impl InputPin + 'd>,
                    _wp_active_high: Option<bool>,
                ) -> Result<Self, EspError> {
                    Ok(Self {
                        _spi_driver: spi_driver,
                        _p: PhantomData,
                    })
                }
            }
        }
    }

    pub mod units {
        pub struct Hertz(pub u32);
        pub struct MegaHertz(pub u32);

        pub trait FromValueType {
            #[allow(non_snake_case)]
            fn MHz(self) -> MegaHertz;
        }

        impl FromValueType for u32 {
            fn MHz(self) -> MegaHertz {
                MegaHertz(self)
            }
        }

        impl From<MegaHertz> for Hertz {
            fn from(value: MegaHertz) -> Self {
                Hertz(value.0 * 1_000_000)
            }
        }
    }
}

/// The host directory passed at mount time stands in for the card's filesystem.
pub mod fs {
    pub mod fatfs {
        use crate::sys::EspError;

        pub struct Fatfs<T> {
            _card: T,
        }

        impl<T> Fatfs<T> {
            pub fn new_sdcard(_drive: u8, card: T) -> Result<Self, EspError> {
                Ok(Self { _card: card })
            }
        }
    }
}

pub mod io {
    pub mod vfs {
        use crate::sys::EspError;

        pub struct MountedFatfs<T> {
            _fatfs: T,
        }

        impl<T> MountedFatfs<T> {
            pub fn mount(fatfs: T, _path: &str, _max_fds: usize) -> Result<Self, EspError> {
                Ok(Self { _fatfs: fatfs })
            }
        }
    }
}
