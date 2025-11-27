#!/bin/bash
# Format and lint the project with all features.
set -euo pipefail

# Format first so clippy runs on the normalized code.
cargo fmt

# Lint everything with all features.
cargo clippy --workspace --all-features

echo "✅ fmt + clippy completed"
