#!/usr/bin/env bash
# Keep the executable and logs together so a core can be symbolicated later.
set -euo pipefail
SCRIPT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
RESULT_DIR=${1:?usage: run-stress.sh RESULT_DIR [threads waves iterations fonts]}
shift
mkdir -p "$RESULT_DIR"
RESULT_DIR=$(cd "$RESULT_DIR" && pwd)
{
    date -u +'%Y-%m-%dT%H:%M:%SZ'
    uname -srmo
    cc --version | head -1
    for library in cairo pango freetype2 fontconfig; do
        printf '%s pkg-config: ' "$library"
        pkg-config --modversion "$library"
    done
} > "$RESULT_DIR/versions.txt"
fc-list --format='%{family}\t%{style}\t%{file}\n' | sort > "$RESULT_DIR/fonts.txt"
# pkg-config produces compiler/linker arguments, intentionally split here.
# shellcheck disable=SC2046
cc -std=c11 -O1 -g -Wall -Wextra -pthread "$SCRIPT_DIR/stress.c" \
    -o "$RESULT_DIR/stress" $(pkg-config --cflags --libs pangocairo pangoft2 fontconfig)
if [[ $# -eq 0 ]]; then set -- 24 20 96 64; fi
printf 'timeout 45s stress' > "$RESULT_DIR/result.txt"
printf ' %s' "$@" >> "$RESULT_DIR/result.txt"
printf '\n' >> "$RESULT_DIR/result.txt"
set +e
timeout 45s "$RESULT_DIR/stress" "$@" > "$RESULT_DIR/stress.log" 2>&1
DIAGNOSTIC_STATUS=$?
set -e
printf 'exit_status=%s\n' "$DIAGNOSTIC_STATUS" | tee -a "$RESULT_DIR/result.txt"
exit "$DIAGNOSTIC_STATUS"
