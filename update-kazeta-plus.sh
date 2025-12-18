#!/bin/bash

# Exit immediately if any command fails.
set -e
# Add pipefail to ensure pipeline failures are caught
set -o pipefail

# --- Color Definitions for pretty output ---
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m'

### ===================================================================
###                       PRE-FLIGHT CHECKS
### ===================================================================

echo -e "${GREEN}Starting Kazeta+ Update...${NC}"

# 1. Check for Root Privileges
if [ "$EUID" -ne 0 ]; then
  echo -e "${RED}Error: This script must be run with sudo.${NC}"
  exit 1
fi

# 2. Find Paths
SCRIPT_DIR=$( cd -- "$( dirname -- "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )
DEPLOYMENT_DIR=$(find /frzr_root/deployments -name "kazeta-*" -type d | head -n 1)

if [ -z "$DEPLOYMENT_DIR" ]; then
    echo -e "${RED}Error: Could not find Kazeta installation. Is frzr-unlock running?${NC}"
    exit 1
fi

echo -e "Found Kazeta installation at: ${YELLOW}$DEPLOYMENT_DIR${NC}"
echo "--------------------------------------------------"

### ===================================================================
###                 CHECK FOR NEW PACKAGES
### ===================================================================

echo -e "${YELLOW}Step 1: Checking for new packages...${NC}"

# Store the package list in a marker file
PACKAGE_MARKER_FILE="$DEPLOYMENT_DIR/.kazeta-installed-packages"

# Define the current package list (must match upgrade-to-plus.sh)
CURRENT_PACKAGES=(
    "brightnessctl" "keyd" "rsync" "xxhash" "iwd" "networkmanager"
    "ffmpeg" "unzip" "bluez" "bluez-utils"
    "base-devel" "dkms" "linux-headers"
    "noto-fonts" "ttf-dejavu" "ttf-liberation" "noto-fonts-emoji"
    "pipewire-alsa" "alsa-utils"
    "mangohud" "lib32-mangohud" "gamemode" "lib32-gamemode" "openssh" "nano"
    "clang"
    "steam"
)

NEW_PACKAGES=()

if [ -f "$PACKAGE_MARKER_FILE" ]; then
    # Read previously installed packages
    mapfile -t PREVIOUS_PACKAGES < "$PACKAGE_MARKER_FILE"

    # Find packages in CURRENT_PACKAGES but not in PREVIOUS_PACKAGES
    for pkg in "${CURRENT_PACKAGES[@]}"; do
        found=false
        for prev_pkg in "${PREVIOUS_PACKAGES[@]}"; do
            if [ "$pkg" = "$prev_pkg" ]; then
                found=true
                break
            fi
        done
        if [ "$found" = false ]; then
            NEW_PACKAGES+=("$pkg")
        fi
    done

    if [ ${#NEW_PACKAGES[@]} -gt 0 ]; then
        echo -e "${YELLOW}  -> Detected ${#NEW_PACKAGES[@]} new package(s):${NC}"
        for pkg in "${NEW_PACKAGES[@]}"; do
            echo "     - $pkg"
        done

        echo "  -> Installing new packages..."
        for pkg in "${NEW_PACKAGES[@]}"; do
            if [ "$pkg" = "steam" ]; then
                # Special handling for Steam
                pacman -S --noconfirm --needed --assume-installed lsb-release steam
            else
                pacman -S --noconfirm --needed "$pkg"
            fi
        done
        echo -e "${GREEN}  -> New packages installed.${NC}"
    else
        echo -e "${GREEN}  -> No new packages detected.${NC}"
    fi
else
    echo -e "${YELLOW}  -> No package marker file found. Creating it now...${NC}"
    echo "  -> This will allow future updates to detect new packages."
fi

# Update the marker file with the current package list
printf '%s\n' "${CURRENT_PACKAGES[@]}" > "$PACKAGE_MARKER_FILE"

echo "--------------------------------------------------"

### ===================================================================
###                 SYSTEM FILE COPY & SERVICES
### ===================================================================

echo -e "${YELLOW}Step 2: Copying updated system files...${NC}"

# Track which services need to be restarted
SERVICES_TO_RESTART=()

# Copy configuration files
echo "  -> Copying /etc files (sudoers, systemd, udev, etc)..."
rsync -av "$SCRIPT_DIR/rootfs/etc/" "$DEPLOYMENT_DIR/etc/"

echo "  -> Copying /usr/share files (inputplumber)..."
rsync -av "$SCRIPT_DIR/rootfs/usr/share/" "$DEPLOYMENT_DIR/usr/share/"

# Enforce strict permissions for sudoers to ensure passwordless rules work
echo "  -> Correcting ownership and permissions for sudoers.d..."
SUDOERS_D_DIR="$DEPLOYMENT_DIR/etc/sudoers.d"
if [ -d "$SUDOERS_D_DIR" ]; then
    chown -R root:root "$SUDOERS_D_DIR"
    chmod 755 "$SUDOERS_D_DIR"
    find "$SUDOERS_D_DIR" -type f -exec chmod 440 {} \;
fi

# Ensure udev rules are root owned
echo "  -> Correcting ownership and permissions for udev rules..."
UDEV_RULES_DEST_DIR="$DEPLOYMENT_DIR/etc/udev/rules.d"
if [ -d "$UDEV_RULES_DEST_DIR" ]; then
    chown -R root:root "$UDEV_RULES_DEST_DIR"
    chmod 755 "$UDEV_RULES_DEST_DIR"
    find "$UDEV_RULES_DEST_DIR" -type f -exec chmod 644 {} \;
fi

# Function to copy binary, set executable, AND set ROOT ownership
# Also detects if the binary has changed and marks service for restart
backup_and_copy() {
    local source_file=$1
    local dest_file=$2
    local filename=$(basename "$source_file")
    local needs_restart=false

    echo "  -> Processing executable: $filename"

    # Check if file changed by comparing checksums
    if [ -f "$dest_file" ]; then
        if ! cmp -s "$source_file" "$dest_file"; then
            needs_restart=true
            echo "     (detected changes - will restart associated services)"
        fi
        mv "$dest_file" "$dest_file.bak"
    fi

    cp "$source_file" "$dest_file"
    chown root:root "$dest_file"
    chmod 755 "$dest_file"

    # Map binaries to their services
    case "$filename" in
        "kazeta-profile-loader")
            if [ "$needs_restart" = true ]; then
                SERVICES_TO_RESTART+=("kazeta-profile-loader.service")
            fi
            ;;
        "inputplumber")
            if [ "$needs_restart" = true ]; then
                SERVICES_TO_RESTART+=("inputplumber.service")
            fi
            ;;
        "kazeta-ra")
            if [ "$needs_restart" = true ]; then
                SERVICES_TO_RESTART+=("kazeta-ra.service")
            fi
            ;;
        "kazeta-input-daemon")
            if [ "$needs_restart" = true ]; then
                SERVICES_TO_RESTART+=("kazeta-input-daemon.service")
            fi
            ;;
        "kazeta-overlay")
            if [ "$needs_restart" = true ]; then
                SERVICES_TO_RESTART+=("kazeta-overlay.service")
            fi
            ;;
        # Add other daemon mappings here as needed
    esac
}

DEST_BIN_DIR="$DEPLOYMENT_DIR/usr/bin"
for executable in "$SCRIPT_DIR/rootfs/usr/bin/"*; do
    if [ -f "$executable" ]; then
        backup_and_copy "$executable" "$DEST_BIN_DIR/$(basename "$executable")"
    fi
done

echo -e "${GREEN}System files updated.${NC}"
echo "--------------------------------------------------"

### ===================================================================
###                  CLEANUP DEPRECATED FILES
### ===================================================================

echo -e "${YELLOW}Step 3: Cleaning up deprecated files and services...${NC}"

# List of deprecated files to remove
DEPRECATED_FILES=(
    # Old Wayland session (replaced with X11 session)
    "$DEPLOYMENT_DIR/usr/share/wayland-sessions/kazeta.desktop"
    # Old overlay service (deprecated)
    "$DEPLOYMENT_DIR/etc/systemd/system/kazeta-overlay.service"
)

# List of deprecated systemd services to stop and disable
DEPRECATED_SERVICES=(
    "kazeta-overlay.service"
)

# Remove deprecated files
FILES_REMOVED=0
for file in "${DEPRECATED_FILES[@]}"; do
    if [ -f "$file" ]; then
        echo "  -> Removing deprecated file: $(basename "$file")"
        rm -f "$file"
        FILES_REMOVED=$((FILES_REMOVED + 1))
    fi
done

# Stop and disable deprecated services
SERVICES_CLEANED=0
for service in "${DEPRECATED_SERVICES[@]}"; do
    if systemctl is-enabled --quiet "$service" 2>/dev/null; then
        echo "  -> Disabling deprecated service: $service"
        systemctl disable --now "$service" 2>/dev/null || true
        SERVICES_CLEANED=$((SERVICES_CLEANED + 1))
    fi
done

if [ $FILES_REMOVED -eq 0 ] && [ $SERVICES_CLEANED -eq 0 ]; then
    echo "  -> No deprecated files or services found."
else
    echo -e "${GREEN}Cleanup complete: removed $FILES_REMOVED file(s), cleaned $SERVICES_CLEANED service(s).${NC}"
fi

echo "--------------------------------------------------"

### ===================================================================
###                       RELOAD UDEV RULES
### ===================================================================

echo -e "${YELLOW}Step 4: Reloading udev rules...${NC}"
udevadm control --reload-rules && udevadm trigger
echo -e "${GREEN}Udev rules reloaded.${NC}"
echo "--------------------------------------------------"

### ===================================================================
###                  RELOAD SYSTEMD & RESTART SERVICES
### ===================================================================

echo -e "${YELLOW}Step 5: Reloading systemd and restarting updated services...${NC}"

# Reload systemd to pick up any changed service files
echo "  -> Reloading systemd daemon..."
systemctl daemon-reload

# Restart services that had binary changes
if [ ${#SERVICES_TO_RESTART[@]} -gt 0 ]; then
    # Remove duplicates
    SERVICES_TO_RESTART=($(printf '%s\n' "${SERVICES_TO_RESTART[@]}" | sort -u))

    echo "  -> Restarting services with updated binaries:"
    for service in "${SERVICES_TO_RESTART[@]}"; do
        echo "     - $service"
        if systemctl is-active --quiet "$service"; then
            systemctl restart "$service"
        else
            echo "       (service not running, enabling and starting...)"
            systemctl enable --now "$service"
        fi
    done
else
    echo "  -> No service binaries were changed, skipping service restarts."
fi

echo -e "${GREEN}Services updated.${NC}"
echo "--------------------------------------------------"

### ===================================================================
###                  INSTALL RUNTIME PACKAGES
### ===================================================================

echo -e "${YELLOW}Step 6: Installing runtime packages...${NC}"

RUNTIMES_DIR="$SCRIPT_DIR/runtimes"
KAZETA_RUNTIMES_DIR="/usr/share/kazeta/runtimes"

if [ -d "$RUNTIMES_DIR" ] && [ -n "$(ls -A "$RUNTIMES_DIR"/*.kzr 2>/dev/null)" ]; then
    echo "  -> Found runtime packages in upgrade kit."

    # Ensure the kazeta runtimes directory exists
    mkdir -p "$KAZETA_RUNTIMES_DIR"

    RUNTIMES_INSTALLED=()
    for runtime_file in "$RUNTIMES_DIR"/*.kzr; do
        runtime_name=$(basename "$runtime_file")
        dest_file="$KAZETA_RUNTIMES_DIR/$runtime_name"

        # Check if runtime already exists and compare
        if [ -f "$dest_file" ]; then
            if cmp -s "$runtime_file" "$dest_file"; then
                echo "  -> $runtime_name already up to date, skipping."
                continue
            else
                echo "  -> Updating $runtime_name..."
            fi
        else
            echo "  -> Installing new runtime: $runtime_name..."
        fi

        cp "$runtime_file" "$dest_file"
        RUNTIMES_INSTALLED+=("$runtime_name")
    done

    if [ ${#RUNTIMES_INSTALLED[@]} -gt 0 ]; then
        echo -e "${GREEN}  -> Installed/updated runtimes:${NC}"
        for runtime in "${RUNTIMES_INSTALLED[@]}"; do
            echo "     - $runtime"
        done
    else
        echo -e "${GREEN}  -> All runtimes already up to date.${NC}"
    fi
else
    echo "  -> No runtime packages found in upgrade kit, skipping."
fi

echo "--------------------------------------------------"

### ===================================================================
###                             COMPLETE
### ===================================================================

echo -e "${GREEN}Kazeta+ update complete!${NC}"

if [ ${#SERVICES_TO_RESTART[@]} -gt 0 ]; then
    echo -e "${YELLOW}The following services were restarted:${NC}"
    for service in "${SERVICES_TO_RESTART[@]}"; do
        echo "  - $service"
    done
else
    echo -e "${YELLOW}No services needed to be restarted.${NC}"
fi

echo ""
echo -e "${YELLOW}Note: If you updated system service files or udev rules,${NC}"
echo -e "${YELLOW}you may want to reboot for all changes to take full effect.${NC}"
