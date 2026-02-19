#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BUILD_DIR="${1:-$ROOT_DIR/prime}"

WAYSCRIBER_DESKTOP="$BUILD_DIR/meta/gui/wayscriber.desktop"
CONFIG_DESKTOP="$BUILD_DIR/meta/gui/wayscriber-configurator.desktop"

normalized_lines() {
    local file="$1"
    # Normalize leading/trailing whitespace and CRLF endings for robust checks.
    sed -e 's/\r$//' -e 's/[[:space:]]*$//' -e 's/^[[:space:]]*//' "$file"
}

desktop_entry_section() {
    local file="$1"
    normalized_lines "$file" | awk '
        BEGIN { in_section = 0; section_count = 0 }
        $0 == "[Desktop Entry]" {
            section_count++
            if (section_count > 1) exit 3
            in_section = 1
            next
        }
        /^\[.*\]$/ { if (in_section) in_section = 0; next }
        in_section { print }
        END {
            if (section_count == 0) exit 2
            if (section_count > 1) exit 3
        }
    '
}

assert_single_key_value() {
    local file="$1"
    local key="$2"
    local expected_value="$3"
    local label="$4"

    local section
    if section="$(desktop_entry_section "$file")"; then
        :
    else
        local rc=$?
        if [[ "$rc" -eq 2 ]]; then
            echo "ERROR: missing [Desktop Entry] section in $file" >&2
        elif [[ "$rc" -eq 3 ]]; then
            echo "ERROR: multiple [Desktop Entry] sections found in $file" >&2
        else
            echo "ERROR: failed parsing [Desktop Entry] section in $file" >&2
        fi
        exit 1
    fi

    local matches
    matches="$(printf '%s\n' "$section" | grep -nE "^${key}=" || true)"

    local count
    count="$(printf '%s\n' "$matches" | sed '/^$/d' | wc -l | tr -d ' ')"

    if [[ "$count" -ne 1 ]]; then
        echo "ERROR: expected exactly one $key line for $label in $file, found $count" >&2
        if [[ -n "$matches" ]]; then
            printf '%s\n' "$matches" >&2
        fi
        exit 1
    fi

    local actual
    actual="$(printf '%s\n' "$matches" | cut -d: -f2-)"
    local expected="${key}=${expected_value}"
    if [[ "$actual" != "$expected" ]]; then
        echo "ERROR: invalid $label in $file" >&2
        echo "Expected: $expected" >&2
        echo "Actual:   $actual" >&2
        exit 1
    fi
}

if [[ ! -f "$WAYSCRIBER_DESKTOP" || ! -f "$CONFIG_DESKTOP" ]]; then
    echo "ERROR: built desktop files not found under: $BUILD_DIR/meta/gui" >&2
    echo "Run snapcraft first, then re-run this check." >&2
    exit 1
fi

assert_single_key_value "$WAYSCRIBER_DESKTOP" "Exec" "wayscriber" "main app Exec"
assert_single_key_value "$WAYSCRIBER_DESKTOP" "TryExec" "wayscriber" "main app TryExec"
assert_single_key_value "$CONFIG_DESKTOP" "Exec" "wayscriber.wayscriber-configurator" "configurator Exec"
assert_single_key_value "$CONFIG_DESKTOP" "TryExec" "wayscriber.wayscriber-configurator" "configurator TryExec"

echo "Built desktop check passed: $BUILD_DIR/meta/gui"
