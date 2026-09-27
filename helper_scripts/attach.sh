#!/usr/bin/env bash
set -euo pipefail

if ! grep -qi microsoft /proc/sys/kernel/osrelease; then
    printf 'Run this script inside WSL2.\n' >&2
    exit 1
fi
if ! command -v powershell.exe >/dev/null || ! command -v wslpath >/dev/null; then
    printf 'WSL Windows interop is required to run powershell.exe and wslpath.\n' >&2
    exit 1
fi

script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
windows_script=$(wslpath -w "$script_dir/usbipd_attach.ps1")
case "${1:-}" in
    --list)
        if (( $# != 1 )); then printf 'Usage: %s [--setup] [--instance-id ID] | --list\n' "$0" >&2; exit 2; fi
        powershell.exe -NoProfile -ExecutionPolicy Bypass -File "$windows_script" -List
        exit $?
        ;;
    --setup)
        if (( $# == 1 )); then
            powershell.exe -NoProfile -ExecutionPolicy Bypass -File "$windows_script" -Setup
        elif (( $# == 3 )) && [[ $2 == --instance-id ]]; then
            powershell.exe -NoProfile -ExecutionPolicy Bypass -File "$windows_script" -Setup -InstanceId "$3"
        else
            printf 'Usage: %s [--setup] [--instance-id ID] | --list\n' "$0" >&2
            exit 2
        fi
        ;;
    --instance-id)
        if (( $# != 2 )); then printf 'Usage: %s [--setup] [--instance-id ID] | --list\n' "$0" >&2; exit 2; fi
        powershell.exe -NoProfile -ExecutionPolicy Bypass -File "$windows_script" -InstanceId "$2"
        ;;
    '')
        powershell.exe -NoProfile -ExecutionPolicy Bypass -File "$windows_script"
        ;;
    *)
        printf 'Usage: %s [--setup] [--instance-id ID] | --list\n' "$0" >&2
        exit 2
        ;;
esac

printf 'WSL serial ports:\n'
found=0
for port in /dev/serial/by-id/* /dev/ttyUSB* /dev/ttyACM*; do
    if [[ -e "$port" ]]; then
        printf '  %s\n' "$port"
        found=1
    fi
done
if (( ! found )); then
    printf '  None yet; check dmesg or reconnect the USB cable if it does not appear.\n'
fi