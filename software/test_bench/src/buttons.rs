use esp_idf_svc::hal::gpio::{Input, InputPin, PinDriver, Pull};
use esp_idf_svc::sys::EspError;

pub fn button<'a, Pin: InputPin + 'a>(pin: Pin) -> Result<PinDriver<'a, Input>, EspError> {
    PinDriver::input(pin, Pull::Up)
}

pub fn is_pressed(button: &PinDriver<'_, Input>) -> bool {
    button.is_low()
}
