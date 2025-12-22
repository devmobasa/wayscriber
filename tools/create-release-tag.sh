#!/usr/bin/env bash
set -euo pipefail

usage() {
    cat <<'EOF'
Usage: tools/create-release-tag.sh <version>

Creates an annotated git tag "v<version>".

Requirements:
- Clean working tree (no staged/unstaged/untracked changes)
- Tag must not already exist

Examples:
  tools/create-release-tag.sh 0.9.2
EOF
}

require_bin() {
    if ! command -v "$1" >/dev/null 2>&1; then
        echo "error: $1 is required" >&2
        exit 1
    fi
}

if [[ "${1-}" == "-h" || "${1-}" == "--help" || $# -ne 1 ]]; then
    usage
    exit 0
fi

require_bin git

version="$1"
tag="v${version}"

if ! [[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
    echo "error: invalid version format: $version (expected MAJOR.MINOR.PATCH)" >&2
    exit 1
fi

if [[ -n "$(git status --porcelain --untracked-files=all)" ]]; then
    echo "error: working tree is not clean; commit/stash changes before tagging" >&2
    exit 1
fi

if git rev-parse -q --verify "refs/tags/${tag}" >/dev/null; then
    echo "error: tag ${tag} already exists" >&2
    exit 1
fi

git tag -a "${tag}" -m "Release ${tag}"
echo "Created tag ${tag}"
