#!/usr/bin/env bash

# Fetch the dependencies for the main crate and the configurator, if present.
# This mirrors the logic used by the packaging scripts to ensure a frozen build
# has everything cached before going offline.

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

echo "Fetching wayscriber dependencies..."
cargo fetch --locked --manifest-path "$repo_root/Cargo.toml"

if [[ -f "$repo_root/configurator/Cargo.toml" ]]; then
    echo "Fetching configurator dependencies..."
    cargo fetch --locked --manifest-path "$repo_root/configurator/Cargo.toml"
fi

echo "Dependency fetch complete."
