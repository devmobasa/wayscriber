#!/bin/bash
# Test helper for wayscriber with optional tablet-input feature toggle.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

echo "Running tests (default features)..."
(cd "$PROJECT_ROOT" && cargo test --workspace)
echo "✅ Tests complete."
