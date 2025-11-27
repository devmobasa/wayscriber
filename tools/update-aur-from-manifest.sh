#!/usr/bin/env bash
# Update AUR PKGBUILDs using the checksums produced by CI (dist/manifest.json).
# Intended to be run in CI after artifacts are built, but can also be used locally.
#
# Requires:
#   - jq
#   - git
#   - access to AUR clones (dirs provided via flags/env)
#
# Example (CI):
#   VERSION=$(jq -r '.version' dist/manifest.json)
#   ./tools/update-aur-from-manifest.sh \
#     --version "$VERSION" \
#     --manifest dist/manifest.json \
#     --source-dir aur-wayscriber \
#     --bin-dir aur-wayscriber-bin \
#     --config-dir aur-wayscriber-configurator \
#     --push

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

VERSION=""
MANIFEST="${REPO_ROOT}/dist/manifest.json"
AUR_SOURCE_DIR="${AUR_SOURCE_DIR:-${REPO_ROOT}/../aur-wayscriber}"
AUR_BIN_DIR="${AUR_BIN_DIR:-${REPO_ROOT}/../aur-wayscriber-bin}"
AUR_CONFIG_DIR="${AUR_CONFIG_DIR:-${REPO_ROOT}/../aur-wayscriber-configurator}"
DO_PUSH=0

usage() {
    cat <<'EOF'
update-aur-from-manifest.sh
Update wayscriber AUR PKGBUILDs using checksums from a manifest.json.

Flags:
  --version <ver>          Version to write (default: manifest.version)
  --manifest <path>        Path to manifest.json (default: dist/manifest.json)
  --source-dir <path>      wayscriber AUR repo path
  --bin-dir <path>         wayscriber-bin AUR repo path
  --config-dir <path>      wayscriber-configurator AUR repo path
  --push                   Git add/commit/push changes (default: dry-run)
  -h, --help               Show this help
EOF
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --version) VERSION="$2"; shift 2 ;;
        --manifest) MANIFEST="$2"; shift 2 ;;
        --source-dir) AUR_SOURCE_DIR="$2"; shift 2 ;;
        --bin-dir) AUR_BIN_DIR="$2"; shift 2 ;;
        --config-dir) AUR_CONFIG_DIR="$2"; shift 2 ;;
        --push) DO_PUSH=1; shift ;;
        -h|--help) usage; exit 0 ;;
        *) echo "Unknown arg: $1" >&2; usage; exit 1 ;;
    esac
done

need() { command -v "$1" >/dev/null 2>&1 || { echo "Missing required command: $1" >&2; exit 1; }; }
need jq
need git

[[ -f "$MANIFEST" ]] || { echo "Manifest not found: $MANIFEST" >&2; exit 1; }

if [[ -z "$VERSION" ]]; then
    VERSION="$(jq -r '.version' "$MANIFEST")"
fi

sha_for() {
    local name="$1"
    jq -r --arg n "$name" '.artifacts[] | select(.name==$n) | .sha256' "$MANIFEST"
}

replace_line() {
    local file="$1" pattern="$2" replacement="$3"
    python - "$file" "$pattern" "$replacement" <<'PY'
import re, sys, pathlib
path = pathlib.Path(sys.argv[1])
pattern = sys.argv[2]
replacement = sys.argv[3]
data = path.read_text()
data = re.sub(pattern, replacement, data, flags=re.MULTILINE)
path.write_text(data)
PY
}

update_bin() {
    local dir="$1"
    [[ -d "$dir" ]] || { echo "Skip bin: $dir not found" >&2; return; }
    local sha
    sha="$(sha_for "wayscriber-v${VERSION}-linux-x86_64.tar.gz")"
    [[ -n "$sha" && "$sha" != "null" ]] || { echo "Bin checksum missing in manifest" >&2; exit 1; }

    pushd "$dir" >/dev/null
    replace_line PKGBUILD '^pkgver=.*' "pkgver=${VERSION}"
    replace_line PKGBUILD '^pkgrel=.*' "pkgrel=1"
    replace_line PKGBUILD '^source_x86_64=.*' "source_x86_64=(\"wayscriber-v${VERSION}-linux-x86_64.tar.gz::https://github.com/devmobasa/wayscriber/releases/download/v${VERSION}/wayscriber-v${VERSION}-linux-x86_64.tar.gz\")"
    replace_line PKGBUILD '^sha256sums_x86_64=.*' "sha256sums_x86_64=('${sha}')"

    replace_line .SRCINFO '^\s*pkgver = .*' "pkgver = ${VERSION}"
    replace_line .SRCINFO '^\s*pkgrel = .*' "pkgrel = 1"
    replace_line .SRCINFO '^\s*source_x86_64 = .*' "source_x86_64 = wayscriber-v${VERSION}-linux-x86_64.tar.gz::https://github.com/devmobasa/wayscriber/releases/download/v${VERSION}/wayscriber-v${VERSION}-linux-x86_64.tar.gz"
    replace_line .SRCINFO '^\s*sha256sums_x86_64 = .*' "sha256sums_x86_64 = ${sha}"

    git status --short
    if [[ "$DO_PUSH" -eq 1 && -n "$(git status --porcelain)" ]]; then
        git add PKGBUILD .SRCINFO
        git commit -m "wayscriber-bin ${VERSION}"
        git push
    fi
    popd >/dev/null
}

update_source() {
    local dir="$1"
    [[ -d "$dir" ]] || { echo "Skip source: $dir not found" >&2; return; }
    pushd "$dir" >/dev/null
    replace_line PKGBUILD '^pkgver=.*' "pkgver=${VERSION}"
    replace_line PKGBUILD '^pkgrel=.*' "pkgrel=1"

    replace_line .SRCINFO '^\s*pkgver = .*' "pkgver = ${VERSION}"
    replace_line .SRCINFO '^\s*pkgrel = .*' "pkgrel = 1"

    git status --short
    if [[ "$DO_PUSH" -eq 1 && -n "$(git status --porcelain)" ]]; then
        git add PKGBUILD .SRCINFO
        git commit -m "wayscriber ${VERSION}"
        git push
    fi
    popd >/dev/null
}

update_configurator() {
    local dir="$1"
    [[ -d "$dir" ]] || { echo "Skip configurator: $dir not found" >&2; return; }
    pushd "$dir" >/dev/null
    replace_line PKGBUILD '^pkgver=.*' "pkgver=${VERSION}"
    replace_line PKGBUILD '^pkgrel=.*' "pkgrel=1"
    replace_line PKGBUILD '^source=.*' "source=(\"git+https://github.com/devmobasa/wayscriber.git#tag=v${VERSION}\")"
    replace_line PKGBUILD '^sha256sums=.*' "sha256sums=('SKIP')"

    replace_line .SRCINFO '^\s*pkgver = .*' "pkgver = ${VERSION}"
    replace_line .SRCINFO '^\s*pkgrel = .*' "pkgrel = 1"
    replace_line .SRCINFO '^\s*source = .*' "source = git+https://github.com/devmobasa/wayscriber.git#tag=v${VERSION}"
    replace_line .SRCINFO '^\s*sha256sums = .*' "sha256sums = SKIP"

    git status --short
    if [[ "$DO_PUSH" -eq 1 && -n "$(git status --porcelain)" ]]; then
        git add PKGBUILD .SRCINFO
        git commit -m "wayscriber-configurator ${VERSION}"
        git push
    fi
    popd >/dev/null
}

echo "Updating AUR using manifest: ${MANIFEST}"
echo "Version: ${VERSION}"
update_source "${AUR_SOURCE_DIR}"
update_bin "${AUR_BIN_DIR}"
update_configurator "${AUR_CONFIG_DIR}"
