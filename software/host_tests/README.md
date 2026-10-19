# Host Tests

Run these from `software/host_tests`:

```sh
cargo test
cargo html
```

- `cargo test` runs the host tests, including the real-actor integration test.
- `cargo html` uses the local Cargo alias to run that integration test with
  coverage, generate HTML, and open the report in a Windows browser from WSL.
- The report is saved at [target/llvm-cov/html/index.html](target/llvm-cov/html/index.html).
  The launcher requires `wslpath`, `cmd.exe`, and `/mnt/c/Windows`. Coverage can
  also be generated without opening a browser:

  ```sh
  cargo llvm-cov --test actors --html --ignore-filename-regex '/host_tests/src/(bin/|lib[.]rs$)'
  ```

Install coverage tools once if needed:

```sh
rustup component add llvm-tools-preview
cargo install cargo-llvm-cov --locked
```

## How the host build works

[src/lib.rs](src/lib.rs) imports the production actor runtime, DAC, screen, SD,
and shared types directly from [the firmware sources](../test_bench/src).
There are no copies of the actor handlers in this crate.

The host [Cargo.toml](Cargo.toml) binds the dependency name `esp-idf-svc` to the
local [hardware_fakes crate](hardware_fakes/). This satisfies the actors'
existing `esp_idf_svc` imports with a small set of compatible types and methods,
including lifetimes, timeout arguments, and error signatures. It is not a full
ESP-IDF emulator, and it does not patch or replace the firmware's dependencies.
The firmware still builds against the real ESP-IDF crates.

The production SD worker accepts a mount path at initialization. Firmware passes
`/sdcard`; the host passes a temporary directory. Both use `mp3_samples/` and
`JPODTEST.TXT` underneath that path. No privileged access to `/sdcard` is needed.

## What actually runs

| Component | Real code executed on the host |
|---|---|
| Actor runtime | [active_object.rs](../test_bench/src/active_object.rs): scoped worker threads, bounded mailboxes, message delivery, queue draining, and shutdown when addresses are dropped. |
| Screen actor | [screen.rs](../test_bench/src/screen.rs): production initialization and handlers, song-list/menu rendering, highlight rendering, status rendering, and playback request posting. |
| Display libraries | Real `embedded-graphics` 0.7.1 drawing/text code, `mipidsi` 0.5.0 ST7789 initialization and command generation, and `display-interface-spi` 0.4.1 transport logic. Only their underlying GPIO/SPI operations are replaced. The display remains 240 by 320 RGB565. |
| SD actor | [sd.rs](../test_bench/src/sd.rs): initialization, the write/read-back/delete check, song discovery, `FetchSongs` replies, and `DecodeMP3` handling. File operations use the real host filesystem. |
| MP3 decoding | Real Symphonia 0.6.1 probing, packet reading, MP3 decoding, and conversion to interleaved `i16` samples. |
| DAC actor | [dac.rs](../test_bench/src/dac.rs): production reset/page selection/register-write sequences, initialization, and conversion of `PlayChunk` samples to little-endian bytes for I2S. |

The `launch_dac`, `launch_sd`, and `launch_screen` helpers call the production
`spawn_*_worker` functions rather than substituting test handlers. Mailboxes have
eight slots, matching firmware. Worker stack sizes are also unchanged: 8 KiB for
DAC and screen, and 64 KiB for SD. The host build mirrors the firmware's
optimization settings for Symphonia and the actor crate; unoptimized decoding
can overflow the SD worker's stack.

## Hardware replacements

All replacements below live in
[hardware_fakes/src/lib.rs](hardware_fakes/src/lib.rs). They are lightweight
fakes or no-op stubs, not strict expectation-based mocks. Device operations
return success; the current fakes do not inject hardware failures.

| Hardware or SDK boundary | How it is replaced | What is recorded or checked |
|---|---|---|
| DAC I2C bus | `I2cDriver` records each write's device address and bytes in shared memory. `write_read` records the outgoing request and fills its read buffer from a configurable FIFO. Unconfigured read bytes default to zero. Timeout arguments are accepted but ignored. | The integration test checks the first write: address `0x18`, bytes `[0x00, 0x00]` (page selection). All initialization writes are recorded, but the full sequence is not asserted. No DAC register model is simulated. |
| I2S audio output | `tx_enable` sets an `enabled` flag; `write_all` appends bytes to an in-memory buffer. Timeouts are ignored; no audio clock, DMA, pacing, or physical output exists. | After shutdown, the test checks that TX was enabled and exactly 13,824 zero PCM bytes were written. The fake does not enforce enable-before-write ordering. |
| Shared SPI bus and LCD transport | `SpiDriver` and `SpiDeviceDriver` provide compatible construction and SPI-write methods. SPI configuration setters and writes are no-ops. | The real ST7789 driver generates commands and pixel data, but SPI bytes are discarded. There is no framebuffer, screenshot, pixel assertion, or bus arbitration check. |
| LCD/SD GPIO pins | Stand-in types represent GPIO 4 (LCD backlight), 5 (LCD chip select), 27 (LCD data/command), 13 (SD chip select), and `AnyIOPin`. `PinDriver::output`, `set_high`, and `set_low` succeed without touching hardware. | Pin levels and transitions are not recorded or asserted. The screen's backlight decisions still run. |
| SD card and SPI host | `SdSpiHostDriver`, `SdCardDriver`, and `SdCardConfiguration` are construction stubs. | No SD commands, sectors, card detection, controller behavior, or SPI transfers are emulated. |
| FAT filesystem and VFS mount | `Fatfs::new_sdcard` and `MountedFatfs::mount` return placeholder wrappers. Mounting does not create or mount a filesystem. The test creates a temporary host directory and passes it to the real SD actor. | Real file creation, read-back, deletion, directory scanning, and MP3 reads run against that directory. This does not test FAT semantics or real card mounting. |
| Hardware delays | `FreeRtos::delay_ms` and the `Ets` delay implementation return immediately. | Delay durations and real timing are not checked. Mailbox blocking and host thread scheduling remain real. |
| Supporting SDK types and restart | Minimal `EspError`, timeout, and frequency-unit types satisfy imports. The host crate's `restart_on_failure` panics instead of restarting a chip. | Compatibility only; ESP-IDF error handling and chip reset behavior are not emulated. |

## Integration test flow and assertions

The single test in [tests/actors.rs](tests/actors.rs) is named
`given_songs_on_sd_when_select_is_short_pressed_then_decoded_audio_reaches_i2s`.

1. Create a temporary SD directory containing `mp3_samples/silence.mp3`. The
   fixture is generated in the test: three 417-byte MPEG-1 Layer III stereo
   frames at 44.1 kHz, each beginning `FF FB 90 00` with zero-filled payload.
   No personal MP3 files or external encoder are needed.
2. Launch the real DAC, SD, and screen workers with the fake device objects.
   Their production initialization routines run.
3. Post the real `SdMessage::FetchSongs` with a screen address. SD discovers the
   song and replies with `ScreenMessage::ShareSongs`; the screen renders it.
4. Post `ScreenMessage::ShortSelectPress(false)`. The screen requests
   `SdMessage::DecodeMP3` for its current position, SD decodes the file and posts
   `DacMessage::PlayChunk` messages, and DAC writes PCM bytes to fake I2S:

   ```text
   ShortSelectPress(false)
       -> screen: DecodeMP3(current_position)
       -> SD/Symphonia: PlayChunk(interleaved i16 samples)
       -> DAC: little-endian PCM bytes
       -> fake I2S buffer
   ```

5. Drop the test-owned addresses and explicitly join screen, SD, and DAC.
   Queued messages drain, and actor-owned addresses are released as workers
   exit. There are no sleeps or added readiness handshake; the test does not
   assert that `ShareSongs` was handled before the select press.
6. Assert successful worker completion, the first I2C write, I2S TX enabled,
   and exactly `3 * 1152 * 2 * 2 = 13,824` zero PCM bytes.

## Coverage and limits

`cargo html` measures the `actors` integration test. The report includes the
imported production actor sources, not just the mailbox runtime. Host library
glue and the HTML launcher are explicitly excluded; the separate fake dependency
is outside the default package coverage scope. Keep exclusions narrow: imported
production paths can appear as `host_tests/src/../../test_bench/src/...`, so
excluding all of `host_tests/src` would also hide the code being tested.

This is a successful actor-flow test, not complete feature or hardware coverage.
It does not run firmware `main`, real peripheral acquisition/bus setup, the
button worker or GPIO interrupts, or the startup `DacMessage::TestI2c` read-back
check. It does not assert every menu action, display pixels, register write,
failure path, or nonzero sample conversion.

Real ESP-IDF behavior, electrical signals, bus timing, interrupt behavior, audio
clock accuracy, and headphone output still require hardware tests. A passing
host test does not replace building and testing the real ESP32 firmware.