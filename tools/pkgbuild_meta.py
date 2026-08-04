#!/usr/bin/env python3
"""Read PKGBUILD and .SRCINFO metadata without executing any shell.

A PKGBUILD is a bash script, so a fully general reader would have to run it.
These checkers deliberately refuse to: an AUR recipe is untrusted-by-construction
release input, and `bash eval` on it would turn a metadata check into arbitrary
code execution. The parser therefore accepts only the declarative subset the
repository's own recipes use -- top-level scalar and array assignments, function
bodies skipped whole -- and raises `MetadataError` on anything else, so an
unsupported construct fails loudly instead of being silently dropped.

This module is imported, never run. `tools/check-aur-templates.py` uses it to
compare a rendered template pair; `tools/check-srcinfo-canonical.py` uses the
.SRCINFO half to compare against live `makepkg --printsrcinfo` output.
"""

from __future__ import annotations

import re
import shlex
from collections import Counter
from dataclasses import dataclass

SECTION_KEYS = ("pkgbase", "pkgname")

ASSIGNMENT_RE = re.compile(r"^([A-Za-z_][A-Za-z0-9_]*)=(.*)$")
FUNCTION_RE = re.compile(r"^[A-Za-z_][A-Za-z0-9_]*\s*\(\)\s*\{\s*$")
FIELD_RE = re.compile(r"^[ \t]+([A-Za-z_][A-Za-z0-9_]*)\s*=\s?(.*)$")
SECTION_RE = re.compile(r"^([A-Za-z_][A-Za-z0-9_]*)\s*=\s*(\S.*)$")
VARIABLE_RE = re.compile(r"\$\{([A-Za-z_][A-Za-z0-9_]*)\}|\$([A-Za-z_][A-Za-z0-9_]*)")


class MetadataError(RuntimeError):
    """A PKGBUILD or .SRCINFO used a construct this parser refuses to guess at."""


@dataclass(frozen=True)
class SrcinfoSection:
    """One `pkgbase = x` or `pkgname = y` block and the fields indented under it."""

    kind: str
    name: str
    fields: tuple[tuple[str, str], ...]

    @property
    def label(self) -> str:
        return f"{self.kind} = {self.name}"

    def values(self, key: str) -> list[str]:
        return [value for field_key, value in self.fields if field_key == key]


@dataclass(frozen=True)
class Srcinfo:
    """A parsed .SRCINFO document."""

    sections: tuple[SrcinfoSection, ...]

    @property
    def base(self) -> SrcinfoSection:
        bases = [section for section in self.sections if section.kind == "pkgbase"]
        if len(bases) != 1:
            raise MetadataError(f"expected exactly one pkgbase section, found {len(bases)}")
        return bases[0]

    @property
    def packages(self) -> tuple[SrcinfoSection, ...]:
        return tuple(section for section in self.sections if section.kind == "pkgname")

    def field_multiset(self) -> Counter[tuple[str, str, str]]:
        """Every field as a (section label, key, value) triple.

        Multiset equality over this is the order-independent comparison the Arch
        job blocks on, so a reordered but equivalent .SRCINFO is not a failure.
        """
        return Counter(
            (section.label, key, value)
            for section in self.sections
            for key, value in section.fields
        )


def parse_srcinfo(text: str, *, origin: str) -> Srcinfo:
    """Parse .SRCINFO text into its sections.

    `origin` only labels error messages.
    """
    sections: list[SrcinfoSection] = []
    kind = ""
    name = ""
    fields: list[tuple[str, str]] = []

    def close_section() -> None:
        if kind:
            sections.append(SrcinfoSection(kind=kind, name=name, fields=tuple(fields)))

    for number, raw_line in enumerate(text.splitlines(), start=1):
        line = raw_line.rstrip("\n")
        if not line.strip() or line.lstrip().startswith("#"):
            continue

        field_match = FIELD_RE.match(line)
        if field_match is not None:
            if not kind:
                raise MetadataError(f"{origin}:{number}: field before any pkgbase/pkgname section")
            fields.append((field_match.group(1), field_match.group(2).strip()))
            continue

        section_match = SECTION_RE.match(line)
        if section_match is None or section_match.group(1) not in SECTION_KEYS:
            raise MetadataError(f"{origin}:{number}: unsupported .SRCINFO line: {line!r}")

        close_section()
        kind = section_match.group(1)
        name = section_match.group(2).strip()
        fields = []

    close_section()
    if not sections:
        raise MetadataError(f"{origin}: no pkgbase/pkgname section found")
    return Srcinfo(sections=tuple(sections))


def _paren_depth(text: str) -> int:
    depth = 0
    quote = ""
    escaped = False
    for character in text:
        if escaped:
            escaped = False
            continue
        if quote:
            if character == "\\" and quote == '"':
                escaped = True
            elif character == quote:
                quote = ""
            continue
        if character == "\\":
            escaped = True
        elif character in "'\"":
            quote = character
        elif character == "(":
            depth += 1
        elif character == ")":
            depth -= 1
    return depth


def _split_words(text: str, *, origin: str, number: int) -> list[str]:
    try:
        return shlex.split(text)
    except ValueError as error:
        raise MetadataError(f"{origin}:{number}: could not split {text!r}: {error}") from error


def parse_pkgbuild(text: str, *, origin: str) -> dict[str, list[str]]:
    """Parse the top-level assignments of a PKGBUILD.

    Scalars come back as one-element lists so callers compare every field the
    same way. Function bodies are skipped whole: nothing inside `build()` is
    metadata, and reading it would require the bash semantics this parser
    refuses to emulate.
    """
    lines = text.splitlines()
    assignments: dict[str, list[str]] = {}
    index = 0

    while index < len(lines):
        line = lines[index]
        number = index + 1
        stripped = line.strip()

        if not stripped or stripped.startswith("#"):
            index += 1
            continue

        if FUNCTION_RE.match(line) is not None:
            index += 1
            while index < len(lines) and lines[index].rstrip() != "}":
                index += 1
            if index >= len(lines):
                raise MetadataError(f"{origin}:{number}: function body is never closed")
            index += 1
            continue

        assignment = ASSIGNMENT_RE.match(line)
        if assignment is None:
            raise MetadataError(f"{origin}:{number}: unsupported top-level statement: {stripped!r}")

        name = assignment.group(1)
        rest = assignment.group(2)

        if rest.lstrip().startswith("("):
            buffer = rest
            while _paren_depth(buffer) > 0:
                index += 1
                if index >= len(lines):
                    raise MetadataError(f"{origin}:{number}: array {name} is never closed")
                buffer = f"{buffer}\n{lines[index]}"
            inner = buffer.strip()
            values = _split_words(inner[1:-1], origin=origin, number=number)
        else:
            values = _split_words(rest, origin=origin, number=number)
            if len(values) > 1:
                raise MetadataError(
                    f"{origin}:{number}: scalar {name} expands to {len(values)} words"
                )

        if name in assignments:
            raise MetadataError(f"{origin}:{number}: {name} is assigned more than once")
        assignments[name] = values
        index += 1

    if not assignments:
        raise MetadataError(f"{origin}: no top-level assignments found")
    return assignments


def expand_values(
    values: list[str], assignments: dict[str, list[str]], *, origin: str, key: str
) -> list[str]:
    """Substitute `$name`/`${name}` from the PKGBUILD's own scalar assignments.

    .SRCINFO stores expanded values, so a PKGBUILD `source=(...v$pkgver...)` can
    only be compared against it after this step. An unknown variable raises
    rather than being left in place, because a silently unexpanded value would
    look like a genuine mismatch.
    """
    scalars = {name: value[0] for name, value in assignments.items() if len(value) == 1}

    def substitute(match: re.Match[str]) -> str:
        name = match.group(1) or match.group(2)
        if name not in scalars:
            raise MetadataError(f"{origin}: {key} references unknown variable ${name}")
        return scalars[name]

    return [VARIABLE_RE.sub(substitute, value) for value in values]
