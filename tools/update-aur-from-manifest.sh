#!/usr/bin/env bash
# Update AUR PKGBUILDs using the checksums produced by CI (dist/manifest.json).
# Intended to be run in CI after artifacts are built, but can also be used locally.
#
# Requires:
#   - jq
#   - git
#   - curl
#   - sha256sum
#   - perl
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
need curl
need sha256sum
need perl

[[ -f "$MANIFEST" ]] || { echo "Manifest not found: $MANIFEST" >&2; exit 1; }

if [[ -z "$VERSION" ]]; then
    VERSION="$(jq -r '.version' "$MANIFEST")"
fi

read_pkgver_from_pkgbuild() {
    awk -F= '/^pkgver=/{print $2; exit}' "$1"
}

read_pkgrel_from_pkgbuild() {
    local value
    value="$(awk -F= '/^pkgrel=/{print $2; exit}' "$1")"
    if [[ -z "$value" || ! "$value" =~ ^[0-9]+$ ]]; then
        echo 0
    else
        echo "$value"
    fi
}

next_pkgrel() {
    local dir="$1"
    local pkgfile="$dir/PKGBUILD"
    local current_pkgver current_pkgrel

    current_pkgver="$(read_pkgver_from_pkgbuild "$pkgfile")"
    current_pkgrel="$(read_pkgrel_from_pkgbuild "$pkgfile")"

    if [[ "$current_pkgver" == "$VERSION" ]]; then
        echo $((current_pkgrel + 1))
    else
        echo 1
    fi
}

sha_for() {
    local name="$1"
    jq -r --arg n "$name" '.artifacts[] | select(.name==$n) | .sha256' "$MANIFEST"
}

SOURCE_ARCHIVE_SHA=""

source_archive_url() {
    printf 'https://github.com/devmobasa/wayscriber/archive/refs/tags/v%s.tar.gz' "$VERSION"
}

source_archive_sha() {
    if [[ -z "$SOURCE_ARCHIVE_SHA" ]]; then
        local tmp
        tmp="$(mktemp)"
        curl -fsSL "$(source_archive_url)" -o "$tmp"
        SOURCE_ARCHIVE_SHA="$(sha256sum "$tmp" | awk '{print $1}')"
        rm -f "$tmp"
    fi

    printf '%s' "$SOURCE_ARCHIVE_SHA"
}

replace_line() {
    local file="$1" pattern="$2" replacement="$3"
    perl -0pi -e 'BEGIN { our ($pattern, $replacement) = splice @ARGV, 0, 2 } s/$pattern/$replacement/mg' \
        "$pattern" "$replacement" "$file"
}

replace_pkgbuild_array() {
    local file="$1" array="$2" replacement="$3"
    perl -0pi -e 'BEGIN { our ($array, $replacement) = splice @ARGV, 0, 2 } s/^\Q$array\E=\([^)]*\)/$replacement/ms' \
        "$array" "$replacement" "$file"
}

set_srcinfo_field() {
    local file="$1" field="$2" value="$3"
    perl -0pi -e '
        BEGIN { our ($field, $value) = splice @ARGV, 0, 2 }
        my $line = "\t$field = $value\n";
        my $seen = 0;
        s{^[ \t]*\Q$field\E = .*\R?}{ $seen++ ? "" : $line }gme;
        if (!$seen) {
            s{\n(pkgname = )}{\n$line\n$1}m or $_ .= "\n$line";
        }
    ' "$field" "$value" "$file"
}

remove_pkgbuild_array_item() {
    local file="$1" item="$2"
    sed -i "/^[[:space:]]*'${item}'[[:space:]]*$/d" "$file"
}

remove_srcinfo_field_value() {
    local file="$1" field="$2" value="$3"
    sed -i "/^[[:space:]]*${field} = ${value}$/d" "$file"
}

remove_install_hook() {
    local install_file="$1"
    sed -i '/^install=/d' PKGBUILD
    sed -i '/^[[:space:]]*install = /d' .SRCINFO
    rm -f "$install_file"
}

rewrite_packaging_asset_paths() {
    perl -0pi -e '
        s{"\$srcdir/(wayscriber(?:-configurator)?\.desktop)"}{packaging/$1}g;
        s{"\$srcdir/(wayscriber(?:-configurator)?-[0-9]+\.png)"}{packaging/icons/$1}g;
    ' PKGBUILD
}

ensure_libxkbcommon_dependency() {
    if ! grep -Eq "^[[:space:]]*'libxkbcommon'[[:space:]]*$" PKGBUILD; then
        sed -i "/^[[:space:]]*'gcc-libs'/i\\    'libxkbcommon'" PKGBUILD
    fi

    if ! grep -Eq "^[[:space:]]*depends = libxkbcommon$" .SRCINFO; then
        sed -i "/^[[:space:]]*depends = gcc-libs/i\\\tdepends = libxkbcommon" .SRCINFO
    fi
}

# The GTK4 toolbar frontend (default feature) needs the gtk4 runtime;
# insert before wl-clipboard to match the repo PKGBUILD ordering.
ensure_gtk4_dependencies() {
    local dep
    for dep in gtk4 gtk4-layer-shell; do
        if ! grep -Eq "^[[:space:]]*'${dep}'[[:space:]]*\$" PKGBUILD; then
            sed -i "/^[[:space:]]*'wl-clipboard'/i\\    '${dep}'" PKGBUILD
        fi
        if ! grep -Eq "^[[:space:]]*depends = ${dep}\$" .SRCINFO; then
            sed -i "/^[[:space:]]*depends = wl-clipboard/i\\\tdepends = ${dep}" .SRCINFO
        fi
    done
}

commit_metadata_changes() {
    local message="$1"
    shift

    local commit_paths=(PKGBUILD .SRCINFO)
    git add PKGBUILD .SRCINFO

    local deleted_file
    for deleted_file in "$@"; do
        if git ls-files --error-unmatch "$deleted_file" >/dev/null 2>&1; then
            git rm -f --ignore-unmatch "$deleted_file"
            commit_paths+=("$deleted_file")
        fi
    done

    git commit -m "$message" -- "${commit_paths[@]}"
}


update_bin() {
    local dir="$1"
    local pkgrel
    [[ -d "$dir" ]] || { echo "Skip bin: $dir not found" >&2; return; }
    local sha
    sha="$(sha_for "wayscriber-v${VERSION}-linux-x86_64.tar.gz")"
    [[ -n "$sha" && "$sha" != "null" ]] || { echo "Bin checksum missing in manifest" >&2; exit 1; }
    pkgrel="$(next_pkgrel "$dir")"

    pushd "$dir" >/dev/null
    remove_install_hook "wayscriber-bin.install"
    ensure_libxkbcommon_dependency
    ensure_gtk4_dependencies
    replace_line PKGBUILD '^pkgver=.*' "pkgver=${VERSION}"
    replace_line PKGBUILD '^pkgrel=.*' "pkgrel=${pkgrel}"
    replace_pkgbuild_array PKGBUILD source_x86_64 "source_x86_64=(\"wayscriber-v${VERSION}-linux-x86_64.tar.gz::https://github.com/devmobasa/wayscriber/releases/download/v${VERSION}/wayscriber-v${VERSION}-linux-x86_64.tar.gz\")"
    replace_pkgbuild_array PKGBUILD sha256sums_x86_64 "sha256sums_x86_64=('${sha}')"

    set_srcinfo_field .SRCINFO pkgver "${VERSION}"
    set_srcinfo_field .SRCINFO pkgrel "${pkgrel}"
    set_srcinfo_field .SRCINFO source_x86_64 "wayscriber-v${VERSION}-linux-x86_64.tar.gz::https://github.com/devmobasa/wayscriber/releases/download/v${VERSION}/wayscriber-v${VERSION}-linux-x86_64.tar.gz"
    set_srcinfo_field .SRCINFO sha256sums_x86_64 "${sha}"

    git status --short
    if [[ "$DO_PUSH" -eq 1 && -n "$(git status --porcelain)" ]]; then
        commit_metadata_changes "wayscriber-bin ${VERSION}" "wayscriber-bin.install"
        git push
    fi
    popd >/dev/null
}

update_source() {
    local dir="$1"
    local pkgrel
    [[ -d "$dir" ]] || { echo "Skip source: $dir not found" >&2; return; }
    pushd "$dir" >/dev/null
    pkgrel="$(next_pkgrel "$dir")"
    local source_sha source_url
    source_sha="$(source_archive_sha)"
    source_url="$(source_archive_url)"

    remove_install_hook "wayscriber.install"
    remove_pkgbuild_array_item PKGBUILD git
    remove_srcinfo_field_value .SRCINFO makedepends git
    ensure_libxkbcommon_dependency
    ensure_gtk4_dependencies
    replace_line PKGBUILD '^pkgver=.*' "pkgver=${VERSION}"
    replace_line PKGBUILD '^pkgrel=.*' "pkgrel=${pkgrel}"
    replace_pkgbuild_array PKGBUILD source 'source=("wayscriber-$pkgver.tar.gz::https://github.com/devmobasa/wayscriber/archive/refs/tags/v$pkgver.tar.gz")'
    replace_pkgbuild_array PKGBUILD sha256sums "sha256sums=('${source_sha}')"
    replace_line PKGBUILD 'cd "\$pkgname"' 'cd "$pkgname-$pkgver"'
    rewrite_packaging_asset_paths

    set_srcinfo_field .SRCINFO pkgver "${VERSION}"
    set_srcinfo_field .SRCINFO pkgrel "${pkgrel}"
    set_srcinfo_field .SRCINFO source "wayscriber-${VERSION}.tar.gz::${source_url}"
    set_srcinfo_field .SRCINFO sha256sums "${source_sha}"

    git status --short
    if [[ "$DO_PUSH" -eq 1 && -n "$(git status --porcelain)" ]]; then
        commit_metadata_changes "wayscriber ${VERSION}" "wayscriber.install"
        git push
    fi
    popd >/dev/null
}

update_configurator() {
    local dir="$1"
    local pkgrel
    [[ -d "$dir" ]] || { echo "Skip configurator: $dir not found" >&2; return; }
    pushd "$dir" >/dev/null
    pkgrel="$(next_pkgrel "$dir")"
    local source_sha source_url
    source_sha="$(source_archive_sha)"
    source_url="$(source_archive_url)"

    remove_pkgbuild_array_item PKGBUILD git
    remove_srcinfo_field_value .SRCINFO makedepends git
    ensure_libxkbcommon_dependency
    ensure_gtk4_dependencies
    replace_line PKGBUILD '^pkgver=.*' "pkgver=${VERSION}"
    replace_line PKGBUILD '^pkgrel=.*' "pkgrel=${pkgrel}"
    replace_pkgbuild_array PKGBUILD source 'source=("wayscriber-$pkgver.tar.gz::https://github.com/devmobasa/wayscriber/archive/refs/tags/v$pkgver.tar.gz")'
    replace_pkgbuild_array PKGBUILD sha256sums "sha256sums=('${source_sha}')"
    replace_line PKGBUILD '^    cd wayscriber$' '    cd "wayscriber-$pkgver"'
    rewrite_packaging_asset_paths

    set_srcinfo_field .SRCINFO pkgver "${VERSION}"
    set_srcinfo_field .SRCINFO pkgrel "${pkgrel}"
    set_srcinfo_field .SRCINFO source "wayscriber-${VERSION}.tar.gz::${source_url}"
    set_srcinfo_field .SRCINFO sha256sums "${source_sha}"

    git status --short
    if [[ "$DO_PUSH" -eq 1 && -n "$(git status --porcelain)" ]]; then
        commit_metadata_changes "wayscriber-configurator ${VERSION}"
        git push
    fi
    popd >/dev/null
}

echo "Updating AUR using manifest: ${MANIFEST}"
echo "Version: ${VERSION}"
update_source "${AUR_SOURCE_DIR}"
update_bin "${AUR_BIN_DIR}"
update_configurator "${AUR_CONFIG_DIR}"
