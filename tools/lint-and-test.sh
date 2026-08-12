#!/usr/bin/env bash
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
cd "$REPO_ROOT"

bash tools/check-version-consistency.sh
bash tools/test-package-repo-layout.sh
bash tools/test-release-packaging.sh
./tools/check-nixpkgs-recipe.py
./tools/check-rust-source-coverage.py
./tools/check-process-sites.py
./tools/check-config-writers.py
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo build --workspace --all-features --bins
cargo test --workspace --all-features
# Linted as strictly as the all-features build: code reachable only behind an
# optional feature leaves its callers dead without it, and building alone does
# not promote that to an error.
cargo clippy --workspace --all-targets --no-default-features -- -D warnings
cargo build --workspace --no-default-features --bins
cargo test --workspace --no-default-features
