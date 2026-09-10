#!/usr/bin/env bash
# Standalone asset recipe generator for contributors who do not use .NET.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"

if ! command -v python3 >/dev/null 2>&1; then
    echo 'AUR packaging checks require Python 3; see CONTRIBUTING.md.' >&2
    exit 1
fi
if [[ "${1:-}" == --check && "$#" -eq 1 ]]; then
    exit 0
fi
if [[ "$#" -ne 1 ]]; then
    echo 'Usage: bash tools/aur-desktop-assets.sh REPO_ROOT | --check' >&2
    exit 1
fi

exec python3 "$SCRIPT_DIR/aur-desktop-assets.py" "$1"
