# JPod BOM Selection Record

## Availability Basis

`bom_prototype.csv` and `bom_schematic.csv` were checked with the DigiKey Product Information API on 2026-09-19. An item described as **in stock** below is Active with a positive quantity returned by that API. The API returned no on-hand quantity for a few active listings; those are marked **cart check required** and must not be released to purchase until their cart quantity is confirmed. This avoids treating an Active listing as proof of stock.

The prototype and first hardware build share the ESP32-WROVER platform, TLV320DAC3100 audio family, and microSD approach. The prototype uses the Adafruit 4311 display breakout, while the final hardware build uses the ER-TFT020-7 display through its dedicated FFC connector.

## Prototype Test Bench

| Part | DigiKey part | Selection and role |
| --- | --- | --- |
| ESP32-DEVKITC-VIE | `1965-ESP32-DEVKITC-VIE-ND` | This stocked ESP32-WROVER-IE development board preserves the original ESP32 classic Bluetooth and WROVER PSRAM for prototype firmware and uses the separate IPEX antenna below. |
| ANTX200P001B24003 | `311-1553-ND` | This stocked 2.4 GHz flat-patch IPEX antenna connects to the VIE board for prototype Wi-Fi and Bluetooth, and should be kept away from metal and the battery. |
| Adafruit 6309 | `1528-6309-ND` | This stocked TLV320DAC3100 breakout converts I2S audio to headphone-level analog output and lets the bench use the same audio codec family as the hardware build. |
| Adafruit 4682 | `1528-4682-ND` | This stocked microSD breakout holds the music library and exposes the card interface to the ESP32. |
| Adafruit 4311 | `1528-4311-ND` | This stocked 2-inch ST7789 display module provides the player UI for the prototype bench. |
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
| APT1608SGC | `N/A` | Green 1608 SMD charge-indicator LED; verify supply, footprint, LED polarity, current-limiting resistor, and charger status wiring before release. |
| APT1608SYCK | `N/A` | Yellow 1608 SMD charge-indicator LED; verify supply, footprint, LED polarity, current-limiting resistor, and charger status wiring before release. |
| TPS63020DSJT | `296-27230-1-ND` | This stocked buck-boost regulator holds the system rail at 3.3 V as the battery voltage rises and falls. |
| SRP4020TA-1R5M | `SRP4020TA-1R5MCT-ND` | Stocked Bourns shielded 1.5 uH +/-20% inductor for the TPS63020; 7 A saturation, 4.5 A rated current, 42 milliohm max DCR. Its 4.45 x 4.06 x 2.00 mm body requires the Bourns land pattern for L1, not an assumed 4.0 x 4.0 mm footprint. Verify current and enclosure clearance before layout. [DigiKey product page](https://www.digikey.com/en/products/detail/bourns-inc/SRP4020TA-1R5M/4901003). |
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
| Adafruit 328 | `1528-1840-ND` | This stocked 3.7 V 2500 mAh LiPo pouch cell has a built-in protection circuit and a JST PHR-2 lead that mates with `S2B-PH-SM4-TB`. Adafruit wires red (+) to pin 1, so the connector must be BAT+ on pin 1 and GND on pin 2; check this against the cell drawing. Use Adafruit 2011 (`1528-1857-ND`, 2000 mAh) as the alternate if the enclosure is too tight. |
| ER-TFT020-7 | `N/A` | Final 2-inch TFT display for the custom board. Confirm the controller pinout, power rails, mechanical fit, and compatible 22-position 0.5 mm FFC cable before release. |
| FH12-22S-0.5SH(55) | `H125180CT-ND` | Hirose 22-position, 0.5 mm pitch FFC/FPC PCB header for the ER-TFT020-7. Confirm FFC contact orientation and cable length; [DigiKey product page](https://www.digikey.com/en/products/detail/hirose-electric-co-ltd/FH12-22S-0-5SH-55/1110380). |
| Passives | `N/A` | Decoupling capacitors, pull-ups, and regulator/charger-setting resistors are a **hold** until the schematic specifies their values, quantities, voltage ratings, and footprints. |

## Obsolete Part Note

- `TPS65810RTQT` is considered obsolete for this project and should not be added to future BOM revisions.
- If that PMIC family is required for a derivative design, prefer `TPS65810RTQR` first, with `TPS65811RTQR` as a secondary option.
- Before release, perform rail-by-rail output verification, I2C register-map comparison, startup-sequence validation, and charger behavior checks.

## Purchase Release Checklist

1. Order the listed IPEX antenna with the ESP32-DEVKITC-VIE and keep it clear of metal and the battery during bench testing.
2. Fit-check the Adafruit 328 pouch cell (or 2011 alternate) against the enclosure. Before PCB release, add the `S2B-PH-SM4-TB` battery connector to the schematic with pin 1 = BAT+ and pin 2 = GND, and confirm this matches the cell's lead drawing.
3. Finish the schematic, add both charge-indicator LEDs with suitable current-limiting resistors and validated charger status wiring, and replace the passive placeholder with individual, stocked component lines.
4. For the custom PCB, keep the WROVER-E module's PCB antenna at the board edge and follow Espressif's antenna keepout; do not add a chip antenna or external antenna unless switching to an external-antenna ESP32 module.
5. Confirm the TPS63020 inductor current and thermal limits, transistor pinout, USB ESD SOT-666 footprint and pinout, jack-detect wiring, and ER-TFT020-7 FFC pinout, cable orientation, and mechanical mounting before ordering the custom PCB.