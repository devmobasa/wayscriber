#!/usr/bin/env python3
"""Fail when a repository Rust source is absent from the supported Cargo matrix."""

from __future__ import annotations

import json
import shlex
import subprocess
import sys
from pathlib import Path
from typing import Any


REPO_ROOT = Path(__file__).resolve().parent.parent
CARGO_CONFIGURATIONS = (
    ("all features", "--all-features"),
    ("no default features", "--no-default-features"),
)
EXPLICIT_ENTRY_POINTS = {Path("build.rs")}


class CoverageError(RuntimeError):
    """A source-coverage prerequisite or Cargo check failed."""


def run_text(command: list[str]) -> str:
    try:
        result = subprocess.run(
            command,
            cwd=REPO_ROOT,
            check=False,
            capture_output=True,
            text=True,
        )
    except OSError as error:
        raise CoverageError(f"failed to run {command[0]}: {error}") from error

    if result.returncode != 0:
        detail = result.stderr.strip() or result.stdout.strip() or "no diagnostic output"
        raise CoverageError(f"{' '.join(command)} failed:\n{detail}")
    return result.stdout


def workspace_package_ids() -> set[str]:
    raw = run_text(["cargo", "metadata", "--locked", "--no-deps", "--format-version", "1"])
    try:
        metadata = json.loads(raw)
        package_ids = {package["id"] for package in metadata["packages"]}
    except (KeyError, TypeError, json.JSONDecodeError) as error:
        raise CoverageError(f"could not parse cargo metadata: {error}") from error

    if not package_ids:
        raise CoverageError("cargo metadata returned no workspace packages")
    return package_ids


def dep_info_for_artifact(filename: Path) -> Path | None:
    if filename.suffix:
        stem = filename.stem
    else:
        stem = filename.name

    candidates = [filename.with_name(f"{stem}.d")]
    if stem.startswith("lib"):
        candidates.append(filename.with_name(f"{stem[3:]}.d"))

    return next((candidate for candidate in candidates if candidate.is_file()), None)


def cargo_dep_info(package_ids: set[str], configuration_flag: str) -> set[Path]:
    command = [
        "cargo",
        "check",
        "--workspace",
        "--locked",
        "--all-targets",
        configuration_flag,
        "--message-format=json-render-diagnostics",
    ]
    try:
        process = subprocess.Popen(
            command,
            cwd=REPO_ROOT,
            stdout=subprocess.PIPE,
            text=True,
        )
    except OSError as error:
        raise CoverageError(f"failed to run cargo: {error}") from error

    if process.stdout is None:
        process.kill()
        raise CoverageError("cargo check did not provide a JSON output stream")

    dep_info_paths: set[Path] = set()
    diagnostics: list[str] = []
    parse_errors: list[str] = []

    for line_number, line in enumerate(process.stdout, start=1):
        try:
            message: dict[str, Any] = json.loads(line)
        except json.JSONDecodeError as error:
            parse_errors.append(f"line {line_number}: {error}")
            continue

        if message.get("reason") == "compiler-message":
            rendered = message.get("message", {}).get("rendered")
            if rendered:
                diagnostics.append(rendered.rstrip())
            continue

        if message.get("reason") != "compiler-artifact":
            continue
        if message.get("package_id") not in package_ids:
            continue
        if "custom-build" in message.get("target", {}).get("kind", []):
            continue

        artifact_dep_info = {
            dep_info
            for raw_filename in message.get("filenames", [])
            if (dep_info := dep_info_for_artifact(Path(raw_filename))) is not None
        }
        if not artifact_dep_info:
            target_name = message.get("target", {}).get("name", "unknown target")
            process.kill()
            process.wait()
            raise CoverageError(f"cargo artifact for {target_name} has no adjacent dep-info file")
        dep_info_paths.update(artifact_dep_info)

    return_code = process.wait()
    if return_code != 0:
        detail = "\n".join(diagnostics) or f"cargo exited with status {return_code}"
        raise CoverageError(f"{' '.join(command)} failed:\n{detail}")
    if parse_errors:
        raise CoverageError("invalid cargo JSON output:\n" + "\n".join(parse_errors))
    if not dep_info_paths:
        raise CoverageError("cargo check returned no workspace dep-info files")
    return dep_info_paths


def sources_from_dep_info(path: Path) -> set[Path]:
    try:
        text = path.read_text(encoding="utf-8")
    except OSError as error:
        raise CoverageError(f"could not read dep-info {path}: {error}") from error

    first_rule = text.replace("\\\n", " ").split("\n\n", maxsplit=1)[0]
    if ":" not in first_rule:
        raise CoverageError(f"dep-info has no dependency rule: {path}")
    dependencies = first_rule.split(":", maxsplit=1)[1]
    try:
        tokens = shlex.split(dependencies)
    except ValueError as error:
        raise CoverageError(f"could not parse dep-info {path}: {error}") from error

    sources: set[Path] = set()
    for token in tokens:
        if not token.endswith(".rs"):
            continue
        candidate = Path(token)
        if not candidate.is_absolute():
            candidate = REPO_ROOT / candidate
        try:
            relative = candidate.resolve().relative_to(REPO_ROOT)
        except ValueError:
            continue
        if candidate.is_file():
            sources.add(relative)
    return sources


def repository_rust_sources() -> set[Path]:
    raw = run_text(
        [
            "git",
            "ls-files",
            "-co",
            "--exclude-standard",
            "--",
            "*.rs",
        ]
    )
    return {
        path
        for line in raw.splitlines()
        if line and (path := Path(line)) and (REPO_ROOT / path).is_file()
    }


def main() -> int:
    try:
        package_ids = workspace_package_ids()

        dep_info_paths: set[Path] = set()
        for label, flag in CARGO_CONFIGURATIONS:
            print(f"Checking Rust source coverage ({label})...", file=sys.stderr)
            dep_info_paths.update(cargo_dep_info(package_ids, flag))

        covered = set(EXPLICIT_ENTRY_POINTS)
        for dep_info_path in dep_info_paths:
            covered.update(sources_from_dep_info(dep_info_path))
        repository_sources = repository_rust_sources()
        uncovered = sorted(repository_sources - covered)
    except CoverageError as error:
        print(f"Rust source coverage check failed: {error}", file=sys.stderr)
        return 2

    if uncovered:
        print(
            f"Rust source coverage check failed: {len(uncovered)} source file(s) are not compiled "
            "by the supported Cargo matrix:",
            file=sys.stderr,
        )
        for path in uncovered:
            print(f"- {path.as_posix()}", file=sys.stderr)
        return 1

    print(
        f"Rust source coverage OK: {len(repository_sources)} source files covered across "
        f"{len(CARGO_CONFIGURATIONS)} Cargo configurations "
        f"({len(dep_info_paths)} current dep-info files)."
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
