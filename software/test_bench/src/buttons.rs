use core::num::NonZero;
use esp_idf_svc::hal::{
    delay::FreeRtos,
    gpio::{Gpio32, Gpio34, Gpio35, Gpio36, Gpio39, Input, InterruptType, PinDriver, Pull},
    task::notification::{Notification, Notifier},
};
use esp_idf_svc::sys::EspError;
use std::sync::{mpsc::TrySendError, Arc};
use std::time::{Duration, Instant};

use crate::active_object::Address;
use crate::screen::{ScreenMessage, ScreenStatus};

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
    // All pins except select are pulled up via hardware due to pin constraints
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

fn register_button_interrupt(
    button: &mut PinDriver<'static, Input>,
    waker: Arc<Notifier>,
) -> Result<(), EspError> {
    button.set_interrupt_type(InterruptType::NegEdge)?;

    unsafe {
        button.subscribe_nonstatic(move || {
            let _ = waker.notify(NonZero::new(1).unwrap());
        })?;
    }

    button.enable_interrupt()
}


pub fn register_button_interrupts(
    mut buttons: Buttons,
    screen_address: Address<ScreenMessage>,
) -> Result<(), EspError> {
    let notification = Notification::new();
    register_button_interrupt(&mut buttons.up, notification.notifier())?;
    register_button_interrupt(&mut buttons.down, notification.notifier())?;
    register_button_interrupt(&mut buttons.left, notification.notifier())?;
    register_button_interrupt(&mut buttons.right, notification.notifier())?;
    register_button_interrupt(&mut buttons.select, notification.notifier())?;

    let held_down_threshold = Duration::from_secs(5);

    loop {
        notification.wait_any();
        // Button press interrupt will arrive here, debounce check, reenable, back to sleep
        FreeRtos::delay_ms(30);
        if is_pressed(&buttons.up) {
            
            match screen_address.try_post(ScreenMessage::SetStatus(ScreenStatus::UpPressed)) {
                Ok(()) | Err(TrySendError::Full(_)) => {}
                Err(TrySendError::Disconnected(_)) => break,
            }
            match screen_address.try_post(ScreenMessage::UpPressed) {
                Ok(()) | Err(TrySendError::Full(_)) => {}
                Err(TrySendError::Disconnected(_)) => break,
            }
        }
        if is_pressed(&buttons.down) {
            match screen_address.try_post(ScreenMessage::SetStatus(ScreenStatus::DownPressed)) {
                Ok(()) | Err(TrySendError::Full(_)) => {}
                Err(TrySendError::Disconnected(_)) => break,
            }
            match screen_address.try_post(ScreenMessage::DownPressed) {
                Ok(()) | Err(TrySendError::Full(_)) => {}
                Err(TrySendError::Disconnected(_)) => break,
            }
        }
        if is_pressed(&buttons.left) {
            match screen_address.try_post(ScreenMessage::SetStatus(ScreenStatus::LeftPressed)) {
                Ok(()) | Err(TrySendError::Full(_)) => {}
                Err(TrySendError::Disconnected(_)) => break,
            }
        }
        if is_pressed(&buttons.right) {
            match screen_address.try_post(ScreenMessage::SetStatus(ScreenStatus::RightPressed)) {
                Ok(()) | Err(TrySendError::Full(_)) => {}
                Err(TrySendError::Disconnected(_)) => break,
            }
        }
        if is_pressed(&buttons.select) {
            let start_time = Instant::now();
            let long_press;
            while is_pressed(&buttons.select) && start_time.elapsed() < held_down_threshold {
                FreeRtos::delay_ms(10);
                continue;
            }
            if start_time.elapsed() >= held_down_threshold {
                long_press = true;
            } else {
                long_press = false;
            }
            match screen_address.try_post(ScreenMessage::ShortSelectPress(long_press)) {
                Ok(()) | Err(TrySendError::Full(_)) => {}
                Err(TrySendError::Disconnected(_)) => break,
            }
        }
        buttons.up.enable_interrupt()?;
        buttons.down.enable_interrupt()?;
        buttons.left.enable_interrupt()?;
        buttons.right.enable_interrupt()?;
        buttons.select.enable_interrupt()?;
    }
    // Pin driver destructor will unsubscribe pin and restart GPIO, do not need to disable/unsubscribe

    Ok(())
}