#!/usr/bin/env python3
"""Guard explicit upward Rust paths in the agreed shared-layer source boundary.

This is a source-path guard, not a full Rust dependency graph. Domain compatibility
path tests deliberately mention old public paths. Pure board normalization lives
in the domain layer; runtime board lifecycle stays in input.
"""
from pathlib import Path
import re
import sys

ROOT = Path(__file__).resolve().parent.parent
errors = []
for directory, forbidden in [
    ("src/domain", "config|input|draw|backend|ui|session"),
    ("src/config/validate", "input|backend"),
]:
    for path in (ROOT / directory).rglob("*.rs"):
        if path == ROOT / "src/domain/tests.rs":
            continue  # #[cfg(test)] public-path compatibility assertions.
        source = re.sub(r"/\*.*?\*/|//[^\n]*", "", path.read_text(), flags=re.S)
        # Also catch multiline grouped imports such as use crate::{input::...}.
        pattern = rf"crate\s*::\s*(?:{forbidden})\b|use\s+crate\s*::\s*\{{[^;]*\b(?:{forbidden})\s*::"
        if re.search(pattern, source):
            errors.append(f"{path.relative_to(ROOT)}: upward dependency in shared layer")
if errors:
    print("\n".join(errors), file=sys.stderr)
    sys.exit(1)
print("Shared domain and configuration-validation dependency paths passed.")
