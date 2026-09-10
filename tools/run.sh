#!/usr/bin/env bash
# Run the release binary as a development daemon.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
BINARY="${PROJECT_ROOT}/target/release/wayscriber"

if [[ "${1-}" == "-h" || "${1-}" == "--help" ]]; then
    echo "Usage: $0 [WAYSCRIBER_OPTIONS...]"
    echo "Run target/release/wayscriber in daemon mode with RUST_LOG=info by default."
    exit 0
fi

if [[ ! -x "${BINARY}" ]]; then
    echo "Wayscriber release binary not found: ${BINARY}" >&2
    echo "Run ./tools/build.sh first." >&2
    exit 1
fi

cd "${PROJECT_ROOT}"
RUST_LOG="${RUST_LOG:-info}" exec "${BINARY}" --daemon "$@"
