#!/usr/bin/env python3
# Fixture, not a live checker. The same trimmed source-coverage gate after
# somebody inlined the check vectors: `cargo_lanes` is no longer imported and
# the `source-coverage` consumer is never named, so the matrix this gate
# compiles has quietly forked from tools/cargo-lanes.json. Editing the manifest
# would no longer change what this file checks.
"""Fail when a Rust source file is compiled by no source-coverage lane."""

from __future__ import annotations

import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent

VECTORS = [
    ["cargo", "check", "-p", "wayscriber", "--all-features"],
    ["cargo", "check", "-p", "wayscriber-configurator", "--no-default-features"],
]


def compiled_sources(vector: list[str]) -> set[Path]:
    """Every file the dep-info of this vector's check names."""
    return set()


def repository_rust_sources() -> set[Path]:
    return set(REPO_ROOT.rglob("*.rs"))


def main() -> int:
    compiled: set[Path] = set()
    for vector in VECTORS:
        compiled |= compiled_sources(vector)

    uncovered = sorted(repository_rust_sources() - compiled)
    if uncovered:
        for path in uncovered:
            print(f"- {path}", file=sys.stderr)
        return 1

    print(f"Rust source coverage OK: {len(VECTORS)} lane vector(s).")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
