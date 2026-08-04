#!/usr/bin/env python3
# Fixture, not a live checker. The half-way state the name-presence rule alone
# cannot see: the vectors have been inlined and `cargo_lanes` is no longer
# imported, but the string "source-coverage" survives in a label the gate
# prints. The consumer name is present, so the rule that looks for it is
# satisfied while the manifest has stopped reaching this file entirely.
"""Fail when a Rust source file is compiled by no source-coverage lane."""

from __future__ import annotations

import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent

SOURCE_COVERAGE_CONSUMER = "source-coverage"

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

    print(f"{SOURCE_COVERAGE_CONSUMER} OK: {len(VECTORS)} lane vector(s).")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
