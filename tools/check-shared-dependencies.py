#!/usr/bin/env python3
"""Guard explicit upward Rust paths in shared layers.

This partial source guard understands rooted/parent-relative paths and grouped
use trees, and ignores comments and literals. It does not resolve aliases,
macro expansion, reexports, or Rust's complete module/dependency graph.
"""
from pathlib import Path
import re
import sys

NON_CODE = re.compile(
    r'r(?P<hashes>#{0,16})".*?"(?P=hashes)|"(?:\\.|[^"\\])*"|'
    r"'(?:\\.|[^'\\\n])'|//[^\n]*|/\*", re.S
)
TOKENS = re.compile(r'r#[A-Za-z_][A-Za-z_0-9]*|[A-Za-z_][A-Za-z_0-9]*|::|[{},;*]')
BOUNDARIES = [
    ("src/domain", {"config", "input", "draw", "backend", "ui", "session"}),
    ("src/config/validate", {"input", "backend"}),
]


def strip_non_code(source):
    pieces = []
    position = 0
    while match := NON_CODE.search(source, position):
        pieces.append(source[position:match.start()])
        position = match.end()
        if match.group() == "/*":
            depth = 1
            while depth and (marker := re.search(r'/\*|\*/', source[position:])):
                depth += 1 if marker.group() == "/*" else -1
                position += marker.end()
            if depth:
                position = len(source)
        pieces.append(" ")
    return ''.join(pieces) + source[position:]


def module_path(relative_path):
    parts = list(Path(relative_path).with_suffix('').parts[1:])
    if parts[-1] == "mod":
        parts.pop()
    return parts


def has_upward_path(source, relative_path, forbidden):
    tokens = [token.removeprefix("r#") for token in TOKENS.findall(strip_non_code(source))]
    module = module_path(relative_path)

    def tree(index, prefix):
        path = list(prefix)
        while index < len(tokens):
            token = tokens[index]
            if token in {",", ";", "}", "as"}:
                break
            if token == "{":
                index += 1
                while index < len(tokens) and tokens[index] != "}":
                    rejected, index = tree(index, path)
                    if rejected:
                        return True, index
                    if index < len(tokens) and tokens[index] == "as":
                        index += 2
                    if index < len(tokens) and tokens[index] == ",":
                        index += 1
                    elif index < len(tokens) and tokens[index] != "}":
                        break
                return False, index + 1
            if token == "crate":
                path = []
            elif token == "super":
                path = path[:-1]
            elif token not in {"self", "::", "*"}:
                path.append(token)
            if path and path[0] in forbidden:
                return True, index
            index += 1
            if index >= len(tokens) or tokens[index] != "::":
                break
            index += 1
        return False, index

    return any(
        tree(index, module)[0]
        for index, token in enumerate(tokens[:-1])
        if token in {"crate", "super", "self"} and tokens[index + 1] == "::"
    )


def check(root):
    errors = []
    for directory, forbidden in BOUNDARIES:
        for path in (root / directory).rglob("*.rs"):
            relative = path.relative_to(root).as_posix()
            if relative == "src/domain/tests.rs":
                continue  # Public-path compatibility assertions only.
            if has_upward_path(path.read_text(), relative, forbidden):
                errors.append(f"{relative}: upward dependency in shared layer")
    return errors


if __name__ == "__main__":
    errors = check(Path(__file__).resolve().parent.parent)
    if errors:
        print("\n".join(errors), file=sys.stderr)
        sys.exit(1)
    print("Shared domain and configuration-validation dependency paths passed.")
