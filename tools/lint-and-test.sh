#!/usr/bin/env bash
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
cd "$REPO_ROOT"

bash tools/check-version-consistency.sh
bash tools/test-package-repo-layout.sh
bash tools/test-release-packaging.sh
./tools/check-nixpkgs-recipe.py
./tools/check-aur-templates.py --self-test
./tools/check-aur-templates.py
./tools/check-cargo-lanes.py --self-test
./tools/check-cargo-lanes.py
./tools/check-rust-source-coverage.py
./tools/check-process-sites.py
./tools/check-config-writers.py
cargo fmt --all -- --check
./tools/run-cargo-consumer.py lint-and-test
