#!/usr/bin/env python3
# Fixture, not a live checker. A trimmed `tools/check-rust-source-coverage.py`
# that satisfies the loader half of the entry-point contract: it imports the
# shared loader and asks it for the `source-coverage` consumer's operations
# instead of spelling any Cargo argv out here. It invokes the driver nowhere,
# because the streamed JSON messages are why this entry point loads the
# manifest directly.
"""Fail when a Rust source file is compiled by no source-coverage lane."""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from cargo_lanes import REPO_ROOT, ManifestError, Operation, load_manifest

SOURCE_COVERAGE_CONSUMER = "source-coverage"


def compiled_sources(operation: Operation) -> set[Path]:
    """Every file the dep-info of this lane's check names."""
    return set()


def repository_rust_sources() -> set[Path]:
    return set(REPO_ROOT.rglob("*.rs"))


def main() -> int:
    try:
        operations = load_manifest().consumer(SOURCE_COVERAGE_CONSUMER).operations
    except ManifestError as error:
        print(f"source coverage failed: {error}", file=sys.stderr)
        return 2

    compiled: set[Path] = set()
    for operation in operations:
        compiled |= compiled_sources(operation)

    uncovered = sorted(repository_rust_sources() - compiled)
    if uncovered:
        for path in uncovered:
            print(f"- {path}", file=sys.stderr)
        return 1

    lanes = ", ".join(operation.lane for operation in operations)
    print(f"Rust source coverage OK: {len(operations)} lane vector(s) ({lanes}).")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
