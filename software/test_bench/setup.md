# ESP32 build and flash setup (WSL2)

This project builds ESP-IDF firmware for the original Xtensa ESP32. The Cargo
configuration targets `xtensa-esp32-espidf`, uses ESP-IDF v5.5.3, links with
`ldproxy`, and runs firmware with `espflash flash --monitor`. Firmware cannot
run as a Linux executable: `cargo run` builds it, flashes an attached ESP32,
and opens its serial monitor.

## Rust and Cargo in WSL

On a fresh Ubuntu WSL installation, install the basic host tools first:

```bash
sudo apt update
sudo apt install -y curl ca-certificates git build-essential python3 python3-venv
```

Install Rust and Cargo through the official rustup installer (accept the
default stable toolchain when prompted):

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"
rustc +stable --version
cargo +stable --version
```

Rustup adds Cargo to the PATH for future shells; sourcing `~/.cargo/env`
makes it available immediately in the current shell. The `+stable` commands
below explicitly use stable Rust even inside this project, which selects
the `esp` toolchain through `rust-toolchain.toml`.

## Tools installed in WSL

Rust and Cargo were already installed on this WSL instance, so the preceding
fresh-install commands were not run here. These commands were used to add the
missing ESP tools:

```bash
cargo +stable install espup --locked
espup install
cargo +stable install ldproxy --locked
cargo +stable install espflash --locked
```

`espup install` installs the `esp` Rust toolchain, the Xtensa cross-compiler,
and Clang tooling. It generates `~/export-esp.sh`, which sets the compiler
`PATH` and `LIBCLANG_PATH`. Source it in each new WSL shell before building:

```bash
source "$HOME/export-esp.sh"
```

`ldproxy` is the linker named in `.cargo/config.toml`; without it, the build
fails with `linker 'ldproxy' not found`. `espflash` is the configured Cargo
runner; without it, `cargo run` builds but cannot launch the flash command.
ESP-IDF and its build tools are fetched by the project build as needed. No
system package installation was needed for the successful build here.

## Build

From this project directory in WSL:

```bash
source "$HOME/export-esp.sh"
cargo clean  # optional: removes previous build artifacts
cargo build
```

The clean and build completed successfully after installing the tools above.
The first build can take a while as it downloads and compiles ESP-IDF. You do
not need a connected ESP32 to run `cargo build`.

## Attach the board and flash

Connect the board to Windows, then in **Windows PowerShell**:

```powershell
usbipd list
usbipd bind --busid <BUSID>       # run as Administrator; normally once per device
usbipd attach --wsl --busid <BUSID>
```

Replace `<BUSID>` with the board's bus ID from `usbipd list`. In **WSL**:

```bash
ls /dev/ttyUSB* /dev/ttyACM* 2>/dev/null
source "$HOME/export-esp.sh"
cargo run
```

Linux has the `cp210x`, `ch341`, and `cdc_acm` kernel modules available for
common ESP32 USB connections; no separate Linux USB-serial driver was installed.
A Windows CP210x driver is not required solely for USB passthrough, though it
may be useful to access the board as a Windows COM port. If the WSL serial port
exists but access is denied, add your user to `dialout` and start a new WSL
session:

```bash
sudo usermod -aG dialout "$USER"
```

`cargo run` was tested without the board connected: the firmware built, then
`espflash` reported `No serial ports could be detected`. Flashing and the
`Hello, world!` log message still need verification with an attached board.
