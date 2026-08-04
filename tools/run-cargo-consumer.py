#!/usr/bin/env python3
"""Run every Cargo operation one consumer owns, in the order the manifest states.

Usage: ./tools/run-cargo-consumer.py <consumer>

The consumer names come from `tools/cargo-lanes.json`. Each operation streams its
own output; the first failure stops the run and becomes this script's exit code,
so a caller (`tools/lint-and-test.sh`, `clean.sh`, a CI step) needs no extra
error handling.
"""

from __future__ import annotations

import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from cargo_lanes import REPO_ROOT, Consumer, Manifest, ManifestError, load_manifest


def usage(manifest: Manifest | None) -> str:
    lines = ["usage: ./tools/run-cargo-consumer.py <consumer>"]
    if manifest is not None:
        lines.append("")
        lines.append("consumers declared in tools/cargo-lanes.json:")
        for name in sorted(manifest.consumers):
            consumer = manifest.consumers[name]
            lines.append(f"  {name} - {consumer.description} (called by {consumer.caller})")
    return "\n".join(lines)


def run_consumer(consumer: Consumer) -> int:
    total = len(consumer.operations)
    print(f"==> consumer `{consumer.name}`: {total} Cargo operation(s)", flush=True)
    for index, operation in enumerate(consumer.operations, start=1):
        print(
            f"--> [{index}/{total}] {consumer.name}: {operation.label} "
            f"[lane {operation.lane}]\n    {operation.display()}",
            flush=True,
        )
        try:
            result = subprocess.run(list(operation.argv), cwd=REPO_ROOT, check=False)
        except OSError as error:
            print(
                f"{consumer.name}: could not run `{operation.display()}`: {error}",
                file=sys.stderr,
            )
            return 127
        if result.returncode != 0:
            print(
                f"{consumer.name}: `{operation.display()}` failed with exit status "
                f"{result.returncode} (operation {index}/{total}, lane {operation.lane})",
                file=sys.stderr,
            )
            return result.returncode
    print(f"==> consumer `{consumer.name}` completed {total} operation(s)", flush=True)
    return 0


def main(argv: list[str]) -> int:
    try:
        manifest = load_manifest()
    except ManifestError as error:
        print(f"cargo lane manifest error: {error}", file=sys.stderr)
        return 2

    if len(argv) != 1:
        print(usage(manifest), file=sys.stderr)
        return 2

    try:
        consumer = manifest.consumer(argv[0])
    except ManifestError as error:
        print(f"cargo lane manifest error: {error}", file=sys.stderr)
        return 2

    return run_consumer(consumer)


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
