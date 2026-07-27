#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
PACKAGE_CONFIG="${REPO_ROOT}/packaging/package.wayscriber.yaml"
CONFIGURATOR_PACKAGE_CONFIG="${REPO_ROOT}/packaging/package.configurator.yaml"
RELEASE_WORKFLOW="${REPO_ROOT}/.github/workflows/build-packages.yml"
CI_WORKFLOW="${REPO_ROOT}/.github/workflows/ci.yml"
PACKAGE_SCRIPT="${REPO_ROOT}/tools/package.sh"
INSTALL_SCRIPT="${REPO_ROOT}/tools/install-gtk4-layer-shell.sh"
STATIC_LINK_VERIFIER="${REPO_ROOT}/tools/verify-static-gtk4-layer-shell.sh"
WORK_DIR="$(mktemp -d)"

cleanup() {
    rm -rf "${WORK_DIR}"
}
trap cleanup EXIT

assert_contains() {
    local file="$1" value="$2"
    grep -Fq -- "${value}" "${file}" || {
        echo "Expected ${file} to contain: ${value}" >&2
        exit 1
    }
}

assert_not_contains() {
    local file="$1" value="$2"
    if grep -Fq -- "${value}" "${file}"; then
        echo "Expected ${file} not to contain: ${value}" >&2
        exit 1
    fi
}

# Prebuilt deb/rpm packages must declare the supported ABI floors, while the
# statically embedded layer-shell implementation must not remain a dependency.
sed -n '/^overrides:/,$p' "${PACKAGE_CONFIG}" > "${WORK_DIR}/package-overrides.yml"
assert_contains "${PACKAGE_CONFIG}" "- libc6 (>= 2.39)"
assert_contains "${PACKAGE_CONFIG}" "- libgtk-4-1 (>= 4.12)"
assert_contains "${PACKAGE_CONFIG}" "- glibc >= 2.39"
assert_contains "${PACKAGE_CONFIG}" "- gtk4 >= 4.12"
assert_not_contains "${WORK_DIR}/package-overrides.yml" "- libgtk4-layer-shell0"
assert_not_contains "${WORK_DIR}/package-overrides.yml" "- gtk4-layer-shell"
assert_contains "${CONFIGURATOR_PACKAGE_CONFIG}" "- libc6 (>= 2.39)"
assert_contains "${CONFIGURATOR_PACKAGE_CONFIG}" "- glibc >= 2.39"

# Release jobs must not silently raise the glibc floor when ubuntu-latest
# changes; the package job is the binary floor's defining runner.
sed -n '/^  package:/,/^  package-repos:/p' "${RELEASE_WORKFLOW}" \
    > "${WORK_DIR}/release-package-job.yml"
assert_contains "${WORK_DIR}/release-package-job.yml" "runs-on: ubuntu-24.04"
assert_not_contains "${RELEASE_WORKFLOW}" "runs-on: ubuntu-latest"

# Ordinary CI exercises the dynamic source-build path, while a dedicated step
# checks the static release path.
assert_contains "${CI_WORKFLOW}" "GTK4_LAYER_SHELL_LIBRARY_MODE=both"
assert_contains "${CI_WORKFLOW}" "Check dynamic gtk4-layer-shell linkage"
assert_contains "${CI_WORKFLOW}" "Check static gtk4-layer-shell linkage"
if grep -Eq 'SYSTEM_DEPS_GTK4_LAYER_SHELL_0_LINK.*GITHUB_ENV|GITHUB_ENV.*SYSTEM_DEPS_GTK4_LAYER_SHELL_0_LINK' \
    "${CI_WORKFLOW}"; then
    echo "Static gtk4-layer-shell linkage must not be exported globally in CI" >&2
    exit 1
fi

# Release builds use the private static archive. Reusable prefixes must carry
# the exact commit/checksum stamp rather than matching on version alone.
assert_contains "${PACKAGE_SCRIPT}" 'SYSTEM_DEPS_GTK4_LAYER_SHELL_0_LINK=static'
assert_contains "${PACKAGE_SCRIPT}" 'verify-static-gtk4-layer-shell.sh'
assert_contains "${INSTALL_SCRIPT}" 'PIN_STAMP='
assert_contains "${INSTALL_SCRIPT}" 'commit=%s'
assert_contains "${INSTALL_SCRIPT}" 'archive_sha256=%s'

# A private cache created before the stamp recorded library mode must be
# rebuilt even when leftover shared/static artifacts satisfy the new request.
INSTALLER_FAKE_BIN="${WORK_DIR}/installer-fake-bin"
INSTALLER_PREFIX="${WORK_DIR}/installer-prefix"
INSTALLER_STAMP="${INSTALLER_PREFIX}/share/wayscriber/build-deps/gtk4-layer-shell.pin"
mkdir -p "${INSTALLER_FAKE_BIN}" "${INSTALLER_PREFIX}/lib" "$(dirname "${INSTALLER_STAMP}")"
touch "${INSTALLER_PREFIX}/lib/libgtk4-layer-shell.a" \
    "${INSTALLER_PREFIX}/lib/libgtk4-layer-shell.so"

cat > "${INSTALLER_FAKE_BIN}/pkg-config" <<'EOF'
#!/usr/bin/env bash
case "$*" in
    *--atleast-version=*) exit 0 ;;
    *--variable=libdir*) printf '%s\n' "${INSTALLER_TEST_LIBDIR:?}" ;;
    *--modversion*) printf '%s\n' '1.3.0' ;;
    *) echo "Unexpected pkg-config arguments: $*" >&2; exit 2 ;;
esac
EOF
cat > "${INSTALLER_FAKE_BIN}/curl" <<'EOF'
#!/usr/bin/env bash
echo 'INSTALLER_TEST_REBUILD_REQUIRED' >&2
exit 97
EOF
cat > "${INSTALLER_FAKE_BIN}/meson" <<'EOF'
#!/usr/bin/env bash
echo 'Meson must not run in installer cache tests' >&2
exit 98
EOF
chmod +x "${INSTALLER_FAKE_BIN}/pkg-config" \
    "${INSTALLER_FAKE_BIN}/curl" \
    "${INSTALLER_FAKE_BIN}/meson"

run_installer_cache_check() {
    PATH="${INSTALLER_FAKE_BIN}:${PATH}" \
    INSTALLER_TEST_LIBDIR="${INSTALLER_PREFIX}/lib" \
    GTK4_LAYER_SHELL_PREFIX="${INSTALLER_PREFIX}" \
    GTK4_LAYER_SHELL_SYSTEM_PREFIX="${2:-/usr}" \
    GTK4_LAYER_SHELL_LIBRARY_MODE="$1" \
        bash "${INSTALL_SCRIPT}"
}

expect_installer_rebuild() {
    local description="$1" mode="$2" system_prefix="${3:-/usr}" output status
    set +e
    output="$(run_installer_cache_check "${mode}" "${system_prefix}" 2>&1)"
    status=$?
    set -e

    if [[ "${status}" -eq 0 ]]; then
        echo "Installer unexpectedly reused ${description}" >&2
        exit 1
    fi
    grep -Fq 'INSTALLER_TEST_REBUILD_REQUIRED' <<< "${output}" || {
        echo "Installer rejected ${description} for an unrelated reason:" >&2
        echo "${output}" >&2
        exit 1
    }
}

expect_installer_skip() {
    local description="$1" mode="$2" system_prefix="${3:-/usr}" output
    output="$(run_installer_cache_check "${mode}" "${system_prefix}" 2>&1)" || {
        echo "Installer failed to reuse ${description}:" >&2
        echo "${output}" >&2
        exit 1
    }
    grep -Fq 'skipping build' <<< "${output}" || {
        echo "Installer did not report reusing ${description}" >&2
        exit 1
    }
    if grep -Fq 'INSTALLER_TEST_REBUILD_REQUIRED' <<< "${output}"; then
        echo "Installer downloaded while reusing ${description}" >&2
        exit 1
    fi
}

write_installer_stamp() {
    cat > "${INSTALLER_STAMP}" <<EOF
version=1.3.0
commit=1c963c51514581c41b9bdae08cdf69171265cdda
archive_sha256=22be5f5edf487cfb87266f0e71c400b11322082a4dc99832e5a54a4fca3d5a7c
library_mode=$1
EOF
}

cat > "${INSTALLER_STAMP}" <<'EOF'
version=1.3.0
commit=1c963c51514581c41b9bdae08cdf69171265cdda
archive_sha256=22be5f5edf487cfb87266f0e71c400b11322082a4dc99832e5a54a4fca3d5a7c
EOF
expect_installer_rebuild "a mode-less private cache" shared

write_installer_stamp static
expect_installer_skip "a matching static private cache" static
expect_installer_rebuild "a static private cache requested as shared" shared

write_installer_stamp shared
expect_installer_skip "a matching shared private cache" shared

rm -f "${INSTALLER_STAMP}"
expect_installer_rebuild "an unstamped private cache" shared
expect_installer_skip "an unstamped system-managed cache" shared "${INSTALLER_PREFIX}"

# Exercise the linkage verifier itself, including every exact shim name. The
# shorter marshal names must not pass by matching longer symbol substrings.
FAKE_BIN="${WORK_DIR}/fake-bin"
READELF_OK="${WORK_DIR}/readelf-ok.txt"
NM_OK="${WORK_DIR}/nm-ok.txt"
mkdir -p "${FAKE_BIN}"

cat > "${FAKE_BIN}/readelf" <<'EOF'
#!/usr/bin/env bash
cat "${VERIFY_TEST_READELF_OUTPUT:?}"
EOF
cat > "${FAKE_BIN}/nm" <<'EOF'
#!/usr/bin/env bash
cat "${VERIFY_TEST_NM_OUTPUT:?}"
EOF
chmod +x "${FAKE_BIN}/readelf" "${FAKE_BIN}/nm"

cat > "${READELF_OK}" <<'EOF'
Dynamic section at offset 0x1 contains 1 entry:
 0x0000000000000001 (NEEDED)             Shared library: [libwayland-client.so.0]
EOF
cat > "${NM_OK}" <<'EOF'
0000000000000001 T wl_proxy_destroy
0000000000000002 T wl_proxy_marshal_array_flags
0000000000000003 T wl_proxy_marshal_flags
0000000000000004 T wl_proxy_marshal
0000000000000005 T wl_proxy_marshal_array
0000000000000006 T wl_proxy_marshal_constructor
0000000000000007 T wl_proxy_marshal_constructor_versioned
0000000000000008 T wl_proxy_marshal_array_constructor
0000000000000009 T wl_proxy_marshal_array_constructor_versioned
EOF

run_link_verifier() {
    PATH="${FAKE_BIN}:${PATH}" \
    VERIFY_TEST_READELF_OUTPUT="$1" \
    VERIFY_TEST_NM_OUTPUT="$2" \
        bash "${STATIC_LINK_VERIFIER}" /bin/true
}

expect_link_verifier_failure() {
    local description="$1" expected_message="$2" readelf_output="$3" nm_output="$4"
    local output status
    set +e
    output="$(run_link_verifier "${readelf_output}" "${nm_output}" 2>&1)"
    status=$?
    set -e

    if [[ "${status}" -eq 0 ]]; then
        echo "Static linkage verifier unexpectedly accepted ${description}" >&2
        exit 1
    fi
    grep -Fq "${expected_message}" <<< "${output}" || {
        echo "Static linkage verifier rejected ${description} for an unrelated reason:" >&2
        echo "${output}" >&2
        exit 1
    }
}

run_link_verifier "${READELF_OK}" "${NM_OK}" >/dev/null

READELF_DYNAMIC_LAYER="${WORK_DIR}/readelf-dynamic-layer.txt"
cp "${READELF_OK}" "${READELF_DYNAMIC_LAYER}"
printf '%s\n' ' 0x0000000000000001 (NEEDED)             Shared library: [libgtk4-layer-shell.so.0]' \
    >> "${READELF_DYNAMIC_LAYER}"
expect_link_verifier_failure "a dynamic layer-shell dependency" \
    "still dynamically requires gtk4-layer-shell" \
    "${READELF_DYNAMIC_LAYER}" "${NM_OK}"

READELF_NO_WAYLAND="${WORK_DIR}/readelf-no-wayland.txt"
grep -Fv 'libwayland-client.so.0' "${READELF_OK}" > "${READELF_NO_WAYLAND}"
expect_link_verifier_failure "a missing libwayland-client dependency" \
    "does not retain libwayland-client.so.0 in DT_NEEDED" \
    "${READELF_NO_WAYLAND}" "${NM_OK}"

for shim in \
    wl_proxy_destroy \
    wl_proxy_marshal_array_flags \
    wl_proxy_marshal_flags \
    wl_proxy_marshal \
    wl_proxy_marshal_array \
    wl_proxy_marshal_constructor \
    wl_proxy_marshal_constructor_versioned \
    wl_proxy_marshal_array_constructor \
    wl_proxy_marshal_array_constructor_versioned; do
    nm_without_shim="${WORK_DIR}/nm-without-${shim}.txt"
    awk -v omitted="${shim}" '$3 != omitted' "${NM_OK}" > "${nm_without_shim}"
    expect_link_verifier_failure "a binary missing ${shim}" \
        "does not export required gtk4-layer-shell shim ${shim}" \
        "${READELF_OK}" "${nm_without_shim}"
done

# All three release formats retain the notice for statically embedded code.
test -f "${REPO_ROOT}/packaging/licenses/gtk4-layer-shell.LICENSE"
assert_contains "${PACKAGE_CONFIG}" "dst: /usr/share/licenses/wayscriber/LICENSE.gtk4-layer-shell"
assert_contains "${PACKAGE_SCRIPT}" 'LICENSE.gtk4-layer-shell'

# Exercise the actual AUR updater against prebuilt metadata. The source AUR
# recipe intentionally keeps gtk4-layer-shell; wayscriber-bin must remove it.
AUR_BIN_DIR="${WORK_DIR}/wayscriber-bin"
MANIFEST="${WORK_DIR}/manifest.json"
mkdir -p "${AUR_BIN_DIR}"

cat > "${AUR_BIN_DIR}/PKGBUILD" <<'EOF'
pkgname=wayscriber-bin
pkgver=0.9.22
pkgrel=1
depends=(
    'cairo'
    'wayland'
    'pango'
    'gcc-libs'
    'glibc'
    'gtk4'
    'gtk4-layer-shell'
    'wl-clipboard'
)
source_x86_64=("old.tar.gz::https://example.invalid/old.tar.gz")
sha256sums_x86_64=('old')

_tarball="wayscriber-v${pkgver}-linux-${CARCH}.tar.gz"

package() {
    local srcdir_tmp="${srcdir}/extract"
    rm -rf "${srcdir_tmp}"
    mkdir -p "${srcdir_tmp}"
    tar -xzf "${srcdir}/${_tarball}" -C "${srcdir_tmp}" --strip-components=1

    install -Dm755 "${srcdir_tmp}/usr/bin/wayscriber" "$pkgdir/usr/bin/wayscriber"
    install -Dm644 "${srcdir_tmp}/usr/lib/systemd/user/wayscriber.service" "$pkgdir/usr/lib/systemd/user/wayscriber.service"
    install -Dm644 "${srcdir_tmp}/usr/share/doc/wayscriber/config.example.toml" "$pkgdir/usr/share/doc/wayscriber/config.example.toml"
    install -Dm644 "${srcdir_tmp}/usr/share/doc/wayscriber/README.md" "$pkgdir/usr/share/doc/wayscriber/README.md"
    [ -f "${srcdir_tmp}/usr/share/doc/wayscriber/LICENSE" ] && install -Dm644 "${srcdir_tmp}/usr/share/doc/wayscriber/LICENSE" "$pkgdir/usr/share/licenses/$pkgname/LICENSE" || true
}
EOF

cat > "${AUR_BIN_DIR}/.SRCINFO" <<'EOF'
pkgbase = wayscriber-bin
	pkgver = 0.9.22
	pkgrel = 1
	depends = gcc-libs
	depends = gtk4-layer-shell
	depends = wl-clipboard
	source_x86_64 = old.tar.gz::https://example.invalid/old.tar.gz
	sha256sums_x86_64 = old

pkgname = wayscriber-bin
EOF

cat > "${MANIFEST}" <<'EOF'
{
  "version": "9.9.9",
  "artifacts": [
    {
      "name": "wayscriber-v9.9.9-linux-x86_64.tar.gz",
      "sha256": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
      "size": 1
    }
  ]
}
EOF

git -C "${AUR_BIN_DIR}" init -q
git -C "${AUR_BIN_DIR}" add PKGBUILD .SRCINFO

bash "${REPO_ROOT}/tools/update-aur-from-manifest.sh" \
    --version 9.9.9 \
    --manifest "${MANIFEST}" \
    --source-dir "${WORK_DIR}/missing-source" \
    --bin-dir "${AUR_BIN_DIR}" \
    --config-dir "${WORK_DIR}/missing-config" >/dev/null

assert_contains "${AUR_BIN_DIR}/PKGBUILD" "'gtk4'"
assert_contains "${AUR_BIN_DIR}/.SRCINFO" "depends = gtk4"
assert_not_contains "${AUR_BIN_DIR}/PKGBUILD" "'gtk4-layer-shell'"
assert_not_contains "${AUR_BIN_DIR}/.SRCINFO" "depends = gtk4-layer-shell"
assert_contains "${AUR_BIN_DIR}/PKGBUILD" 'install -Dm644 "${srcdir_tmp}/usr/share/licenses/wayscriber/LICENSE.gtk4-layer-shell" "$pkgdir/usr/share/licenses/$pkgname/LICENSE.gtk4-layer-shell"'
[[ "$(grep -Fc 'LICENSE.gtk4-layer-shell' "${AUR_BIN_DIR}/PKGBUILD")" -eq 1 ]] || {
    echo "wayscriber-bin must install the gtk4-layer-shell notice exactly once" >&2
    exit 1
}

assert_contains "${REPO_ROOT}/packaging/PKGBUILD" "'gtk4-layer-shell'"
assert_contains "${REPO_ROOT}/packaging/.SRCINFO" "depends = gtk4-layer-shell"

echo "Release packaging contract checks passed."
