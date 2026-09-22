#!/usr/bin/env bash
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../../.." && pwd)
SOURCE="$ROOT/tools/integration/held-buffers"
BUILD=$(mktemp -d)
trap 'rm -rf "$BUILD"' EXIT

for program in cc pkg-config wayland-scanner sway swaymsg python3; do
    command -v "$program" >/dev/null || { echo "missing $program" >&2; exit 2; }
done

# pkg-config returns individual compiler/linker arguments.
# shellcheck disable=SC2046
cc -shared -fPIC -Wall -Wextra -Werror -o "$BUILD/release_fixture.so" \
    "$SOURCE/release_fixture.c" $(pkg-config --cflags --libs wayland-server) -ldl
wayland-scanner client-header "$SOURCE/wlr-virtual-pointer-unstable-v1.xml" \
    "$BUILD/v5_virtual_pointer_protocol.h"
wayland-scanner private-code "$SOURCE/wlr-virtual-pointer-unstable-v1.xml" \
    "$BUILD/virtual_pointer_protocol.c"
# shellcheck disable=SC2046
cc -Wall -Wextra -Werror -I "$BUILD" -o "$BUILD/virtual_pointer" \
    "$SOURCE/virtual_pointer.c" "$BUILD/virtual_pointer_protocol.c" \
    $(pkg-config --cflags --libs wayland-client)

APP=${1:-"$ROOT/target/debug/wayscriber"}
if [[ ! -x "$APP" ]]; then
    echo "build the app first: cargo build --locked --bins" >&2
    exit 2
fi
PYTHONDONTWRITEBYTECODE=1 python3 -m unittest discover -s "$SOURCE" -p 'test_*.py' -q
PYTHONDONTWRITEBYTECODE=1 python3 "$SOURCE/check.py" "$APP" "$BUILD/release_fixture.so" "$BUILD/virtual_pointer"
