#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

count_trimmed_line_matches() {
    local file="$1"
    local expected_line="$2"
    awk -v expected="$expected_line" '
        {
            sub(/^[[:space:]]+/, "", $0)
            sub(/\r$/, "", $0)
            sub(/[[:space:]]+$/, "", $0)
            if ($0 == expected) count++
        }
        END { print count + 0 }
    ' "$file"
}

assert_single_trimmed_line() {
    local file="$1"
    local expected_line="$2"
    local label="$3"
    local count
    count="$(count_trimmed_line_matches "$file" "$expected_line")"
    if [[ "$count" -ne 1 ]]; then
        echo "ERROR: expected exactly one $label in $file, found $count" >&2
        exit 1
    fi
}

assert_at_least_one_trimmed_line() {
    local file="$1"
    local expected_line="$2"
    local label="$3"
    local count
    count="$(count_trimmed_line_matches "$file" "$expected_line")"
    if [[ "$count" -lt 1 ]]; then
        echo "ERROR: expected at least one $label in $file, found $count" >&2
        exit 1
    fi
}

assert_single_trimmed_line "$ROOT_DIR/snap/snapcraft.yaml" "sed -i 's|^Exec=.*|Exec=wayscriber|' \"\$CRAFT_PART_INSTALL/meta/gui/wayscriber.desktop\"" "main app Exec snap namespace rewrite"
assert_single_trimmed_line "$ROOT_DIR/snap/snapcraft.yaml" "sed -i 's|^TryExec=.*|TryExec=wayscriber|' \"\$CRAFT_PART_INSTALL/meta/gui/wayscriber.desktop\"" "main app TryExec snap namespace rewrite"
assert_single_trimmed_line "$ROOT_DIR/snap/snapcraft.yaml" "sed -i 's|^Exec=.*|Exec=wayscriber.wayscriber-configurator|' \"\$CRAFT_PART_INSTALL/meta/gui/wayscriber-configurator.desktop\"" "configurator Exec snap namespace rewrite"
assert_single_trimmed_line "$ROOT_DIR/snap/snapcraft.yaml" "sed -i 's|^TryExec=.*|TryExec=wayscriber.wayscriber-configurator|' \"\$CRAFT_PART_INSTALL/meta/gui/wayscriber-configurator.desktop\"" "configurator TryExec snap namespace rewrite"
assert_at_least_one_trimmed_line "$ROOT_DIR/snap/README.md" "wayscriber.wayscriber-configurator" "namespaced configurator run command docs"
assert_at_least_one_trimmed_line "$ROOT_DIR/snap/README.md" "wayscriber.wayscriber-daemon" "namespaced daemon run command docs"

echo "Snap smoke check passed."
