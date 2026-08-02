#!/bin/bash
# Fix a flashed Kazeta Zero USB stick in place (no reflash needed).
# Repairs: btrfs size mismatch, missing kernel on EFI partition,
# read-only/default subvolume, and missing var subvolume.
#
# Usage: sudo bash fix-usb.sh [/dev/sdX]
set -e
set -x

USB_DEV=${1:-/dev/sdb}
EFI_PART=${USB_DEV}1
ROOT_PART=${USB_DEV}2
SUBVOL_NAME=kazeta-zero-1.43
ROOT_MNT=/mnt/usb-root
EFI_MNT=/mnt/usb-efi

if [ $EUID -ne 0 ]; then
    echo "Must be run as root"
    exit 1
fi

# 0. Make sure nothing is mounted
umount ${ROOT_MNT} 2>/dev/null || true
umount ${EFI_MNT} 2>/dev/null || true

# 1. Fix "block device size is smaller than total_bytes" -
#    shrinks the recorded fs size to the actual partition size
btrfs rescue fix-device-size ${ROOT_PART}

# 2. Mount the btrfs top-level
mkdir -p ${ROOT_MNT}
mount -o subvolid=5 ${ROOT_PART} ${ROOT_MNT}
btrfs subvolume list ${ROOT_MNT}

# 3. Received subvolumes are read-only - make it writable
#    -f is needed because received_uuid is set (we don't do incremental
#    sends to installed systems, so clearing it is fine)
btrfs property set -f -ts ${ROOT_MNT}/${SUBVOL_NAME} ro false

# 4. Set it as the default subvolume so root=LABEL=frzr_root mounts it
btrfs subvolume set-default ${ROOT_MNT}/${SUBVOL_NAME}

# 5. Create the 'var' subvolume required by /etc/fstab (subvol=var)
if ! btrfs subvolume show ${ROOT_MNT}/var >/dev/null 2>&1; then
    btrfs subvolume create ${ROOT_MNT}/var
fi
# Populate it from the deployment's /var (contains kazeta dir + custom edition)
cp -a ${ROOT_MNT}/${SUBVOL_NAME}/var/. ${ROOT_MNT}/var/ 2>/dev/null || true
mkdir -p ${ROOT_MNT}/var/kazeta/state/wireplumber
chown -R 1000:1000 ${ROOT_MNT}/var/kazeta

# 6. Ensure kernel + initramfs are on the EFI partition
fsck.vfat -a ${EFI_PART} || true
mkdir -p ${EFI_MNT}
mount ${EFI_PART} ${EFI_MNT}
echo "--- EFI partition contents before fix ---"
ls -la ${EFI_MNT}
cp ${ROOT_MNT}/${SUBVOL_NAME}/boot/vmlinuz-linux ${EFI_MNT}/
cp ${ROOT_MNT}/${SUBVOL_NAME}/boot/initramfs-linux.img ${EFI_MNT}/

# 7. Make the boot entry explicit about the subvolume (belt and braces)
cat > ${EFI_MNT}/loader/entries/kazeta-zero.conf <<ENTRYCONF
title Kazeta Zero
linux /vmlinuz-linux
initrd /initramfs-linux.img
options root=LABEL=frzr_root rootflags=subvol=${SUBVOL_NAME} rw quiet splash
ENTRYCONF
cat ${EFI_MNT}/loader/entries/kazeta-zero.conf

# 8. Clean unmount
umount ${EFI_MNT}
umount ${ROOT_MNT}
sync

# 9. Verify the filesystem is now healthy
btrfs check ${ROOT_PART}

echo ""
echo "=========================================="
echo "USB FIX COMPLETE - try booting the mini PC"
echo "=========================================="
