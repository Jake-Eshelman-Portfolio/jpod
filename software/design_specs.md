# Software Design

## Main and Workers

- `main` initializes the hardware and starts the workers.
- Each worker owns its device state and runs on its own thread.
- Screen and SD workers are active objects, have mailbox and address that serves as interface

## Shared SPI Bus

- `main` owns one SPI bus driver.
- Screen and SD workers borrow the bus and create separate device handles with separate chip-select pins.
- ESP-IDF serializes transfers from these device handles on the shared bus.
- Lifetimes keep the device wrappers from outliving the bus

## Button Interrupts

- A normal thread runs `register_button_interrupts` and owns the button pins.
- A full screen mailbox drops that event. 
- A disconnected mailbox ends the button worker since program currently will restart if an active object fails

## Runtime and Failures

- Workers are intended to run for the whole powered session.
- Screen and SD threads use a scope because they borrow the SPI bus.
- The button thread uses normal `spawn` because it owns its inputs and does not borrow the bus.
- A worker exit or handled startup failure logs a short reason and restarts the chip.
- We do not join running workers for shutdown since we plan to full restart anyways
- TODO: Log detailed worker errors before restarting.

## Power and Sleep

- Add explicit worker shutdown later only if we need task restarts or tests that must end cleanly.