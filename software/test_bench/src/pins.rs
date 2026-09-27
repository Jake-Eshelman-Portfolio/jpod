use esp_idf_svc::hal::gpio::{
    Gpio13, Gpio14, Gpio18, Gpio19, Gpio21, Gpio22, Gpio23, Gpio25, Gpio26, Gpio27, Gpio32, Gpio33,
    Gpio34, Gpio35, Gpio36, Gpio39, Gpio4, Gpio5, Pins,
};

// Board pin mappings
pub struct BoardPins {
    pub i2c_sda: Gpio21<'static>,
    pub i2c_scl: Gpio22<'static>,
    pub i2s_bclk: Gpio26<'static>,
    pub i2s_ws: Gpio25<'static>,
    pub i2s_dout: Gpio14<'static>,
    pub spi_sck: Gpio18<'static>,
    pub spi_mosi: Gpio23<'static>,
    pub spi_miso: Gpio19<'static>,
    pub lcd_cs: Gpio5<'static>,
    pub lcd_dc: Gpio27<'static>,
    pub lcd_bl: Gpio4<'static>,
    pub sd_cs: Gpio13<'static>,
    pub btn_up: Gpio34<'static>,
    pub btn_down: Gpio35<'static>,
    pub btn_left: Gpio36<'static>,
    pub btn_right: Gpio39<'static>,
    pub btn_sel: Gpio32<'static>,
    pub hp_detect: Gpio33<'static>,
}

// Init the board pins struct
pub fn board_pins(pins: Pins) -> BoardPins {
    BoardPins {
        i2c_sda: pins.gpio21,
        i2c_scl: pins.gpio22,
        i2s_bclk: pins.gpio26,
        i2s_ws: pins.gpio25,
        i2s_dout: pins.gpio14,
        spi_sck: pins.gpio18,
        spi_mosi: pins.gpio23,
        spi_miso: pins.gpio19,
        lcd_cs: pins.gpio5,
        lcd_dc: pins.gpio27,
        lcd_bl: pins.gpio4,
        sd_cs: pins.gpio13,
        btn_up: pins.gpio34,
        btn_down: pins.gpio35,
        btn_left: pins.gpio36,
        btn_right: pins.gpio39,
        btn_sel: pins.gpio32,
        hp_detect: pins.gpio33,
    }
}