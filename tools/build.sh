#!/bin/bash
# Build helper for wayscriber with optional tablet-input feature toggle.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

echo "Building wayscriber (default features)..."
(cd "$PROJECT_ROOT" && cargo build --release --bins)
echo "✅ Build complete."
