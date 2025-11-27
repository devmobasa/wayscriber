#!/bin/bash
# Installation script for wayscriber-configurator only

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

INSTALL_DIR="${WAYSCRIBER_INSTALL_DIR:-/usr/bin}"
BINARY_NAME="wayscriber-configurator"

echo "================================"
echo " Wayscriber Configurator Install"
echo "================================"
echo ""

die() {
    echo "❌ $*" >&2
    exit 1
}

echo "Building configurator (release)..."
(cd "$PROJECT_ROOT" && cargo build --release --bins --manifest-path configurator/Cargo.toml)

BIN_PATH="$PROJECT_ROOT/configurator/target/release/$BINARY_NAME"
if [ ! -f "$BIN_PATH" ]; then
    die "Configurator binary not found at $BIN_PATH"
fi

if [ ! -d "$INSTALL_DIR" ] || [ ! -w "$INSTALL_DIR" ]; then
    if [ "$(id -u)" -ne 0 ]; then
        if command -v sudo >/dev/null 2>&1; then
            SUDO="sudo"
            echo "Using sudo to install into $INSTALL_DIR"
        else
            die "Write access to $INSTALL_DIR required. Re-run with sudo or set WAYSCRIBER_INSTALL_DIR."
        fi
    fi
fi

${SUDO:-} install -d "$INSTALL_DIR"
echo "Installing configurator to $INSTALL_DIR/$BINARY_NAME"
${SUDO:-} install -Dm755 "$BIN_PATH" "$INSTALL_DIR/$BINARY_NAME"

echo ""
echo "✅ Configurator installation complete!"
echo ""
echo "Run: $BINARY_NAME --help"
