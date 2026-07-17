#!/usr/bin/env bash
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
cd "$REPO_ROOT"

bash tools/check-version-consistency.sh
bash tools/test-package-repo-layout.sh
./tools/check-rust-source-coverage.py
./tools/check-process-sites.py
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo build --workspace --all-features --bins
cargo test --workspace --all-features
cargo build --workspace --no-default-features --bins
cargo test --workspace --no-default-features
