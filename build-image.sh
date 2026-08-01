#!/bin/bash

set -e

cleanup_on_error() {
    echo "Script failed! Attempting to clean up mounts..."
    umount "${BUILD_PATH}" 2>/dev/null || true
    umount "${MOUNT_PATH}" 2>/dev/null || true
    losetup -j "${BUILD_IMG}" 2>/dev/null | cut -d : -f 1 | xargs -r losetup -d 2>/dev/null || true
    losetup -j "${TEMP_ROOT_IMG}" 2>/dev/null | cut -d : -f 1 | xargs -r losetup -d 2>/dev/null || true
}
trap cleanup_on_error ERR

set -x

# Timing helper
start_time=$(date +%s)
log_time() {
    local elapsed=$(( $(date +%s) - start_time ))
    echo "[TIMER] $1 took ${elapsed}s"
}

# Parse arguments
FORCE_FULL_BUILD=false
SKIP_COMPRESS=false
USE_DOCKER=false
DOCKER_IMAGE=${DOCKER_IMAGE:-kazeta-zero-builder}
while [[ $# -gt 0 ]]; do
    case $1 in
        --force|-f)
            FORCE_FULL_BUILD=true
            shift
            ;;
        --no-compress|-n)
            SKIP_COMPRESS=true
            shift
            ;;
        --docker|-d)
            USE_DOCKER=true
            shift
            ;;
        --docker-image)
            DOCKER_IMAGE="$2"
            shift 2
            ;;
        --help|-h)
            echo "Usage: $0 [OPTIONS]"
            echo ""
            echo "Options:"
            echo "  --force, -f        Force full rebuild (ignore cache)"
            echo "  --no-compress, -n  Skip xz compression of final image"
            echo "  --docker, -d       Run build inside Docker container"
            echo "  --docker-image     Docker image to use (default: kazeta-zero-builder)"
            echo "  --help, -h         Show this help"
            echo ""
            echo "Caching behavior:"
            echo "  - If btrfs stream exists and is newer than source changes, reuse it"
            echo "  - Use --force to rebuild everything from scratch"
            echo ""
            echo "Docker mode:"
            echo "  - Runs build in isolated Docker container"
            echo "  - Slower but more reproducible across machines"
            echo "  - Requires Docker image with build tools"
            exit 0
            ;;
        *)
            echo "Unknown option: $1"
            exit 1
            ;;
    esac
done

# Docker mode: re-execute inside container
if [ "$USE_DOCKER" = true ]; then
    echo "Running build in Docker container..."
    
    # Check if Docker image exists
    if ! docker image inspect "$DOCKER_IMAGE" >/dev/null 2>&1; then
        echo "Docker image '$DOCKER_IMAGE' not found. Build it first:"
        echo "  docker build -t $DOCKER_IMAGE ."
        exit 1
    fi
    
    # Prepare output directory
    mkdir -p output
    
    # Run build in Docker
    docker run \
        -u root --rm --privileged=true \
        --entrypoint=/workdir/build-image.sh \
        -v "$(pwd):/workdir" \
        -v "$(pwd)/output:/output" \
        -e FORCE_FULL_BUILD="$FORCE_FULL_BUILD" \
        -e SKIP_COMPRESS="$SKIP_COMPRESS" \
        -e NO_COMPRESS="$NO_COMPRESS" \
        "$DOCKER_IMAGE"
    
    exit $?
fi

if [ $EUID -ne 0 ]; then
    echo "$(basename $0) must be run as root"
    exit 1
fi

BUILD_USER=${BUILD_USER:-}
OUTPUT_DIR=${OUTPUT_DIR:-}

source manifest

if [ -z "${SYSTEM_NAME}" ]; then
  echo "SYSTEM_NAME must be specified"
  exit
fi

if [ -z "${VERSION}" ]; then
  echo "VERSION must be specified"
  exit
fi

DISPLAY_VERSION=${VERSION}
LSB_VERSION=${VERSION}
VERSION_NUMBER=${VERSION}

if [ -n "$1" ]; then
    DISPLAY_VERSION="${VERSION} (${1})"
    VERSION="${VERSION}_${1}"
    LSB_VERSION="${LSB_VERSION}　(${1})"
    BUILD_ID="${1}"
fi

# Calculate sizes for final image
# EFI partition: 512MB fixed
# Root partition size will be determined after stream creation
EFI_SIZE=512

# Use home directory for build (more space than /tmp)
BUILD_ROOT=${BUILD_ROOT:-$HOME/kazeta-build}
MOUNT_PATH=${BUILD_ROOT}/${SYSTEM_NAME}-build
BUILD_PATH=${MOUNT_PATH}/subvolume
SNAP_PATH=${MOUNT_PATH}/${SYSTEM_NAME}-${VERSION}
BUILD_IMG=${BUILD_ROOT}/${SYSTEM_NAME}-build.img
EFI_IMG=${BUILD_ROOT}/${SYSTEM_NAME}-efi.img
TEMP_ROOT_IMG=${BUILD_ROOT}/temp-root.img
STREAM_IMG=${SYSTEM_NAME}-${VERSION}.img
FINAL_IMG=${SYSTEM_NAME}-${VERSION}-final.img

mkdir -p ${BUILD_ROOT}
mkdir -p ${MOUNT_PATH}

# Check if we can reuse the btrfs stream
REUSE_STREAM=false
if [ "$FORCE_FULL_BUILD" = false ] && [ -f "${STREAM_IMG}" ]; then
    # Check if stream is newer than key source files
    STREAM_TIME=$(stat -c %Y "${STREAM_IMG}" 2>/dev/null || echo 0)
    
    # Check if any source files are newer than the stream
    NEWER_FILES=$(find bios overlay ra input-daemon rootfs manifest -type f -newer "${STREAM_IMG}" 2>/dev/null | head -1)
    
    if [ -z "$NEWER_FILES" ]; then
        echo "Found cached btrfs stream, no source changes detected"
        REUSE_STREAM=true
    else
        echo "Source files changed, will rebuild"
    fi
fi

if [ "$REUSE_STREAM" = false ]; then
    echo "Building fresh btrfs stream..."
    
    # Create main btrfs image with generous size (we'll shrink it later)
    # Use SIZE * 2 to ensure we have enough space for packages
    fallocate -l $((${SIZE/MB/} * 2))M ${BUILD_IMG}
    mkfs.btrfs -f ${BUILD_IMG}
    mount -t btrfs -o loop,compress-force=zstd:15 ${BUILD_IMG} ${MOUNT_PATH}
    btrfs subvolume create ${BUILD_PATH}

    # copy the makepkg.conf into chroot
    cp /etc/makepkg.conf rootfs/etc/makepkg.conf

    # bootstrap using our configuration
    pacstrap -K -C rootfs/etc/pacman.conf ${BUILD_PATH}

    # copy the builder mirror list into chroot
    mkdir -p rootfs/etc/pacman.d
    cp /etc/pacman.d/mirrorlist rootfs/etc/pacman.d/mirrorlist

    # copy files into chroot
    cp -R manifest rootfs/. ${BUILD_PATH}/

    # Include custom edition config if present (gitignored, for personalized builds)
    if [ -f custom-edition.toml ]; then
        echo "Including custom edition config..."
        mkdir -p ${BUILD_PATH}/var/kazeta
        cp custom-edition.toml ${BUILD_PATH}/var/kazeta/
    fi

    mkdir ${BUILD_PATH}/own_pkgs
    mkdir ${BUILD_PATH}/extra_pkgs

    cp -rv aur-pkgs/*.pkg.tar* ${BUILD_PATH}/extra_pkgs

    # Only copy if there are actual package files (not just .gitkeep)
    if compgen -G "pkgs/*.pkg.tar*" > /dev/null; then
        cp -rv pkgs/*.pkg.tar* ${BUILD_PATH}/own_pkgs
    fi

    if [ -n "${PACKAGE_OVERRIDES}" ]; then
        wget --directory-prefix=/tmp/extra_pkgs ${PACKAGE_OVERRIDES}
        cp -rv /tmp/extra_pkgs/*.pkg.tar* ${BUILD_PATH}/own_pkgs
    fi

    # Build Rust projects (BIOS, overlay, and RA) before entering chroot
    echo "Building Rust projects..."

    # Set Rust toolchain explicitly (sudo doesn't preserve user rustup config)
    export RUSTUP_HOME=${RUSTUP_HOME:-$HOME/.rustup}
    export CARGO_HOME=${CARGO_HOME:-$HOME/.cargo}
    rustup default stable 2>/dev/null || true

    # Build BIOS
    echo "Building kazeta-bios..."
    cd bios
    cargo build --release
    if [ ! -f "target/release/kazeta-bios" ]; then
        echo "ERROR: Failed to build kazeta-bios"
        exit 1
    fi
    cd ..

    # Build overlay
    echo "Building kazeta-overlay..."
    cd overlay
    cargo build --release --bin kazeta-overlay --features daemon
    if [ ! -f "target/release/kazeta-overlay" ]; then
        echo "ERROR: Failed to build kazeta-overlay"
        exit 1
    fi
    cd ..

    # Build RetroAchievements CLI
    echo "Building kazeta-ra..."
    cd ra
    cargo build --release
    if [ ! -f "target/release/kazeta-ra" ]; then
        echo "ERROR: Failed to build kazeta-ra"
        exit 1
    fi
    cd ..

    # Build input daemon
    echo "Building kazeta-input..."
    cd input-daemon
    cargo build --release
    if [ ! -f "target/release/kazeta-input" ]; then
        echo "ERROR: Failed to build kazeta-input"
        exit 1
    fi
    cd ..

    # chroot into target
    mount --bind ${BUILD_PATH} ${BUILD_PATH}
    arch-chroot ${BUILD_PATH} /bin/bash <<EOF
set -e
set -x

source /manifest

pacman-key --populate

echo "LANG=en_US.UTF-8" > /etc/locale.conf
locale-gen

# Disable parallel downloads
sed -i '/ParallelDownloads/s/^/#/g' /etc/pacman.conf

# Cannot check space in chroot
sed -i '/CheckSpace/s/^/#/g' /etc/pacman.conf

# update package databases
pacman --noconfirm -Syy

# Disable check and debug for makepkg on the final image
sed -i '/BUILDENV/s/ check/ !check/g' /etc/makepkg.conf
sed -i '/OPTIONS/s/ debug/ !debug/g' /etc/makepkg.conf

# install kernel package
if [ "$KERNEL_PACKAGE_ORIGIN" == "local" ] ; then
    pacman --noconfirm -U --overwrite '*' \
    /own_pkgs/${KERNEL_PACKAGE}-*.pkg.tar.zst
else
    pacman --noconfirm -S "${KERNEL_PACKAGE}" "${KERNEL_PACKAGE}-headers"
fi

# install own override packages
if [ -n "$(ls -A '/own_pkgs')" ]; then
    pacman --noconfirm -U --overwrite '*' /own_pkgs/*
    rm -rf /var/cache/pacman/pkg
fi

# install packages
pacman --noconfirm -S --overwrite '*' --disable-download-timeout ${PACKAGES}
rm -rf /var/cache/pacman/pkg

# install AUR packages
pacman --noconfirm -U --overwrite '*' /extra_pkgs/*
rm -rf /var/cache/pacman/pkg

# Install the new iptables
yes | pacman -S iptables-nft

# enable services
if [ -n "${SERVICES}" ]; then
    systemctl enable ${SERVICES}
fi

# enable user services
if [ -n "${USER_SERVICES}" ]; then
    systemctl --global enable ${USER_SERVICES}
fi

# disable root login
passwd --lock root

# create user
groupadd -r autologin
useradd -m ${USERNAME} -G autologin,wheel
echo "${USERNAME}:${USERNAME}" | chpasswd

# set the default editor, so visudo works
echo "export EDITOR=/usr/bin/vim" >> /etc/bash.bashrc

echo "${SYSTEM_NAME}" > /etc/hostname

# enable multicast dns in avahi
sed -i "/^hosts:/ s/resolve/mdns resolve/" /etc/nsswitch.conf

# configure ssh
echo "
AuthorizedKeysFile	.ssh/authorized_keys
PasswordAuthentication yes
ChallengeResponseAuthentication no
UsePAM yes
PrintMotd no # pam does that
Subsystem	sftp	/usr/lib/ssh/sftp-server
" > /etc/ssh/sshd_config

echo "
LABEL=frzr_root /var       btrfs defaults,subvol=var,rw,noatime,nodatacow 0 0
LABEL=frzr_root /frzr_root btrfs defaults,subvol=/,rw,noatime,nodatacow 0 0
" > /etc/fstab

echo "
LSB_VERSION=1.4
DISTRIB_ID=${SYSTEM_NAME}
DISTRIB_RELEASE=\"${LSB_VERSION}\"
DISTRIB_DESCRIPTION=${SYSTEM_DESC}
" > /etc/lsb-release

echo 'NAME="${SYSTEM_DESC}"
VERSION="${DISPLAY_VERSION}"
VERSION_ID="${VERSION_NUMBER}"
BUILD_ID="${BUILD_ID}"
PRETTY_NAME="${SYSTEM_DESC} ${DISPLAY_VERSION}"
ID=${SYSTEM_NAME}
ID_LIKE=arch
ANSI_COLOR="1;31"
HOME_URL="${WEBSITE}"
DOCUMENTATION_URL="${DOCUMENTATION_URL}"
BUG_REPORT_URL="${BUG_REPORT_URL}"' > /usr/lib/os-release

# install extra certificates
if [ -n "$(ls -A '/extra_certs')" ]; then
    trust anchor --store /extra_certs/*.crt
fi

# run post install hook
postinstallhook

# record installed packages & versions
pacman -Q > /manifest

# preserve installed package database
mkdir -p /usr/var/lib/pacman
cp -r /var/lib/pacman/local /usr/var/lib/pacman/

# move kernel image and initrd to a default location if "linux" is not used
if [ ${KERNEL_PACKAGE} != 'linux' ] ; then
    mv /boot/vmlinuz-${KERNEL_PACKAGE} /boot/vmlinuz-linux
    mv /boot/initramfs-${KERNEL_PACKAGE}.img /boot/initramfs-linux.img
    mv /boot/initramfs-${KERNEL_PACKAGE}-fallback.img /boot/initramfs-linux-fallback.img
fi

# clean up/remove unnecessary files
rm -rf \
/own_pkgs \
/extra_pkgs \
/extra_certs \
/var \

rm -rf ${FILES_TO_DELETE}

# create necessary directories
mkdir -p /var
mkdir -p /var/kazeta
mkdir -p /frzr_root
mkdir -p /efi

chown ${USERNAME}:${USERNAME} /var/kazeta
EOF

    #defrag the image
    btrfs filesystem defragment -r ${BUILD_PATH}
    log_time "btrfs defragment"

    # copy files into chroot again
    cp -R rootfs/. ${BUILD_PATH}/
    rm -rf ${BUILD_PATH}/extra_certs

    # Copy built Rust binaries to the image
    echo "Installing Rust binaries..."
    cp bios/target/release/kazeta-bios ${BUILD_PATH}/usr/bin/
    cp overlay/target/release/kazeta-overlay ${BUILD_PATH}/usr/bin/
    cp ra/target/release/kazeta-ra ${BUILD_PATH}/usr/bin/
    cp input-daemon/target/release/kazeta-input ${BUILD_PATH}/usr/bin/
    chmod +x ${BUILD_PATH}/usr/bin/kazeta-bios
    chmod +x ${BUILD_PATH}/usr/bin/kazeta-overlay
    chmod +x ${BUILD_PATH}/usr/bin/kazeta-ra
    chmod +x ${BUILD_PATH}/usr/bin/kazeta-input

    # Copy bundled runtimes (.kzr) into the image if present
    echo "Installing bundled runtimes..."
    RUNTIME_SRC="$(pwd)/runtimes"
    RUNTIME_DST="${BUILD_PATH}/usr/share/kazeta/runtimes"
    if [ -d "${RUNTIME_SRC}" ]; then
        mapfile -t runtime_files < <(find "${RUNTIME_SRC}" -maxdepth 2 -type f -name "*.kzr")
        if [ "${#runtime_files[@]}" -gt 0 ]; then
            mkdir -p "${RUNTIME_DST}"
            for runtime in "${runtime_files[@]}"; do
                echo "  Copying $(basename "${runtime}")..."
                cp "${runtime}" "${RUNTIME_DST}/"
            done
        else
            echo "No .kzr runtime files found in ${RUNTIME_SRC}"
        fi
    else
        echo "Runtime source directory not found: ${RUNTIME_SRC}"
    fi

    echo "${SYSTEM_NAME}-${VERSION}" > ${BUILD_PATH}/build_info
    echo "" >> ${BUILD_PATH}/build_info
    cat ${BUILD_PATH}/manifest >> ${BUILD_PATH}/build_info
    rm ${BUILD_PATH}/manifest

    # freeze archive date of build to avoid package drift on unlock
    if [ -z "${ARCHIVE_DATE}" ]; then
        export TODAY_DATE=$(date +%Y/%m/%d)
        echo "Server=https://archive.archlinux.org/repos/${TODAY_DATE}/\$repo/os/\$arch" > \
        ${BUILD_PATH}/etc/pacman.d/mirrorlist
    fi

    btrfs subvolume snapshot -r ${BUILD_PATH} ${SNAP_PATH}
    log_time "btrfs snapshot"

    btrfs send -f ${STREAM_IMG} ${SNAP_PATH}
    log_time "btrfs send"

    cp ${BUILD_PATH}/build_info build_info.txt

    # clean up
    if mountpoint -q "${BUILD_PATH}"; then
        echo "Unmounting subvolume..."
        umount "${BUILD_PATH}" || sleep 2 && umount -l "${BUILD_PATH}" 2>/dev/null || true
    fi

    if mountpoint -q "${MOUNT_PATH}"; then
        echo "Unmounting image..."
        umount "${MOUNT_PATH}" || sleep 2 && umount -l "${MOUNT_PATH}" 2>/dev/null || true
    fi

    losetup -j "${BUILD_IMG}" | cut -d : -f 1 | xargs -r losetup -d

    sync

    echo "Removing temporary files..."
    rm -rf "${MOUNT_PATH}"
    rm -f "${BUILD_IMG}"
else
    echo "Reusing cached btrfs stream: ${STREAM_IMG}"
fi

# UEFI Assembly (always run this part)
echo "Creating UEFI boot partition..."
fallocate -l 512M ${EFI_IMG}
mkfs.vfat -F32 ${EFI_IMG}

# Create directory structure for EFI files
mkdir -p ${MOUNT_PATH}-efi-staging/EFI/BOOT
mkdir -p ${MOUNT_PATH}-efi-staging/loader/entries

# Copy systemd-boot EFI binary
cp ${BUILD_PATH}/usr/lib/systemd/boot/efi/systemd-bootx64.efi ${MOUNT_PATH}-efi-staging/EFI/BOOT/BOOTX64.EFI 2>/dev/null || \
    cp /usr/lib/systemd/boot/efi/systemd-bootx64.efi ${MOUNT_PATH}-efi-staging/EFI/BOOT/BOOTX64.EFI

# Create loader configuration
cat > ${MOUNT_PATH}-efi-staging/loader/loader.conf <<LOADERCONF
default kazeta-zero
timeout 0
editor no
LOADERCONF

# Create boot entry
# Note: We don't specify rootflags=subvol= here because the btrfs stream
# creates a subvolume with the snapshot name (e.g., kazeta-zero-1.43), not @
# The kernel will auto-detect the default subvolume set by btrfs receive
cat > ${MOUNT_PATH}-efi-staging/loader/entries/kazeta-zero.conf <<ENTRYCONF
title Kazeta Zero
linux /vmlinuz-linux
initrd /initramfs-linux.img
options root=LABEL=frzr_root quiet splash
ENTRYCONF

# Copy kernel and initramfs to staging
cp ${BUILD_PATH}/boot/vmlinuz-linux ${MOUNT_PATH}-efi-staging/ 2>/dev/null || true
cp ${BUILD_PATH}/boot/initramfs-linux.img ${MOUNT_PATH}-efi-staging/ 2>/dev/null || true

# Copy files to FAT image using mtools
mcopy -i ${EFI_IMG} -s ${MOUNT_PATH}-efi-staging/* ::/

# Clean up staging
rm -rf ${MOUNT_PATH}-efi-staging

# Measure the actual uncompressed size of the stream
echo "Measuring stream size..."
STREAM_SIZE_BYTES=$(stat -c %s "${STREAM_IMG}")
# Uncompressed size is roughly 1.5-2x compressed for zstd
# We'll create a test receive to measure exactly
TEST_IMG=/tmp/test-measure.img
fallocate -l $((${SIZE/MB/} * 3))M ${TEST_IMG}
mkfs.btrfs -f ${TEST_IMG}
mkdir -p /tmp/test-measure-mount
mount -o loop ${TEST_IMG} /tmp/test-measure-mount
btrfs receive /tmp/test-measure-mount < ${STREAM_IMG} 2>/dev/null || true
ACTUAL_ROOT_SIZE=$(df -m /tmp/test-measure-mount | tail -1 | awk '{print $2}')
umount /tmp/test-measure-mount
losetup -j ${TEST_IMG} | cut -d : -f 1 | xargs -r losetup -d
rm -rf /tmp/test-measure-mount ${TEST_IMG}

# Use exact measured size (no margin - partition must match filesystem metadata)
ROOT_SIZE=${ACTUAL_ROOT_SIZE}
FINAL_SIZE=$((EFI_SIZE + ROOT_SIZE))

echo "Measured root size: ${ACTUAL_ROOT_SIZE}MB, Using exact size: ${ROOT_SIZE}MB"

# Create final disk image with calculated size
echo "Creating final disk image with GPT partition table..."
echo "EFI partition: ${EFI_SIZE}MB, Root partition: ${ROOT_SIZE}MB, Total: ${FINAL_SIZE}MB"
fallocate -l ${FINAL_SIZE}M ${FINAL_IMG}

# Create GPT partition table
parted -s ${FINAL_IMG} mklabel gpt
parted -s ${FINAL_IMG} mkpart primary fat32 1MiB ${EFI_SIZE}MiB
parted -s ${FINAL_IMG} set 1 esp on
parted -s ${FINAL_IMG} mkpart primary btrfs ${EFI_SIZE}MiB 100%

# Write EFI partition
dd if=${EFI_IMG} of=${FINAL_IMG} bs=1M seek=1 conv=notrunc

# Mount the root partition directly and receive the btrfs stream
# This avoids the temp image size mismatch entirely
mkdir -p ${MOUNT_PATH}-final-root

# Use losetup with partition offset to mount the root partition
LOOP_DEV=$(losetup -f --show -o $((${EFI_SIZE}*1024*1024)) ${FINAL_IMG})
mkfs.btrfs -f ${LOOP_DEV}
mount ${LOOP_DEV} ${MOUNT_PATH}-final-root
btrfs receive ${MOUNT_PATH}-final-root < ${STREAM_IMG}
btrfs filesystem label ${MOUNT_PATH}-final-root frzr_root
umount ${MOUNT_PATH}-final-root
losetup -d ${LOOP_DEV}
rm -rf ${MOUNT_PATH}-final-root

# Clean up
rm -f ${EFI_IMG}

# Rename final image
mv ${FINAL_IMG} ${SYSTEM_NAME}-${VERSION}.img

log_time "UEFI image creation"

# Compress if not skipped
IMG_FILENAME="${SYSTEM_NAME}-${VERSION}.img.tar.xz"
if [ "$SKIP_COMPRESS" = false ] && [ -z "${NO_COMPRESS}" ]; then
    XZ_THREADS=${XZ_THREADS:-$(nproc)}
    echo "Compressing with xz level 6 using ${XZ_THREADS} threads..."
    tar -c -I"xz -6 -T${XZ_THREADS}" -f ${IMG_FILENAME} ${SYSTEM_NAME}-${VERSION}.img
    log_time "xz compression"
    rm ${SYSTEM_NAME}-${VERSION}.img

    sha256sum ${IMG_FILENAME} > sha256sum.txt
    cat sha256sum.txt

    # Move the image to the output directory, if one was specified.
    if [ -n "${OUTPUT_DIR}" ]; then
        mkdir -p "${OUTPUT_DIR}"
        mv ${IMG_FILENAME} ${OUTPUT_DIR}
        mv build_info.txt ${OUTPUT_DIR} 2>/dev/null || true
        mv sha256sum.txt ${OUTPUT_DIR}
    fi
else
    echo "Skipping compression (use --no-compress to skip this step)"
    sha256sum ${SYSTEM_NAME}-${VERSION}.img > sha256sum.txt
    cat sha256sum.txt
fi

echo ""
echo "=========================================="
echo "BUILD COMPLETE!"
echo "=========================================="
if [ "$SKIP_COMPRESS" = false ] && [ -z "${NO_COMPRESS}" ]; then
    echo "Image: ${IMG_FILENAME}"
else
    echo "Image: ${SYSTEM_NAME}-${VERSION}.img (uncompressed)"
fi
echo "Size: $(ls -lh ${SYSTEM_NAME}-${VERSION}.img* 2>/dev/null | awk '{print $5}')"
echo ""
echo "To flash to USB:"
echo "  sudo dd if=${SYSTEM_NAME}-${VERSION}.img of=/dev/sdX bs=4M status=progress oflag=sync"
echo "=========================================="
