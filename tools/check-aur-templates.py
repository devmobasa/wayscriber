#!/usr/bin/env python3
"""Fail when the external wayscriber-configurator AUR recipe drifts from its contract.

The AUR package `wayscriber-configurator` lives in a repository this checkout
does not contain. What this repository owns is the template pair
`packaging/aur/wayscriber-configurator/{PKGBUILD,.SRCINFO}.tmpl`, which
`tools/update-aur-from-manifest.sh` renders and installs over the clone on every
release. That makes the templates the only reviewable copy of the recipe, so they
are checked here rather than after publication.

Two modes, one rule set:

* no arguments -- render the checked-in templates with fixture values and validate
  the result, plus the template-only gates (token vocabulary, and the
  `git check-ignore` exit-1 proof that `packaging/**` has not swallowed them);
* `--pair DIR` -- validate an already-rendered `PKGBUILD`/`.SRCINFO` pair. The
  updater calls this on its temporary render before anything reaches the clone.

Validation is asymmetric on purpose. `.SRCINFO` cannot represent a build body, so
the modern-feature rule is asserted against the PKGBUILD alone; the two files are
only required to agree on the metadata both can express. Agreement is also not
sufficient: the live external recipe agreed with its own .SRCINFO while declaring
none of the GTK4 dependencies, so the required dependency set is asserted
outright.
"""

from __future__ import annotations

import argparse
import re
import shlex
import subprocess
import sys
from collections import Counter
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(Path(__file__).resolve().parent))

from pkgbuild_meta import MetadataError, Srcinfo, expand_values, parse_pkgbuild, parse_srcinfo

TEMPLATE_DIR = Path("packaging/aur/wayscriber-configurator")
PKGBUILD_TEMPLATE = TEMPLATE_DIR / "PKGBUILD.tmpl"
SRCINFO_TEMPLATE = TEMPLATE_DIR / ".SRCINFO.tmpl"

TOKEN_RE = re.compile(r"@[A-Za-z0-9_]+@")
KNOWN_TOKENS = ("VERSION", "PKGREL", "SOURCE_SHA256")
REQUIRED_PKGBUILD_TOKENS = ("VERSION", "PKGREL", "SOURCE_SHA256")

# Deliberately unlike any real release so a value that leaks out of a fixture is
# recognisable on sight.
FIXTURE_VALUES = {
    "VERSION": "9.9.9",
    "PKGREL": "7",
    "SOURCE_SHA256": "1" * 64,
}

CONFIGURATOR_MANIFEST = "configurator/Cargo.toml"
MODERN_FEATURE = "adw-modern"

REQUIRED_DEPENDS = ("libadwaita>=1.7", "gtk4", "libxkbcommon")
REQUIRED_MAKEDEPENDS = ("cargo",)

# Every field .SRCINFO can express. `pkgname` is compared against the .SRCINFO
# `pkgbase`, since this recipe is a single-package build.
SHARED_ARRAY_FIELDS = (
    "arch",
    "license",
    "depends",
    "makedepends",
    "optdepends",
    "source",
    "sha256sums",
)
SHARED_SCALAR_FIELDS = ("pkgver", "pkgrel", "pkgdesc", "url")
SINGLETON_FIELDS = ("pkgdesc", "pkgver", "pkgrel", "url")


class TemplateError(RuntimeError):
    """A template or a rendered pair violated the AUR channel contract."""


def read_text(path: Path) -> str:
    try:
        return path.read_text(encoding="utf-8")
    except OSError as error:
        raise TemplateError(f"could not read {path}: {error}") from error


def render(text: str, values: dict[str, str]) -> str:
    for name, value in values.items():
        text = text.replace(f"@{name}@", value)
    return text


def check_no_unresolved_tokens(text: str, origin: str, errors: list[str]) -> None:
    leftovers = sorted(set(TOKEN_RE.findall(text)))
    if leftovers:
        errors.append(f"{origin}: unresolved template token(s): {', '.join(leftovers)}")


def check_token_vocabulary(text: str, origin: str, errors: list[str]) -> set[str]:
    used = {token.strip("@") for token in TOKEN_RE.findall(text)}
    unknown = sorted(used - set(KNOWN_TOKENS))
    if unknown:
        errors.append(
            f"{origin}: unknown template token(s) {', '.join(unknown)}; "
            f"the renderer only substitutes {', '.join(KNOWN_TOKENS)}"
        )
    return used


def cargo_build_commands(pkgbuild_text: str, origin: str, errors: list[str]) -> list[list[str]]:
    commands: list[list[str]] = []
    for number, raw_line in enumerate(pkgbuild_text.splitlines(), start=1):
        line = raw_line.strip()
        if line.startswith("#") or "cargo build" not in line:
            continue
        try:
            words = shlex.split(line)
        except ValueError as error:
            errors.append(f"{origin}:{number}: could not split build command: {error}")
            continue
        if "cargo" in words and "build" in words:
            commands.append(words)
    return commands


def option_values(words: list[str], option: str) -> list[str]:
    values: list[str] = []
    for index, word in enumerate(words):
        if word == option and index + 1 < len(words):
            values.append(words[index + 1])
        elif word.startswith(f"{option}="):
            values.append(word.split("=", maxsplit=1)[1])
    return values


def check_modern_feature(pkgbuild_text: str, origin: str, errors: list[str]) -> None:
    commands = cargo_build_commands(pkgbuild_text, origin, errors)
    if not commands:
        errors.append(f"{origin}: no cargo build command found")
        return

    configurator_with_feature = 0
    for words in commands:
        manifests = option_values(words, "--manifest-path")
        features = {
            feature
            for value in option_values(words, "--features")
            for feature in re.split(r"[ ,]+", value)
            if feature
        }
        builds_configurator = CONFIGURATOR_MANIFEST in manifests
        if MODERN_FEATURE in features and not builds_configurator:
            errors.append(
                f"{origin}: `{MODERN_FEATURE}` is passed to a build that is not the "
                f"configurator (`--manifest-path {CONFIGURATOR_MANIFEST}`): "
                f"{' '.join(words)}"
            )
        if builds_configurator and MODERN_FEATURE in features:
            configurator_with_feature += 1

    if configurator_with_feature == 0:
        errors.append(
            f"{origin}: the configurator build command must pass "
            f"`--features {MODERN_FEATURE}`; this channel targets Arch, which always "
            "ships libadwaita >= 1.7"
        )


def check_required_dependencies(
    assignments: dict[str, list[str]], origin: str, errors: list[str]
) -> None:
    for field, required in (
        ("depends", REQUIRED_DEPENDS),
        ("makedepends", REQUIRED_MAKEDEPENDS),
    ):
        declared = set(assignments.get(field, []))
        missing = [value for value in required if value not in declared]
        if missing:
            errors.append(
                f"{origin}: {field} is missing {', '.join(missing)}; cross-file agreement "
                "alone would accept a recipe that declares none of them"
            )


def check_srcinfo_structure(srcinfo: Srcinfo, origin: str, errors: list[str]) -> None:
    try:
        base = srcinfo.base
    except MetadataError as error:
        errors.append(f"{origin}: {error}")
        return

    if not srcinfo.packages:
        errors.append(f"{origin}: no pkgname section")

    for field in SINGLETON_FIELDS:
        count = len(base.values(field))
        if count != 1:
            errors.append(f"{origin}: expected exactly one `{field}` in {base.label}, found {count}")

    sources = len(base.values("source"))
    checksums = len(base.values("sha256sums"))
    if sources != checksums:
        errors.append(
            f"{origin}: {sources} source entr(ies) but {checksums} sha256sums entr(ies)"
        )


def check_shared_metadata(
    assignments: dict[str, list[str]],
    srcinfo: Srcinfo,
    pkgbuild_origin: str,
    srcinfo_origin: str,
    errors: list[str],
) -> None:
    try:
        base = srcinfo.base
    except MetadataError as error:
        errors.append(f"{srcinfo_origin}: {error}")
        return

    pkgnames = assignments.get("pkgname", [])
    if pkgnames != [base.name]:
        errors.append(
            f"{pkgbuild_origin}: pkgname {pkgnames} does not match "
            f"{srcinfo_origin} pkgbase {base.name!r}"
        )
    for package in srcinfo.packages:
        if package.name not in pkgnames:
            errors.append(
                f"{srcinfo_origin}: pkgname section {package.name!r} is absent from "
                f"{pkgbuild_origin}"
            )

    for field in SHARED_SCALAR_FIELDS + SHARED_ARRAY_FIELDS:
        try:
            declared = expand_values(
                assignments.get(field, []), assignments, origin=pkgbuild_origin, key=field
            )
        except MetadataError as error:
            errors.append(str(error))
            continue
        recorded = base.values(field)
        if Counter(declared) != Counter(recorded):
            errors.append(
                f"{field}: {pkgbuild_origin} has {sorted(declared)} but "
                f"{srcinfo_origin} has {sorted(recorded)}"
            )


def validate_pair(
    pkgbuild_text: str,
    srcinfo_text: str,
    pkgbuild_origin: str,
    srcinfo_origin: str,
    errors: list[str],
) -> None:
    check_no_unresolved_tokens(pkgbuild_text, pkgbuild_origin, errors)
    check_no_unresolved_tokens(srcinfo_text, srcinfo_origin, errors)
    check_modern_feature(pkgbuild_text, pkgbuild_origin, errors)

    try:
        assignments = parse_pkgbuild(pkgbuild_text, origin=pkgbuild_origin)
    except MetadataError as error:
        errors.append(str(error))
        return
    try:
        srcinfo = parse_srcinfo(srcinfo_text, origin=srcinfo_origin)
    except MetadataError as error:
        errors.append(str(error))
        return

    check_required_dependencies(assignments, pkgbuild_origin, errors)
    check_srcinfo_structure(srcinfo, srcinfo_origin, errors)
    check_shared_metadata(assignments, srcinfo, pkgbuild_origin, srcinfo_origin, errors)


def check_unignored(path: Path, errors: list[str]) -> None:
    """`git check-ignore` must exit 1 for a tracked template.

    The non-verbose form is required: `-v` exits 0 for a negation match too, so
    it cannot tell "not ignored" from "ignored".
    """
    try:
        result = subprocess.run(
            ["git", "check-ignore", path.as_posix()],
            cwd=REPO_ROOT,
            check=False,
            capture_output=True,
            text=True,
        )
    except OSError as error:
        errors.append(f"could not run git check-ignore for {path}: {error}")
        return

    if result.returncode != 1:
        errors.append(
            f"{path}: git check-ignore exited {result.returncode}, expected 1 "
            "(the file is ignored, so the release would publish a stale recipe); "
            "re-include the whole parent chain in .gitignore"
        )


def check_templates(errors: list[str]) -> None:
    pkgbuild_template_text = read_text(REPO_ROOT / PKGBUILD_TEMPLATE)
    srcinfo_template_text = read_text(REPO_ROOT / SRCINFO_TEMPLATE)

    pkgbuild_tokens = check_token_vocabulary(
        pkgbuild_template_text, PKGBUILD_TEMPLATE.as_posix(), errors
    )
    srcinfo_tokens = check_token_vocabulary(
        srcinfo_template_text, SRCINFO_TEMPLATE.as_posix(), errors
    )

    missing = [token for token in REQUIRED_PKGBUILD_TOKENS if token not in pkgbuild_tokens]
    if missing:
        errors.append(
            f"{PKGBUILD_TEMPLATE.as_posix()}: missing token(s) {', '.join(missing)}; "
            "a value the release must supply would be frozen into every published recipe"
        )
    orphans = sorted(srcinfo_tokens - pkgbuild_tokens)
    if orphans:
        errors.append(
            f"{SRCINFO_TEMPLATE.as_posix()}: token(s) {', '.join(orphans)} do not appear in "
            f"{PKGBUILD_TEMPLATE.as_posix()}, so the two files cannot describe the same build"
        )

    for path in (PKGBUILD_TEMPLATE, SRCINFO_TEMPLATE):
        check_unignored(path, errors)

    validate_pair(
        render(pkgbuild_template_text, FIXTURE_VALUES),
        render(srcinfo_template_text, FIXTURE_VALUES),
        f"{PKGBUILD_TEMPLATE.as_posix()} (fixture render)",
        f"{SRCINFO_TEMPLATE.as_posix()} (fixture render)",
        errors,
    )


def check_rendered_pair(directory: Path, errors: list[str]) -> None:
    pkgbuild_path = directory / "PKGBUILD"
    srcinfo_path = directory / ".SRCINFO"
    validate_pair(
        read_text(pkgbuild_path),
        read_text(srcinfo_path),
        pkgbuild_path.as_posix(),
        srcinfo_path.as_posix(),
        errors,
    )


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument(
        "--pair",
        metavar="DIR",
        type=Path,
        help="validate an already-rendered PKGBUILD/.SRCINFO pair in DIR "
        "instead of the checked-in templates",
    )
    arguments = parser.parse_args()

    errors: list[str] = []
    try:
        if arguments.pair is None:
            check_templates(errors)
            subject = "AUR template pair"
        else:
            check_rendered_pair(arguments.pair, errors)
            subject = f"rendered AUR pair in {arguments.pair}"
    except TemplateError as error:
        print(f"AUR template check failed: {error}", file=sys.stderr)
        return 2

    if errors:
        print("AUR template check failed:", file=sys.stderr)
        for error in errors:
            print(f"- {error}", file=sys.stderr)
        return 1

    print(f"{subject} OK: modern build flag, required dependencies, and .SRCINFO agreement hold.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
