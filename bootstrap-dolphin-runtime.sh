#!/bin/bash
# ===================================================================
# Kazeta Zero — Dolphin Runtime Bootstrap Script
# ===================================================================
# Builds the dolphin-standalone .kzr runtime for offline GameCube
# RetroAchievements, and optionally fetches achievement definitions.
#
# dolphin-emu and erofs-utils are included in the Kazeta OS image
# (see the `manifest` file). This script just builds the .kzr runtime
# package (run script + Dolphin config) and the Rust binaries.
#
# The resulting .kzr contains only the run script and config —
# the emulator binary itself is installed system-wide via the OS image.
#
# Usage:
#   ./bootstrap-dolphin-runtime.sh [options]
#
# Options:
#   --skip-build       Skip building Rust binaries (use existing)
#   --skip-kzr         Skip building the .kzr runtime
#   --fetch GAME_ID    Fetch achievement definitions for a game ID
#                      (requires RA credentials via --username/--api-key)
#   --username USER    RetroAchievements username (for --fetch)
#   --api-key KEY      RetroAchievements web API key (for --fetch)
#   --output-dir DIR   Output directory for fetched definitions
#                      (default: ./cartridge-data)
#   --help             Show this help message
# ===================================================================

set -e
set -o pipefail

GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
BLUE='\033[0;34m'
NC='\033[0m'

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
RA_DIR="${SCRIPT_DIR}/ra"
RUNTIME_NAME="dolphin-standalone-1.0"
RUNTIME_LABEL="dolphin"
KZR_FILE="${SCRIPT_DIR}/${RUNTIME_NAME}.kzr"

SKIP_BUILD=false
SKIP_KZR=false
FETCH_GAME_ID=""
RA_USERNAME=""
RA_API_KEY=""
OUTPUT_DIR="${SCRIPT_DIR}/cartridge-data"

while [[ $# -gt 0 ]]; do
    case $1 in
        --skip-build)
            SKIP_BUILD=true
            shift
            ;;
        --skip-kzr)
            SKIP_KZR=true
            shift
            ;;
        --fetch)
            FETCH_GAME_ID="$2"
            shift 2
            ;;
        --username)
            RA_USERNAME="$2"
            shift 2
            ;;
        --api-key)
            RA_API_KEY="$2"
            shift 2
            ;;
        --output-dir)
            OUTPUT_DIR="$2"
            shift 2
            ;;
        --help)
            echo "Usage: $0 [options]"
            echo ""
            echo "Options:"
            echo "  --skip-build       Skip building Rust binaries"
            echo "  --skip-kzr         Skip building the .kzr runtime"
            echo "  --fetch GAME_ID    Fetch achievement definitions for a game ID"
            echo "  --username USER    RetroAchievements username (for --fetch)"
            echo "  --api-key KEY      RetroAchievements web API key (for --fetch)"
            echo "  --output-dir DIR   Output directory for fetched definitions"
            echo "                      (default: ./cartridge-data)"
            echo "  --help             Show this help message"
            exit 0
            ;;
        *)
            echo -e "${RED}Unknown option: $1${NC}"
            exit 1
            ;;
    esac
done

echo -e "${BLUE}╔════════════════════════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║     Kazeta Zero — Dolphin Runtime Bootstrap                ║${NC}"
echo -e "${BLUE}╚════════════════════════════════════════════════════════════╝${NC}"
echo ""

# ===================================================================
# STEP 1: Build Rust Binaries
# ===================================================================

if [ "$SKIP_BUILD" = false ]; then
    echo -e "${BLUE}═══ Step 1: Building Rust Binaries ═══${NC}"
    echo ""

    echo -e "${YELLOW}→ Building kazeta-ra (library + CLI)...${NC}"
    cd "$RA_DIR"
    if cargo build --release; then
        echo -e "${GREEN}  ✓ kazeta-ra built${NC}"
    else
        echo -e "${RED}  ERROR: Failed to build kazeta-ra${NC}"
        exit 1
    fi

    echo -e "${YELLOW}→ Building kazeta-ra-export (achievement fetcher)...${NC}"
    if cargo build --release --bin kazeta-ra-export; then
        echo -e "${GREEN}  ✓ kazeta-ra-export built${NC}"
    else
        echo -e "${RED}  ERROR: Failed to build kazeta-ra-export${NC}"
        exit 1
    fi

    cd "$SCRIPT_DIR"
    echo ""
else
    echo -e "${YELLOW}Skipping Rust build (--skip-build)${NC}"
    echo ""
fi

# ===================================================================
# STEP 2: Build the dolphin-standalone .kzr Runtime
# ===================================================================

if [ "$SKIP_KZR" = false ]; then
    echo -e "${BLUE}═══ Step 2: Building ${RUNTIME_NAME}.kzr ═══${NC}"
    echo ""

    if ! command -v mkfs.erofs &>/dev/null; then
        echo -e "${RED}  ERROR: mkfs.erofs not found${NC}"
        echo -e "${YELLOW}  Install erofs-utils: sudo pacman -S erofs-utils${NC}"
        exit 1
    fi

    # Create the runtime directory structure
    RUNTIME_DIR=$(mktemp -d)
    trap "rm -rf $RUNTIME_DIR" EXIT

    SHARE_DIR="${RUNTIME_DIR}/.kazeta/share"
    CONFIG_DIR="${RUNTIME_DIR}/Config"
    mkdir -p "$SHARE_DIR/licenses"
    mkdir -p "$CONFIG_DIR"

    # --- Write the run script ---
    # The Kazeta launcher passes a file containing the ROM path as $1.
    # dolphin-emu is installed system-wide via the OS image manifest.
    cat > "$SHARE_DIR/run" << 'RUNSCRIPT'
#!/bin/bash
set -e

# The Kazeta launcher passes a file containing the ROM path as $1
ROM_PATH=$(cat "$1")

# Determine the Dolphin binary
if command -v dolphin-emu &>/dev/null; then
    DOLPHIN="dolphin-emu"
elif command -v dolphin-emu-qt &>/dev/null; then
    DOLPHIN="dolphin-emu-qt"
else
    echo "ERROR: dolphin-emu not found" >&2
    exit 1
fi

# Launch Dolphin in batch mode (no GUI) with the ROM
# -b: batch mode — exit when game exits
# --exec: the game to execute
exec "$DOLPHIN" -b --exec="$ROM_PATH"
RUNSCRIPT
    chmod +x "$SHARE_DIR/run"
    echo -e "${GREEN}  ✓ Written .kazeta/share/run${NC}"

    # --- Write Dolphin.ini ---
    # Dual core MUST be disabled for deterministic memory state
    # (required for the MemoryWatcher-based achievement evaluation).
    cat > "$CONFIG_DIR/Dolphin.ini" << 'DOLPHIN_INI'
[Core]
bCPUThread = False
bDSPHLE = True
[DSP]
EnableJIT = False
Backend = LLE
Volume = 100
[Display]
Fullscreen = True
RenderToMain = False
HideCursor = True
AspectRatio = 1
[General]
ISOPath = 0
DOLPHIN_INI
    echo -e "${GREEN}  ✓ Written Config/Dolphin.ini${NC}"

    # --- Write RetroAchievements.ini ---
    # Dolphin's built-in RA is disabled — we evaluate locally via kazeta-ra
    # using the MemoryWatcher socket. This prevents any network contact.
    cat > "$CONFIG_DIR/RetroAchievements.ini" << 'RA_INI'
[Achievements]
Enabled = False
HardcoreEnabled = False
UnofficialEnabled = False
EncoreEnabled = False
SpectatorEnabled = False
RA_INI
    echo -e "${GREEN}  ✓ Written Config/RetroAchievements.ini${NC}"

    # --- Write GFX config ---
    cat > "$CONFIG_DIR/GFX.ini" << 'GFX_INI'
[Settings]
AspectRatio = 1
Backend = OGL
Fullscreen = True
VSync = True
[Enhancements]
ForceFiltering = False
ArbitraryMipmapDetection = True
GFX_INI
    echo -e "${GREEN}  ✓ Written Config/GFX.ini${NC}"

    # --- Write license/readme ---
    cat > "$SHARE_DIR/licenses/README" << 'LICENSE_README'
Kazeta Zero — Dolphin Standalone Runtime
========================================

This runtime package (.kzr) contains only launch scripts and configuration
files. The Dolphin emulator itself is installed system-wide via the
Kazeta OS image manifest (dolphin-emu package).

Dolphin is licensed under GPL-2.0-or-later.
See: https://github.com/dolphin-emu/dolphin/blob/master/COPYING

Achievement evaluation is handled by kazeta-ra, which reads Dolphin's
memory via the built-in MemoryWatcher feature and evaluates conditions
using the rcheevos engine. Dolphin's own RetroAchievements integration
is disabled to prevent network contact.
LICENSE_README
    echo -e "${GREEN}  ✓ Written licenses/README${NC}"

    # --- Build the EROFS image ---
    echo ""
    echo -e "${YELLOW}→ Building EROFS image: ${KZR_FILE}${NC}"

    if mkfs.erofs -L "$RUNTIME_LABEL" "$KZR_FILE" "$RUNTIME_DIR"; then
        echo -e "${GREEN}  ✓ Built: ${KZR_FILE} ($(du -h "$KZR_FILE" | cut -f1))${NC}"
    else
        echo -e "${RED}  ERROR: Failed to build .kzr image${NC}"
        exit 1
    fi

    echo ""
else
    echo -e "${YELLOW}Skipping .kzr build (--skip-kzr)${NC}"
    echo ""
fi

# ===================================================================
# STEP 3: Fetch Achievement Definitions (Optional)
# ===================================================================

if [ -n "$FETCH_GAME_ID" ]; then
    echo -e "${BLUE}═══ Step 3: Fetching Achievement Definitions ═══${NC}"
    echo ""

    if [ -z "$RA_USERNAME" ] || [ -z "$RA_API_KEY" ]; then
        echo -e "${RED}  ERROR: --fetch requires --username and --api-key${NC}"
        echo -e "${YELLOW}  Get your API key from: https://retroachievements.org/controlpanel.php${NC}"
        exit 1
    fi

    EXPORT_BIN="${RA_DIR}/target/release/kazeta-ra-export"
    if [ ! -f "$EXPORT_BIN" ]; then
        echo -e "${RED}  ERROR: kazeta-ra-export not found at ${EXPORT_BIN}${NC}"
        echo -e "${YELLOW}  Run without --skip-build first${NC}"
        exit 1
    fi

    mkdir -p "$OUTPUT_DIR"

    echo -e "${YELLOW}→ Fetching definitions for game ID ${FETCH_GAME_ID}...${NC}"
    echo -e "${YELLOW}  Output: ${OUTPUT_DIR}${NC}"
    echo ""

    if "$EXPORT_BIN" fetch \
        --username "$RA_USERNAME" \
        --api-key "$RA_API_KEY" \
        --game-id "$FETCH_GAME_ID" \
        --output-dir "$OUTPUT_DIR"; then
        echo ""
        echo -e "${GREEN}  ✓ Definitions fetched to ${OUTPUT_DIR}${NC}"
        echo -e "${YELLOW}  Next: copy achievements.json and badges/ to the cartridge SD card${NC}"
    else
        echo -e "${RED}  ERROR: Failed to fetch definitions${NC}"
        exit 1
    fi

    echo ""
fi

# ===================================================================
# Summary
# ===================================================================

echo -e "${BLUE}╔════════════════════════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║                    Bootstrap Complete!                    ║${NC}"
echo -e "${BLUE}╚════════════════════════════════════════════════════════════╝${NC}"
echo ""

if [ "$SKIP_KZR" = false ]; then
    echo -e "${GREEN}Runtime:${NC}"
    echo -e "  ${KZR_FILE} ($(du -h "$KZR_FILE" | cut -f1))"
    echo -e "    Contains: run script + Dolphin config (no emulator binary)"
    echo ""
fi

echo -e "${GREEN}Binaries:${NC}"
if [ -f "${RA_DIR}/target/release/kazeta-ra" ]; then
    echo -e "  ${RA_DIR}/target/release/kazeta-ra"
fi
if [ -f "${RA_DIR}/target/release/kazeta-ra-export" ]; then
    echo -e "  ${RA_DIR}/target/release/kazeta-ra-export"
fi
echo ""

if [ -n "$FETCH_GAME_ID" ]; then
    echo -e "${GREEN}Achievement Data:${NC}"
    if [ -f "${OUTPUT_DIR}/achievements.json" ]; then
        echo -e "  ${OUTPUT_DIR}/achievements.json"
    fi
    if [ -d "${OUTPUT_DIR}/badges" ]; then
        BADGE_COUNT=$(find "${OUTPUT_DIR}/badges" -name "*.png" | wc -l)
        echo -e "  ${OUTPUT_DIR}/badges/ (${BADGE_COUNT} badge images)"
    fi
    echo ""
fi

echo -e "${YELLOW}Next Steps:${NC}"
echo -e "  On this machine (prep):"
echo -e "  1. (Optional) Fetch achievement definitions for each game:"
echo -e "     ./bootstrap-dolphin-runtime.sh --skip-build --skip-kzr \\"
echo -e "       --fetch GAME_ID --username USER --api-key KEY \\"
echo -e "       --output-dir ./cartridge-data/GAME/"
echo ""
echo -e "  2. Copy achievements.json + badges/ to each cartridge's SD card"
echo -e "     alongside cart.kzi, icon.png, and the game ROM"
echo ""
echo -e "  On the Kazeta Zero machine (playing device):"
echo -e "  3. Install the .kzr runtime:"
echo -e "     sudo kazeta-runtime-helper install ${RUNTIME_NAME}.kzr"
echo -e "     (copy the .kzr to the device first, e.g. via SD card or scp)"
echo ""
echo -e "${GREEN}Done!${NC}"
