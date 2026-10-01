#!/usr/bin/env bash
set -euo pipefail

script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
project_root=$(cd -- "$script_dir/.." && pwd)
samples_dir="$project_root/mp3_samples"

usage() {
    printf 'Usage: sudo %s /dev/WHOLE_REMOVABLE_USB_DISK\n' "$0" >&2
    printf 'Example: sudo %s /dev/sde\n' "$0" >&2
}

if (( $# != 1 )); then
    usage
    exit 2
fi

if (( EUID != 0 )); then
    printf 'Run this script with sudo so it can partition and format the card.\n' >&2
    usage
    exit 1
fi

for command_name in lsblk findmnt readlink wipefs sfdisk partprobe udevadm mkfs.vfat mount umount mktemp mkdir rmdir cp sync find awk tr sed stat sha256sum; do
    if ! command -v "$command_name" >/dev/null 2>&1; then
        printf 'Required command not found: %s\n' "$command_name" >&2
        printf 'Ubuntu/WSL packages: sudo apt install util-linux fdisk parted dosfstools udev coreutils findutils gawk\n' >&2
        exit 1
    fi
done

if [[ ! -d "$samples_dir" ]]; then
    printf 'Sample directory does not exist: %s\n' "$samples_dir" >&2
    exit 1
fi

mapfile -d '' -t mp3_files < <(find "$samples_dir" -type f -iname '*.mp3' -print0)
if (( ${#mp3_files[@]} == 0 )); then
    printf 'No MP3 files found under %s. Add test audio before formatting the card.\n' "$samples_dir" >&2
    exit 1
fi

requested_device=$1
if [[ "$requested_device" != /dev/* ]]; then
    printf 'The target must be a /dev block-device path.\n' >&2
    exit 1
fi

device=$(readlink -f -- "$requested_device")
if [[ ! -b "$device" ]]; then
    printf 'Not a block device: %s\n' "$requested_device" >&2
    exit 1
fi

device_type=$(lsblk -dnro TYPE -- "$device" | tr -d '[:space:]')
if [[ "$device_type" != disk ]]; then
    printf 'Refusing %s: select the whole disk, not a partition.\n' "$device" >&2
    exit 1
fi

removable=$(lsblk -dnro RM -- "$device" | tr -d '[:space:]')
transport=$(lsblk -dnro TRAN -- "$device" | tr -d '[:space:]')
if [[ "$transport" != usb || "$removable" != 1 ]]; then
    printf 'Refusing %s: expected a whole disk identified as removable USB storage.\n' "$device" >&2
    exit 1
fi

assert_unused() {
    local node node_name mount_paths swap_path rest
    local nodes
    nodes=$(lsblk -nrpo NAME -- "$device")
    while IFS= read -r node; do
        node_name=${node##*/}
        mount_paths=$(lsblk -nro MOUNTPOINTS -- "$node")
        if [[ -n "$mount_paths" ]]; then
            printf 'Refusing in-use device %s: mounted filesystem or active swap. Unmount it manually first.\n' "$node" >&2
            return 1
        fi
        if [[ ! -d "/sys/class/block/$node_name/holders" ]]; then
            printf 'Cannot inspect device holders for %s; refusing.\n' "$node" >&2
            return 1
        fi
        for holder in "/sys/class/block/$node_name/holders/"*; do
            if [[ -e "$holder" ]]; then
                printf 'Refusing %s: another block device uses it.\n' "$node" >&2
                return 1
            fi
        done
        while read -r swap_path rest; do
            [[ "$swap_path" == Filename ]] && continue
            if [[ "$(readlink -f -- "$swap_path")" == "$node" ]]; then
                printf 'Refusing %s: active swap.\n' "$node" >&2
                return 1
            fi
        done < /proc/swaps
    done <<< "$nodes"
}

device_identity() {
    readlink -f -- "$requested_device"
    udevadm info --query=path --name="$device"
    lsblk -bdnro MAJ:MIN,SIZE,RO,RM,TRAN,MODEL,SERIAL -- "$device"
}

if [[ "$(lsblk -dnro RO -- "$device" | tr -d '[:space:]')" != 0 ]]; then
    printf 'Refusing read-only device %s.\n' "$device" >&2
    exit 1
fi
assert_unused
initial_identity=$(device_identity | sha256sum)

target_major_minor=$(lsblk -dnro MAJ:MIN -- "$device" | tr -d '[:space:]')
root_source=$(findmnt -nro SOURCE --target /)
if [[ -b "$root_source" ]]; then
    while IFS= read -r root_major_minor; do
        if [[ "$target_major_minor" == "$root_major_minor" ]]; then
            printf 'Refusing %s: it contains the mounted root filesystem.\n' "$device" >&2
            exit 1
        fi
    done < <(lsblk -srnpo MAJ:MIN -- "$root_source")
fi

model=$(lsblk -dnro MODEL -- "$device" | sed 's/^[[:space:]]*//;s/[[:space:]]*$//')
size=$(lsblk -dnro SIZE -- "$device" | tr -d '[:space:]')
size_bytes=$(lsblk -bdnro SIZE -- "$device" | tr -d '[:space:]')
printf 'Target disk: %s (resolved from %s)\n' "$device" "$requested_device"
printf 'Model: %s\nTransport: %s\nRemovable: %s\nSize: %s\n' \
    "${model:-unknown}" "${transport:-unknown}" "$removable" "$size"
printf 'Disk and current partitions/filesystems:\n'
lsblk -o NAME,TYPE,FSTYPE,SIZE,MOUNTPOINTS -- "$device"
printf 'This will ERASE the entire disk, create one MBR/FAT32 partition, and copy %d MP3 file(s) to /mp3_samples.\n' \
    "${#mp3_files[@]}"
printf 'Linux identifies the USB reader, not the card type. Confirm the printed model and capacity match your SD card.\n'
confirmation_text="ERASE $requested_device $size_bytes BYTES"
printf 'Type exactly: %s\n> ' "$confirmation_text"
IFS= read -r confirmation
if [[ "$confirmation" != "$confirmation_text" ]]; then
    printf 'Confirmation did not match; nothing was changed.\n' >&2
    exit 1
fi

if [[ "$(device_identity | sha256sum)" != "$initial_identity" ]]; then
    printf 'Device identity changed during confirmation; refusing to erase.\n' >&2
    exit 1
fi
assert_unused

wipefs --all --force "$device"
printf 'label: dos\nunit: sectors\nstart=2048, type=c\n' | sfdisk --wipe always "$device"
partprobe "$device"
udevadm settle

mapfile -t partitions < <(lsblk -nrpo NAME,TYPE -- "$device" | awk '$2 == "part" { print $1 }')
if (( ${#partitions[@]} != 1 )); then
    printf 'Expected one new partition on %s, found %d. Stopping.\n' "$device" "${#partitions[@]}" >&2
    exit 1
fi
partition=${partitions[0]}
mkfs.vfat -F 32 -n JPOD "$partition"

mount_dir=$(mktemp -d)
mounted=0
cleanup() {
    if (( mounted )); then
        umount -- "$mount_dir" || true
    fi
    rmdir -- "$mount_dir" 2>/dev/null || true
}
trap cleanup EXIT

mount "$partition" "$mount_dir"
mounted=1
mkdir -p "$mount_dir/mp3_samples"
cp -R -- "$samples_dir/." "$mount_dir/mp3_samples/"
sync
umount -- "$mount_dir"
mounted=0
rmdir -- "$mount_dir"
trap - EXIT

printf 'Done. The card is FAT32, labeled JPOD, with samples in /mp3_samples.\n'