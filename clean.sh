#!/bin/bash
# Format the project, then lint every portable Cargo lane leniently.
set -euo pipefail

# Both commands below are repository-wide, and the driver is named by a path
# relative to the root, so run from the root whatever the caller's directory is.
cd "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# Format first so clippy runs on the normalized code.
cargo fmt

# Lint the lanes the `clean` consumer owns in tools/cargo-lanes.json. That
# consumer deliberately keeps this pass lenient: no --all-targets and no
# -D warnings, so warnings stay visible without failing the run.
./tools/run-cargo-consumer.py clean

echo "✅ fmt + clippy completed"
