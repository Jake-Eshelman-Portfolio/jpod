# Config changes vs `main`

`.cargo/config.toml`, `rust-toolchain.toml` and `build.rs` are unchanged.

## `Cargo.toml`
Add the dependency:
```toml
symphonia = { version = "0.6.1", default-features = false, features = ["mp3"] }
```
- **What:** Symphonia is a pure-Rust audio decoder; this turns on only the MP3 demuxer/decoder.
- **Why:** The SD card holds MP3s and the DAC needs raw PCM. `default-features = false` keeps out the other codecs, so the firmware stays small.

Optimize the MP3 decoder crates:
```toml
[profile.dev.package.symphonia-core]
opt-level = 3
[profile.dev.package.symphonia-bundle-mp3]
opt-level = 3
[profile.release.package.symphonia-core]
opt-level = 3
[profile.release.package.symphonia-bundle-mp3]
opt-level = 3
```
- **What `opt-level` is:** how hard the Rust compiler works at optimizing the machine code it generates. `"z"` means smallest possible code (it skips inlining and loop unrolling), and `3` means fastest possible code (it inlines functions, unrolls loops and reorders instructions), at the cost of a somewhat larger binary. The behavior of the code is identical; only speed and size change.
- **What this does:** compiles just these two decoder crates with `3` while the rest of the project stays size-optimized (`"z"` dev, `"s"` release). Cargo's `[profile.*.package.<crate>]` table overrides the setting for one dependency.
- **Why:** MP3 decoding is heavy math run on every audio frame. At `"z"` it ran at ~0.58x real time (audio slow and stuttering). At `3` it keeps up. Only the decoder needs it, so flash size barely grows.

## `sdkconfig.defaults`
Append:
```
CONFIG_SPIRAM=y
CONFIG_SPIRAM_USE_MALLOC=y
CONFIG_FREERTOS_WATCHPOINT_END_OF_STACK=y
CONFIG_ESP_DEFAULT_CPU_FREQ_MHZ_240=y
```
- `CONFIG_SPIRAM=y`
  - **What PSRAM is:** extra RAM (pseudo-static RAM). The ESP32 chip has only ~520 KB of RAM built in, and about 350 KB of that is left for your program's heap (see the `heap_init` lines in the boot log). Your WROVER module has an additional 8 MB RAM chip packaged next to the ESP32.
  - **What "external" means:** that RAM is a separate chip, not part of the ESP32 die. The ESP32 reaches it over its own SPI-style bus, so it is much larger but slower than internal RAM.
  - **What this setting does:** powers up and initializes that chip at boot (the `esp_psram: Found 8MB PSRAM device` line). Without it the 8 MB is unused.
  - **Why:** the MP3 decoder and the PCM buffers are large heap allocations, and internal RAM alone is tight.
- `CONFIG_SPIRAM_USE_MALLOC=y`
  - **What malloc is:** the C function that hands out heap memory. Rust's `Vec`, `Box` and `String` call it under the hood on ESP-IDF.
  - **What this setting does:** lets `malloc` return PSRAM memory. Without it PSRAM is initialized but can only be used by calling a special ESP-IDF function (`heap_caps_malloc`) by hand. Small allocations still go to fast internal RAM first; only large ones spill into PSRAM.
  - **Why:** the Rust code needs no changes. Only ~4 MB of the 8 MB gets mapped (the `4MB is mapped` line in the boot log).
- `CONFIG_FREERTOS_WATCHPOINT_END_OF_STACK=y`: **What:** a hardware watchpoint at the end of each task's stack. **Why:** a stack overflow faults immediately at the overflow instead of silently corrupting memory. A debug aid, so it is optional.
- `CONFIG_ESP_DEFAULT_CPU_FREQ_MHZ_240=y`: **What:** CPU runs at 240 MHz instead of the default 160 MHz. **Why:** extra headroom for real-time MP3 decode.
- Changing this file triggers a full ESP-IDF rebuild (~4 min).

## Code-level settings
These are in Rust source files, not config files:

- **SD worker thread stack, 8 KiB -> 64 KiB:** [sd.rs](src/sd.rs#L240), `ActiveObject::new("sd", NonZeroUsize::new(64 * 1024).unwrap())`. The decoder is the only thing that uses it, and it needs ~40 KiB. The old size overflows.
- **I2C at 400 kHz, internal pull-ups off:** [main.rs](src/main.rs#L63-L78), `new_i2c_bus()`, via `.baudrate(Hertz(400_000))`, `.sda_enable_pullup(false)` and `.scl_enable_pullup(false)`. The board has external 4.7k pull-ups, so the weak internal ones are not needed. 400 kHz is the DAC's fast mode.
- **I2S `StdConfig::philips(44100, DataBitWidth::Bits16)`, no MCLK:** [main.rs](src/main.rs#L80-L93), `new_i2s_bus()`. The master clock pin is `AnyIOPin::none()`. Matches the DAC's I2S mode and the 44.1 kHz source MP3s.
- **MP3s must be 44.1 kHz:** [sd.rs](src/sd.rs#L141), `OUTPUT_SAMPLE_RATE`. Other sample rates are rejected.
- **DAC clocked from BCLK:** [dac.rs](src/dac.rs#L185), `(CLOCK_MUX, 0x07)` in `configure_playback()`. MCLK is tied to GND on the board, so the DAC's internal PLL makes its clock from BCLK.
