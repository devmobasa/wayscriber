#!/usr/bin/env bash
set -euo pipefail

usage() {
    cat <<'EOF'
Usage: tools/bump-version.sh [--dry-run] [new_version]

- If new_version is omitted, bumps the patch version (e.g., 0.9.2 -> 0.9.3).
- new_version can be MAJOR.MINOR.PATCH or MAJOR.MINOR.PATCH.HOTFIX.
- Updates:
  * Cargo.toml (wayscriber)
  * configurator/Cargo.toml
  * Cargo.lock (via cargo generate-lockfile)
  * packaging/PKGBUILD pkgver
  * packaging/.SRCINFO (via makepkg --printsrcinfo)

Requires: cargo, jq, makepkg, sed, perl.
EOF
}

require_bin() {
    if ! command -v "$1" >/dev/null 2>&1; then
        echo "error: $1 is required" >&2
        exit 1
    fi
}

if [[ "${1-}" == "-h" || "${1-}" == "--help" ]]; then
    usage
    exit 0
fi

require_bin cargo
require_bin jq
require_bin makepkg
require_bin sed
require_bin perl

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

current_version="$(cargo metadata --no-deps --format-version 1 | jq -r '.packages[] | select(.name=="wayscriber") | .version')"
if [[ -z "$current_version" ]]; then
    echo "error: unable to detect current wayscriber version" >&2
    exit 1
fi

DRY_RUN=false
while [[ $# -gt 0 ]]; do
    case "$1" in
        --dry-run)
            DRY_RUN=true
            shift
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        --)
            shift
            break
            ;;
        -*)
            usage
            exit 1
            ;;
        *)
            break
            ;;
    esac
done

if [[ $# -gt 1 ]]; then
    usage
    exit 1
fi

if [[ $# -eq 1 ]]; then
    next_version="$1"
else
    IFS='.' read -r major minor patch <<<"$current_version"
    patch=$((patch + 1))
    next_version="${major}.${minor}.${patch}"
fi

if ! [[ "$next_version" =~ ^[0-9]+\.[0-9]+\.[0-9]+(\.[0-9]+)?$ ]]; then
    echo "error: invalid version format: $next_version (expected MAJOR.MINOR.PATCH[.HOTFIX])" >&2
    exit 1
fi

IFS='.' read -r major minor patch hotfix extra <<<"$next_version"
if [[ -n "${extra:-}" ]]; then
    echo "error: invalid version format: $next_version (too many segments)" >&2
    exit 1
fi

cargo_version="${major}.${minor}.${patch}"
package_version="${next_version}"
if [[ -z "${hotfix:-}" ]]; then
    cargo_version="${next_version}"
fi

echo "Current version: $current_version"
echo "Bumping to:      $next_version"
if [[ "$cargo_version" != "$next_version" ]]; then
    echo "Cargo version:   $cargo_version (release version has hotfix)"
fi

update_version_field() {
    local file="$1"
    if ! [[ -f "$file" ]]; then
        echo "warn: $file not found, skipping" >&2
        return
    fi
    if $DRY_RUN; then
        echo "dry-run: would update version in $file"
    else
        NEXT_VERSION="$cargo_version" perl -0777 -pi -e 's/(\[package\][^\[]*?\nversion\s*=\s*")\K[^"]+/$ENV{NEXT_VERSION}/s' "$file"
    fi
}

update_version_field "Cargo.toml"
update_version_field "configurator/Cargo.toml"

if [[ -f Cargo.lock ]]; then
    if $DRY_RUN; then
        echo "dry-run: would regenerate Cargo.lock"
    else
        cargo generate-lockfile >/dev/null
    fi
else
    echo "warn: Cargo.lock not found, skipping lockfile update" >&2
fi

if [[ -f packaging/PKGBUILD ]]; then
    if $DRY_RUN; then
        echo "dry-run: would set pkgver=${package_version} in packaging/PKGBUILD and regenerate .SRCINFO"
    else
        sed -i "s/^pkgver=.*/pkgver=${package_version}/" packaging/PKGBUILD
        (cd packaging && makepkg --printsrcinfo > .SRCINFO)
    fi
else
    echo "warn: packaging/PKGBUILD not found, skipping PKGBUILD/.SRCINFO" >&2
fi

if $DRY_RUN; then
    echo "Dry run complete (no changes made)"
else
    echo "Updated versions to $next_version"
fi
