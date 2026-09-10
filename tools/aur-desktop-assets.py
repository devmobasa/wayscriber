#!/usr/bin/env python3
"""Standalone counterpart of the C# AUR desktop-asset recipe command."""

import json
import pathlib
import re
import sys


class ManifestError(Exception):
    pass


ASSET_PREFIXES = (
    "/usr/share/applications/",
    "/usr/share/icons/",
    "/usr/share/pixmaps/",
)


def parse_contents(root: pathlib.Path, package: str, filename: str):
    path = root / "packaging" / filename
    lines = path.read_text(encoding="utf-8").splitlines()
    content_headers = [index for index, line in enumerate(lines) if line == "contents:"]
    if len(content_headers) != 1:
        raise ManifestError(f"{filename}: expected one package contents sequence")

    entries = []
    current = None
    in_contents = False
    for line in lines[content_headers[0] + 1 :]:
        if line and not line.startswith(" "):
            break
        if line.startswith("  - "):
            if current is not None:
                entries.append(current)
            current = {}
            in_contents = True
            parse_field(current, line[4:], filename)
        elif current is not None and line.startswith("    "):
            stripped = line[4:]
            if stripped == "file_info:":
                current["file_info"] = True
            elif line.startswith("      "):
                parse_field(current, line[6:], filename)
            elif stripped and not stripped.startswith("#"):
                parse_field(current, stripped, filename)
        elif line.strip() and in_contents:
            raise ManifestError(f"{filename}: invalid content entry")
    if current is not None:
        entries.append(current)
    if not in_contents:
        raise ManifestError(f"{filename}: expected one package contents sequence")

    assets = []
    destinations = set()
    for entry in entries:
        destination = scalar(entry, "dst")
        if not destination.startswith(ASSET_PREFIXES):
            continue
        source = scalar(entry, "src")
        validate_asset(root, filename, entry, source, destination)
        if destination in destinations:
            raise ManifestError(f"{filename}: duplicate desktop destination {destination}")
        destinations.add(destination)
        assets.append((source, destination))

    if (
        f"/usr/share/applications/{package}.desktop" not in destinations
        or not any(item.startswith("/usr/share/icons/") for item in destinations)
    ):
        raise ManifestError(f"{filename}: launcher and icons are required")
    return assets


def parse_field(entry, text, filename):
    if ":" not in text:
        raise ManifestError(f"{filename}: invalid content entry")
    key, value = text.split(":", 1)
    entry[key.strip()] = value.strip()


def scalar(entry, key):
    value = entry.get(key)
    if not value:
        raise ManifestError(f"Expected scalar {key}")
    return value


def validate_asset(root, filename, entry, source, destination):
    if (
        not re.fullmatch(r"packaging/[A-Za-z0-9_./-]+", source)
        or not re.fullmatch(r"/usr/share/[A-Za-z0-9_./-]+", destination)
        or ".." in source.split("/")
        or ".." in destination.split("/")
    ):
        raise ManifestError(f"{filename}: unsupported asset path {source} -> {destination}")
    if not entry.get("file_info"):
        raise ManifestError(
            f"{filename}: desktop asset {destination} has no file_info mapping"
        )
    mode = entry.get("mode", "")
    try:
        if not re.fullmatch(r"(?:0o[0-7]+|0[0-7]+|[1-9][0-9]*|0)", mode):
            raise ValueError
        number = int(mode[2:], 8) if mode.lower().startswith("0o") else int(mode, 8 if mode.startswith("0") else 10)
    except ValueError:
        number = -1
    if number != 0o644:
        raise ManifestError(
            f"{filename}: desktop asset {destination} must have mode 0644"
        )
    if not (root / source).is_file():
        raise ManifestError(f"{filename}: missing desktop asset {source}")


def recipe(label, title, binary, package, assets, from_archive):
    lines = []
    destinations = []
    for source, destination in assets:
        input_path = f'"${{srcdir_tmp}}{destination}"' if from_archive else source
        lines.append(f'    install -Dm644 {input_path} "$pkgdir{destination}"')
        destinations.append(destination)
    return {
        "label": label,
        "marker": f"# {title} desktop integration",
        "end_marker": f"# End {title} desktop integration",
        "anchor": f'    install -Dm755 "{binary}" "$pkgdir/usr/bin/{package}"',
        "lines": lines,
        "destinations": destinations,
    }


def main():
    if len(sys.argv) != 2:
        raise ManifestError("Usage: aur-desktop-assets.py REPO_ROOT")
    root = pathlib.Path(sys.argv[1]).resolve()
    main_assets = parse_contents(root, "wayscriber", "package.wayscriber.yaml")
    configurator_assets = parse_contents(
        root, "wayscriber-configurator", "package.configurator.yaml"
    )
    result = {
        "desktop_path_pattern": "/usr/share/applications/|/usr/share/icons/|/usr/share/pixmaps/",
        "source": recipe(
            "wayscriber source",
            "Wayscriber",
            "target/release/wayscriber",
            "wayscriber",
            main_assets,
            False,
        ),
        "bin": recipe(
            "wayscriber bin",
            "Wayscriber",
            "${srcdir_tmp}/usr/bin/wayscriber",
            "wayscriber",
            main_assets,
            True,
        ),
        "configurator": recipe(
            "wayscriber-configurator",
            "Wayscriber configurator",
            "target/release/wayscriber-configurator",
            "wayscriber-configurator",
            configurator_assets,
            False,
        ),
    }
    output = json.dumps(result, separators=(",", ":")).replace('\\"', "\\u0022")
    print(output)


try:
    main()
except Exception as error:
    print(f"Desktop asset manifest error: {error}", file=sys.stderr)
    sys.exit(1)
