# POC feature bring-up: WiFi, BLE, DAC, SD card

Bare-minimum code to prove each block is wired and alive. Each feature is its own
file in `src/`, so you can copy one out without the others.

| Feature | File | What "working" looks like |
| --- | --- | --- |
| WiFi | [src/wifi.rs](src/wifi.rs) | Log lists nearby networks (and an IP if you gave it credentials) |
| BLE | [src/ble.rs](src/ble.rs) | Phone BLE scanner app shows a device named **JPod** |
| DAC | [src/dac.rs](src/dac.rs) | 3 s, 1 kHz tone in the headphones |
| SD card | [src/sd.rs](src/sd.rs) | Log says `write + read-back OK` and lists files on the card |

[src/main.rs](src/main.rs) runs them in order: WiFi, BLE, DAC, SD, then the screen
(which loops forever). A failure logs `<name>: FAILED: ...` and moves on to the next one.

## How to run

1. `source ~/export-esp.sh`
2. Scan only: `cargo run`
3. Scan + connect: `WIFI_SSID=MyNet WIFI_PASS=secret cargo run`
4. Plug in headphones before the DAC step.
5. Put a FAT32-formatted microSD card in before boot.
6. Open a BLE scanner app on your phone (e.g. nRF Connect) and look for "JPod".

Expected log (roughly):

```
wifi: scan found 7 APs
wifi:   MyNet (ch 6, -52 dBm)
wifi: WIFI_SSID not set, skipping connect
ble: advertising as "JPod": Success
dac: found on I2C at 0x18
dac: powered up (flags 0xaa)
dac: playing 1000 Hz tone for 3 s
dac: done
sd: mounted at /sdcard
sd: write + read-back OK
sd:   "JPODTEST.TXT"
```

## Shared setup (all features)

1. `link_patches()` and `EspLogger::initialize_default()` once, first thing.
2. `Peripherals::take()` once. Everything else is moved out of it.
3. `board_pins()` in [src/pins.rs](src/pins.rs) is the only place GPIO numbers live.
4. WiFi and BLE both need `EspSystemEventLoop` / `EspDefaultNvsPartition`. Take each once and
   `.clone()` it to hand it out.
5. WiFi and BLE share one radio (`modem`). Call `modem.split()` to get a WiFi half and a BT half.

Config changes made in [sdkconfig.defaults](sdkconfig.defaults):

- `CONFIG_SPIRAM=y` turns on the 8 MB PSRAM on the WROVER. Without it, WiFi, BT and
  audio buffers together run out of RAM.
- Bluedroid BLE settings (see BLE section).
- `CONFIG_FATFS_LFN_HEAP=y` turns on long file names. Without it, names are 8.3 only
  (`SONGNAME.MP3`).

Changing `sdkconfig.defaults` triggers a full ESP-IDF rebuild (a few minutes).

---

## 1. WiFi ([src/wifi.rs](src/wifi.rs))

No pins. Uses the internal radio.

### Init steps

1. Create `EspWifi::new(wifi_modem, sys_loop, Some(nvs))`.
2. Wrap it in `BlockingWifi` so calls wait until they finish.
3. `set_configuration(Configuration::Client(...))` to set station mode, plus SSID/password if you have them.
4. `start()` powers on the radio.
5. `scan()` lists access points. **If this works, the RF path and antenna are OK.**
6. (optional) `connect()` then `wait_netif_up()` to get an IP over DHCP.

### Things to think about

1. **Credentials are compiled in** through `option_env!`. They end up in the firmware image.
   Fine for testing, but never commit them. For a real product, store them in NVS
   (`EspNvs`) and add a way to enter them (BLE provisioning, SoftAP page, etc.).
2. `BlockingWifi` blocks the calling thread. For a UI that must stay responsive, use
   `AsyncWifi` or run WiFi in its own thread.
3. Dropping the `BlockingWifi` turns WiFi off. Keep it alive (store it somewhere) if you
   need to stay connected.
4. Power: WiFi is the biggest battery drain on the board. Only turn it on when you need it
   (e.g. syncing songs) and turn it off afterwards. Modem sleep is on by default while connected.
5. WiFi and BLE at the same time works (ESP-IDF time-shares the radio), but each gets less
   airtime. Expect slower throughput and occasional BLE hiccups.
6. `AuthMethod::WPA2Personal` is the *minimum* security accepted, so WPA2/WPA3 networks both work.
7. Next steps: `EspHttpServer` for uploading songs over a web page, `EspSntp` for clock
   time, OTA updates (`EspOta`).

---

## 2. BLE ([src/ble.rs](src/ble.rs))

No pins. Uses the internal radio.

### Init steps

1. Set the Bluedroid options in `sdkconfig.defaults` (`CONFIG_BT_ENABLED`, `CONFIG_BT_BLUEDROID_ENABLED`,
   BLE-only controller mode).
2. Create `BtDriver::new(bt_modem, Some(nvs))`. This starts the controller and host stack.
3. Create `EspBleGap::new(driver)` (GAP = advertising and scanning).
4. `subscribe()` a callback for GAP events **before** configuring anything, so no events get missed.
5. `set_device_name("JPod")`.
6. `set_adv_conf(...)` sets what goes in the advertising packet.
7. Wait for the `AdvertisingConfigured` event, **then** call `start_advertising()`. It's
   event-driven: calling start right after config can fail.
8. `AdvertisingStarted(Success)` means the board is on air.

### Things to think about

1. **Keep the returned handle alive.** `run()` returns the GAP handle; if it gets dropped, BLE stops.
2. The callback runs on the Bluedroid task (BTC), not your thread. Keep it short and
   don't block in it. Send work to your own thread through a channel if needed.
   `CONFIG_BT_BTC_TASK_STACK_SIZE=15000` gives Rust + logging enough stack there.
3. The callback holds a `Weak` reference to the GAP handle. A strong `Arc` would create a
   cycle and BLE could never shut down.
4. **Bluedroid vs NimBLE**: we picked Bluedroid on purpose. NimBLE is smaller but only
   does BLE. If we ever want **Bluetooth headphones**, that's A2DP over Bluetooth *Classic*,
   which needs Bluedroid with `CONFIG_BTDM_CTRL_MODE_BTDM=y` and `CONFIG_BT_CLASSIC_ENABLED=y`
   (costs a lot more RAM, so PSRAM matters).
5. Right now we only advertise. To send data (e.g. control from a phone app), add a GATT
   server (`EspGatts`) with a service and characteristics. See `bt_gatt_server.rs` in the
   esp-idf-svc examples.
6. Advertising interval affects battery. Slower advertising = less power, slower discovery.
7. Only one `EspBleGap` can exist at a time (it's a singleton inside the library).

---

## 3. DAC / audio ([src/dac.rs](src/dac.rs))

TLV320DAC3100. Two buses:

| Signal | GPIO | Bus | What it's for |
| --- | --- | --- | --- |
| SDA | IO21 | I²C | Register setup (address `0x18`) |
| SCL | IO22 | I²C | |
| BCLK | IO26 | I²S | Bit clock (ESP32 drives it) |
| WS / WCLK | IO25 | I²S | Left/right clock = sample rate |
| DOUT / DIN | IO14 | I²S | Audio samples |

Board facts that shape the code (from the schematic):

- DAC `MCLK` pin is tied to **GND**. The DAC has to make its own clock from BCLK using its PLL.
- DAC `RESET` is tied to **EN**. It resets with the ESP32, so we also do a software reset.
- Only the headphone outputs (HPL/HPR) are used. Speaker pins are not connected.

### Init steps

1. Start the I²C driver at 100 kHz.
2. Start the I²S driver: Philips I²S, 48 kHz, 16-bit, stereo, no MCLK pin.
   `auto_clear(true)` means silence is sent if we run out of data.
3. `tx_enable()` so **BCLK is running before the DAC PLL is powered up**. The PLL needs an input clock.
4. Write page 0, reg 0x01 = 0x01 (software reset), wait 2 ms. If this I²C write fails,
   the DAC isn't answering: check solder, pull-ups, 3V3/1V8 rails.
5. Clock setup (page 0):
   1. reg 0x04 = 0x07: PLL input = BCLK, codec clock = PLL.
   2. PLL J = 32, D = 0, then P = 1, R = 2 + power on. Result: 1.536 MHz × 2 × 32 = 98.304 MHz.
   3. Wait 10 ms for the PLL to lock.
6. DAC dividers: NDAC = 8, MDAC = 2, DOSR = 128. Result: 98.304 MHz / (8 × 2 × 128) = 48 kHz.
7. Interface: reg 0x1B = 0x00 (I²S, 16-bit, DAC is clock slave).
8. Analog (page 1): route left DAC → HPL and right DAC → HPR, set analog volume -9 dB,
   unmute headphone drivers, power them up.
9. Digital (page 0): power up both DAC channels, unmute them.
10. Read page 0 reg 0x25 (power flags). `0xAA` = both DACs and both headphone drivers are on.
11. Push samples with `i2s.write_all()`. Format is interleaved `L, R, L, R` as 16-bit little-endian.
12. Mute (reg 0x40 = 0x0C) when done.

### Things to think about

1. **BCLK must be exactly 32 × sample rate.** The PLL numbers depend on that ratio
   (not on the sample rate itself), so 44.1 kHz works with the same PLL settings.
   If you switch to 24/32-bit samples, BCLK becomes 64 × fs and **J must change to 16**.
2. Music files come in 44.1 kHz (most MP3/FLAC) and 48 kHz. Either reconfigure I²S when the
   rate changes (mute first, stop I²S, change, restart, unmute) or resample to one fixed rate.
3. ESP32 I²S clock comes from a 160 MHz PLL with a fractional divider, so BCLK has some
   jitter. If audio quality is audible-bad, try the APLL clock source (`ClockSource::Apll`),
   which can hit audio rates exactly.
4. **Keep the DMA fed.** Audio must be written continuously. Run playback in its own thread
   with a decent stack and priority. SD reads and MP3 decoding must keep ahead of it.
   Gaps = clicks.
5. Volume: use DAC digital volume (page 0, reg 0x41/0x42, 0.5 dB steps, signed) for the
   user volume control. Leave the analog gains fixed.
6. **Pops**: always mute before stopping I²S or changing clocks, and unmute after the
   clocks are stable. Page 1 reg 0x21 controls headphone power-up ramp timing if
   there's a pop on start-up.
7. Headphone detect (IO33 on our board) can be used to mute or pause on unplug.
8. The register values came from the TLV320DAC3100 datasheet. If `flags` isn't `0xAA`,
   re-check them against the datasheet register map first.
9. I²C (IO21/22) is shared with the MAX17048 fuel gauge. Create **one** `I2cDriver` and
   share it (e.g. `Arc<Mutex<I2cDriver>>`) instead of creating it inside `dac::run`.

---

## 4. SD card ([src/sd.rs](src/sd.rs))

DM3AT microSD slot on the **same SPI bus as the display**.

| Signal | GPIO |
| --- | --- |
| SCK | IO18 |
| MOSI | IO23 |
| MISO | IO19 |
| SD CS | IO13 |
| (LCD CS, for reference) | IO5 |

### Init steps

1. Create the SPI bus **once** with DMA on (`Dma::Auto(4096)` in [src/spi.rs](src/spi.rs)).
   The SD driver needs DMA because sectors are 512 bytes. Without DMA, SPI is limited to 64-byte transfers.
2. `SdSpiHostDriver::new(&bus, Some(sd_cs), None, None, None, None)` adds the card as a device
   on that bus. It takes `&bus`, so the display can use the same bus.
3. `SdCardDriver::new_spi(host, &SdCardConfiguration::new())` talks to the card
   (starts at 400 kHz, then speeds up).
4. `Fatfs::new_sdcard(0, card)` attaches FAT filesystem driver 0 to the card.
5. `MountedFatfs::mount(fatfs, "/sdcard", 4)` makes it available at `/sdcard`, max 4 open files.
6. Now use normal Rust `std::fs` (`File::create`, `File::open`, `read_dir`, ...).
7. Dropping the mount handle unmounts the card.

### Things to think about

1. **Shared bus**: only one device can talk at a time. The ESP-IDF driver locks the bus per
   transaction, so SD and display can live in different threads. But a long SD read
   will delay a screen update (and the other way round).
2. **CS lines must be high when idle.** If SD CS floats while the display is talking,
   the card may respond to display traffic and corrupt it. Add a 10k pull-up on SD CS
   (IO13) if the board doesn't have one.
3. The SD card must be set up (step 3) while the display is **not** talking. SD init sends
   special clock pulses at 400 kHz. Easiest: set up SD before starting the display, like `main.rs` does.
4. Card must be **FAT32** (or FAT16). exFAT (default for >32 GB cards) needs
   `CONFIG_FATFS_USE_EXFAT` or reformat the card as FAT32.
5. Long file names are turned on by `CONFIG_FATFS_LFN_HEAP=y`. Our test file still uses an
   8.3 name so it works either way.
6. Speed: the default SD clock is 20 MHz. SPI mode tops out around 1–2 MB/s actual. That's
   plenty for MP3/FLAC playback, slow for copying lots of files. Read in big chunks
   (e.g. 4–16 KB), not byte by byte.
7. Card removed while mounted = errors on every file call. Handle errors instead of
   `unwrap()`, and remount when the card comes back (no card-detect pin is wired right now).
8. Always close files / drop the mount before power off, or the FAT may get corrupted.
   Tie this into the low-battery shutdown path.

---

## Other gotchas

1. `espflash` uses its own partition table with a ~4 MB app slot. The image with WiFi +
   BT is ~1.8 MB, which would **not** fit the stock ESP-IDF 1 MB table. If you add a custom
   `partitions.csv` later (OTA, storage), size the app slots for this.
2. Main task stack is 8 KB (`CONFIG_ESP_MAIN_TASK_STACK_SIZE`). If you see
   `stack overflow` in the log, raise it or move work into threads with bigger stacks.
3. Strapping pins: IO5 (LCD CS) is a strapping pin. Fine as an output, but don't hold it low at boot.
