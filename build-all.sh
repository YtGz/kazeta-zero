#!/bin/bash

# ===================================================================
# Kazeta+ Complete Build Script
# ===================================================================
# Builds all Rust binaries and runtimes for deployment
#
# Usage:
#   ./build-all.sh [--release] [--skip-runtimes]
#
# Options:
#   --release        Build in release mode (recommended for production)
#   --debug          Build in debug mode (faster compilation, larger binaries)
#   --skip-runtimes  Skip building runtime packages
#   --help           Show this help message
# ===================================================================

set -e
set -o pipefail

# --- Color Definitions ---
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
BLUE='\033[0;34m'
NC='\033[0m'

# --- Configuration ---
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BUILD_MODE="release"
BUILD_RUNTIMES=true

# --- Argument Parsing ---
while [[ $# -gt 0 ]]; do
    case $1 in
        --release)
            BUILD_MODE="release"
            shift
            ;;
        --debug)
            BUILD_MODE="debug"
            shift
            ;;
        --skip-runtimes)
            BUILD_RUNTIMES=false
            shift
            ;;
        --help)
            echo "Usage: $0 [--release|--debug] [--skip-runtimes]"
            exit 0
            ;;
        *)
            echo -e "${RED}Unknown option: $1${NC}"
            exit 1
            ;;
    esac
done

# --- Build Configuration ---
if [ "$BUILD_MODE" = "release" ]; then
    CARGO_FLAGS="--release"
    BUILD_DIR="release"
    echo -e "${GREEN}Building in RELEASE mode${NC}"
else
    CARGO_FLAGS=""
    BUILD_DIR="debug"
    echo -e "${YELLOW}Building in DEBUG mode${NC}"
fi

echo -e "${BLUE}╔════════════════════════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║           Kazeta+ Complete Build Script                   ║${NC}"
echo -e "${BLUE}╚════════════════════════════════════════════════════════════╝${NC}"
echo ""

# ===================================================================
# STEP 1: Build Rust Binaries
# ===================================================================

echo -e "${BLUE}═══ Step 1: Building Rust Binaries ═══${NC}"
echo ""

# Helper function to build a Rust project
build_rust_binary() {
    local project_name=$1
    local project_path=$2
    local binary_name=$3
    local extra_flags=$4

    echo -e "${YELLOW}→ Building $project_name...${NC}"

    if [ ! -d "$project_path" ]; then
        echo -e "${RED}  ERROR: Project directory not found: $project_path${NC}"
        return 1
    fi

    cd "$project_path"

    if [ -n "$extra_flags" ]; then
        cargo build $CARGO_FLAGS $extra_flags
    else
        cargo build $CARGO_FLAGS
    fi

    if [ -f "target/$BUILD_DIR/$binary_name" ]; then
        echo -e "${GREEN}  ✓ Built: target/$BUILD_DIR/$binary_name${NC}"
    else
        echo -e "${RED}  ERROR: Binary not found after build: target/$BUILD_DIR/$binary_name${NC}"
        return 1
    fi

    cd "$SCRIPT_DIR"
    echo ""
}

# Build kazeta-bios
build_rust_binary "Kazeta BIOS" "$SCRIPT_DIR/bios" "kazeta-bios"

# Build RA (RetroAchievements daemon)
build_rust_binary "RA Daemon" "$SCRIPT_DIR/ra" "ra"

# Build Input Daemon
build_rust_binary "Input Daemon" "$SCRIPT_DIR/input-daemon" "input-daemon"

# Build Overlay Daemon (requires daemon feature)
build_rust_binary "Overlay Daemon" "$SCRIPT_DIR/overlay" "kazeta-overlay" "--features daemon"

echo -e "${GREEN}✓ All Rust binaries built successfully!${NC}"
echo ""

# ===================================================================
# STEP 2: Copy Binaries to rootfs/usr/bin
# ===================================================================

echo -e "${BLUE}═══ Step 2: Copying Binaries to rootfs/usr/bin ═══${NC}"
echo ""

DEST_BIN_DIR="$SCRIPT_DIR/rootfs/usr/bin"
mkdir -p "$DEST_BIN_DIR"

# Helper function to copy binary
copy_binary() {
    local source_path=$1
    local dest_name=$2
    local description=$3

    if [ ! -f "$source_path" ]; then
        echo -e "${RED}  ERROR: Source binary not found: $source_path${NC}"
        return 1
    fi

    echo -e "${YELLOW}→ Copying $description...${NC}"
    cp "$source_path" "$DEST_BIN_DIR/$dest_name"
    chmod +x "$DEST_BIN_DIR/$dest_name"

    # Get file size
    local size=$(du -h "$DEST_BIN_DIR/$dest_name" | cut -f1)
    echo -e "${GREEN}  ✓ Copied: $dest_name ($size)${NC}"
    echo ""
}

# Copy kazeta-bios as 'kazeta'
copy_binary "$SCRIPT_DIR/bios/target/$BUILD_DIR/kazeta-bios" "kazeta" "Kazeta BIOS"

# Copy RA daemon
copy_binary "$SCRIPT_DIR/ra/target/$BUILD_DIR/ra" "kazeta-ra" "RA Daemon"

# Copy Input daemon
copy_binary "$SCRIPT_DIR/input-daemon/target/$BUILD_DIR/input-daemon" "kazeta-input-daemon" "Input Daemon"

# Copy Overlay daemon
copy_binary "$SCRIPT_DIR/overlay/target/$BUILD_DIR/kazeta-overlay" "kazeta-overlay" "Overlay Daemon"

echo -e "${GREEN}✓ All binaries copied to rootfs/usr/bin${NC}"
echo ""

# ===================================================================
# STEP 3: Build Runtime Packages (Optional)
# ===================================================================

if [ "$BUILD_RUNTIMES" = true ]; then
    echo -e "${BLUE}═══ Step 3: Building Runtime Packages ═══${NC}"
    echo ""

    # Build GBA Runtime
    if [ -f "$SCRIPT_DIR/runtimes/gba/build.sh" ]; then
        echo -e "${YELLOW}→ Building GBA Runtime...${NC}"
        cd "$SCRIPT_DIR/runtimes/gba"
        # Check if we're on macOS or Linux and adjust accordingly
        if [[ "$OSTYPE" == "darwin"* ]]; then
            echo -e "${YELLOW}  Skipping GBA runtime on macOS (Linux-only)${NC}"
        else
            bash build.sh --use-system --clean
            echo -e "${GREEN}  ✓ GBA runtime built${NC}"
        fi
        cd "$SCRIPT_DIR"
        echo ""
    fi

    # Build PS2 Runtime
    if [ -f "$SCRIPT_DIR/runtimes/ps2/build.sh" ]; then
        echo -e "${YELLOW}→ Building PS2 Runtime...${NC}"
        cd "$SCRIPT_DIR/runtimes/ps2"
        if [[ "$OSTYPE" == "darwin"* ]]; then
            echo -e "${YELLOW}  Skipping PS2 runtime on macOS (Linux-only)${NC}"
        else
            bash build.sh --use-system --clean
            echo -e "${GREEN}  ✓ PS2 runtime built${NC}"
        fi
        cd "$SCRIPT_DIR"
        echo ""
    fi

    echo -e "${GREEN}✓ Runtime packages processed${NC}"
    echo ""
else
    echo -e "${YELLOW}Skipping runtime package builds (--skip-runtimes specified)${NC}"
    echo ""
fi

# ===================================================================
# STEP 4: Summary
# ===================================================================

echo -e "${BLUE}╔════════════════════════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║                    Build Complete!                         ║${NC}"
echo -e "${BLUE}╚════════════════════════════════════════════════════════════╝${NC}"
echo ""
echo -e "${GREEN}Built Binaries (in rootfs/usr/bin):${NC}"
echo -e "  • kazeta              (Kazeta BIOS)"
echo -e "  • kazeta-ra           (RetroAchievements daemon)"
echo -e "  • kazeta-input-daemon (Input management daemon)"
echo -e "  • kazeta-overlay      (Overlay display daemon)"
echo ""

if [ "$BUILD_RUNTIMES" = true ]; then
    echo -e "${GREEN}Built Runtimes:${NC}"
    if [ -f "$SCRIPT_DIR/gba-1.0.kzr" ]; then
        echo -e "  • gba-1.0.kzr         ($(du -h "$SCRIPT_DIR/gba-1.0.kzr" | cut -f1))"
    fi
    if [ -f "$SCRIPT_DIR/ps2-1.0.kzr" ]; then
        echo -e "  • ps2-1.0.kzr         ($(du -h "$SCRIPT_DIR/ps2-1.0.kzr" | cut -f1))"
    fi
    echo ""
fi

echo -e "${YELLOW}Next Steps:${NC}"
echo -e "  1. Run ${BLUE}./create-upgrade-kit.sh${NC} to package for deployment"
echo -e "  2. Or use ${BLUE}./update-kazeta-plus.sh${NC} to deploy to a running system"
echo ""

# Show binary sizes
echo -e "${GREEN}Binary Sizes:${NC}"
ls -lh "$DEST_BIN_DIR"/kazeta* | awk '{printf "  %-25s %5s\n", $9, $5}'
echo ""

echo -e "${GREEN}Build completed successfully!${NC}"
