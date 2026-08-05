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
VERSION_PROVIDED=0
MANIFEST="${REPO_ROOT}/dist/manifest.json"
AUR_SOURCE_DIR="${AUR_SOURCE_DIR:-${REPO_ROOT}/../aur-wayscriber}"
AUR_BIN_DIR="${AUR_BIN_DIR:-${REPO_ROOT}/../aur-wayscriber-bin}"
AUR_CONFIG_DIR="${AUR_CONFIG_DIR:-${REPO_ROOT}/../aur-wayscriber-configurator}"
DO_PUSH=0
NO_CONFIGURATOR=0
SOURCE_ARCHIVE_SHA="${AUR_SOURCE_ARCHIVE_SHA256:-}"
BIN_ARCHIVE_SHA=""
PREPARED_PUSH_DIRS=()
RECIPE_PREFLIGHT_ROOT=""

cleanup_recipe_preflight() {
    if [[ -n "$RECIPE_PREFLIGHT_ROOT" && -d "$RECIPE_PREFLIGHT_ROOT" ]]; then
        rm -rf -- "$RECIPE_PREFLIGHT_ROOT"
    fi
}

trap cleanup_recipe_preflight EXIT

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
        --version)
            [[ $# -ge 2 ]] || { echo "--version requires a value" >&2; exit 1; }
            VERSION="$2"
            VERSION_PROVIDED=1
            shift 2
            ;;
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

if [[ "$VERSION_PROVIDED" -eq 0 ]]; then
    if ! VERSION="$(jq -er '
        if type != "object" then
            empty
        elif (.version | type) != "string" then
            empty
        else
            .version
        end
    ' "$MANIFEST")"; then
        echo "Manifest version must be a string matching MAJOR.MINOR.PATCH[.HOTFIX]: $MANIFEST" >&2
        exit 1
    fi
fi

if [[ ! "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+(\.[0-9]+)?$ ]]; then
    if [[ "$VERSION_PROVIDED" -eq 1 ]]; then
        echo "Invalid --version '${VERSION}' (expected MAJOR.MINOR.PATCH[.HOTFIX])" >&2
    else
        echo "Invalid manifest version '${VERSION}' (expected MAJOR.MINOR.PATCH[.HOTFIX])" >&2
    fi
    exit 1
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

artifact_sha_for() {
    local name="$1"
    local match_count
    if ! match_count="$(jq -r --arg n "$name" '[.artifacts[]? | select(.name==$n)] | length' "$MANIFEST")"; then
        echo "Could not read artifact checksums from manifest: $MANIFEST" >&2
        return 1
    fi

    if [[ "$match_count" != 1 ]]; then
        echo "Expected exactly one checksum for artifact ${name}, found ${match_count}" >&2
        return 1
    fi

    local digest
    if ! digest="$(jq -r --arg n "$name" '[.artifacts[]? | select(.name==$n)][0].sha256 // empty' "$MANIFEST")"; then
        echo "Could not read artifact checksums from manifest: $MANIFEST" >&2
        return 1
    fi
    if [[ ! "$digest" =~ ^[0-9a-fA-F]{64}$ ]]; then
        echo "Artifact checksum for ${name} is not a 64-character hexadecimal digest: '${digest}'" >&2
        return 1
    fi

    printf '%s' "$digest"
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

    local dir_abs git_root
    dir_abs="$(absolute_dir "$dir")"
    if ! git_root="$(git -C "$dir_abs" rev-parse --show-toplevel 2>/dev/null)"; then
        echo "${channel} AUR path is not a Git worktree: $dir" >&2
        return 1
    fi
    git_root="$(absolute_dir "$git_root")"
    if [[ "$git_root" != "$dir_abs" ]]; then
        echo "${channel} AUR path is not the root of its Git worktree: $dir" >&2
        return 1
    fi
    git -C "$dir_abs" status --porcelain >/dev/null || {
        echo "${channel} AUR Git worktree is not usable: $dir" >&2
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

ensure_configurator_desktop_assets() {
    local marker='# Wayscriber configurator desktop integration'
    local binary_install='    install -Dm755 "target/release/wayscriber-configurator" "$pkgdir/usr/bin/wayscriber-configurator"'
    local asset_block

    if grep -Fq "$marker" PKGBUILD; then
        return
    fi
    if grep -Fq 'packaging/wayscriber-configurator.desktop' PKGBUILD; then
        echo "wayscriber-configurator PKGBUILD has unmanaged desktop integration" >&2
        return 1
    fi
    grep -Fq "$binary_install" PKGBUILD || {
        echo "wayscriber-configurator PKGBUILD has no binary install line to extend" >&2
        return 1
    }

    asset_block="${marker}
    install -Dm644 packaging/wayscriber-configurator.desktop \"\$pkgdir/usr/share/applications/wayscriber-configurator.desktop\"
    install -Dm644 packaging/icons/wayscriber-configurator-16.png \"\$pkgdir/usr/share/icons/hicolor/16x16/apps/wayscriber-configurator.png\"
    install -Dm644 packaging/icons/wayscriber-configurator-19.png \"\$pkgdir/usr/share/icons/hicolor/19x19/apps/wayscriber-configurator.png\"
    install -Dm644 packaging/icons/wayscriber-configurator-22.png \"\$pkgdir/usr/share/icons/hicolor/22x22/apps/wayscriber-configurator.png\"
    install -Dm644 packaging/icons/wayscriber-configurator-24.png \"\$pkgdir/usr/share/icons/hicolor/24x24/apps/wayscriber-configurator.png\"
    install -Dm644 packaging/icons/wayscriber-configurator-38.png \"\$pkgdir/usr/share/icons/hicolor/38x38/apps/wayscriber-configurator.png\"
    install -Dm644 packaging/icons/wayscriber-configurator-64.png \"\$pkgdir/usr/share/icons/hicolor/64x64/apps/wayscriber-configurator.png\"
    install -Dm644 packaging/icons/wayscriber-configurator-128.png \"\$pkgdir/usr/share/icons/hicolor/128x128/apps/wayscriber-configurator.png\"
    install -Dm644 packaging/icons/wayscriber-configurator.svg \"\$pkgdir/usr/share/icons/hicolor/scalable/apps/wayscriber-configurator.svg\"
    install -Dm644 packaging/icons/wayscriber-configurator-128.png \"\$pkgdir/usr/share/pixmaps/wayscriber-configurator.png\""
    CONFIGURATOR_ASSET_BLOCK="$asset_block" perl -0pi -e '
        my $anchor = q{    install -Dm755 "target/release/wayscriber-configurator" "$pkgdir/usr/bin/wayscriber-configurator"};
        s{^\Q$anchor\E$}{$anchor . "\n\n" . $ENV{CONFIGURATOR_ASSET_BLOCK}}me
            or die "Failed to add wayscriber-configurator desktop integration\n";
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

ensure_runtime_dependency() {
    local dep="$1" anchor="${2:-wl-clipboard}"
    if ! grep -Eq "^[[:space:]]*'${dep}'[[:space:]]*\$" PKGBUILD; then
        grep -Eq "^[[:space:]]*'${anchor}'[[:space:]]*\$" PKGBUILD || {
            echo "PKGBUILD is missing dependency anchor '${anchor}' needed to add '${dep}'" >&2
            return 1
        }
        sed -i "/^[[:space:]]*'${anchor}'/i\\    '${dep}'" PKGBUILD
    fi
    if ! grep -Eq "^[[:space:]]*depends = ${dep}\$" .SRCINFO; then
        grep -Eq "^[[:space:]]*depends = ${anchor}\$" .SRCINFO || {
            echo ".SRCINFO is missing dependency anchor '${anchor}' needed to add '${dep}'" >&2
            return 1
        }
        sed -i "/^[[:space:]]*depends = ${anchor}/i\\\tdepends = ${dep}" .SRCINFO
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

validate_recipe_pair() {
    local channel="$1" dir="$2" checksum_field="$3" expected_checksum="$4"
    local pkgbuild_pkgrel srcinfo_pkgver srcinfo_pkgrel srcinfo_checksum

    pushd "$dir" >/dev/null
    [[ "$(read_pkgver_from_pkgbuild PKGBUILD)" == "$VERSION" ]] || {
        echo "${channel} PKGBUILD did not receive pkgver=${VERSION}" >&2
        return 1
    }
    pkgbuild_pkgrel="$(read_pkgrel_from_pkgbuild PKGBUILD)"
    [[ "$pkgbuild_pkgrel" =~ ^[1-9][0-9]*$ ]] || {
        echo "${channel} PKGBUILD has invalid pkgrel=${pkgbuild_pkgrel}" >&2
        return 1
    }
    srcinfo_pkgver="$(awk -F ' = ' '/^[[:space:]]*pkgver = / {print $2}' .SRCINFO)"
    srcinfo_pkgrel="$(awk -F ' = ' '/^[[:space:]]*pkgrel = / {print $2}' .SRCINFO)"
    srcinfo_checksum="$(awk -F ' = ' -v field="$checksum_field" '$1 ~ "^[[:space:]]*" field "$" {print $2}' .SRCINFO)"
    [[ "$srcinfo_pkgver" == "$VERSION" && "$srcinfo_pkgrel" == "$pkgbuild_pkgrel" ]] || {
        echo "${channel} PKGBUILD and .SRCINFO version metadata disagree" >&2
        return 1
    }
    grep -Fq "${checksum_field}=('${expected_checksum}')" PKGBUILD || {
        echo "${channel} PKGBUILD did not receive the expected checksum" >&2
        return 1
    }
    [[ "$srcinfo_checksum" == "$expected_checksum" ]] || {
        echo "${channel} .SRCINFO did not receive the expected checksum" >&2
        return 1
    }
    popd >/dev/null
}

validate_configurator_recipe() {
    local dir="$1"
    validate_recipe_pair \
        "wayscriber-configurator" "$dir" sha256sums "$SOURCE_ARCHIVE_SHA"

    pushd "$dir" >/dev/null
    grep -Fxq "pkgdesc='GUI configurator for wayscriber (GTK4/libadwaita)'" PKGBUILD || {
        echo "wayscriber-configurator PKGBUILD has stale GUI metadata" >&2
        return 1
    }
    grep -Eq "^[[:space:]]*'gtk4'[[:space:]]*$" PKGBUILD \
        && grep -Eq "^[[:space:]]*'libadwaita>=1.4'[[:space:]]*$" PKGBUILD \
        && grep -Fxq $'\tpkgdesc = GUI configurator for wayscriber (GTK4/libadwaita)' .SRCINFO \
        && grep -Fxq $'\tdepends = gtk4' .SRCINFO \
        && grep -Fxq $'\tdepends = libadwaita>=1.4' .SRCINFO || {
        echo "wayscriber-configurator recipe lacks GTK4/libadwaita metadata" >&2
        return 1
    }
    grep -Fq 'packaging/wayscriber-configurator.desktop' PKGBUILD \
        && grep -Fq 'packaging/icons/wayscriber-configurator.svg' PKGBUILD \
        && grep -Fq '$pkgdir/usr/share/applications/wayscriber-configurator.desktop' PKGBUILD \
        && grep -Fq '$pkgdir/usr/share/icons/hicolor/scalable/apps/wayscriber-configurator.svg' PKGBUILD \
        && grep -Fq '$pkgdir/usr/share/pixmaps/wayscriber-configurator.png' PKGBUILD || {
        echo "wayscriber-configurator recipe lacks desktop launcher assets" >&2
        return 1
    }
    local size
    for size in 16 19 22 24 38 64 128; do
        grep -Fq "packaging/icons/wayscriber-configurator-${size}.png" PKGBUILD \
            && grep -Fq "\$pkgdir/usr/share/icons/hicolor/${size}x${size}/apps/wayscriber-configurator.png" PKGBUILD || {
            echo "wayscriber-configurator recipe lacks the ${size}x${size} launcher icon" >&2
            return 1
        }
    done
    popd >/dev/null
}

prepare_channel_commit() {
    local dir="$1" message="$2"
    shift 2
    local dir_abs
    dir_abs="$(absolute_dir "$dir")"

    pushd "$dir_abs" >/dev/null
    local paths=(PKGBUILD .SRCINFO "$@")
    if [[ -z "$(git status --porcelain -- "${paths[@]}")" ]]; then
        popd >/dev/null
        return
    fi
    commit_metadata_changes "$message" "$@"
    PREPARED_PUSH_DIRS+=("$dir_abs")
    popd >/dev/null
}

push_prepared_channels() {
    local dir
    for dir in "${PREPARED_PUSH_DIRS[@]}"; do
        git -C "$dir" push
    done
}


update_bin() {
    local dir="$1"
    local pkgrel
    [[ -d "$dir" ]] || { echo "Skip bin: $dir not found" >&2; return; }
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
    replace_pkgbuild_array PKGBUILD sha256sums_x86_64 "sha256sums_x86_64=('${BIN_ARCHIVE_SHA}')"

    set_srcinfo_field .SRCINFO pkgver "${VERSION}"
    set_srcinfo_field .SRCINFO pkgrel "${pkgrel}"
    set_srcinfo_field .SRCINFO source_x86_64 "wayscriber-v${VERSION}-linux-x86_64.tar.gz::https://github.com/devmobasa/wayscriber/releases/download/v${VERSION}/wayscriber-v${VERSION}-linux-x86_64.tar.gz"
    set_srcinfo_field .SRCINFO sha256sums_x86_64 "${BIN_ARCHIVE_SHA}"

    git status --short
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
    ensure_runtime_dependency gtk4 gcc-libs
    ensure_runtime_dependency 'libadwaita>=1.4' gcc-libs
    replace_line PKGBUILD '^pkgver=.*' "pkgver=${VERSION}"
    replace_line PKGBUILD '^pkgrel=.*' "pkgrel=${pkgrel}"
    replace_line PKGBUILD '^pkgdesc=.*' "pkgdesc='GUI configurator for wayscriber (GTK4/libadwaita)'"
    replace_pkgbuild_array PKGBUILD source 'source=("wayscriber-$pkgver.tar.gz::https://github.com/devmobasa/wayscriber/archive/refs/tags/v$pkgver.tar.gz")'
    replace_pkgbuild_array PKGBUILD sha256sums "sha256sums=('${source_sha}')"
    replace_line PKGBUILD '^    cd wayscriber$' '    cd "wayscriber-$pkgver"'
    rewrite_packaging_asset_paths
    ensure_configurator_desktop_assets

    set_srcinfo_field .SRCINFO pkgver "${VERSION}"
    set_srcinfo_field .SRCINFO pkgrel "${pkgrel}"
    set_srcinfo_field .SRCINFO pkgdesc "GUI configurator for wayscriber (GTK4/libadwaita)"
    set_srcinfo_field .SRCINFO source "wayscriber-${VERSION}.tar.gz::${source_url}"
    set_srcinfo_field .SRCINFO sha256sums "${source_sha}"

    git status --short
    popd >/dev/null
}

copy_recipe_for_preflight() {
    local source_dir="$1" destination="$2"
    mkdir -p "$destination"
    cp -a -- "$source_dir/." "$destination/"
    if [[ -e "$destination/.git" || -L "$destination/.git" ]]; then
        rm -rf -- "$destination/.git"
    fi
    git -C "$destination" init -q
    git -C "$destination" add -A
}

transform_selected_recipes() {
    local source_dir="$1" bin_dir="$2" config_dir="$3"
    if [[ "$SOURCE_SELECTED" -eq 1 ]]; then
        update_source "$source_dir"
    fi
    if [[ "$BIN_SELECTED" -eq 1 ]]; then
        update_bin "$bin_dir"
    fi
    if [[ "$CONFIGURATOR_SELECTED" -eq 1 ]]; then
        update_configurator "$config_dir"
    fi
}

validate_selected_recipes() {
    local source_dir="$1" bin_dir="$2" config_dir="$3"
    if [[ "$SOURCE_SELECTED" -eq 1 ]]; then
        validate_recipe_pair "wayscriber" "$source_dir" sha256sums "$SOURCE_ARCHIVE_SHA"
    fi
    if [[ "$BIN_SELECTED" -eq 1 ]]; then
        validate_recipe_pair \
            "wayscriber-bin" "$bin_dir" sha256sums_x86_64 "$BIN_ARCHIVE_SHA"
    fi
    if [[ "$CONFIGURATOR_SELECTED" -eq 1 ]]; then
        validate_configurator_recipe "$config_dir"
    fi
}

preflight_recipe_transformations() {
    RECIPE_PREFLIGHT_ROOT="$(mktemp -d)"
    local source_preview="${RECIPE_PREFLIGHT_ROOT}/source"
    local bin_preview="${RECIPE_PREFLIGHT_ROOT}/bin"
    local config_preview="${RECIPE_PREFLIGHT_ROOT}/configurator"

    if [[ "$SOURCE_SELECTED" -eq 1 ]]; then
        copy_recipe_for_preflight "$AUR_SOURCE_DIR" "$source_preview"
    fi
    if [[ "$BIN_SELECTED" -eq 1 ]]; then
        copy_recipe_for_preflight "$AUR_BIN_DIR" "$bin_preview"
    fi
    if [[ "$CONFIGURATOR_SELECTED" -eq 1 ]]; then
        copy_recipe_for_preflight "$AUR_CONFIG_DIR" "$config_preview"
    fi

    transform_selected_recipes "$source_preview" "$bin_preview" "$config_preview"
    validate_selected_recipes "$source_preview" "$bin_preview" "$config_preview"
    cleanup_recipe_preflight
    RECIPE_PREFLIGHT_ROOT=""
}

echo "Updating AUR using manifest: ${MANIFEST}"
echo "Version: ${VERSION}"

# The configurator is a release channel, not a best-effort extra. Check all
# known late failures before any earlier channel can commit and push.
SOURCE_SELECTED=0
BIN_SELECTED=0
CONFIGURATOR_SELECTED=0
if [[ "$NO_CONFIGURATOR" -eq 0 ]]; then
    require_recipe "wayscriber-configurator" "$AUR_CONFIG_DIR"
    CONFIGURATOR_SELECTED=1
fi
if [[ -d "$AUR_SOURCE_DIR" ]]; then
    require_recipe "wayscriber" "$AUR_SOURCE_DIR"
    SOURCE_SELECTED=1
fi
if [[ -d "$AUR_BIN_DIR" ]]; then
    require_recipe "wayscriber-bin" "$AUR_BIN_DIR"
    BIN_SELECTED=1
    BIN_ARCHIVE_SHA="$(artifact_sha_for "wayscriber-v${VERSION}-linux-x86_64.tar.gz")"
fi
if [[ -d "$AUR_SOURCE_DIR" || ( "$NO_CONFIGURATOR" -eq 0 && -d "$AUR_CONFIG_DIR" ) ]]; then
    SOURCE_ARCHIVE_SHA="$(source_archive_sha)"
fi

if [[ "$NO_CONFIGURATOR" -eq 1 ]]; then
    echo "Skipping wayscriber-configurator: --no-configurator was passed." >&2
fi

# Prove every deterministic transformation and validation against isolated
# copies before touching a selected checkout. Then repeat the proven route on
# the real worktrees before the commit/push boundary. Separate AUR repositories
# still cannot be pushed atomically; a later remote/network failure can split
# publication after this point.
preflight_recipe_transformations
transform_selected_recipes "$AUR_SOURCE_DIR" "$AUR_BIN_DIR" "$AUR_CONFIG_DIR"
validate_selected_recipes "$AUR_SOURCE_DIR" "$AUR_BIN_DIR" "$AUR_CONFIG_DIR"

if [[ "$DO_PUSH" -eq 1 ]]; then
    if [[ "$SOURCE_SELECTED" -eq 1 ]]; then
        prepare_channel_commit "$AUR_SOURCE_DIR" "wayscriber ${VERSION}" wayscriber.install
    fi
    if [[ "$BIN_SELECTED" -eq 1 ]]; then
        prepare_channel_commit "$AUR_BIN_DIR" "wayscriber-bin ${VERSION}" wayscriber-bin.install
    fi
    if [[ "$CONFIGURATOR_SELECTED" -eq 1 ]]; then
        prepare_channel_commit "$AUR_CONFIG_DIR" "wayscriber-configurator ${VERSION}"
    fi
    push_prepared_channels
fi
