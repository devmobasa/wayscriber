#!/usr/bin/env python3
"""Fail when a process-creation site is not in the reviewed ownership map."""

from __future__ import annotations

import re
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parent.parent
BROKER_ROOT = Path("src/process_broker")
BROKER_BOOTSTRAP = BROKER_ROOT / "bootstrap.rs"
DIRECT_PRODUCTION_ALLOWLIST = {
    Path("src/about_window/clipboard.rs"),  # standalone, descriptor-free About process
    Path("src/daemon/setup.rs"),  # pre-runtime systemd setup
    Path("configurator/src/app/session_catalog.rs"),  # separate configurator process
    Path("configurator/src/app/daemon_setup/command.rs"),
    Path("configurator/src/app/daemon_setup/service.rs"),
}

PROCESS_PATTERNS = (
    re.compile(r"\b(?:std::process::)?Command::new\b"),
    re.compile(r"\bstd::process::Child\b"),
    re.compile(r"\blibc::(?:fork|vfork|posix_spawn|posix_spawnp|pthread_atfork)\b"),
    re.compile(r"\blibc::SYS_(?:clone|clone3|fork|vfork)\b"),
    re.compile(r"\b(?:sh|bash|zsh)\s+-c\b"),
)


def is_test_source(path: Path) -> bool:
    parts = path.parts
    return path.parts[0] == "tests" or "tests" in parts or path.name == "tests.rs"


def rust_sources() -> list[Path]:
    roots = (ROOT / "src", ROOT / "configurator" / "src", ROOT / "tests")
    return sorted(path for root in roots for path in root.rglob("*.rs"))


def audit_sites() -> list[str]:
    failures: list[str] = []
    for absolute in rust_sources():
        relative = absolute.relative_to(ROOT)
        allowed = (
            relative.parts[:2] == BROKER_ROOT.parts
            or relative in DIRECT_PRODUCTION_ALLOWLIST
            or is_test_source(relative)
        )
        for line_number, line in enumerate(absolute.read_text().splitlines(), 1):
            code = line.split("//", 1)[0]
            if any(pattern.search(code) for pattern in PROCESS_PATTERNS) and not allowed:
                failures.append(f"{relative}:{line_number}: unclassified process site: {line.strip()}")
    return failures


def audit_child_stub() -> list[str]:
    source = (ROOT / BROKER_BOOTSTRAP).read_text()
    start_marker = "    if pid == 0 {"
    end_marker = "    drop(child_socket);"
    if start_marker not in source or end_marker not in source:
        return [f"{BROKER_BOOTSTRAP}: raw-clone child-stub markers changed"]
    stub = source.split(start_marker, 1)[1].split(end_marker, 1)[0]
    failures: list[str] = []
    banned = (
        "format!(",
        "log::",
        "panic!(",
        ".unwrap(",
        ".expect(",
        "drop(",
        "Command::",
        "CString::",
        "Vec::",
        "String::",
        "Box::",
    )
    for token in banned:
        if token in stub:
            failures.append(f"{BROKER_BOOTSTRAP}: child stub reaches banned token {token!r}")
    libc_calls = set(re.findall(r"libc::([A-Za-z0-9_]+)\s*\(", stub))
    unexpected_calls = libc_calls - {"syscall", "_exit"}
    if unexpected_calls:
        failures.append(
            f"{BROKER_BOOTSTRAP}: child stub reaches unapproved libc calls: "
            + ", ".join(sorted(unexpected_calls))
        )
    syscall_names = set(re.findall(r"libc::SYS_([A-Za-z0-9_]+)", stub))
    unexpected_syscalls = syscall_names - {
        "fcntl",
        "dup3",
        "setpgid",
        "exit_group",
        "close_range",
        "execve",
    }
    if unexpected_syscalls:
        failures.append(
            f"{BROKER_BOOTSTRAP}: child stub reaches unapproved syscalls: "
            + ", ".join(sorted(unexpected_syscalls))
        )
    return failures


def main() -> int:
    failures = audit_sites() + audit_child_stub()
    if failures:
        print("process-site audit failed:", file=sys.stderr)
        for failure in failures:
            print(f"  {failure}", file=sys.stderr)
        return 1
    print("process-site audit passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
