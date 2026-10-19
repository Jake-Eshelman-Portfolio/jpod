use std::{convert::Infallible, io};

use embedded_graphics::{pixelcolor::Rgb565, prelude::*};
use embedded_hal::{delay::DelayNs, i2c::I2c};

use crate::dac::DacHardware;

pub use embedded_hal_mock::eh1::{delay, digital, i2c};

#[derive(Debug)]
pub struct FakeDisplay {
    size: Size,
    pixels: Vec<Rgb565>,
}

impl FakeDisplay {
    pub fn new(width: u32, height: u32) -> Self {
        let count = usize::try_from(u64::from(width) * u64::from(height))
            .expect("display dimensions exceed addressable memory");
        Self {
            size: Size::new(width, height),
            pixels: vec![Rgb565::BLACK; count],
        }
    }

    pub fn pixels(&self) -> &[Rgb565] {
        &self.pixels
    }

    pub fn pixel(&self, point: Point) -> Option<Rgb565> {
        self.index(point).map(|index| self.pixels[index])
    }

    fn index(&self, point: Point) -> Option<usize> {
        if point.x < 0
            || point.y < 0
            || point.x as u32 >= self.size.width
            || point.y as u32 >= self.size.height
        {
            return None;
        }
        Some(point.y as usize * self.size.width as usize + point.x as usize)
    }
}

impl OriginDimensions for FakeDisplay {
    fn size(&self) -> Size {
        self.size
    }
}

impl DrawTarget for FakeDisplay {
    type Color = Rgb565;
    type Error = Infallible;

    fn draw_iter<Pixels>(&mut self, pixels: Pixels) -> Result<(), Self::Error>
    where
        Pixels: IntoIterator<Item = Pixel<Self::Color>>,
    {
        for Pixel(point, color) in pixels {
            if let Some(index) = self.index(point) {
                self.pixels[index] = color;
            }
        }
        Ok(())
    }
}

#[derive(Debug, Default)]
pub struct FakeI2s {
    pub bytes: Vec<u8>,
    pub enabled: bool,
    fail_next_write: bool,
}

impl FakeI2s {
    pub fn tx_enable(&mut self) -> io::Result<()> {
        self.enabled = true;
        Ok(())
    }

    pub fn fail_next_write(&mut self) {
        self.fail_next_write = true;
    }
}

impl io::Write for FakeI2s {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if !self.enabled {
            return Err(io::Error::new(
                io::ErrorKind::NotConnected,
                "I2S TX is disabled",
            ));
        }
        if std::mem::take(&mut self.fail_next_write) {
            return Err(io::Error::other("injected I2S write failure"));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

pub struct FakeDacHardware<'output> {
    pub i2c: i2c::Mock,
    pub i2s: &'output mut FakeI2s,
    pub delay: delay::CheckedDelay,
}

impl DacHardware for FakeDacHardware<'_> {
    type Error = io::Error;

    fn write(&mut self, address: u8, bytes: &[u8]) -> io::Result<()> {
        self.i2c
            .write(address, bytes)
            .map_err(|error| io::Error::other(format!("{error:?}")))
    }

    fn write_read(&mut self, address: u8, bytes: &[u8], read: &mut [u8]) -> io::Result<()> {
        self.i2c
            .write_read(address, bytes, read)
            .map_err(|error| io::Error::other(format!("{error:?}")))
    }

    fn tx_enable(&mut self) -> io::Result<()> {
        self.i2s.tx_enable()
    }

    fn write_audio(&mut self, bytes: &[u8]) -> io::Result<()> {
        io::Write::write_all(self.i2s, bytes)
    }

    fn delay_ms(&mut self, milliseconds: u32) {
        self.delay.delay_ms(milliseconds);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use embedded_hal::{
        delay::DelayNs,
        digital::{InputPin, OutputPin},
        i2c::I2c,
    };
    use std::io::Write;

    #[test]
    fn display_records_pixels_and_clips_outside_the_screen() {
        let mut display = FakeDisplay::new(240, 320);
        display
            .draw_iter([
                Pixel(Point::new(239, 319), Rgb565::RED),
                Pixel(Point::new(-1, 0), Rgb565::WHITE),
                Pixel(Point::new(240, 0), Rgb565::WHITE),
                Pixel(Point::new(0, 320), Rgb565::WHITE),
            ])
            .unwrap();
        assert_eq!(display.pixel(Point::new(239, 319)), Some(Rgb565::RED));
        assert_eq!(display.pixel(Point::zero()), Some(Rgb565::BLACK));
        assert_eq!(display.pixel(Point::new(-1, 0)), None);
        assert_eq!(display.pixels().len(), 240 * 320);
        display.clear(Rgb565::BLUE).unwrap();
        assert!(display.pixels().iter().all(|color| *color == Rgb565::BLUE));
    }

    #[test]
    fn i2s_records_audio_bytes_and_supports_write_failures() {
        let mut i2s = FakeI2s::default();
        assert_eq!(
            i2s.write(&[1]).unwrap_err().kind(),
            io::ErrorKind::NotConnected
        );
        i2s.tx_enable().unwrap();
        i2s.write_all(&[0x34, 0x12, 0xff, 0xff]).unwrap();
        i2s.fail_next_write();
        assert!(i2s.write_all(&[0]).is_err());
        assert_eq!(i2s.bytes, [0x34, 0x12, 0xff, 0xff]);
        i2s.write_all(&[1, 2]).unwrap();
        assert_eq!(&i2s.bytes[4..], &[1, 2]);
    }

    #[test]
    fn hal_mocks_check_i2c_pins_and_delays() {
        let mut bus = i2c::Mock::new(&[
            i2c::Transaction::write(0x18, vec![0, 1]),
            i2c::Transaction::write_read(0x18, vec![40], vec![0x06]),
        ]);
        bus.write(0x18, &[0, 1]).unwrap();
        let mut readback = [0];
        bus.write_read(0x18, &[40], &mut readback).unwrap();
        assert_eq!(readback, [0x06]);
        bus.done();

        let mut pin = digital::Mock::new(&[
            digital::Transaction::set(digital::State::High),
            digital::Transaction::get(digital::State::Low),
        ]);
        pin.set_high().unwrap();
        assert!(pin.is_low().unwrap());
        pin.done();

        let mut delay = delay::CheckedDelay::new(&[delay::Transaction::delay_ms(1500)]);
        delay.delay_ms(1500);
        delay.done();
    }
}
