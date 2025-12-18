#!/bin/bash
# Build and deploy Kazeta+ binaries and runtimes directly on a device.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

BUILD_MODE="release"
BUILD_RUNTIMES=true
DEPLOY_PREFIX="${DEPLOY_PREFIX:-/usr}"
RUNTIME_DIR="${RUNTIME_DIR:-/usr/share/kazeta/runtimes}"

usage() {
    cat <<EOF
Usage: $0 [--release|--debug] [--skip-runtimes] [--deploy-prefix PATH] [--runtime-dir PATH]

Builds binaries via ./build-all.sh, then installs them to the live system.

Options:
  --release           Build release binaries (default)
  --debug             Build debug binaries
  --skip-runtimes     Do not build/copy .kzr runtimes
  --deploy-prefix     Target prefix for binaries (default: /usr)
  --runtime-dir       Target directory for .kzr runtimes (default: /usr/share/kazeta/runtimes)
  --help              Show this help

Environment:
  DEPLOY_PREFIX, RUNTIME_DIR override the install paths.
EOF
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --release) BUILD_MODE="release"; shift ;;
        --debug) BUILD_MODE="debug"; shift ;;
        --skip-runtimes) BUILD_RUNTIMES=false; shift ;;
        --deploy-prefix) DEPLOY_PREFIX="$2"; shift 2 ;;
        --runtime-dir) RUNTIME_DIR="$2"; shift 2 ;;
        --help) usage; exit 0 ;;
        *) echo "Unknown option: $1" >&2; usage; exit 1 ;;
    esac
done

MISSING=()
check_cmd() {
    local cmd="$1"
    local help="$2"
    if ! command -v "$cmd" >/dev/null 2>&1; then
        MISSING+=("$help")
    fi
}

check_cmd "git" "git (install: sudo pacman -S git or sudo apt install git)"
check_cmd "cargo" "Rust toolchain (install: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh ; then rustup default stable)"
if [ "$BUILD_RUNTIMES" = true ]; then
    if ! command -v mkfs.erofs >/dev/null 2>&1 && ! command -v mksquashfs >/dev/null 2>&1; then
        MISSING+=("Filesystem packer (install: sudo pacman -S erofs-utils or sudo apt install erofs-utils | squashfs-tools)")
    fi
    check_cmd "mgba-qt" "mGBA Qt frontend for runtime build (install: sudo pacman -S mgba-qt or sudo apt install mgba-qt)"
fi

if [ "${#MISSING[@]}" -ne 0 ]; then
    echo "Missing dependencies:"
    for item in "${MISSING[@]}"; do
        echo "  - $item"
    done
    exit 1
fi

SUDO_BIN=""
if [ "$EUID" -ne 0 ]; then
    SUDO_BIN="sudo"
fi

echo "Repo root: $REPO_ROOT"
echo "Build mode: $BUILD_MODE"
echo "Build runtimes: $BUILD_RUNTIMES"
echo "Deploy prefix: $DEPLOY_PREFIX"
echo "Runtime dir: $RUNTIME_DIR"

pushd "$REPO_ROOT" >/dev/null

BUILD_FLAGS=()
[ "$BUILD_MODE" = "release" ] || BUILD_FLAGS+=(--debug)
[ "$BUILD_RUNTIMES" = true ] || BUILD_FLAGS+=(--skip-runtimes)

./build-all.sh "${BUILD_FLAGS[@]}"

BIN_SRC="$REPO_ROOT/rootfs/usr/bin"
BIN_DEST="$DEPLOY_PREFIX/bin"

if [ ! -d "$BIN_SRC" ]; then
    echo "Binary source path missing: $BIN_SRC" >&2
    exit 1
fi

echo "Installing binaries to $BIN_DEST"
$SUDO_BIN install -d "$BIN_DEST"
while IFS= read -r bin_file; do
    [ -f "$bin_file" ] || continue
    name="$(basename "$bin_file")"
    $SUDO_BIN install -m 755 "$bin_file" "$BIN_DEST/$name"
    echo "  - $name"
done < <(find "$BIN_SRC" -maxdepth 1 -type f)

if [ "$BUILD_RUNTIMES" = true ]; then
    shopt -s nullglob
    runtime_files=( "$REPO_ROOT"/*.kzr "$REPO_ROOT"/runtimes/*.kzr )
    shopt -u nullglob

    if [ "${#runtime_files[@]}" -eq 0 ]; then
        echo "No .kzr runtime files found; skipping runtime copy."
    else
        echo "Installing runtimes to $RUNTIME_DIR"
        $SUDO_BIN install -d "$RUNTIME_DIR"
        for rt in "${runtime_files[@]}"; do
            name="$(basename "$rt")"
            $SUDO_BIN install -m 644 "$rt" "$RUNTIME_DIR/$name"
            echo "  - $name"
        done
    fi
fi

popd >/dev/null

echo "Build and deploy complete."
