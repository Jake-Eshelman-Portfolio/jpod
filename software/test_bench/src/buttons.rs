use esp_idf_svc::hal::gpio::{Gpio32, Gpio34, Gpio35, Gpio36, Gpio39, Input, PinDriver, Pull};
use esp_idf_svc::sys::EspError;

#[repr(usize)]
pub enum Button {
    Up,
    Down,
    Left,
    Right,
    Select,
}

pub fn init_buttons(
    up: Gpio34<'static>,
    down: Gpio35<'static>,
    left: Gpio36<'static>,
    right: Gpio39<'static>,
    select: Gpio32<'static>,
) -> Result<Vec<PinDriver<'static, Input>>, EspError> {
    Ok(vec![
        PinDriver::input(up, Pull::None)?,
        PinDriver::input(down, Pull::None)?,
        PinDriver::input(left, Pull::None)?,
        PinDriver::input(right, Pull::None)?,
        PinDriver::input(select, Pull::Up)?,
    ])
}

pub fn is_pressed(buttons: &[PinDriver<'static, Input>], button: Button) -> bool {
    buttons[button as usize].is_low()
}
