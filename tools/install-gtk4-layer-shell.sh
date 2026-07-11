#!/usr/bin/env bash
# Build and install gtk4-layer-shell from a pinned upstream release into
# /usr, so the GTK4 toolbar frontend can link against it.
#
# Why this exists: gtk4-layer-shell landed in the Debian/Ubuntu archives
# only recently (Ubuntu 25.04 "plucky" and Debian 13 "trixie"), so the
# GitHub runner images (currently Ubuntu 24.04) and older LTS releases have
# no `libgtk4-layer-shell-dev` package. CI and release builds compile it
# from source here. Both .github/workflows/ci.yml and
# .github/workflows/build-packages.yml call this one script so the pin
# lives in a single place.
#
# The source is pinned to a specific commit (not the moving v1.3.0 tag) and
# the downloaded archive is verified against a SHA-256, so a moved tag or a
# tampered download fails loudly rather than building unexpected code.
#
# It installs system-wide (/usr) because that is what every caller needs:
# CI, release packaging, and a from-source build all link it as a system
# library. Run as root, or as a user with passwordless sudo (CI runners).
# A from-source builder who cannot install system-wide should instead build
# wayscriber without the GTK toolbars:
#   cargo build --release --no-default-features --features tablet-input,portal,tray
#
# Prerequisites (install via the workflow/distro, not here): a C toolchain,
# pkg-config, meson, ninja, libgtk-4-dev, libwayland-dev (wayland-scanner),
# and wayland-protocols. The build uses only those — introspection and vapi
# are disabled so gobject-introspection and vala tooling are not required.
#
# Usage: bash tools/install-gtk4-layer-shell.sh
# Env:
#   FORCE_BUILD  set to 1 to build even if a suitable version is installed
set -euo pipefail

GLS_VERSION="1.3.0"
# Commit that tag v1.3.0 points at; pinning the commit (not the tag) keeps
# the build reproducible even if the tag is ever moved.
GLS_COMMIT="1c963c51514581c41b9bdae08cdf69171265cdda"
GLS_ARCHIVE_SHA256="22be5f5edf487cfb87266f0e71c400b11322082a4dc99832e5a54a4fca3d5a7c"
GLS_URL="https://github.com/wmww/gtk4-layer-shell/archive/${GLS_COMMIT}.tar.gz"

# System prefix so pkg-config and the linker find it on their default
# paths without extra environment.
PREFIX="/usr"

log() { printf '[install-gtk4-layer-shell] %s\n' "$*"; }

# Already satisfied by a distro package (25.04+/trixie) or a previous run?
if [[ "${FORCE_BUILD:-0}" != "1" ]] \
    && pkg-config --atleast-version="$GLS_VERSION" gtk4-layer-shell-0 2>/dev/null; then
    log "gtk4-layer-shell $(pkg-config --modversion gtk4-layer-shell-0) already available; skipping build"
    exit 0
fi

missing=()
for tool in curl sha256sum tar meson ninja pkg-config; do
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

# Installing into /usr needs root; escalate with sudo when not already root.
if [[ "$(id -u)" -eq 0 ]]; then
    as_root() { "$@"; }
elif command -v sudo >/dev/null 2>&1; then
    as_root() { sudo "$@"; }
else
    log "ERROR: installing to ${PREFIX} needs root, but this is not root and sudo is unavailable."
    log "Run as root, or build wayscriber without the GTK toolbars:"
    log "  cargo build --release --no-default-features --features tablet-input,portal,tray"
    exit 1
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

log "Configuring (introspection/vapi/examples/docs/tests off)"
meson setup "$src/_build" "$src" \
    --prefix="$PREFIX" \
    --libdir=lib \
    --buildtype=release \
    -Dexamples=false \
    -Ddocs=false \
    -Dtests=false \
    -Dintrospection=false \
    -Dvapi=false

log "Building"
meson compile -C "$src/_build"

log "Installing to ${PREFIX}"
as_root meson install -C "$src/_build"

# Refresh the linker cache so the freshly installed .so is found at link
# and run time.
as_root ldconfig 2>/dev/null || true

# The install went to /usr, which is on pkg-config's default search path,
# so a plain query resolves the copy we just installed.
installed="$(pkg-config --modversion gtk4-layer-shell-0 2>/dev/null || true)"
if [[ -z "$installed" ]]; then
    log "ERROR: pkg-config cannot find gtk4-layer-shell-0 after install"
    exit 1
fi
log "Installed gtk4-layer-shell ${installed}"
