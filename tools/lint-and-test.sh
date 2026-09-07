#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
cd "$REPO_ROOT"

run_check() {
    printf '\nRunning:'
    printf ' %s' "$@"
    printf '\n'
    "$@"
}

run_check bash tools/check-version-consistency.sh
run_check bash tools/test-package-repo-layout.sh
run_check bash tools/test-release-packaging.sh
run_check ./tools/check-nixpkgs-recipe.py
run_check ./tools/check-rust-source-coverage.py
run_check ./tools/check-process-sites.py
run_check ./tools/check-config-writers.py
run_check cargo fmt --all -- --check
run_check cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
run_check cargo build --locked --workspace --all-features --bins
run_check cargo test --locked --workspace --all-features
# Linted as strictly as the all-features build: code reachable only behind an
# optional feature leaves its callers dead without it, and building alone does
# not promote that to an error.
run_check cargo clippy --locked --workspace --all-targets --no-default-features -- -D warnings
run_check cargo build --locked --workspace --no-default-features --bins
run_check cargo test --locked --workspace --no-default-features
