#!/usr/bin/env python3
"""Fail when a checked-in .SRCINFO disagrees with what makepkg would generate.

`.SRCINFO` is a generated file that no Arch tool regenerates for us: it is
committed by hand next to its PKGBUILD and read by the AUR instead of the
PKGBUILD. A PKGBUILD edit that never reaches the .SRCINFO therefore publishes
metadata that describes the previous release. This checker closes that gap by
running the real `makepkg --printsrcinfo` and comparing.

The comparison is deliberately two-tiered. Parsed field-multiset equality is the
blocking assertion -- it is a property of the recipe, and a reordered but
equivalent serialization is not a defect. A byte-level difference is only a
warning that names the regenerate task, because makepkg is a rolling package and
a serialization change on its side must not fail unrelated pull requests.

makepkg refuses to run as root and aborts on a root-owned working directory, so
in a container this must be pointed at an unprivileged account with
`--builder-user`; the working directory is created for, and handed to, that user.

Both inputs may be templates: repeat `--token NAME=VALUE` to substitute `@NAME@`
before the comparison, which is how the AUR template pair is validated without
ever contacting the AUR.
"""

from __future__ import annotations

import argparse
import os
import pwd
import re
import shutil
import subprocess
import sys
import tempfile
from collections import Counter
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(Path(__file__).resolve().parent))

from pkgbuild_meta import MetadataError, parse_srcinfo

TOKEN_RE = re.compile(r"@[A-Za-z0-9_]+@")


class CanonicalError(RuntimeError):
    """The canonical .SRCINFO comparison could not be carried out."""


def resolve_input(path: Path) -> Path:
    """Anchor a relative input at the repository, not at the caller.

    The documented commands name repository-relative paths
    (`--pkgbuild packaging/PKGBUILD`), and the CI jobs pass exactly those. Read
    against the working directory they would only resolve from the repository
    root, so running the documented command from any subdirectory would fail on
    a missing file. An absolute path is the caller's own choice and is left
    alone. Messages keep quoting the path as it was given, because that is the
    string the reader typed.
    """
    return path if path.is_absolute() else REPO_ROOT / path


def read_text(path: Path) -> str:
    try:
        return path.read_text(encoding="utf-8")
    except OSError as error:
        raise CanonicalError(f"could not read {path}: {error}") from error


def render(text: str, tokens: dict[str, str], origin: str) -> str:
    if not tokens:
        return text
    for name, value in tokens.items():
        text = text.replace(f"@{name}@", value)
    leftovers = sorted(set(TOKEN_RE.findall(text)))
    if leftovers:
        raise CanonicalError(f"{origin}: unresolved template token(s): {', '.join(leftovers)}")
    return text


def parse_token(raw: str) -> tuple[str, str]:
    name, separator, value = raw.partition("=")
    if not separator or not name:
        raise CanonicalError(f"--token expects NAME=VALUE, got {raw!r}")
    return name, value


def prepare_work_dir(work_root: Path | None, builder_user: str | None) -> Path:
    base = work_root if work_root is not None else Path(tempfile.gettempdir())
    try:
        base.mkdir(parents=True, exist_ok=True)
        work_dir = Path(tempfile.mkdtemp(prefix="srcinfo-canonical-", dir=str(base)))
    except OSError as error:
        raise CanonicalError(f"could not create a working directory under {base}: {error}") from error

    if builder_user is None:
        return work_dir

    try:
        account = pwd.getpwnam(builder_user)
    except KeyError as error:
        raise CanonicalError(f"no such user: {builder_user}") from error
    try:
        os.chown(work_dir, account.pw_uid, account.pw_gid)
        os.chmod(work_dir, 0o755)
    except OSError as error:
        raise CanonicalError(f"could not hand {work_dir} to {builder_user}: {error}") from error
    return work_dir


def write_pkgbuild(work_dir: Path, text: str, builder_user: str | None) -> Path:
    path = work_dir / "PKGBUILD"
    try:
        path.write_text(text, encoding="utf-8")
    except OSError as error:
        raise CanonicalError(f"could not write {path}: {error}") from error

    if builder_user is not None:
        try:
            account = pwd.getpwnam(builder_user)
            os.chown(path, account.pw_uid, account.pw_gid)
        except (KeyError, OSError) as error:
            raise CanonicalError(f"could not hand {path} to {builder_user}: {error}") from error
    return path


def run_printsrcinfo(work_dir: Path, builder_user: str | None) -> str:
    if builder_user is None:
        if os.geteuid() == 0:
            raise CanonicalError(
                "makepkg refuses to run as root; pass --builder-user with an unprivileged account"
            )
        command = ["makepkg", "--printsrcinfo"]
        cwd: str | None = str(work_dir)
    else:
        # runuser resets HOME to the target account, which is why the working
        # directory has to belong to that account as well.
        command = [
            "runuser",
            "-u",
            builder_user,
            "--",
            "bash",
            "-c",
            'cd "$1" && makepkg --printsrcinfo',
            "bash",
            str(work_dir),
        ]
        cwd = None

    try:
        result = subprocess.run(command, cwd=cwd, check=False, capture_output=True, text=True)
    except OSError as error:
        raise CanonicalError(f"failed to run {command[0]}: {error}") from error

    if result.returncode != 0:
        detail = result.stderr.strip() or result.stdout.strip() or "no diagnostic output"
        raise CanonicalError(f"makepkg --printsrcinfo failed:\n{detail}")
    if not result.stdout.strip():
        raise CanonicalError("makepkg --printsrcinfo produced no output")
    return result.stdout


def report_field_differences(
    expected: Counter[tuple[str, str, str]],
    generated: Counter[tuple[str, str, str]],
) -> list[str]:
    lines: list[str] = []
    for section, key, value in sorted((expected - generated).elements()):
        lines.append(f"  only in the checked-in file: [{section}] {key} = {value}")
    for section, key, value in sorted((generated - expected).elements()):
        lines.append(f"  only in makepkg output:      [{section}] {key} = {value}")
    return lines


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--pkgbuild", required=True, type=Path, help="PKGBUILD (or template) path")
    parser.add_argument("--srcinfo", required=True, type=Path, help=".SRCINFO (or template) path")
    parser.add_argument(
        "--token",
        action="append",
        default=[],
        metavar="NAME=VALUE",
        help="substitute @NAME@ in both inputs before comparing (repeatable)",
    )
    parser.add_argument("--label", default="", help="name for this pair in the output")
    parser.add_argument(
        "--builder-user",
        default=None,
        help="unprivileged account that runs makepkg (required when running as root)",
    )
    parser.add_argument(
        "--work-root",
        default=None,
        type=Path,
        help="directory to create the makepkg working directory under",
    )
    parser.add_argument(
        "--refresh-task",
        default="regenerate the .SRCINFO from its PKGBUILD with `makepkg --printsrcinfo`",
        help="what a reviewer should run when only the serialization differs",
    )
    arguments = parser.parse_args()

    label = arguments.label or f"{arguments.pkgbuild} vs {arguments.srcinfo}"
    work_dir: Path | None = None

    try:
        tokens = dict(parse_token(raw) for raw in arguments.token)
        pkgbuild_text = render(
            read_text(resolve_input(arguments.pkgbuild)), tokens, arguments.pkgbuild.as_posix()
        )
        expected_text = render(
            read_text(resolve_input(arguments.srcinfo)), tokens, arguments.srcinfo.as_posix()
        )

        work_dir = prepare_work_dir(arguments.work_root, arguments.builder_user)
        write_pkgbuild(work_dir, pkgbuild_text, arguments.builder_user)
        generated_text = run_printsrcinfo(work_dir, arguments.builder_user)

        expected = parse_srcinfo(expected_text, origin=arguments.srcinfo.as_posix()).field_multiset()
        generated = parse_srcinfo(generated_text, origin="makepkg --printsrcinfo").field_multiset()
    except (CanonicalError, MetadataError) as error:
        print(f"Canonical .SRCINFO check failed ({label}): {error}", file=sys.stderr)
        return 2
    finally:
        if work_dir is not None:
            shutil.rmtree(work_dir, ignore_errors=True)

    if expected != generated:
        print(f"Canonical .SRCINFO check failed ({label}): fields differ.", file=sys.stderr)
        for line in report_field_differences(expected, generated):
            print(line, file=sys.stderr)
        print(f"\nFix by regenerating: {arguments.refresh_task}", file=sys.stderr)
        return 1

    if expected_text != generated_text:
        print(
            f"WARNING: {label} agrees on every field but not byte-for-byte. "
            "makepkg's serialization has most likely moved.\n"
            f"         Refresh when convenient: {arguments.refresh_task}"
        )
        return 0

    print(f"Canonical .SRCINFO OK ({label}): byte-identical to `makepkg --printsrcinfo`.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
