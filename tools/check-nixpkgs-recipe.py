#!/usr/bin/env python3
"""Fail when a default Cargo feature needs a system library the Nix builds omit.

The nixpkgs recipe (`packaging/nixpkgs/package.nix`, mirrored from
`pkgs/by-name/wa/wayscriber/package.nix`) is bumped automatically by the
nixpkgs-update bot, which only rewrites the version and hashes. When a release
enables a dependency that links a C library, the bot's next pull request fails
to build unless a human has already added that library. This check catches the
mismatch here instead.

Every direct normal dependency in Cargo.toml must appear in SYSTEM_LIBRARIES,
mapped to the nixpkgs attributes it needs (an empty tuple for pure Rust crates).
A new normal dependency therefore fails this check until its system requirements
are stated.
"""

from __future__ import annotations

import json
import re
import subprocess
import sys
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parent.parent
CARGO_TOML = Path("Cargo.toml")
RECIPE = Path("packaging/nixpkgs/package.nix")
FLAKE = Path("flake.nix")
FLAKE_PACKAGE_MARKER = "wayscriber = pkgs.rustPlatform.buildRustPackage"
LINUX_TARGET = "x86_64-unknown-linux-gnu"

# Native tools/hooks required by the default GTK-enabled package.
NATIVE_BUILD_INPUTS = {"pkg-config", "wrapGAppsHook4"}

# Direct dependency -> nixpkgs attributes its build or link step requires.
# Attributes that nixpkgs propagates through another entry are left out: pango
# propagates cairo, glib and harfbuzz, and gtk4 propagates its own stack.
SYSTEM_LIBRARIES: dict[str, tuple[str, ...]] = {
    "anyhow": (),
    "cairo-rs": ("cairo",),
    "flate2": (),
    "getrandom": (),
    "glib": (),
    "gtk4": ("gtk4",),
    "gtk4-layer-shell": ("gtk4-layer-shell",),
    # Behind the opt-in `input-monitor` feature (system-wide input capture for
    # the input HUD), so these are mapped but not required by the default
    # package. Adding `input-monitor` to the default features would make the
    # attributes below mandatory in package.nix and flake.nix.
    "input": ("libinput",),
    "ksni": (),
    "libc": (),
    "log": (),
    "pango": ("pango",),
    "pangocairo": ("pango", "cairo"),
    "png": (),
    "schemars": (),
    "serde": (),
    "serde_ignored": (),
    "serde_json": (),
    "smithay-client-toolkit": ("libxkbcommon",),
    # Pure Rust: temporary files for the OCR engine handoff.
    "tempfile": (),
    "tokio": (),
    "toml": (),
    "toml_edit": (),
    "udev": ("udev",),
    "unicode-segmentation": (),
    "wayland-client": ("wayland",),
    "wayland-protocols": (),
    "wayland-protocols-wlr": (),
    "xkbcommon": ("libxkbcommon",),
    "zbus": (),
    "zune-jpeg": (),
}


class RecipeError(RuntimeError):
    """The recipe or flake could not be inspected."""


def read_text(path: Path) -> str:
    return (REPO_ROOT / path).read_text(encoding="utf-8")


def load_metadata() -> tuple[dict, dict, dict[str, str]]:
    """Load Cargo's parsed manifest and Linux default-feature dependency graph."""
    try:
        result = subprocess.run(
            [
                "cargo",
                "metadata",
                "--locked",
                "--format-version",
                "1",
                "--filter-platform",
                LINUX_TARGET,
            ],
            cwd=REPO_ROOT,
            check=False,
            capture_output=True,
            text=True,
        )
    except OSError as error:
        raise RecipeError(f"could not run cargo metadata: {error}") from error

    if result.returncode != 0:
        detail = result.stderr.strip() or f"exit status {result.returncode}"
        raise RecipeError(f"cargo metadata failed: {detail}")

    try:
        metadata = json.loads(result.stdout)
    except json.JSONDecodeError as error:
        raise RecipeError(f"could not parse cargo metadata: {error}") from error

    manifest_path = str((REPO_ROOT / CARGO_TOML).resolve())
    manifest = next(
        (
            package
            for package in metadata.get("packages", [])
            if package.get("manifest_path") == manifest_path
        ),
        None,
    )
    if manifest is None:
        raise RecipeError(f"cargo metadata did not contain {CARGO_TOML}")

    resolve = metadata.get("resolve") or {}
    node = next(
        (entry for entry in resolve.get("nodes", []) if entry.get("id") == manifest.get("id")),
        None,
    )
    if node is None:
        raise RecipeError(f"cargo metadata did not resolve {CARGO_TOML}")

    package_names = {
        package["id"]: package["name"]
        for package in metadata.get("packages", [])
        if "id" in package and "name" in package
    }
    return manifest, node, package_names


def direct_normal_dependencies(manifest: dict) -> set[str]:
    """All declared direct normal dependencies, including target-specific ones."""
    return {
        dependency["name"]
        for dependency in manifest.get("dependencies", [])
        if dependency.get("kind") is None and "name" in dependency
    }


def crates_enabled_by_default(node: dict, package_names: dict[str, str]) -> set[str]:
    """Direct normal dependencies resolved for a default-feature Linux build."""
    crates: set[str] = set()
    for dependency in node.get("deps", []):
        if not any(kind.get("kind") is None for kind in dependency.get("dep_kinds", [])):
            continue
        package_id = dependency.get("pkg")
        if package_id not in package_names:
            raise RecipeError(f"cargo metadata did not describe dependency {package_id!r}")
        crates.add(package_names[package_id])
    return crates


def nix_list_entries(text: str, attribute: str, *, start: int = 0) -> list[str]:
    """Return the identifiers inside the first `attribute = [ ... ];` after start."""
    # The lookbehind keeps `buildInputs` from matching inside `nativeBuildInputs`
    # should a future edit change its capitalisation.
    pattern = re.compile(
        rf"(?<![A-Za-z]){re.escape(attribute)}\s*=\s*(?:with\s+[\w.]+;\s*)?\[(.*?)\]",
        re.DOTALL,
    )
    match = pattern.search(text, start)
    if match is None:
        return []
    body = re.sub(r"#[^\n]*", "", match.group(1))
    return re.findall(r"[A-Za-z_][A-Za-z0-9_'-]*(?:\.[A-Za-z0-9_'-]+)*", body)


def recipe_inputs() -> tuple[set[str], set[str], set[str]]:
    text = read_text(RECIPE)
    native_build_inputs = set(nix_list_entries(text, "nativeBuildInputs"))
    build_inputs = set(nix_list_entries(text, "buildInputs"))

    arguments_match = re.match(r"\s*\{(.*?)\}\s*:", text, re.DOTALL)
    if arguments_match is None:
        raise RecipeError(f"{RECIPE}: could not read the function argument set")
    arguments = set(re.findall(r"[A-Za-z_][A-Za-z0-9_'-]*", arguments_match.group(1)))

    if not native_build_inputs:
        raise RecipeError(f"{RECIPE}: no nativeBuildInputs list found")
    if not build_inputs:
        raise RecipeError(f"{RECIPE}: no buildInputs list found")
    return native_build_inputs, build_inputs, arguments


def flake_inputs() -> tuple[set[str], set[str]]:
    text = read_text(FLAKE)
    marker = text.find(FLAKE_PACKAGE_MARKER)
    if marker == -1:
        raise RecipeError(
            f"{FLAKE}: could not find `{FLAKE_PACKAGE_MARKER}`; "
            "update FLAKE_PACKAGE_MARKER after restructuring the flake"
        )

    native_build_inputs = set(nix_list_entries(text, "nativeBuildInputs", start=marker))
    build_inputs = set(nix_list_entries(text, "buildInputs", start=marker))
    if not native_build_inputs:
        raise RecipeError(
            f"{FLAKE}: no nativeBuildInputs list found for the wayscriber package"
        )
    if not build_inputs:
        raise RecipeError(f"{FLAKE}: no buildInputs list found for the wayscriber package")
    return native_build_inputs, build_inputs


def main() -> int:
    try:
        manifest, node, package_names = load_metadata()
        crates = crates_enabled_by_default(node, package_names)
        recipe_native, declared_recipe, recipe_arguments = recipe_inputs()
        flake_native, declared_flake = flake_inputs()
    except (OSError, RecipeError) as error:
        print(f"nixpkgs recipe check failed: {error}", file=sys.stderr)
        return 1

    errors: list[str] = []

    unmapped = sorted(direct_normal_dependencies(manifest) - set(SYSTEM_LIBRARIES))
    for crate in unmapped:
        errors.append(
            f"dependency `{crate}` has no entry in SYSTEM_LIBRARIES; add the nixpkgs "
            "attributes it needs (or an empty tuple for a pure Rust crate)"
        )

    required: dict[str, set[str]] = {}
    for crate in sorted(crates):
        for attribute in SYSTEM_LIBRARIES.get(crate, ()):
            required.setdefault(attribute, set()).add(crate)

    for attribute in sorted(NATIVE_BUILD_INPUTS):
        if attribute not in recipe_native:
            errors.append(f"{RECIPE}: nativeBuildInputs is missing `{attribute}`")
        if attribute not in recipe_arguments:
            errors.append(f"{RECIPE}: function arguments are missing `{attribute}`")
        if attribute not in flake_native:
            errors.append(f"{FLAKE}: nativeBuildInputs is missing `{attribute}`")

    for attribute, sources in sorted(required.items()):
        reason = ", ".join(sorted(sources))
        if attribute not in declared_recipe:
            errors.append(f"{RECIPE}: buildInputs is missing `{attribute}` (required by {reason})")
        if attribute not in recipe_arguments:
            errors.append(f"{RECIPE}: function arguments are missing `{attribute}`")
        if attribute not in declared_flake:
            errors.append(f"{FLAKE}: buildInputs is missing `{attribute}` (required by {reason})")

    if errors:
        print("nixpkgs recipe check failed:", file=sys.stderr)
        for error in errors:
            print(f"- {error}", file=sys.stderr)
        print(
            "\nThe nixpkgs-update bot only rewrites version and hashes. A missing build "
            "input here means the next automated bump fails to build in nixpkgs; see "
            "packaging/nixpkgs/README.md.",
            file=sys.stderr,
        )
        return 1

    print(
        f"nixpkgs recipe OK: {len(crates)} default-feature dependencies require "
        f"{len(required)} system package(s) ({', '.join(sorted(required))}), "
        f"plus {len(NATIVE_BUILD_INPUTS)} native input(s), "
        "all declared in packaging/nixpkgs/package.nix and flake.nix."
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
