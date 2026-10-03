# SD SPI Bring-up

Extracted from `poc_dac_ble_wifi_data` onto `main`, without DAC, BLE, or Wi-Fi changes.

The SD card shares SPI2 with the display: SCK GPIO18, MOSI GPIO23,
MISO GPIO19, SD CS GPIO13, and LCD CS GPIO5. The shared SPI driver uses
DMA for transfers larger than the non-DMA limit, including 512-byte sectors.
SD initialization runs before display initialization.

Use a FAT16/FAT32 card. Long filenames are enabled with
`CONFIG_FATFS_LFN_HEAP=y`. Build with `cargo build` from this directory;
`cargo run` flashes the attached ESP32 and opens its serial monitor.

`sd::init` mounts the card at `/sdcard`, creates or overwrites
`/sdcard/JPODTEST.TXT`, verifies its contents, then deletes the test file.
It returns the mount guard so the filesystem stays mounted. `main` calls the
public `sd::list_files_recursive("/sdcard")` to log files in all directories.
Expected log: `sd: write + read-back + delete OK`. SD errors are logged and
the display still starts. The mount is released when the guard is dropped.

The repository's SD preparation helper copies samples into `/mp3_samples`
on the card; their mounted firmware path would be `/sdcard/mp3_samples`.
This diagnostic does not yet browse that folder, decode MP3s, or play audio.
It does not format the card. File reads/writes still need validation on hardware.