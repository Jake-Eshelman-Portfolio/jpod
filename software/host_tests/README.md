# Host Tests

Run these from `software/host_tests`:

```sh
cargo test
cargo html
```

- `cargo test` runs the host tests.
- `cargo html` tests the local Cargo alias, runs actor coverage, and opens the HTML report.
- The report is saved at `target/llvm-cov/html/index.html`.

The actor test runs the production screen, SD/Symphonia decoder, and DAC handlers.
A short select press requests playback through screen -> SD -> DAC. It generates
silent stereo MP3 frames in a temporary directory, checks DAC startup register
writes, and verifies the decoded PCM bytes at the fake I2S output. No personal
MP3 files or ESP32 hardware are needed. HTML coverage excludes host-only helpers;
it includes the shared production handlers, but not ESP-only peripheral setup.

Install coverage tools once if needed:

```sh
rustup component add llvm-tools-preview
cargo install cargo-llvm-cov --locked
```

## Hardware Fakes

Import physical-device replacements from `jpod_host_tests::hardware_fakes`:

- `FakeDisplay::new(240, 320)`: RGB565 `DrawTarget`; inspect `pixel()` or `pixels()`.
- `FakeI2s`: call `tx_enable()`, write through `std::io::Write`, inspect `bytes`.
	`fail_next_write()` injects one output error.
- `FakeDacHardware`: connects I2C/delay mocks and fake I2S to the real DAC's
	hardware interface.
- `i2c::Mock` and `digital::Mock`: embedded-hal 1.0 mocks for DAC I2C and
	backlight/button GPIO. Supply expected transactions and call `done()` to verify.
- `delay::NoopDelay`: skip physical waits; `delay::CheckedDelay` checks expected waits.

Fakes provide an in-memory device; mocks verify expected calls. They replace only
hardware, not actor logic. Screen uses `DrawTarget` and `OutputPin`; DAC uses
`DacHardware`; SD accepts a directory and mount guard. Firmware still supplies
ST7789/GPIO, ESP I2C/I2S/delays, and the mounted SD card through those boundaries.
I2C readbacks are supplied by the test, not a simulated DAC register model.

Run the physical-device checks with `cargo test --lib hardware_fakes`.