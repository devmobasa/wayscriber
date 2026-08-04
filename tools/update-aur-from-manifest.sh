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
NO_CONFIGURATOR=0
SOURCE_ARCHIVE_SHA="${AUR_SOURCE_ARCHIVE_SHA256:-}"

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
  --no-configurator        Deliberately skip the configurator AUR channel
  --source-sha256 <sha>    Use this source archive checksum without downloading
  --push                   Git add/commit/push changes (default: dry-run)
  -h, --help               Show this help

The configurator channel is required unless --no-configurator is passed. The
checksum can also be supplied through AUR_SOURCE_ARCHIVE_SHA256.
EOF
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --version) VERSION="$2"; shift 2 ;;
        --manifest) MANIFEST="$2"; shift 2 ;;
        --source-dir) AUR_SOURCE_DIR="$2"; shift 2 ;;
        --bin-dir) AUR_BIN_DIR="$2"; shift 2 ;;
        --config-dir) AUR_CONFIG_DIR="$2"; shift 2 ;;
        --no-configurator) NO_CONFIGURATOR=1; shift ;;
        --source-sha256) SOURCE_ARCHIVE_SHA="$2"; shift 2 ;;
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

absolute_dir() {
    local dir="$1"
    (cd "$dir" >/dev/null 2>&1 && pwd -P) || {
        echo "Not a directory: $dir" >&2
        return 1
    }
}

next_pkgrel() {
    local dir_abs pkgfile current_pkgver current_pkgrel
    dir_abs="$(absolute_dir "$1")"
    pkgfile="${dir_abs}/PKGBUILD"

    [[ -f "$pkgfile" ]] || {
        echo "Missing AUR recipe: $pkgfile" >&2
        return 1
    }

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

source_archive_url() {
    printf 'https://github.com/devmobasa/wayscriber/archive/refs/tags/v%s.tar.gz' "$VERSION"
}

source_archive_sha() {
    if [[ -z "$SOURCE_ARCHIVE_SHA" ]]; then
        local tmp url
        url="$(source_archive_url)"
        tmp="$(mktemp)"
        if ! curl -fsSL "$url" -o "$tmp"; then
            rm -f "$tmp"
            echo "Failed to download the source archive: ${url}" >&2
            echo "Pass --source-sha256 or set AUR_SOURCE_ARCHIVE_SHA256 to supply it directly." >&2
            return 1
        fi
        if [[ ! -s "$tmp" ]]; then
            rm -f "$tmp"
            echo "Downloaded an empty source archive from ${url}" >&2
            return 1
        fi
        SOURCE_ARCHIVE_SHA="$(sha256sum "$tmp" | awk '{print $1}')"
        rm -f "$tmp"
    fi

    if [[ ! "$SOURCE_ARCHIVE_SHA" =~ ^[0-9a-fA-F]{64}$ ]]; then
        echo "Source archive checksum is not a sha256 digest: '${SOURCE_ARCHIVE_SHA}'" >&2
        return 1
    fi

    printf '%s' "$SOURCE_ARCHIVE_SHA"
}

require_recipe() {
    local channel="$1" dir="$2"
    [[ -d "$dir" ]] || {
        echo "${channel} AUR clone not found: $dir" >&2
        return 1
    }
    [[ -f "$dir/PKGBUILD" && -f "$dir/.SRCINFO" ]] || {
        echo "${channel} AUR clone is missing PKGBUILD or .SRCINFO: $dir" >&2
        return 1
    }
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

# Insert before wl-clipboard to match the AUR recipes' dependency ordering.
ensure_runtime_dependency() {
    local dep="$1"
    if ! grep -Eq "^[[:space:]]*'${dep}'[[:space:]]*\$" PKGBUILD; then
        sed -i "/^[[:space:]]*'wl-clipboard'/i\\    '${dep}'" PKGBUILD
    fi
    if ! grep -Eq "^[[:space:]]*depends = ${dep}\$" .SRCINFO; then
        sed -i "/^[[:space:]]*depends = wl-clipboard/i\\\tdepends = ${dep}" .SRCINFO
    fi
}

remove_runtime_dependency() {
    local dep="$1"
    remove_pkgbuild_array_item PKGBUILD "${dep}"
    remove_srcinfo_field_value .SRCINFO depends "${dep}"
}

ensure_bin_layer_shell_license() {
    local layer_license_line
    layer_license_line='    install -Dm644 "${srcdir_tmp}/usr/share/licenses/wayscriber/LICENSE.gtk4-layer-shell" "$pkgdir/usr/share/licenses/$pkgname/LICENSE.gtk4-layer-shell"'

    if grep -Fq 'usr/share/licenses/wayscriber/LICENSE.gtk4-layer-shell' PKGBUILD \
        && grep -Fq 'usr/share/licenses/$pkgname/LICENSE.gtk4-layer-shell' PKGBUILD; then
        return
    fi

    grep -Fq 'usr/share/doc/wayscriber/LICENSE' PKGBUILD || {
        echo "wayscriber-bin PKGBUILD has no main license install line to extend" >&2
        exit 1
    }

    LAYER_LICENSE_LINE="${layer_license_line}" perl -0pi -e '
        s{^(.*usr/share/doc/wayscriber/LICENSE.*)$}{$1 . "\n" . $ENV{LAYER_LICENSE_LINE}}me
    ' PKGBUILD

    grep -Fq 'usr/share/licenses/wayscriber/LICENSE.gtk4-layer-shell' PKGBUILD \
        && grep -Fq 'usr/share/licenses/$pkgname/LICENSE.gtk4-layer-shell' PKGBUILD || {
        echo "Failed to add gtk4-layer-shell license installation to wayscriber-bin" >&2
        exit 1
    }
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
    ensure_runtime_dependency gtk4
    # The release tarball embeds gtk4-layer-shell. Keep this prebuilt recipe
    # aligned with the binary instead of inheriting source-build dependencies.
    remove_runtime_dependency gtk4-layer-shell
    ensure_bin_layer_shell_license
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
    pkgrel="$(next_pkgrel "$dir")"
    pushd "$dir" >/dev/null
    local source_sha source_url
    source_sha="$(source_archive_sha)"
    source_url="$(source_archive_url)"

    remove_install_hook "wayscriber.install"
    remove_pkgbuild_array_item PKGBUILD git
    remove_srcinfo_field_value .SRCINFO makedepends git
    ensure_libxkbcommon_dependency
    ensure_runtime_dependency gtk4
    ensure_runtime_dependency gtk4-layer-shell
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
    require_recipe "wayscriber-configurator" "$dir"
    pkgrel="$(next_pkgrel "$dir")"
    pushd "$dir" >/dev/null
    local source_sha source_url
    source_sha="$(source_archive_sha)"
    source_url="$(source_archive_url)"

    remove_pkgbuild_array_item PKGBUILD git
    remove_srcinfo_field_value .SRCINFO makedepends git
    ensure_libxkbcommon_dependency
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

# The configurator is a release channel, not a best-effort extra. Check all
# known late failures before any earlier channel can commit and push.
if [[ "$NO_CONFIGURATOR" -eq 0 ]]; then
    require_recipe "wayscriber-configurator" "$AUR_CONFIG_DIR"
fi
if [[ -d "$AUR_SOURCE_DIR" ]]; then
    require_recipe "wayscriber" "$AUR_SOURCE_DIR"
fi
if [[ -d "$AUR_BIN_DIR" ]]; then
    require_recipe "wayscriber-bin" "$AUR_BIN_DIR"
    bin_sha="$(sha_for "wayscriber-v${VERSION}-linux-x86_64.tar.gz")"
    [[ -n "$bin_sha" && "$bin_sha" != "null" ]] || {
        echo "Bin checksum missing in manifest" >&2
        exit 1
    }
fi
if [[ -d "$AUR_SOURCE_DIR" || ( "$NO_CONFIGURATOR" -eq 0 && -d "$AUR_CONFIG_DIR" ) ]]; then
    SOURCE_ARCHIVE_SHA="$(source_archive_sha)"
fi

update_source "${AUR_SOURCE_DIR}"
update_bin "${AUR_BIN_DIR}"
if [[ "$NO_CONFIGURATOR" -eq 1 ]]; then
    echo "Skipping wayscriber-configurator: --no-configurator was passed." >&2
else
    update_configurator "${AUR_CONFIG_DIR}"
fi
