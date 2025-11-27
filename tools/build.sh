#!/bin/bash
# Build helper for wayscriber with optional tablet-input feature toggle.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

echo "Select build mode:"
echo "  1) No default features (disables tablet-input/portal/tray)"
echo "  2) With default features (tablet-input, portal, tray)"
read -p "Enter choice [1-2]: " -r BUILD_CHOICE

FEATURE_ARGS=()
MODE_LABEL=""
case "$BUILD_CHOICE" in
    1)
        FEATURE_ARGS=(--no-default-features)
        MODE_LABEL="no default features"
        ;;
    2)
        FEATURE_ARGS=()
        MODE_LABEL="default features"
        ;;
    *)
        echo "Invalid choice, defaulting to no default features."
        FEATURE_ARGS=(--no-default-features)
        MODE_LABEL="no default features"
        ;;
esac

echo "Building wayscriber ($MODE_LABEL)..."
(cd "$PROJECT_ROOT" && cargo build --release --bins "${FEATURE_ARGS[@]}")
echo "✅ Build complete."
