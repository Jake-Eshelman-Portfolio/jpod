# ESP32-WROVER-E-N16R8 — Pin Map / Plan

DIY music player (JPod). Module: **ESP32-WROVER-E-N16R8** (16 MB flash, 8 MB PSRAM).

> **WROVER note:** GPIO6–11 are used internally for SPI flash and GPIO16/17 for PSRAM — none are broken out on this module, so they don't appear here. Don't plan to use them.

---

## 1. Peripheral assignment (the plan)

| Function | Signal | GPIO | Notes |
| --- | --- | --- | --- |
| **I²C** (DAC ctrl + fuel gauge) | SDA | IO21 | 4.7k pull-up to 3V3 |
| | SCL | IO22 | 4.7k pull-up to 3V3 |
| **I²S** (audio → TLV320DAC3100) | BCLK | IO26 | |
| | WS / LRCLK | IO25 | |
| | DOUT (data to DAC) | IO14 | |
| | MCLK | *(optional)* GPIO0 | DAC can run off BCLK via internal PLL; only wire MCLK if needed — GPIO0 is shared with boot/auto-reset, so avoid if you can |
| **SPI bus** (display + SD, shared) | SCK | IO18 | |
| | MOSI | IO23 | |
| | MISO | IO19 | SD only (ST7789 is write-only) |
| **Display (ST7789)** | CS | IO5 | strapping — safe as an output |
| | DC | IO27 | |
| | RST | *(tie to EN)* | share the ESP32 reset to save a pin, or use a free GPIO |
| | BL (backlight) | IO4 | PWM dimming |
| **microSD (DM3AT)** | CS | IO13 | |
| **Buttons — D-pad** | Up | IO34 | input-only → **needs external 10k pull-up** |
| | Down | IO35 | input-only → **external 10k pull-up** |
| | Left | IO36 (SENSOR_VP) | input-only → **external 10k pull-up** |
| | Right | IO39 (SENSOR_VN) | input-only → **external 10k pull-up** |
| **Button — Select** | SEL | IO32 | internal pull-up OK |
| **HP jack detect** | DET | IO33 | optional; confirm detect contact on SJ-43514 |
| **Fuel gauge ALRT** | ALRT | *(optional)* IO15 | idles high (OK at boot); or leave NC and poll over I²C |
| **Charger status** | CHG / PGOOD | *(optional)* NC | BQ24075 open-drain status; wire only if pins free |

**Reserved — do not reassign:**

| Signal | GPIO / Pin | Why |
| --- | --- | --- |
| EN | pin 3 | reset/enable — pull-up + RC cap |
| IO0 | pin 25 | boot strap + auto-reset circuit |
| TXD0 | GPIO1 (pin 35) | UART0 — programming/serial console |
| RXD0 | GPIO3 (pin 34) | UART0 — programming/serial console |

---

## 2. Full pin reference (all 47)

| Pin | Label | GPIO | Class | Plan |
| --- | --- | --- | --- | --- |
| 1 | GND_2 | — | power | GND |
| 2 | 3V3 | — | power | 3.3V rail |
| 3 | EN | — | reset | pull-up + RC; opt. LCD_RST here |
| 4 | SENSOR_VP | IO36 | **input-only** | Btn Left (ext pull-up) |
| 5 | SENSOR_VN | IO39 | **input-only** | Btn Right (ext pull-up) |
| 6 | IO34 | IO34 | **input-only** | Btn Up (ext pull-up) |
| 7 | IO35 | IO35 | **input-only** | Btn Down (ext pull-up) |
| 8 | IO32 | IO32 | full GPIO | Btn Select |
| 9 | IO33 | IO33 | full GPIO | HP jack detect |
| 10 | IO25 | IO25 | full GPIO (DAC) | I²S WS |
| 11 | IO26 | IO26 | full GPIO (DAC) | I²S BCLK |
| 12 | IO27 | IO27 | full GPIO | LCD DC |
| 13 | IO14 | IO14 | full GPIO | I²S DOUT |
| 14 | IO12 | IO12 | **strap (boot LOW)** | free — keep low/floating at boot |
| 15 | GND_3 | — | power | GND |
| 16 | IO13 | IO13 | full GPIO | SD CS |
| 17–22 | NC_2–7 | — | n/c | leave open |
| 23 | IO15 | IO15 | **strap (idles high)** | opt. fuel-gauge ALRT |
| 24 | IO2 | IO2 | **strap (boot LOW/float)** | free — use carefully |
| 25 | IO0 | IO0 | **strap / boot** | boot + auto-reset (reserved) |
| 26 | IO4 | IO4 | full GPIO | LCD backlight (PWM) |
| 27 | NC_8 | — | n/c | leave open |
| 28 | NC_9 | — | n/c | leave open |
| 29 | IO5 | IO5 | strap (safe as output) | LCD CS |
| 30 | IO18 | IO18 | full GPIO | SPI SCK |
| 31 | IO19 | IO19 | full GPIO | SPI MISO |
| 32 | NC | — | n/c | leave open |
| 33 | IO21 | IO21 | full GPIO | I²C SDA |
| 34 | RXD0 | IO3 | UART0 | serial console (reserved) |
| 35 | TXD0 | IO1 | UART0 | serial console (reserved) |
| 36 | IO22 | IO22 | full GPIO | I²C SCL |
| 37 | IO23 | IO23 | full GPIO | SPI MOSI |
| 38 | GND | — | power | GND |
| 39–47 | EPAD / EPAD_2–9 | — | thermal pad | all to GND |

---

## 3. Constraints cheat-sheet

- **Input-only (no output, no internal pull-up):** GPIO34, 35, 36, 39 → used for the 4 D-pad buttons, each with an **external 10k pull-up** to 3V3, button to GND.
- **Strapping pins** (level at power-up sets boot behavior): IO0, IO2, IO5, IO12, IO15.
  - IO12 must be **LOW at boot** (sets flash voltage) — don't hang a pulled-high signal on it.
  - IO0 → boot/download select, owned by auto-reset circuit.
  - IO5 / IO15 default high — fine as outputs / high-idle inputs.
- **UART0 (IO1/IO3):** keep for CP2102N programming + console.
- Add **test pads / header** for TXD0, RXD0, EN, IO0, GND — manual-flash fallback if the auto-reset circuit has a bug on rev A.

---

## 4. Firmware defines (Arduino / ESP-IDF)

```c
// ---- I2C ----
#define PIN_I2C_SDA    21
#define PIN_I2C_SCL    22

// ---- I2S (audio to TLV320DAC3100) ----
#define PIN_I2S_BCLK   26
#define PIN_I2S_WS     25
#define PIN_I2S_DOUT   14
// #define PIN_I2S_MCLK 0   // only if DAC PLL not used

// ---- SPI (display + SD) ----
#define PIN_SPI_SCK    18
#define PIN_SPI_MOSI   23
#define PIN_SPI_MISO   19
#define PIN_LCD_CS     5
#define PIN_LCD_DC     27
#define PIN_LCD_BL     4
// LCD_RST tied to EN
#define PIN_SD_CS      13

// ---- Buttons ----
#define PIN_BTN_UP     34   // input-only, ext pull-up
#define PIN_BTN_DOWN   35   // input-only, ext pull-up
#define PIN_BTN_LEFT   36   // input-only, ext pull-up
#define PIN_BTN_RIGHT  39   // input-only, ext pull-up
#define PIN_BTN_SEL    32   // internal pull-up

// ---- Misc ----
#define PIN_HP_DETECT  33
// #define PIN_FUELGAUGE_ALRT 15  // optional
```

---

## 5. Open items

- [ ] Confirm SJ-43514 detect-pin behavior (which contact, active level).
- [ ] Decide MCLK: rely on DAC internal PLL (preferred) vs wire GPIO0.
- [ ] Decide LCD_RST: tie to EN vs dedicate a GPIO.
- [ ] Wire BQ24075 CHG/PGOOD status to GPIO? (optional UI nicety)
- [ ] Add external 10k pull-ups for the 4 input-only D-pad buttons.
