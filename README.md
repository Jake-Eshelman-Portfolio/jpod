# jPod Project Summary

## Goal
A handheld, music-only player about the size of an iPod Video. Stores 32–64GB of music, has a small screen with menus, a 4-way D-pad plus select, Bluetooth headphones, and a wired 3.5mm jack as backup.

## Core design
- **Brain:** ESP32-WROVER (original ESP32). It has classic Bluetooth for headphone audio, Wi-Fi, and 8MB of extra RAM for buffering.
- **Audio:** TI TLV320DAC3100 DAC with a built-in headphone amp, feeding a 3.5mm jack.
- **Storage:** microSD card.
- **Screen:** 2" ST7789 color display.
- **Power:** LiPo battery (1500–2500mAh), USB-C charging, battery-level chip.
- **Software:** Rust on ESP-IDF (`std`). Symphonia handles MP3/FLAC decoding and tags.

## Features
- **Local playback** from the SD card, browsing by artist and album.
- **Streaming** from Navidrome on the Pi over Wi-Fi. Away from home, it reaches the Pi through Tailscale Funnel, protected by a password and a secret token.
- **"Find similar":** pick a song, and the Pi asks Last.fm for similar tracks, then plays any matches from the library or adds the rest to a wishlist.
- **Firmware updates** over Wi-Fi.

## Decisions
- Start with one ESP32. Add a second one for Bluetooth only if streaming over Wi-Fi while playing through Bluetooth stutters.
- Use a 3.5mm jack instead of USB-C headphones.
- Leave LTE for v2, but reserve a spare UART and power header on v1.
- Skip Spotify's API, since it's too locked down now. Use Last.fm instead.

## Plan
1. **Prototype on a breadboard** with a dev kit and breakouts, powered over USB.
2. **Write the software:** playback, UI, Bluetooth, streaming, "find similar".
3. **Design the custom PCB in KiCad** using the same chips.
4. **Order parts.** Separate BOM for schematic based final build and the prototype that will be used to create a test bench.

## Prepare an SD Card (Linux/WSL)
1. Insert the card and inspect disks and partitions:
	```bash
	lsblk -o NAME,PATH,TRAN,RM,SIZE,MODEL,TYPE,FSTYPE,MOUNTPOINTS
	```
2. Identify the removable USB **disk** by model and capacity. Use the whole-disk path (for example, `/dev/sde`), not a partition path such as `/dev/sde1`. The script erases the whole selected disk and creates one MBR/FAT32 partition; Linux sees the USB reader and cannot prove what media is inserted.
3. Ensure `mp3_samples/` contains at least one MP3, then run from the repository root, substituting the verified disk path:
	```bash
	sudo ./helper_scripts/partition_sd_card.sh /dev/sde
	```
4. Check the script's printed model, capacity, and partition list carefully. Continue only if they match the SD card you intend to erase, then type the exact confirmation it requests (device path and size in bytes).

5. To check the music on card:
    - sudo mkdir -p /mnt/jpod-card
    - sudo mount /dev/sde1 /mnt/jpod-card
    - ls /mnt/jpod-card/mp3_samples
    - sudo umount /mnt/jpod-card

The script refuses mounted or otherwise in-use disks. Install its Linux tools if needed with `sudo apt install util-linux fdisk parted dosfstools udev coreutils findutils gawk` (`parted` provides `partprobe`).

## Watch out for
- Wi-Fi and Bluetooth share one radio on the ESP32, so buffer streams aggressively.
- The DAC needs its own 1.8V supply on the custom board.
- The CP2102N USB chip has a long lead time, so buy it early or find a good substitution.
