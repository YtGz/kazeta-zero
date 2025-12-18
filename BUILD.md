# Kazeta+ Build & Deployment Guide

This guide explains how to build, package, and deploy Kazeta+ updates.

## Overview

The build system consists of three main scripts:

1. **`build-all.sh`** - Builds all Rust binaries and optionally runtime packages
2. **`create-upgrade-kit.sh`** - Packages everything into a distributable upgrade kit
3. **`update-kazeta-plus.sh`** - Deploys updates to a running Kazeta+ system

## Components Built

### Rust Binaries
- **kazeta** (kazeta-bios) - Main BIOS/launcher
- **kazeta-ra** - RetroAchievements daemon
- **kazeta-input-daemon** - Input management daemon
- **kazeta-overlay** - On-screen overlay daemon

### Runtime Packages
- **gba-1.0.kzr** - Game Boy Advance runtime (mGBA)
- **ps2-1.0.kzr** - PlayStation 2 runtime (PCSX2)

## Build Workflow

### Step 1: Build Everything

```bash
# Build all binaries in release mode (recommended for production)
./build-all.sh --release

# Or build with runtimes included
./build-all.sh --release

# Debug builds (faster compilation, larger binaries)
./build-all.sh --debug --skip-runtimes
```

**What it does:**
- Compiles all Rust projects in the specified mode
- Copies binaries to `rootfs/usr/bin/` with appropriate names
- Optionally builds runtime packages (.kzr files)

**Output locations:**
- Binaries: `rootfs/usr/bin/kazeta*`
- Runtimes: `*.kzr` files in project root

### Step 2: Create Upgrade Kit

```bash
./create-upgrade-kit.sh
```

**What it does:**
- Runs `build-all.sh` automatically to ensure binaries are up-to-date
- Creates a versioned directory with all necessary files
- Copies configuration files, systemd services, udev rules
- Includes all binaries from `rootfs/usr/bin/`
- Includes runtime packages if they exist
- Creates a ZIP archive for distribution

**Prompts:**
- Version number (e.g., `1.3`)
- Confirmation if directory already exists

**Output:**
```
~/Desktop/kazeta_assets/upgrade_kits/kazeta-plus-upgrade-kit-X.X/
~/Desktop/kazeta_assets/upgrade_kits/kazeta-plus-upgrade-kit-X.X.zip
```

### Step 3: Deploy to Kazeta+ System

On your Kazeta+ device:

```bash
# Extract the upgrade kit
unzip kazeta-plus-upgrade-kit-X.X.zip
cd kazeta-plus-upgrade-kit-X.X

# Run the upgrade script
sudo ./upgrade-to-plus.sh    # For initial installation
# OR
sudo ./update-kazeta-plus.sh  # For updates to existing installation
```

## Deployment Scripts

### upgrade-to-plus.sh (Initial Installation)
For upgrading base Kazeta to Kazeta+ or fresh installations:

- Installs WiFi packages if needed
- Configures network connectivity
- Installs all system packages
- Builds DKMS modules
- Copies all configuration files and binaries
- Enables system services
- **Installs runtime packages**
- Requires reboot

### update-kazeta-plus.sh (Updates)
For updating an already-running Kazeta+ system:

- **Detects new packages** and installs them
- Copies updated files
- **Detects changed binaries** and restarts their services
- **Updates runtime packages** if changed
- Reloads udev and systemd
- Usually no reboot required (unless noted)

**Smart Features:**
- Only installs packages added since last update
- Only restarts services whose binaries changed
- Only updates runtimes that have changed
- Minimal disruption to running system

## Service Mappings

The update script automatically restarts services when their binaries change:

| Binary | Service |
|--------|---------|
| `kazeta` | (BIOS - runs on demand) |
| `kazeta-ra` | `kazeta-ra.service` |
| `kazeta-input-daemon` | `kazeta-input-daemon.service` |
| `kazeta-overlay` | `kazeta-overlay.service` |
| `kazeta-profile-loader` | `kazeta-profile-loader.service` |
| `inputplumber` | `inputplumber.service` |

## Development Workflow

### Quick Testing (Local Development)

```bash
# Build only binaries, skip runtimes
./build-all.sh --debug --skip-runtimes

# Test locally with dev-run.sh or individual cargo commands
cd bios && cargo run --features dev
```

### Creating a Release

```bash
# 1. Build everything in release mode
./build-all.sh --release

# 2. Create upgrade kit (prompts for version)
./create-upgrade-kit.sh

# 3. Upload to GitHub releases or distribute as needed
# Users will download and run upgrade-to-plus.sh or update-kazeta-plus.sh
```

### Iterating on Updates

```bash
# Make code changes to any daemon
vim overlay/src/main.rs

# Rebuild only what changed
cd overlay && cargo build --release --features daemon
cd ..

# Copy to rootfs (build-all.sh does this automatically)
cp overlay/target/release/kazeta-overlay rootfs/usr/bin/

# Test on device
scp -r rootfs/usr/bin/kazeta-overlay user@device:/path/to/upgrade-kit/rootfs/usr/bin/
ssh user@device "sudo /path/to/upgrade-kit/update-kazeta-plus.sh"
```

## Troubleshooting

### Binaries not found after build
- Ensure you're building with the correct features: `--features daemon` for overlay
- Check `target/release/` or `target/debug/` for the binary

### Service not restarting
- Check service name in `update-kazeta-plus.sh` matches systemd service file
- Verify binary name matches the case statement in the update script

### Runtime not installing
- Ensure `.kzr` files are in project root before running `create-upgrade-kit.sh`
- Build runtimes with `./build-all.sh --release` (without `--skip-runtimes`)

### Update script says "no changes"
- Compare checksums: `md5sum /path/to/old /path/to/new`
- Ensure files were actually copied to `rootfs/usr/bin/`

## Files Modified by Scripts

### build-all.sh
- Creates: `rootfs/usr/bin/kazeta*` binaries
- Creates: `*.kzr` runtime files in project root

### create-upgrade-kit.sh
- Creates: `~/Desktop/kazeta_assets/upgrade_kits/kazeta-plus-upgrade-kit-X.X/`
- Modifies: None (read-only operation)

### update-kazeta-plus.sh (on target system)
- Creates/Updates: `/frzr_root/deployments/kazeta-*/usr/bin/kazeta*`
- Creates/Updates: `/frzr_root/deployments/kazeta-*/etc/systemd/system/*.service`
- Creates/Updates: `/frzr_root/deployments/kazeta-*/etc/udev/rules.d/*`
- Creates/Updates: `/usr/share/kazeta/runtimes/*.kzr`
- Creates: `/frzr_root/deployments/kazeta-*/.kazeta-installed-packages` (package marker)
- Restarts: Modified systemd services

## Adding New Components

### Adding a New Daemon

1. Create the Rust project
2. Add to `build-all.sh`:
   ```bash
   build_rust_binary "My Daemon" "$SCRIPT_DIR/my-daemon" "my-daemon"
   copy_binary "$SCRIPT_DIR/my-daemon/target/$BUILD_DIR/my-daemon" "kazeta-my-daemon" "My Daemon"
   ```

3. Create systemd service in `rootfs/etc/systemd/system/kazeta-my-daemon.service`

4. Add service mapping in `update-kazeta-plus.sh`:
   ```bash
   "kazeta-my-daemon")
       if [ "$needs_restart" = true ]; then
           SERVICES_TO_RESTART+=("kazeta-my-daemon.service")
       fi
       ;;
   ```

5. Add to services list in `upgrade-to-plus.sh` if it should auto-enable

### Adding a New Runtime

1. Create runtime build script in `runtimes/PLATFORM/build.sh`
2. `build-all.sh` will automatically detect and build it
3. `create-upgrade-kit.sh` will automatically include it
4. `update-kazeta-plus.sh` will automatically install it

No code changes needed - just follow the pattern of existing runtimes!

## Package Management

New packages can be added to both scripts by updating the `CURRENT_PACKAGES` array:

**In `upgrade-to-plus.sh` (line 129-137):**
```bash
PACKAGES_TO_INSTALL=(
    "brightnessctl" "keyd" "rsync" "xxhash" "iwd" "networkmanager"
    # ... existing packages ...
    "your-new-package"  # Add here
)
```

**In `update-kazeta-plus.sh` (line 48-57):**
```bash
CURRENT_PACKAGES=(
    "brightnessctl" "keyd" "rsync" "xxhash" "iwd" "networkmanager"
    # ... existing packages ...
    "your-new-package"  # Add here (must match upgrade-to-plus.sh)
)
```

The update script will automatically detect and install new packages on the next update!
