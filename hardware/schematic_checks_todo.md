# Errors and Loose Ends to be Culled Later

## Schematic

- [ ] Check transistor pinout and orientation.
- [ ] Double-check each pin in the pinmap file to ensure that it matches the schematic and PCB layout.
- [ ] Verify that the power supply section is correctly designed and that all components are rated appropriately.
    - [ ] Check typical operating voltages for components (absolute max/min)
- [ ] Ensure that all decoupling capacitors are placed correctly and have the right values (e.g., closeness to the power pins of ICs).
- [ ] Modify IO4 Pin to be used for the DAC's RST Pin and reroute the screen's backlight pin to something else, or figure out handling sleep mode for the screen via software. (This issue is linked to the ESP32's EN Pin being finicky and prone to errors if used as an IO pin.)
- [ ] Route or check that the jack input voltage spike is correctly handled via ESD protection. 