#!/usr/bin/env bash
set -euo pipefail

usage() {
    cat <<'EOF'
Usage: tools/check-version-consistency.sh [--release-version X.Y.Z[.N]]

Checks that release/version metadata agrees across:
  * Cargo.toml
  * configurator/Cargo.toml
  * Cargo.lock
  * packaging/PKGBUILD
  * packaging/.SRCINFO
  * flake.nix

Repo packaging metadata is a release template and must use sha256sums=('SKIP').
Release/AUR automation writes the real source archive checksum after the tag
exists.

When --release-version is provided, it must either match Cargo.toml exactly
or be a packaging hotfix version of the Cargo version (for example, Cargo
0.9.19 with release 0.9.19.1). Hotfix releases require packaging/PKGBUILD
and packaging/.SRCINFO to use the hotfix version.
EOF
}

release_version=""
while [[ $# -gt 0 ]]; do
    case "$1" in
        --release-version)
            if [[ $# -lt 2 ]]; then
                echo "error: --release-version requires a value" >&2
                usage
                exit 1
            fi
            release_version="$2"
            shift 2
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            echo "Unknown arg: $1" >&2
            usage
            exit 1
            ;;
    esac
done

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

PYTHON=""
for candidate in python3 python; do
    if command -v "$candidate" >/dev/null 2>&1 \
        && "$candidate" -c 'import sys; raise SystemExit(0 if sys.version_info[0] == 3 else 1)' >/dev/null 2>&1; then
        PYTHON="$candidate"
        break
    fi
done

if [[ -z "$PYTHON" ]]; then
    echo "error: python3 or python pointing to Python 3 is required" >&2
    exit 1
fi

"$PYTHON" - "$REPO_ROOT" "$release_version" <<'PY'
import pathlib
import re
import sys

try:
    import tomllib
except ImportError:
    tomllib = None

root = pathlib.Path(sys.argv[1])
release_version = sys.argv[2] or None
errors = []

version_re = re.compile(r"^\d+\.\d+\.\d+(?:\.\d+)?$")
checksum_re = re.compile(r"^[0-9a-fA-F]{64}$")

def read_text(path):
    return (root / path).read_text(encoding="utf-8")

def cargo_version(path):
    text = read_text(path)
    if tomllib is not None:
        return tomllib.loads(text)["package"]["version"]

    package_match = re.search(r"(?ms)^\[package\]\s*(.*?)(?:^\[|\Z)", text)
    if not package_match:
        errors.append(f"{path}: missing [package] section")
        return ""
    version_match = re.search(r'(?m)^version\s*=\s*"([^"]+)"', package_match.group(1))
    if not version_match:
        errors.append(f"{path}: missing package version")
        return ""
    return version_match.group(1)

def lock_package_version(path, package):
    pattern = re.compile(
        r'\[\[package\]\]\s+name\s*=\s*"' + re.escape(package) + r'"\s+version\s*=\s*"([^"]+)"',
        re.MULTILINE,
    )
    match = pattern.search(read_text(path))
    return match.group(1) if match else None

def pkgbuild_version(path):
    match = re.search(r"^pkgver=(.+)$", read_text(path), re.MULTILINE)
    return match.group(1).strip() if match else None

def srcinfo_version(path):
    match = re.search(r"^\s*pkgver = (.+)$", read_text(path), re.MULTILINE)
    return match.group(1).strip() if match else None

def pkgbuild_sha256sums(path):
    match = re.search(r"(?ms)^sha256sums=\((.*?)\)", read_text(path))
    if not match:
        return []

    values = []
    for value_match in re.finditer(r"'([^']*)'|\"([^\"]*)\"|(\S+)", match.group(1)):
        value = next(group for group in value_match.groups() if group is not None)
        values.append(value.strip())
    return values

def srcinfo_sha256sums(path):
    return [
        match.group(1).strip()
        for match in re.finditer(r"^\s*sha256sums = (.+)$", read_text(path), re.MULTILINE)
    ]

def is_hotfix_of(version, base):
    return re.fullmatch(re.escape(base) + r"\.\d+", version) is not None

def require_equal(label, actual, expected):
    if actual != expected:
        errors.append(f"{label}: expected {expected}, got {actual or 'missing'}")

def require_template_sha256sums(label, values):
    if values == ["SKIP"]:
        return

    if not values:
        errors.append(f"{label}: expected SKIP template checksum, got missing")
        return

    actual = ", ".join(values)
    if any(checksum_re.fullmatch(value) for value in values):
        errors.append(
            f"{label}: expected SKIP template checksum, got fixed SHA {actual}; "
            "release/AUR automation writes the real checksum after the tag exists"
        )
    else:
        errors.append(f"{label}: expected SKIP template checksum, got {actual}")

root_version = cargo_version("Cargo.toml")
config_version = cargo_version("configurator/Cargo.toml")
require_equal("configurator/Cargo.toml", config_version, root_version)

require_equal(
    "Cargo.lock wayscriber",
    lock_package_version("Cargo.lock", "wayscriber"),
    root_version,
)
require_equal(
    "Cargo.lock wayscriber-configurator",
    lock_package_version("Cargo.lock", "wayscriber-configurator"),
    root_version,
)

if not version_re.fullmatch(root_version):
    errors.append(f"Cargo.toml version has unsupported format: {root_version}")

packaging_version = pkgbuild_version("packaging/PKGBUILD")
srcinfo_pkgver = srcinfo_version("packaging/.SRCINFO")

if release_version:
    if not version_re.fullmatch(release_version):
        errors.append(f"release version has unsupported format: {release_version}")
    elif release_version != root_version and not is_hotfix_of(release_version, root_version):
        errors.append(
            f"release version {release_version} must equal Cargo version {root_version} "
            f"or be a hotfix of it, such as {root_version}.1"
        )
    expected_packaging_version = release_version
else:
    expected_packaging_version = root_version
    if packaging_version and is_hotfix_of(packaging_version, root_version):
        expected_packaging_version = packaging_version

require_equal("packaging/PKGBUILD pkgver", packaging_version, expected_packaging_version)
require_equal("packaging/.SRCINFO pkgver", srcinfo_pkgver, expected_packaging_version)
require_template_sha256sums(
    "packaging/PKGBUILD sha256sums", pkgbuild_sha256sums("packaging/PKGBUILD")
)
require_template_sha256sums(
    "packaging/.SRCINFO sha256sums", srcinfo_sha256sums("packaging/.SRCINFO")
)

flake_text = read_text("flake.nix")
if "builtins.fromTOML (builtins.readFile ./Cargo.toml)" not in flake_text:
    errors.append("flake.nix package version should be derived from Cargo.toml")

if errors:
    print("Version consistency check failed:", file=sys.stderr)
    for error in errors:
        print(f"- {error}", file=sys.stderr)
    sys.exit(1)

print(
    f"Version consistency OK: Cargo={root_version}, "
    f"packaging={expected_packaging_version}, checksum=SKIP"
)
PY
