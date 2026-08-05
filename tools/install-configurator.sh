#!/bin/bash
# Installation script for wayscriber-configurator only

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

INSTALL_DIR="${WAYSCRIBER_INSTALL_DIR:-/usr/bin}"
BINARY_NAME="wayscriber-configurator"
DESKTOP_NAME="wayscriber-configurator.desktop"

echo "================================"
echo " Wayscriber Configurator Install"
echo "================================"
echo ""

die() {
    echo "❌ $*" >&2
    exit 1
}

trim_trailing_slashes() {
    local path="$1"

    while [ "${path%/}" != "$path" ] && [ -n "${path%/}" ]; do
        path="${path%/}"
    done
    printf '%s' "$path"
}

INSTALL_DIR="$(trim_trailing_slashes "$INSTALL_DIR")"
case "$INSTALL_DIR" in
    /*) ;;
    *) INSTALL_DIR="$PWD/$INSTALL_DIR" ;;
esac
INSTALL_TARGET="${INSTALL_DIR%/}/$BINARY_NAME"

if [ -n "${WAYSCRIBER_DATA_DIR:-}" ]; then
    DATA_DIR="$(trim_trailing_slashes "$WAYSCRIBER_DATA_DIR")"
    case "$DATA_DIR" in
        /*) ;;
        *) DATA_DIR="$PWD/$DATA_DIR" ;;
    esac
elif [ "${INSTALL_DIR##*/}" = "bin" ]; then
    DATA_DIR="${INSTALL_DIR%/bin}/share"
else
    die "Cannot derive the application data directory from $INSTALL_DIR. Set WAYSCRIBER_DATA_DIR."
fi

path_needs_privilege() {
    local path="$1"

    while [ ! -e "$path" ]; do
        path="${path%/*}"
        [ -n "$path" ] || path="/"
    done
    [ ! -w "$path" ]
}

desktop_exec_path() {
    printf '%s' "$1" | sed \
        -e 's/\\/\\\\\\\\/g' \
        -e 's/"/\\\\&/g' \
        -e 's/`/\\\\&/g' \
        -e 's/\$/\\\\&/g' \
        -e 's/%/%%/g'
}

desktop_string_path() {
    printf '%s' "$1" | sed -e 's/\\/\\\\/g'
}

echo "Building configurator (release)..."
(cd "$PROJECT_ROOT" && cargo build --release --bins --manifest-path configurator/Cargo.toml)

BIN_PATH=""
for CANDIDATE in \
    "$PROJECT_ROOT/target/release/$BINARY_NAME" \
    "$PROJECT_ROOT/configurator/target/release/$BINARY_NAME"
do
    if [ -f "$CANDIDATE" ]; then
        BIN_PATH="$CANDIDATE"
        break
    fi
done

if [ -z "$BIN_PATH" ]; then
    die "Configurator binary not found at expected paths under $PROJECT_ROOT/target/release or $PROJECT_ROOT/configurator/target/release"
fi

BINARY_SUDO=()
DATA_SUDO=()
if [ "$(id -u)" -ne 0 ]; then
    if path_needs_privilege "$INSTALL_DIR"; then
        command -v sudo >/dev/null 2>&1 \
            || die "Write access to $INSTALL_DIR required. Re-run with sudo or set WAYSCRIBER_INSTALL_DIR."
        BINARY_SUDO=(sudo)
        echo "Using sudo to install into $INSTALL_DIR"
    fi
    if path_needs_privilege "$DATA_DIR"; then
        command -v sudo >/dev/null 2>&1 \
            || die "Write access to $DATA_DIR required. Re-run with sudo or set WAYSCRIBER_DATA_DIR."
        DATA_SUDO=(sudo)
        echo "Using sudo to install into $DATA_DIR"
    fi
fi

"${BINARY_SUDO[@]}" install -d "$INSTALL_DIR"
echo "Installing configurator to $INSTALL_TARGET"
"${BINARY_SUDO[@]}" install -Dm755 "$BIN_PATH" "$INSTALL_TARGET"

DESKTOP_SOURCE="$PROJECT_ROOT/packaging/$DESKTOP_NAME"
DESKTOP_TEMP="$(mktemp)"
trap 'rm -f "$DESKTOP_TEMP"' EXIT
EXEC_PATH="$(desktop_exec_path "$INSTALL_TARGET")"
TRY_EXEC_PATH="$(desktop_string_path "$INSTALL_TARGET")"
while IFS= read -r line || [ -n "$line" ]; do
    case "$line" in
        Exec=*) printf 'Exec="%s"\n' "$EXEC_PATH" ;;
        TryExec=*) printf 'TryExec=%s\n' "$TRY_EXEC_PATH" ;;
        *) printf '%s\n' "$line" ;;
    esac
done < "$DESKTOP_SOURCE" > "$DESKTOP_TEMP"

echo "Installing desktop entry and icons to $DATA_DIR"
"${DATA_SUDO[@]}" install -Dm644 \
    "$DESKTOP_TEMP" \
    "$DATA_DIR/applications/$DESKTOP_NAME"
for SIZE in 16 19 22 24 38 64 128; do
    "${DATA_SUDO[@]}" install -Dm644 \
        "$PROJECT_ROOT/packaging/icons/wayscriber-configurator-${SIZE}.png" \
        "$DATA_DIR/icons/hicolor/${SIZE}x${SIZE}/apps/wayscriber-configurator.png"
done
"${DATA_SUDO[@]}" install -Dm644 \
    "$PROJECT_ROOT/packaging/icons/wayscriber-configurator.svg" \
    "$DATA_DIR/icons/hicolor/scalable/apps/wayscriber-configurator.svg"
"${DATA_SUDO[@]}" install -Dm644 \
    "$PROJECT_ROOT/packaging/icons/wayscriber-configurator-128.png" \
    "$DATA_DIR/pixmaps/wayscriber-configurator.png"

if command -v update-desktop-database >/dev/null 2>&1; then
    "${DATA_SUDO[@]}" update-desktop-database "$DATA_DIR/applications"
fi
if command -v gtk-update-icon-cache >/dev/null 2>&1 \
    && [ -f "$DATA_DIR/icons/hicolor/index.theme" ]; then
    "${DATA_SUDO[@]}" gtk-update-icon-cache -q -f -t "$DATA_DIR/icons/hicolor"
fi

echo ""
echo "✅ Configurator installation complete!"
echo ""
echo "Run: $INSTALL_TARGET --help"
