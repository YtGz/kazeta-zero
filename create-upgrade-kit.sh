#!/bin/bash

# ---
# Script to automate the creation of a Kazeta+ upgrade kit.
# It checks for debug flags, creates the directory structure, copies all
# required files, and zips the final kit for release.
# ---

# Exit immediately if a command exits with a non-zero status.
set -e

# --- Configuration ---
SOURCE_DIR="$HOME/Programs/kazeta-plus"
DEST_BASE_DIR="$HOME/Desktop/kazeta_assets/upgrade_kits"
MAIN_RS_PATH="$SOURCE_DIR/bios/src/main.rs"

# --- Main Logic ---
read -p "Enter the version number for the new upgrade kit (e.g., 1.2): " VERSION
if [ -z "$VERSION" ]; then
    echo "Error: Version number cannot be empty."
    exit 1
fi

KIT_DIR_NAME="kazeta-plus-upgrade-kit-$VERSION"
KIT_FULL_PATH="$DEST_BASE_DIR/$KIT_DIR_NAME"

echo "Creating Kazeta+ Upgrade Kit v$VERSION"
echo "Source: $SOURCE_DIR"
echo "Destination: $KIT_FULL_PATH"
echo "-----------------------------------------------------"

if [ -d "$KIT_FULL_PATH" ]; then
    read -p "Directory '$KIT_FULL_PATH' already exists. Overwrite? (y/n): " CONFIRM
    if [[ "$CONFIRM" != "y" ]]; then
        echo "Aborted."
        exit 0
    fi
    echo "Removing existing directory..."
    rm -rf "$KIT_FULL_PATH"
fi

# 4. Create the directory structure
echo "Creating directory structure..."
mkdir -p "$KIT_FULL_PATH/rootfs/etc/keyd"
mkdir -p "$KIT_FULL_PATH/rootfs/etc/sudoers.d"
mkdir -p "$KIT_FULL_PATH/rootfs/etc/systemd/system"
mkdir -p "$KIT_FULL_PATH/rootfs/etc/udev/rules.d"
mkdir -p "$KIT_FULL_PATH/rootfs/usr/bin"
mkdir -p "$KIT_FULL_PATH/rootfs/usr/share/inputplumber/profiles"
mkdir -p "$KIT_FULL_PATH/aur-pkgs"
echo "Directory structure created."

# 5. Copy the main upgrade script
echo "Copying upgrade-to-plus.sh script from local source..."
cp "$SOURCE_DIR/upgrade-to-plus.sh" "$KIT_FULL_PATH/upgrade-to-plus.sh"
chmod +x "$KIT_FULL_PATH/upgrade-to-plus.sh"
echo "Copy complete."

# 6. Copy all necessary files from your local dev environment
echo "Copying files from rootfs..."
cp "$SOURCE_DIR/rootfs/etc/keyd/default.conf" "$KIT_FULL_PATH/rootfs/etc/keyd/"
cp "$SOURCE_DIR/rootfs/etc/sudoers.d/99-kazeta-plus" "$KIT_FULL_PATH/rootfs/etc/sudoers.d/"

echo "Copying systemd services..."
cp "$SOURCE_DIR/rootfs/etc/systemd/system/kazeta-profile-loader.service" "$KIT_FULL_PATH/rootfs/etc/systemd/system/"
cp "$SOURCE_DIR/rootfs/etc/systemd/system/optical-mount@.service" "$KIT_FULL_PATH/rootfs/etc/systemd/system/"
cp "$SOURCE_DIR/rootfs/etc/systemd/system/optical-unmount@.service" "$KIT_FULL_PATH/rootfs/etc/systemd/system/"

echo "Copying udev rules..."
cp "$SOURCE_DIR/rootfs/etc/udev/rules.d/51-gcadapter.rules" "$KIT_FULL_PATH/rootfs/etc/udev/rules.d/"
cp "$SOURCE_DIR/rootfs/etc/udev/rules.d/99-optical-automount.rules" "$KIT_FULL_PATH/rootfs/etc/udev/rules.d/"

echo "Building all Rust binaries..."
# Run the build-all script to ensure all binaries are up to date
if [ -f "$SOURCE_DIR/build-all.sh" ]; then
    echo "Running build-all.sh to compile all Rust binaries..."
    cd "$SOURCE_DIR"
    bash ./build-all.sh --release --skip-runtimes
    cd - > /dev/null
    echo "Build complete."
else
    echo "WARNING: build-all.sh not found. Skipping automatic build."
    echo "Make sure binaries are already built!"
fi

echo "Copying all binaries and scripts from rootfs/usr/bin..."
# Copy ALL files from rootfs/usr/bin (includes both shell scripts and compiled binaries)
cp -r "$SOURCE_DIR/rootfs/usr/bin/"* "$KIT_FULL_PATH/rootfs/usr/bin/"
echo "All binaries copied."

echo "Copying inputplumber profiles..."
cp "$SOURCE_DIR/rootfs/usr/share/inputplumber/profiles/"*.yaml "$KIT_FULL_PATH/rootfs/usr/share/inputplumber/profiles/"

echo "Copying gcadapter-oc-dkms source..."
cp -r "$SOURCE_DIR/aur-pkgs/gcadapter-oc-dkms" "$KIT_FULL_PATH/aur-pkgs/"

echo "Copying runtime packages..."
# Create runtimes directory in the kit
mkdir -p "$KIT_FULL_PATH/runtimes"

# Copy any .kzr runtime files
if ls "$SOURCE_DIR"/*.kzr 1> /dev/null 2>&1; then
    cp "$SOURCE_DIR"/*.kzr "$KIT_FULL_PATH/runtimes/"
    echo "Runtime packages copied:"
    ls -lh "$KIT_FULL_PATH/runtimes"/*.kzr | awk '{printf "  - %-30s %5s\n", $9, $5}'
else
    echo "No runtime packages (.kzr files) found. Run build-all.sh without --skip-runtimes first."
fi

echo "All files copied successfully."

# 7. Create the ZIP archive
echo "Creating ZIP archive..."
(
    # Go one level up from the kit directory to include the base folder in the zip
    cd "$DEST_BASE_DIR" && \
    zip -r "$KIT_DIR_NAME.zip" "$KIT_DIR_NAME"
)
echo "ZIP archive created."

echo "-----------------------------------------------------"
echo "Success! Upgrade kit created at:"
echo "$KIT_FULL_PATH"
echo "and"
echo "$DEST_BASE_DIR/$KIT_DIR_NAME.zip" # Corrected zip path display
echo "-----------------------------------------------------"
echo "Reminder: Manually create and upload 'kazeta-wifi-pack.zip' to the release page."
echo "Users will need to place the unzipped 'kazeta-wifi-pack' folder next to the upgrade script."
echo "-----------------------------------------------------"
