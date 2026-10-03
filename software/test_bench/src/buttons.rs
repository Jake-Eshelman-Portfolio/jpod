use esp_idf_svc::hal::gpio::{Gpio32, Gpio34, Gpio35, Gpio36, Gpio39, Input, PinDriver, Pull};
use esp_idf_svc::sys::EspError;

pub struct Buttons {
    pub up: PinDriver<'static, Input>,
    pub down: PinDriver<'static, Input>,
    pub left: PinDriver<'static, Input>,
    pub right: PinDriver<'static, Input>,
    pub select: PinDriver<'static, Input>,
}

pub fn init_buttons(
    up: Gpio34<'static>,
    down: Gpio35<'static>,
    left: Gpio36<'static>,
    right: Gpio39<'static>,
    select: Gpio32<'static>,
) -> Result<Buttons, EspError> {
    Ok(Buttons {
        up: PinDriver::input(up, Pull::Floating)?,
        down: PinDriver::input(down, Pull::Floating)?,
        left: PinDriver::input(left, Pull::Floating)?,
        right: PinDriver::input(right, Pull::Floating)?,
        select: PinDriver::input(select, Pull::Up)?,
    })
}

pub fn is_pressed(button: &PinDriver<'static, Input>) -> bool {
    button.is_low()
}
