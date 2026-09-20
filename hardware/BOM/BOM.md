# JPod BOM Selection Record

## Availability Basis

`bom_prototype.csv` and `bom_schematic.csv` were checked with the DigiKey Product Information API on 2026-09-19. An item described as **in stock** below is Active with a positive quantity returned by that API. The API returned no on-hand quantity for a few active listings; those are marked **cart check required** and must not be released to purchase until their cart quantity is confirmed. This avoids treating an Active listing as proof of stock.

The prototype and first hardware build deliberately share the ESP32-WROVER platform, TLV320DAC3100 audio family, microSD approach, and the Adafruit 4311 ST7789 display module so the firmware drivers and bring-up code can move directly between them.

## Prototype Test Bench

| Part | DigiKey part | Selection and role |
| --- | --- | --- |
| ESP32-DEVKITC-VIE | `1965-ESP32-DEVKITC-VIE-ND` | This stocked ESP32-WROVER-IE development board preserves the original ESP32 classic Bluetooth and WROVER PSRAM for prototype firmware and uses the separate IPEX antenna below. |
| ANTX200P001B24003 | `311-1553-ND` | This stocked 2.4 GHz flat-patch IPEX antenna connects to the VIE board for prototype Wi-Fi and Bluetooth, and should be kept away from metal and the battery. |
| Adafruit 6309 | `1528-6309-ND` | This stocked TLV320DAC3100 breakout converts I2S audio to headphone-level analog output and lets the bench use the same audio codec family as the hardware build. |
| Adafruit 4682 | `1528-4682-ND` | This stocked microSD breakout holds the music library and exposes the card interface to the ESP32. |
| Adafruit 4311 | `1528-4311-ND` | This stocked 2-inch ST7789 display module provides the player UI and is also used for the first hardware build so the display driver stays unchanged. |
| Adafruit 1119 | `1528-1119-ND` | This stocked tactile-button pack provides the five switches needed for the D-pad and select input. |
| Adafruit 239 | `1528-239-ND` | This stocked full-size breadboard provides the reusable wiring surface for the bench setup; it replaces the previous camera-cable part number. |
| WK-3 | `BKWK-3-ND` | This stocked 70-piece 22 AWG jumper kit connects the breakouts during bench work and replaces the unavailable Adafruit wire pack. |

## Hardware Build

| Part | DigiKey part | Selection and role |
| --- | --- | --- |
| ESP32-WROVER-E-N16R8 | `1965-ESP32-WROVER-E-N16R8CT-ND` | This stocked ESP32 module provides 16 MB flash and 8 MB PSRAM for the production compute, Wi-Fi, Bluetooth, and buffered-audio platform and has its own PCB antenna. |
| TLV320DAC3100IRHBT | `296-39266-1-ND` | This stocked TLV320DAC3100 in the same VQFN-32 package as the prototype codec converts I2S audio to headphone-level analog output. |
| TLV75518PDBVR | `296-50410-1-ND` | This stocked 1.8 V regulator powers the DAC digital supply within its required voltage range. |
| BQ24075RGTR | `296-38874-1-ND` | This stocked charger safely charges the single-cell LiPo while powering the player from USB. |
| TPS63020DSJT | `296-27230-1-ND` | This stocked buck-boost regulator holds the system rail at 3.3 V as the battery voltage rises and falls. |
| XAL4020-152MEC | `2457-XAL4020-152MEC-ND` | This stocked 1.5 uH inductor replaces the unresolved XFL4020 part and stores energy for the TPS63020 converter; verify its current and temperature margin against the final power budget. |
| MAX17048G+T10 | `MAX17048G+T10CT-ND` | This stocked optional fuel-gauge IC reports battery state of charge to the firmware over I2C. |
| CP2102N-A02-GQFN24R | `336-5888-1-ND` | This stocked USB-to-UART bridge provides programming and serial debug, though its 31-week manufacturer lead time makes it a buy-early item. |
| MMBT3904-7-F | `MMBT3904-FDICT-ND` | These stocked SOT-23 NPN transistors implement ESP32 automatic reset and boot-mode control; verify pin mapping in the schematic. |
| USB4105-GF-A | `2073-USB4105-GF-ACT-ND` | This stocked USB-C receptacle brings in USB power and data. |
| RC0402FR-075K1L | `311-5.10KLRCT-ND` | These stocked 5.1 kOhm resistors advertise the device as a USB-C power sink on CC1 and CC2. |
| USBLC6-2P6 | `497-5026-1-ND` | This stocked USB ESD protector shields the USB data lines from static discharge, but its SOT-666 footprint and pinout must replace the former SOT-23-6 design. |
| DM3AT-SF-PEJM5 | `HR1964CT-ND` | This stocked push-push socket holds the removable microSD music card. |
| SJ-43514-SMT-TR | `CP-43514SJCT-ND` | This stocked 3.5 mm jack provides wired headphone audio and a detect contact; confirm the detect pin in the schematic. |
| TL3342F160QG/TR | `EG2531CT-ND` | These stocked low-profile switches form the D-pad and select button. |
| JS102011SAQN | `401-1999-1-ND` | This stocked SPDT switch is used for power or hold control. |
| S2B-PH-SM4-TB | `455-S2B-PH-SM4-TBCT-ND` | This stocked JST-PH connector mates the protected LiPo battery to the board. |
| Adafruit 328 | `1528-328-ND` | This planned 3.7 V 2500 mAh LiPo supplies portable power, but it is a **hold** because the API returned a mismatched product and mechanical fit and exact stock are unverified. |
| Adafruit 4311 | `1528-4311-ND` | This stocked ST7789 display module is retained for the first hardware build to reuse the complete prototype display driver; the main PCB must provide a mechanical mounting and electrical connection plan. |
| Passives | `N/A` | Decoupling capacitors, pull-ups, and regulator/charger-setting resistors are a **hold** until the schematic specifies their values, quantities, voltage ratings, and footprints. |

## Purchase Release Checklist

1. Order the listed IPEX antenna with the ESP32-DEVKITC-VIE and keep it clear of metal and the battery during bench testing.
2. Select and fit-check an exact protected LiPo pack, then verify its exact DigiKey listing rather than accepting a fuzzy API match.
3. Finish the schematic and replace the passive placeholder with individual, stocked component lines.
4. For the custom PCB, keep the WROVER-E module's PCB antenna at the board edge and follow Espressif's antenna keepout; do not add a chip antenna or external antenna unless switching to an external-antenna ESP32 module.
5. Confirm the TPS63020 inductor current and thermal limits, transistor pinout, USB ESD SOT-666 footprint and pinout, jack-detect wiring, and display-module mounting before ordering the custom PCB.