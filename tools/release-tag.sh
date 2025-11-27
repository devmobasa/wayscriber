#!/usr/bin/env bash
# Create and push a release tag (vX.Y.Z) using the wayscriber crate version by default.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

VERSION=""
DRY_RUN=0

usage() {
    cat <<'EOF'
release-tag.sh [--version X.Y.Z] [--dry-run]

Creates an annotated git tag "v<version>" and pushes it to origin.
If --version is omitted, uses the wayscriber crate version from Cargo metadata.
EOF
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --version) VERSION="$2"; shift 2 ;;
        --dry-run) DRY_RUN=1; shift ;;
        -h|--help) usage; exit 0 ;;
        *) echo "Unknown arg: $1" >&2; usage; exit 1 ;;
    esac
done

cd "$REPO_ROOT"

if [[ -z "$VERSION" ]]; then
    VERSION="$(cargo metadata --no-deps --format-version 1 | jq -r '.packages[] | select(.name=="wayscriber") | .version')"
fi

TAG="v${VERSION}"

# Safety checks
if [[ -n "$(git status --porcelain)" ]]; then
    echo "Working tree is dirty; aborting." >&2
    exit 1
fi

if git rev-parse "$TAG" >/dev/null 2>&1; then
    echo "Tag $TAG already exists locally; aborting." >&2
    exit 1
fi

if git ls-remote --tags origin "$TAG" | grep -q "$TAG"; then
    echo "Tag $TAG already exists on origin; aborting." >&2
    exit 1
fi

echo "Creating annotated tag ${TAG}"
if [[ "$DRY_RUN" -eq 0 ]]; then
    git tag -a "$TAG" -m "Release $TAG"
    echo "Pushing ${TAG} to origin"
    git push origin "$TAG"
else
    echo "[dry-run] git tag -a \"$TAG\" -m \"Release $TAG\""
    echo "[dry-run] git push origin \"$TAG\""
fi
