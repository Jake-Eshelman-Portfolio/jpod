## Hardware Build

| Part | DigiKey part | Status | Link |
| --- | --- | --- | --- |
| ESP32-WROVER-E-N16R8 | `1965-ESP32-WROVER-E-N16R8CT-ND` | Complete | [Snap Magic](https://www.snapeda.com/parts/ESP32-WROVER-E-N16R8/Espressif%20Systems/view-part/?ab_test_case=b&ref=winsource+electronics&t=ESP32-WROVER-E-N16R8&welcome=home&con_ref=None) |
| TLV320DAC3100IRHBT | `296-39266-1-ND` | Completed | [Snap Magic](https://www.digikey.com/en/models/2353656?tab=snapmagic)
| TLV75518PDBVR | `296-50410-1-ND` | This stocked 1.8 V regulator powers the DAC digital supply within its required voltage range. |
| BQ24075RGTR | `296-38874-1-ND` | This stocked charger safely charges the single-cell LiPo while powering the player from USB. |
| APT1608SGC | `N/A` | Green 1608 SMD charge-indicator LED; add and verify symbol, footprint, polarity, and drive resistor. |
| APT1608SYCK | `N/A` | Yellow 1608 SMD charge-indicator LED; add and verify symbol, footprint, polarity, and drive resistor. |
| TPS63020DSJT | `296-27230-1-ND` | This stocked buck-boost regulator holds the system rail at 3.3 V as the battery voltage rises and falls. |
| SRP4020TA-1R5M | `SRP4020TA-1R5MCT-ND` | Bourns shielded 1.5 uH inductor; assign its manufacturer land pattern to schematic L1 and verify clearance for its 4.45 x 4.06 x 2.00 mm body before layout. |
| MAX17048G+T10 | `MAX17048G+T10CT-ND` | This stocked optional fuel-gauge IC reports battery state of charge to the firmware over I2C. |
| CP2102N-A02-GQFN24R | `336-5888-1-ND` | This stocked USB-to-UART bridge provides programming and serial debug, though its 31-week manufacturer lead time makes it a buy-early item. |
| MMBT3904-7-F | `MMBT3904-FDICT-ND` | These stocked SOT-23 NPN transistors implement ESP32 automatic reset and boot-mode control; verify pin mapping in the schematic. |
| USB4105-GF-A | `2073-USB4105-GF-ACT-ND` | This stocked USB-C receptacle brings in USB power and data. |
| RC0402FR-075K1L | `311-5.10KLRCT-ND` | These stocked 5.1 kOhm resistors advertise the device as a USB-C power sink on CC1 and CC2. |
| USBLC6-2P6 | `497-5026-1-ND` | This stocked USB ESD protector shields the USB data lines from static discharge, but its SOT-666 footprint and pinout must replace the former SOT-23-6 design. |
| DM3AT-SF-PEJM5 | `HR1964CT-ND` | This stocked push-push socket holds the removable microSD music card. |
| SJ-43514-SMT-TR | `CP-43514SJCT-ND` | This stocked 3.5 mm jack provides wired headphone audio and a detect contact; confirm the detect pin in the schematic. |
| TL3342F160QG/TR | `EG2531CT-ND` | Complete | [Snap Magic](https://www.digikey.com/en/models/379003?tab=snapmagic)
| JS102011SAQN | `401-1999-1-ND` | This stocked SPDT switch is used for power or hold control. |
| S2B-PH-SM4-TB | `455-S2B-PH-SM4-TBCT-ND` | This stocked JST-PH connector mates the protected LiPo battery to the board. |
| Adafruit 328 | `1528-1840-ND` | This stocked 3.7 V 2500 mAh protected LiPo pouch cell has a JST PHR-2 lead (red + on pin 1) that mates with `S2B-PH-SM4-TB`; wire pin 1 = BAT+ and pin 2 = GND. Adafruit 2011 (`1528-1857-ND`, 2000 mAh) is the alternate. |
| ER-TFT020-7 | `N/A` | Final display for the custom board; add/verify the display symbol and its 22-pin FFC pinout before layout. |
| FH12-22S-0.5SH(55) | `H125180CT-ND` | Required 22-position 0.5 mm FFC/FPC display header; verify footprint, contact orientation, and the mating cable before layout. [DigiKey](https://www.digikey.com/en/products/detail/hirose-electric-co-ltd/FH12-22S-0-5SH-55/1110380) |
| Passives | `N/A` | Decoupling capacitors, pull-ups, and regulator/charger-setting resistors are a **hold** until the schematic specifies their values, quantities, voltage ratings, and footprints. |