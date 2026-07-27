#!/usr/bin/env bash
# Build and install gtk4-layer-shell from a pinned upstream release so the
# GTK4 toolbar frontend can link against it.
#
# Why this exists: gtk4-layer-shell landed in the Debian/Ubuntu archives
# only recently (Ubuntu 25.04 "plucky" and Debian 13 "trixie"). The Ubuntu
# 24.04 release runner and older LTS releases have no
# `libgtk4-layer-shell-dev` package. CI and release builds compile it from
# source here, keeping the pin in a single place.
#
# The source is pinned to a specific commit (not the moving v1.3.0 tag) and
# the downloaded archive is verified against a SHA-256, so a moved tag or a
# tampered download fails loudly rather than building unexpected code.
#
# It installs system-wide by default for from-source users. CI and release
# packaging set GTK4_LAYER_SHELL_PREFIX to a private build-only prefix so they
# never overwrite a distro package. A from-source builder who cannot install
# system-wide should instead build wayscriber without the GTK toolbars:
#   cargo build --release --no-default-features --features tablet-input,portal,tray
#
# Prerequisites (install via the workflow/distro, not here): a C toolchain,
# pkg-config, meson, ninja, libgtk-4-dev, libwayland-dev (wayland-scanner),
# and wayland-protocols. The build uses only those — introspection and vapi
# are disabled so gobject-introspection and vala tooling are not required.
#
# Usage: bash tools/install-gtk4-layer-shell.sh
# Env:
#   FORCE_BUILD                    build even if the requested artifacts exist
#   GTK4_LAYER_SHELL_PREFIX        install prefix [/usr]
#   GTK4_LAYER_SHELL_SYSTEM_PREFIX system-prefix boundary [/usr]
#   GTK4_LAYER_SHELL_LIBRARY_MODE  shared, static, or both [shared]
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

GLS_VERSION="1.3.0"
# Commit that tag v1.3.0 points at; pinning the commit (not the tag) keeps
# the build reproducible even if the tag is ever moved.
GLS_COMMIT="1c963c51514581c41b9bdae08cdf69171265cdda"
GLS_ARCHIVE_SHA256="22be5f5edf487cfb87266f0e71c400b11322082a4dc99832e5a54a4fca3d5a7c"
GLS_URL="https://github.com/wmww/gtk4-layer-shell/archive/${GLS_COMMIT}.tar.gz"

PREFIX="${GTK4_LAYER_SHELL_PREFIX:-/usr}"
SYSTEM_PREFIX="${GTK4_LAYER_SHELL_SYSTEM_PREFIX:-/usr}"
LIBRARY_MODE="${GTK4_LAYER_SHELL_LIBRARY_MODE:-shared}"
PINNED_LICENSE="${REPO_ROOT}/packaging/licenses/gtk4-layer-shell.LICENSE"
PIN_STAMP="${PREFIX}/share/wayscriber/build-deps/gtk4-layer-shell.pin"

case "${LIBRARY_MODE}" in
    shared|static|both) ;;
    *)
        echo "[install-gtk4-layer-shell] ERROR: GTK4_LAYER_SHELL_LIBRARY_MODE must be shared, static, or both" >&2
        exit 1
        ;;
esac
[[ "${PREFIX}" == /* ]] || {
    echo "[install-gtk4-layer-shell] ERROR: GTK4_LAYER_SHELL_PREFIX must be absolute" >&2
    exit 1
}
[[ "${SYSTEM_PREFIX}" == /* ]] || {
    echo "[install-gtk4-layer-shell] ERROR: GTK4_LAYER_SHELL_SYSTEM_PREFIX must be absolute" >&2
    exit 1
}

log() { printf '[install-gtk4-layer-shell] %s\n' "$*"; }

if [[ -n "${PKG_CONFIG_PATH:-}" ]]; then
    GLS_PKG_CONFIG_PATH="${PREFIX}/lib/pkgconfig:${PKG_CONFIG_PATH}"
else
    GLS_PKG_CONFIG_PATH="${PREFIX}/lib/pkgconfig"
fi

gls_pkg_config() {
    env PKG_CONFIG_PATH="${GLS_PKG_CONFIG_PATH}" pkg-config "$@"
}

expected_pin() {
    printf 'version=%s\ncommit=%s\narchive_sha256=%s\nlibrary_mode=%s\n' \
        "${GLS_VERSION}" "${GLS_COMMIT}" "${GLS_ARCHIVE_SHA256}" \
        "${LIBRARY_MODE}"
}

pin_matches() {
    [[ -f "${PIN_STAMP}" ]] || return 1
    expected_pin | cmp -s - "${PIN_STAMP}"
}

prefix_is_system_path() {
    case "${PREFIX}" in
        "${SYSTEM_PREFIX}"|"${SYSTEM_PREFIX}"/*) return 0 ;;
        *) return 1 ;;
    esac
}

requested_artifacts_exist() {
    # A missing stamp at the system prefix means the matching artifacts may
    # belong to the distro. Never replace package-manager-owned files just to
    # add our cache metadata. Private prefixes and stamped system installs
    # must match the exact source and requested library mode.
    if [[ -f "${PIN_STAMP}" ]]; then
        pin_matches || return 1
    elif [[ "${PREFIX}" != "${SYSTEM_PREFIX}" ]]; then
        return 1
    fi

    gls_pkg_config --atleast-version="${GLS_VERSION}" gtk4-layer-shell-0 2>/dev/null || return 1

    local resolved_libdir
    resolved_libdir="$(gls_pkg_config --variable=libdir gtk4-layer-shell-0 2>/dev/null)" || return 1
    case "${resolved_libdir}" in
        "${PREFIX}"|"${PREFIX}"/*) ;;
        *) return 1 ;;
    esac

    case "${LIBRARY_MODE}" in
        shared)
            [[ -e "${resolved_libdir}/libgtk4-layer-shell.so" \
                || -e "${resolved_libdir}/libgtk4-layer-shell.so.0" ]]
            ;;
        static)
            [[ -f "${resolved_libdir}/libgtk4-layer-shell.a" ]]
            ;;
        both)
            [[ -f "${resolved_libdir}/libgtk4-layer-shell.a" \
                && ( -e "${resolved_libdir}/libgtk4-layer-shell.so" \
                    || -e "${resolved_libdir}/libgtk4-layer-shell.so.0" ) ]]
            ;;
    esac
}

# Already satisfied by the requested prefix and link mode?
if [[ "${FORCE_BUILD:-0}" != "1" ]] && requested_artifacts_exist; then
    log "gtk4-layer-shell $(gls_pkg_config --modversion gtk4-layer-shell-0) (${LIBRARY_MODE}) already available in ${PREFIX}; skipping build"
    exit 0
fi

missing=()
for tool in cmp curl install sha256sum tar meson ninja pkg-config; do
    command -v "$tool" >/dev/null 2>&1 || missing+=("$tool")
done
# A C compiler under any of the usual names.
if ! command -v cc >/dev/null 2>&1 && ! command -v gcc >/dev/null 2>&1 \
    && ! command -v clang >/dev/null 2>&1; then
    missing+=("cc/gcc/clang")
fi
if ((${#missing[@]})); then
    log "ERROR: missing build prerequisites: ${missing[*]}"
    log "Install them first (e.g. apt-get install meson ninja-build wayland-protocols libgtk-4-dev libwayland-dev build-essential pkg-config)."
    exit 1
fi

# The configured system-prefix tree needs privilege escalation. Private CI and
# release prefixes remain owned by the caller.
if prefix_is_system_path; then
    if [[ "$(id -u)" -eq 0 ]]; then
        as_root() { "$@"; }
    elif command -v sudo >/dev/null 2>&1; then
        as_root() { sudo "$@"; }
    else
        log "ERROR: installing to ${PREFIX} needs root, but this is not root and sudo is unavailable."
        log "Run as root, choose a private GTK4_LAYER_SHELL_PREFIX, or build without GTK toolbars:"
        log "  cargo build --release --no-default-features --features tablet-input,portal,tray"
        exit 1
    fi
else
    mkdir -p "${PREFIX}"
    as_root() { "$@"; }
fi

workdir="$(mktemp -d)"
cleanup() { rm -rf "$workdir"; }
trap cleanup EXIT

archive="$workdir/gtk4-layer-shell.tar.gz"
log "Downloading gtk4-layer-shell ${GLS_VERSION} (commit ${GLS_COMMIT})"
curl -fsSL "$GLS_URL" -o "$archive"

log "Verifying SHA-256"
echo "${GLS_ARCHIVE_SHA256}  ${archive}" | sha256sum --check --status \
    || { log "ERROR: checksum mismatch for downloaded archive"; exit 1; }

tar -xzf "$archive" -C "$workdir"
src="$workdir/gtk4-layer-shell-${GLS_COMMIT}"

# The release artifacts contain this code. Keep the tracked notice tied to the
# exact pinned source instead of trusting a stale hand-copied license.
cmp "${src}/LICENSE" "${PINNED_LICENSE}" || {
    log "ERROR: pinned gtk4-layer-shell LICENSE differs from ${PINNED_LICENSE}"
    exit 1
}

log "Configuring ${LIBRARY_MODE} libraries (introspection/vapi/examples/docs/tests off)"
meson setup "$src/_build" "$src" \
    --prefix="$PREFIX" \
    --libdir=lib \
    --buildtype=release \
    --default-library="${LIBRARY_MODE}" \
    -Dexamples=false \
    -Ddocs=false \
    -Dtests=false \
    -Dintrospection=false \
    -Dvapi=false

log "Building"
meson compile -C "$src/_build"

log "Installing to ${PREFIX}"
as_root meson install -C "$src/_build"

# Tie Wayscriber-managed prefixes to the exact source archive and library
# mode, not just the upstream version. A changed pin or mode cannot reuse an
# older cache even if Meson left compatible-looking artifacts behind.
pin_file="${workdir}/gtk4-layer-shell.pin"
expected_pin > "${pin_file}"
as_root install -Dm644 "${pin_file}" "${PIN_STAMP}"

# Refresh the linker cache only for the configured system-prefix tree. Private
# prefixes are build inputs and never become runtime library locations.
if prefix_is_system_path; then
    as_root ldconfig 2>/dev/null || true
fi

# Verify the exact link-mode contract, not only the pkg-config version.
if ! requested_artifacts_exist; then
    log "ERROR: ${LIBRARY_MODE} gtk4-layer-shell artifacts are unavailable in ${PREFIX} after install"
    exit 1
fi
log "Installed gtk4-layer-shell $(gls_pkg_config --modversion gtk4-layer-shell-0) (${LIBRARY_MODE}) to ${PREFIX}"
