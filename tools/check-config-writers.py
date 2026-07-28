#!/usr/bin/env python3
"""Fail when production code outside the reviewed writer can write `config.toml`.

`config.toml` is an authored input. The configurator's explicit Save is the only
application-owned path allowed to change it; the overlay, daemon, tray, startup,
and shutdown read it and never write it. The capability is one `use` away, so
this checks for its absence instead of remembering it.

Scope and limits: this is a name-level guardrail over `src/` and
`configurator/src/`, not the proof. It catches a new caller of the config write
primitives; it cannot catch a brand-new write built directly on
`durable_io::write_text_atomic` under a different name. The behavioural proof is
the loader immutability fixture in `src/config/tests/immutability.rs` plus the
per-flow "file is byte-identical" tests. `src/daemon/tests.rs` keeps its own
in-tree version of this check for the daemon subtree, which stays useful because
it runs under `cargo test` rather than only in the full lint gate.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parent.parent

DOCUMENT_SOURCE = Path("src/config/document.rs")
IO_SOURCE = Path("src/config/io.rs")
CONFIGURATOR_ADAPTER = Path("configurator/src/app/io.rs")

# The files that implement the single durable write, plus the configurator
# adapter that performs it. `src/config/document/merge.rs` is deliberately
# absent: it rewrites a TOML tree in memory and never touches the filesystem, so
# it names none of the primitives below.
WRITE_ALLOWLIST = {
    DOCUMENT_SOURCE,  # ConfigDocument::save_with_backup, the only durable write
    IO_SOURCE,  # the atomic write, the timestamped .bak copy, the parent mkdir
    CONFIGURATOR_ADAPTER,  # the configurator's explicit Save
}

# Every name that reaches the filesystem on the config path.
WRITE_PATTERNS = (
    re.compile(r"\bsave_with_backup\b"),
    re.compile(r"\bwrite_config_text_atomic\b"),
    re.compile(r"\bcreate_config_backup\b"),
    re.compile(r"\bprepare_config_parent\b"),
)

# Subtrees whose former write authority this check replaces. If the walk stops
# reaching them, it proves nothing, so their presence is asserted rather than
# assumed.
EXPECTED_SCANNED = {
    DOCUMENT_SOURCE,
    IO_SOURCE,
    CONFIGURATOR_ADAPTER,
    Path("src/backend/wayland/state.rs"),  # the overlay that owned the writer
    Path("src/daemon/tray/runtime.rs"),  # the tray that owned the resume toggle
    Path("src/backend/wayland/backend/state_init/config.rs"),  # startup load
    Path("configurator/src/app/update/config.rs"),  # Save's message handling
}

# String bodies, char literals, and comments carry no capability, and a `{` in
# one would desynchronise the `#[cfg(test)]` block tracking below. Blanked in
# place so byte offsets and line numbers keep pointing at the real source.
MASKED_SPANS = re.compile(
    r"""
      //[^\n]*                              # line comment
    | /\*.*?\*/                             # block comment
    | (?<![A-Za-z0-9_])b?r(?P<hashes>\#*)"  # raw string, byte or not
        .*?
      "(?P=hashes)
    | b?"(?:\\.|[^"\\])*"                   # string, byte or not
    | '(?:\\.|[^'\\])'                      # char literal, never a lifetime
    """,
    re.VERBOSE | re.DOTALL,
)

CFG_TEST = re.compile(r"\#\[cfg\(test\)\]")


def blank(match: re.Match[str]) -> str:
    return "".join("\n" if character == "\n" else " " for character in match.group(0))


def mask_source(source: str) -> str:
    return MASKED_SPANS.sub(blank, source)


def block_end(masked: str, opening: int) -> int | None:
    """Offset just past the `}` closing the block that starts at `opening`."""
    depth = 0
    for index in range(opening, len(masked)):
        if masked[index] == "{":
            depth += 1
        elif masked[index] == "}":
            depth -= 1
            if depth == 0:
                return index + 1
    return None


def cfg_test_spans(masked: str) -> tuple[list[tuple[int, int]], list[str]]:
    """Offset ranges covered by `#[cfg(test)]` items, and any tracking failure.

    Inline test modules live in production files, so exempting whole files by
    name is not enough; the attributed item is what has to be exempt.
    """
    spans: list[tuple[int, int]] = []
    problems: list[str] = []
    for attribute in CFG_TEST.finditer(masked):
        index = attribute.end()
        while index < len(masked) and masked[index] not in "{;":
            index += 1
        if index >= len(masked):
            problems.append("a `#[cfg(test)]` item has neither a body nor a `;`")
            continue
        if masked[index] == ";":
            # `#[cfg(test)] mod tests;` and `#[cfg(test)] use ...;` bring in no
            # inline code; the module file is exempt by its own path.
            continue
        end = block_end(masked, index)
        if end is None:
            problems.append("a `#[cfg(test)]` block never closes")
            continue
        spans.append((attribute.start(), end))
    return spans, problems


def is_test_source(path: Path) -> bool:
    parts = path.parts
    return path.parts[0] == "tests" or "tests" in parts or path.name == "tests.rs"


def rust_sources() -> list[Path]:
    roots = (ROOT / "src", ROOT / "configurator" / "src")
    return sorted(path for root in roots for path in root.rglob("*.rs"))


def line_of(source: str, offset: int) -> int:
    return source.count("\n", 0, offset) + 1


def audit_sites() -> tuple[list[str], set[Path]]:
    failures: list[str] = []
    scanned: set[Path] = set()
    for absolute in rust_sources():
        relative = absolute.relative_to(ROOT)
        scanned.add(relative)
        if relative in WRITE_ALLOWLIST or is_test_source(relative):
            continue
        source = absolute.read_text()
        masked = mask_source(source)
        spans, problems = cfg_test_spans(masked)
        failures.extend(f"{relative}: {problem}" for problem in problems)
        lines = source.splitlines()
        for pattern in WRITE_PATTERNS:
            for hit in pattern.finditer(masked):
                if any(start <= hit.start() < end for start, end in spans):
                    continue
                number = line_of(source, hit.start())
                text = lines[number - 1].strip() if number <= len(lines) else ""
                failures.append(
                    f"{relative}:{number}: config write capability "
                    f"`{hit.group(0)}` outside the configurator's Save: {text}"
                )
    return failures, scanned


def audit_write_surface() -> list[str]:
    """The implementing files may not widen the capability they own."""
    failures: list[str] = []

    document = (ROOT / DOCUMENT_SOURCE).read_text()
    saves = set(re.findall(r"\n    (?:pub(?:\([^)]*\))? )?fn (\w*save\w*)\s*[(<]", document))
    if "save_with_backup" not in saves:
        failures.append(
            f"{DOCUMENT_SOURCE}: the save-surface scan matched nothing; its shape "
            "assumption about the file no longer holds"
        )
    unexpected = saves - {"save_with_backup"}
    if unexpected:
        failures.append(
            f"{DOCUMENT_SOURCE}: unreviewed document write entry point(s): "
            + ", ".join(sorted(unexpected))
        )
    if "pub fn save_with_backup" not in document:
        failures.append(
            f"{DOCUMENT_SOURCE}: `save_with_backup` is gone or renamed; "
            "this check no longer describes the code"
        )

    io_source = (ROOT / IO_SOURCE).read_text()
    for primitive in ("create_config_backup", "write_config_text_atomic", "prepare_config_parent"):
        if f"pub(super) fn {primitive}" not in io_source:
            failures.append(
                f"{IO_SOURCE}: `{primitive}` is no longer `pub(super)`; the write "
                "primitives must stay inside the config module"
            )
    return failures


def main() -> int:
    failures, scanned = audit_sites()
    missing = sorted(str(path) for path in EXPECTED_SCANNED - scanned)
    if missing:
        failures.append("the walk missed expected sources, so it proves nothing: " + ", ".join(missing))
    failures.extend(audit_write_surface())

    if failures:
        print("config-writer audit failed:", file=sys.stderr)
        for failure in failures:
            print(f"  {failure}", file=sys.stderr)
        return 1
    print(f"config-writer audit passed ({len(scanned)} sources)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
