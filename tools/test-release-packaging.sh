#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
PACKAGE_CONFIG="${REPO_ROOT}/packaging/package.wayscriber.yaml"
CONFIGURATOR_PACKAGE_CONFIG="${REPO_ROOT}/packaging/package.configurator.yaml"
RELEASE_WORKFLOW="${REPO_ROOT}/.github/workflows/build-packages.yml"
CI_WORKFLOW="${REPO_ROOT}/.github/workflows/ci.yml"
PACKAGE_SCRIPT="${REPO_ROOT}/tools/package.sh"
ARCH_INSTALLER_CHECKER="${REPO_ROOT}/tools/check-arch-installer-manifest.sh"
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
assert_contains "${CONFIGURATOR_PACKAGE_CONFIG}" "- libadwaita-1-0 (>= 1.4)"
assert_contains "${CONFIGURATOR_PACKAGE_CONFIG}" "- libadwaita >= 1.4"

# Release jobs must not silently raise the glibc floor when ubuntu-latest
# changes; the package job is the binary floor's defining runner.
sed -n '/^  package:/,/^  package-repos:/p' "${RELEASE_WORKFLOW}" \
    > "${WORK_DIR}/release-package-job.yml"
assert_contains "${WORK_DIR}/release-package-job.yml" "runs-on: ubuntu-24.04"
assert_not_contains "${RELEASE_WORKFLOW}" "runs-on: ubuntu-latest"
assert_contains "${WORK_DIR}/release-package-job.yml" 'dpkg-deb -f dist/wayscriber-amd64.deb Version'
assert_contains "${WORK_DIR}/release-package-job.yml" 'dpkg-deb -f dist/wayscriber-configurator-amd64.deb Version'
assert_contains "${WORK_DIR}/release-package-job.yml" "'%{VERSION}-%{RELEASE}\\n' dist/wayscriber-x86_64.rpm"
assert_contains "${WORK_DIR}/release-package-job.yml" "'%{VERSION}-%{RELEASE}\\n' dist/wayscriber-configurator-x86_64.rpm"
assert_contains "${WORK_DIR}/release-package-job.yml" "grep -Eq '/usr/bin/wayscriber$'"
assert_contains "${WORK_DIR}/release-package-job.yml" 'wayscriber-configurator-v${{ steps.meta.outputs.version }}-linux-x86_64.tar.gz'
assert_contains "${WORK_DIR}/release-package-job.yml" "grep -Eq '/usr/bin/wayscriber-configurator$'"
assert_contains "${WORK_DIR}/release-package-job.yml" \
    "grep -Fq 'libadwaita-1-0 (>= 1.4)' <<< \"\$configurator_deb_depends\""
assert_contains "${WORK_DIR}/release-package-job.yml" \
    "grep -Fxq 'libadwaita >= 1.4' <<< \"\$configurator_rpm_requires\""
assert_contains "${WORK_DIR}/release-package-job.yml" \
    "/dist/wayscriber-configurator-amd64.deb"
assert_contains "${WORK_DIR}/release-package-job.yml" "Check direct Arch installer compatibility"
assert_contains "${WORK_DIR}/release-package-job.yml" "https://wayscriber.com/arch-install.sh"
assert_contains "${WORK_DIR}/release-package-job.yml" "./tools/check-arch-installer-manifest.sh"
assert_contains "${WORK_DIR}/release-package-job.yml" 'wayscriber-v${{ steps.meta.outputs.version }}-linux-x86_64.tar.gz'
assert_not_contains "${WORK_DIR}/release-package-job.yml" 'sh "$RUNNER_TEMP/arch-install.sh"'
test -x "${ARCH_INSTALLER_CHECKER}"
assert_contains "${ARCH_INSTALLER_CHECKER}" "The installer is parsed as data and is not run."

# The deployed installer is parsed as a strict data contract. Unsupported
# shell syntax must fail instead of silently omitting a required release file.
ARCH_INSTALLER_FIXTURE_ROOT="wayscriber-v0.0.0-linux-x86_64"
ARCH_INSTALLER_FIXTURE_STAGE="${WORK_DIR}/arch-installer-stage"
ARCH_INSTALLER_FIXTURE_ARCHIVE="${WORK_DIR}/arch-installer.tar.gz"
ARCH_INSTALLER_VALID_FIXTURE="${WORK_DIR}/arch-installer-valid.sh"
ARCH_INSTALLER_MALFORMED_FIXTURE="${WORK_DIR}/arch-installer-malformed.sh"
ARCH_INSTALLER_MALFORMED_OUTPUT="${WORK_DIR}/arch-installer-malformed-output"
mkdir -p \
    "${ARCH_INSTALLER_FIXTURE_STAGE}/${ARCH_INSTALLER_FIXTURE_ROOT}/usr/bin" \
    "${ARCH_INSTALLER_FIXTURE_STAGE}/${ARCH_INSTALLER_FIXTURE_ROOT}/usr/lib/systemd/user"
touch "${ARCH_INSTALLER_FIXTURE_STAGE}/${ARCH_INSTALLER_FIXTURE_ROOT}/usr/bin/wayscriber"
chmod 0755 "${ARCH_INSTALLER_FIXTURE_STAGE}/${ARCH_INSTALLER_FIXTURE_ROOT}/usr/bin/wayscriber"
cat > "${ARCH_INSTALLER_FIXTURE_STAGE}/${ARCH_INSTALLER_FIXTURE_ROOT}/usr/lib/systemd/user/wayscriber.service" <<'EOF'
[Service]
ExecStart="/usr/bin/wayscriber" --daemon
EOF
chmod 0644 "${ARCH_INSTALLER_FIXTURE_STAGE}/${ARCH_INSTALLER_FIXTURE_ROOT}/usr/lib/systemd/user/wayscriber.service"
tar -czf "${ARCH_INSTALLER_FIXTURE_ARCHIVE}" \
    -C "${ARCH_INSTALLER_FIXTURE_STAGE}" "${ARCH_INSTALLER_FIXTURE_ROOT}"

cat > "${ARCH_INSTALLER_VALID_FIXTURE}" <<'EOF'
#!/bin/sh
# ARCH_INSTALL_MANIFEST_BEGIN
release_manifest() {
    printf '%s\n' \
        '0755 bin/wayscriber' \
        '0644 lib/systemd/user/wayscriber.service'
}
# ARCH_INSTALL_MANIFEST_END
EOF
"${ARCH_INSTALLER_CHECKER}" \
    --installer "${ARCH_INSTALLER_VALID_FIXTURE}" \
    --archive "${ARCH_INSTALLER_FIXTURE_ARCHIVE}" >/dev/null

cat > "${ARCH_INSTALLER_MALFORMED_FIXTURE}" <<'EOF'
#!/bin/sh
# ARCH_INSTALL_MANIFEST_BEGIN
release_manifest() {
    printf '%s\n' \
        '0755 bin/wayscriber' \
        '0644 lib/systemd/user/wayscriber.service' \
        "0644 share/foo"
}
# ARCH_INSTALL_MANIFEST_END
EOF
set +e
"${ARCH_INSTALLER_CHECKER}" \
    --installer "${ARCH_INSTALLER_MALFORMED_FIXTURE}" \
    --archive "${ARCH_INSTALLER_FIXTURE_ARCHIVE}" \
    >"${ARCH_INSTALLER_MALFORMED_OUTPUT}" 2>&1
ARCH_INSTALLER_MALFORMED_STATUS=$?
set -e
if [[ ${ARCH_INSTALLER_MALFORMED_STATUS} -eq 0 ]]; then
    echo "Expected unsupported installer manifest syntax to fail" >&2
    exit 1
fi
assert_contains "${ARCH_INSTALLER_MALFORMED_OUTPUT}" \
    "unsupported installer manifest syntax"

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

# package.sh's documented default discovers the Cargo version itself. Exercise
# that public path with a fake nfpm and require the resolved version to cross
# the process boundary into nfpm's environment.
PACKAGE_VERSION_REPO="${WORK_DIR}/package-version-repo"
PACKAGE_VERSION_FAKE_BIN="${WORK_DIR}/package-version-fake-bin"
PACKAGE_VERSION_ARTIFACTS="${WORK_DIR}/package-version-artifacts"
mkdir -p "${PACKAGE_VERSION_REPO}/tools" \
    "${PACKAGE_VERSION_REPO}/packaging/icons" \
    "${PACKAGE_VERSION_REPO}/packaging/licenses" \
    "${PACKAGE_VERSION_REPO}/target/release" \
    "${PACKAGE_VERSION_FAKE_BIN}"
cp "${PACKAGE_SCRIPT}" "${PACKAGE_VERSION_REPO}/tools/package.sh"
touch "${PACKAGE_VERSION_REPO}/Cargo.toml" \
    "${PACKAGE_VERSION_REPO}/README.md" \
    "${PACKAGE_VERSION_REPO}/config.example.toml" \
    "${PACKAGE_VERSION_REPO}/LICENSE" \
    "${PACKAGE_VERSION_REPO}/packaging/package.wayscriber.yaml" \
    "${PACKAGE_VERSION_REPO}/packaging/package.configurator.yaml" \
    "${PACKAGE_VERSION_REPO}/packaging/wayscriber.service" \
    "${PACKAGE_VERSION_REPO}/packaging/wayscriber.desktop" \
    "${PACKAGE_VERSION_REPO}/packaging/wayscriber-configurator.desktop" \
    "${PACKAGE_VERSION_REPO}/packaging/licenses/gtk4-layer-shell.LICENSE" \
    "${PACKAGE_VERSION_REPO}/packaging/icons/wayscriber.svg" \
    "${PACKAGE_VERSION_REPO}/packaging/icons/wayscriber-symbolic.svg" \
    "${PACKAGE_VERSION_REPO}/packaging/icons/wayscriber-configurator.svg"
for size in 16 19 22 24 38 64 128; do
    touch "${PACKAGE_VERSION_REPO}/packaging/icons/wayscriber-${size}.png"
done
for size in 24 64 128; do
    touch "${PACKAGE_VERSION_REPO}/packaging/icons/wayscriber-configurator-${size}.png"
done

cat > "${PACKAGE_VERSION_REPO}/tools/verify-static-gtk4-layer-shell.sh" <<'EOF'
#!/usr/bin/env bash
exit 0
EOF
cat > "${PACKAGE_VERSION_REPO}/target/release/wayscriber" <<'EOF'
#!/usr/bin/env bash
exit 0
EOF
cat > "${PACKAGE_VERSION_REPO}/target/release/wayscriber-configurator" <<'EOF'
#!/usr/bin/env bash
exit 0
EOF
cat > "${PACKAGE_VERSION_FAKE_BIN}/cargo" <<'EOF'
#!/usr/bin/env bash
[[ "${1:-}" == "metadata" ]] || {
    echo "Unexpected cargo arguments: $*" >&2
    exit 2
}
[[ "${PWD}" == "${PACKAGE_VERSION_EXPECTED_ROOT:?}" ]] || {
    echo "package.sh ran cargo metadata from ${PWD}, expected ${PACKAGE_VERSION_EXPECTED_ROOT}" >&2
    exit 3
}
if [[ "${PACKAGE_VERSION_MISSING:-0}" == 1 ]]; then
    printf '%s\n' '{"packages":[]}'
else
    printf '%s\n' '{"packages":[{"name":"wayscriber","version":"0.9.22"}]}'
fi
EOF
cat > "${PACKAGE_VERSION_FAKE_BIN}/readelf" <<'EOF'
#!/usr/bin/env bash
printf '%s\n' '  0x0010:   Name: GLIBC_2.39  Flags: none  Version: 4'
EOF
cat > "${PACKAGE_VERSION_FAKE_BIN}/nfpm" <<'EOF'
#!/usr/bin/env bash
[[ -n "${VERSION:-}" ]] || {
    echo "nfpm did not receive the resolved package version" >&2
    exit 91
}
target=""
while [[ $# -gt 0 ]]; do
    if [[ "$1" == "--target" ]]; then
        target="$2"
        break
    fi
    shift
done
[[ -n "${target}" ]] || {
    echo "nfpm did not receive --target" >&2
    exit 92
}
mkdir -p "$(dirname "${target}")"
package_kind="main"
[[ "${target}" == *wayscriber-configurator* ]] && package_kind="configurator"
if [[ "${PACKAGE_NFPM_FAIL:-}" == "${package_kind}" ]]; then
    printf '%s\n' 'partial package' > "${target}"
    echo "PACKAGE_TEST_NFPM_FAILURE" >&2
    exit 91
fi
printf '%s\n' "${VERSION}" > "${target}"
EOF
cat > "${PACKAGE_VERSION_FAKE_BIN}/tar" <<'EOF'
#!/usr/bin/env bash
target=""
while [[ $# -gt 0 ]]; do
    if [[ "$1" == "-czf" ]]; then
        target="$2"
        break
    fi
    shift
done
[[ -n "${target}" ]] || {
    echo "tar did not receive -czf" >&2
    exit 92
}
mkdir -p "$(dirname "${target}")"
package_kind="main"
[[ "${target}" == *wayscriber-configurator* ]] && package_kind="configurator"
if [[ "${PACKAGE_TAR_FAIL:-}" == "${package_kind}" ]]; then
    printf '%s\n' 'partial archive' > "${target}"
    echo "PACKAGE_TEST_TAR_FAILURE" >&2
    exit 93
fi
printf '%s\n' 'complete archive' > "${target}"
EOF
cat > "${PACKAGE_VERSION_FAKE_BIN}/cp" <<'EOF'
#!/usr/bin/env bash
package_kind=""
case "${1:-}" in
    */target/release/wayscriber) package_kind="main" ;;
    */target/release/wayscriber-configurator) package_kind="configurator" ;;
esac
if [[ -n "${package_kind}" && "${PACKAGE_COPY_FAIL:-}" == "${package_kind}" ]]; then
    echo "PACKAGE_TEST_COPY_FAILURE" >&2
    exit 95
fi
command -p cp "$@"
EOF
chmod +x "${PACKAGE_VERSION_REPO}/tools/verify-static-gtk4-layer-shell.sh" \
    "${PACKAGE_VERSION_REPO}/target/release/wayscriber" \
    "${PACKAGE_VERSION_REPO}/target/release/wayscriber-configurator" \
    "${PACKAGE_VERSION_FAKE_BIN}/cargo" \
    "${PACKAGE_VERSION_FAKE_BIN}/readelf" \
    "${PACKAGE_VERSION_FAKE_BIN}/nfpm" \
    "${PACKAGE_VERSION_FAKE_BIN}/tar" \
    "${PACKAGE_VERSION_FAKE_BIN}/cp"

expect_package_failure() {
    local expected_message="$1"
    local output_file="${WORK_DIR}/package-failure-output"
    shift

    set +e
    "$@" >"${output_file}" 2>&1
    local test_status=$?
    set -e

    if [[ ${test_status} -eq 0 ]]; then
        echo "Expected package.sh to fail with: ${expected_message}" >&2
        cat "${output_file}" >&2
        exit 1
    fi
    if ! grep -Fq -- "${expected_message}" "${output_file}"; then
        echo "package.sh failed for the wrong reason; expected: ${expected_message}" >&2
        cat "${output_file}" >&2
        exit 1
    fi
}

(
    cd "${WORK_DIR}"
    env -u VERSION \
        PATH="${PACKAGE_VERSION_FAKE_BIN}:${PATH}" \
        PACKAGE_VERSION_EXPECTED_ROOT="${PACKAGE_VERSION_REPO}" \
        bash "${PACKAGE_VERSION_REPO}/tools/package.sh" \
            --formats deb \
            --artifact-root "${PACKAGE_VERSION_ARTIFACTS}" \
            --skip-build \
            --no-strip >/dev/null
)

[[ "$(cat "${PACKAGE_VERSION_ARTIFACTS}/wayscriber-amd64.deb")" == "0.9.22" ]] || {
    echo "package.sh did not pass its auto-detected version to nfpm" >&2
    exit 1
}
[[ "$(cat "${PACKAGE_VERSION_ARTIFACTS}/wayscriber-configurator-amd64.deb")" == "0.9.22" ]] || {
    echo "package.sh did not pass its auto-detected version to configurator nfpm" >&2
    exit 1
}

PACKAGE_OVERRIDE_ARTIFACTS="${WORK_DIR}/package-override-artifacts"
(
    cd "${WORK_DIR}"
    env -u VERSION \
        PATH="${PACKAGE_VERSION_FAKE_BIN}:${PATH}" \
        PACKAGE_VERSION_EXPECTED_ROOT="${PACKAGE_VERSION_REPO}" \
        bash "${PACKAGE_VERSION_REPO}/tools/package.sh" \
            --version 9.8.7 \
            --formats deb \
            --artifact-root "${PACKAGE_OVERRIDE_ARTIFACTS}" \
            --skip-build \
            --no-strip >/dev/null
)

[[ "$(cat "${PACKAGE_OVERRIDE_ARTIFACTS}/wayscriber-amd64.deb")" == "9.8.7" ]] || {
    echo "package.sh did not pass its explicit version override to nfpm" >&2
    exit 1
}
[[ "$(cat "${PACKAGE_OVERRIDE_ARTIFACTS}/wayscriber-configurator-amd64.deb")" == "9.8.7" ]] || {
    echo "package.sh did not pass its explicit version override to configurator nfpm" >&2
    exit 1
}

PACKAGE_MISSING_VERSION_ARTIFACTS="${WORK_DIR}/package-missing-version-artifacts"
(
    cd "${WORK_DIR}"
    expect_package_failure "Could not resolve the package version" \
        env -u VERSION \
            PATH="${PACKAGE_VERSION_FAKE_BIN}:${PATH}" \
            PACKAGE_VERSION_EXPECTED_ROOT="${PACKAGE_VERSION_REPO}" \
            PACKAGE_VERSION_MISSING=1 \
            bash "${PACKAGE_VERSION_REPO}/tools/package.sh" \
                --formats deb \
                --artifact-root "${PACKAGE_MISSING_VERSION_ARTIFACTS}" \
                --skip-build \
                --no-strip
)

PACKAGE_NFPM_FAILURE_ARTIFACTS="${WORK_DIR}/package-nfpm-failure-artifacts"
(
    cd "${WORK_DIR}"
    expect_package_failure \
        "Failed to build deb package: ${PACKAGE_NFPM_FAILURE_ARTIFACTS}/wayscriber-amd64.deb" \
        env -u VERSION \
            PATH="${PACKAGE_VERSION_FAKE_BIN}:${PATH}" \
            PACKAGE_NFPM_FAIL=main \
            bash "${PACKAGE_VERSION_REPO}/tools/package.sh" \
                --version 0.9.22 \
                --formats deb \
                --artifact-root "${PACKAGE_NFPM_FAILURE_ARTIFACTS}" \
                --skip-build \
                --no-strip
)

PACKAGE_CONFIGURATOR_NFPM_FAILURE_ARTIFACTS="${WORK_DIR}/package-configurator-nfpm-failure-artifacts"
(
    cd "${WORK_DIR}"
    expect_package_failure \
        "Failed to build deb package: ${PACKAGE_CONFIGURATOR_NFPM_FAILURE_ARTIFACTS}/wayscriber-configurator-amd64.deb" \
        env -u VERSION \
            PATH="${PACKAGE_VERSION_FAKE_BIN}:${PATH}" \
            PACKAGE_NFPM_FAIL=configurator \
            bash "${PACKAGE_VERSION_REPO}/tools/package.sh" \
                --version 0.9.22 \
                --formats deb \
                --artifact-root "${PACKAGE_CONFIGURATOR_NFPM_FAILURE_ARTIFACTS}" \
                --skip-build \
                --no-strip
)

PACKAGE_TAR_FAILURE_ARTIFACTS="${WORK_DIR}/package-tar-failure-artifacts"
(
    cd "${WORK_DIR}"
    expect_package_failure \
        "Failed to build tarball: ${PACKAGE_TAR_FAILURE_ARTIFACTS}/wayscriber-v0.9.22-linux-x86_64.tar.gz" \
        env -u VERSION \
            PATH="${PACKAGE_VERSION_FAKE_BIN}:${PATH}" \
            PACKAGE_TAR_FAIL=main \
            bash "${PACKAGE_VERSION_REPO}/tools/package.sh" \
                --version 0.9.22 \
                --formats tar \
                --artifact-root "${PACKAGE_TAR_FAILURE_ARTIFACTS}" \
                --skip-build \
                --no-strip \
                --no-configurator
)

PACKAGE_CONFIGURATOR_TAR_FAILURE_ARTIFACTS="${WORK_DIR}/package-configurator-tar-failure-artifacts"
(
    cd "${WORK_DIR}"
    expect_package_failure \
        "Failed to build tarball: ${PACKAGE_CONFIGURATOR_TAR_FAILURE_ARTIFACTS}/wayscriber-configurator-v0.9.22-linux-x86_64.tar.gz" \
        env -u VERSION \
            PATH="${PACKAGE_VERSION_FAKE_BIN}:${PATH}" \
            PACKAGE_TAR_FAIL=configurator \
            bash "${PACKAGE_VERSION_REPO}/tools/package.sh" \
                --version 0.9.22 \
                --formats tar \
                --artifact-root "${PACKAGE_CONFIGURATOR_TAR_FAILURE_ARTIFACTS}" \
                --skip-build \
                --no-strip
)

PACKAGE_MAIN_COPY_FAILURE_ARTIFACTS="${WORK_DIR}/package-main-copy-failure-artifacts"
(
    cd "${WORK_DIR}"
    expect_package_failure "PACKAGE_TEST_COPY_FAILURE" \
        env -u VERSION \
            PATH="${PACKAGE_VERSION_FAKE_BIN}:${PATH}" \
            PACKAGE_COPY_FAIL=main \
            bash "${PACKAGE_VERSION_REPO}/tools/package.sh" \
                --version 0.9.22 \
                --formats tar \
                --artifact-root "${PACKAGE_MAIN_COPY_FAILURE_ARTIFACTS}" \
                --skip-build \
                --no-strip \
                --no-configurator
)

PACKAGE_CONFIGURATOR_COPY_FAILURE_ARTIFACTS="${WORK_DIR}/package-configurator-copy-failure-artifacts"
(
    cd "${WORK_DIR}"
    expect_package_failure "PACKAGE_TEST_COPY_FAILURE" \
        env -u VERSION \
            PATH="${PACKAGE_VERSION_FAKE_BIN}:${PATH}" \
            PACKAGE_COPY_FAIL=configurator \
            bash "${PACKAGE_VERSION_REPO}/tools/package.sh" \
                --version 0.9.22 \
                --formats tar \
                --artifact-root "${PACKAGE_CONFIGURATOR_COPY_FAILURE_ARTIFACTS}" \
                --skip-build \
                --no-strip
)

# PACKAGE_CONFIGURATOR=1 promises every requested configurator artifact. A
# missing executable must fail before packaging; --no-configurator is the
# explicit opt-out.
rm -f "${PACKAGE_VERSION_REPO}/target/release/wayscriber-configurator"
PACKAGE_MISSING_CONFIGURATOR_ARTIFACTS="${WORK_DIR}/package-missing-configurator-artifacts"
(
    cd "${WORK_DIR}"
    expect_package_failure \
        "Missing release binary: ${PACKAGE_VERSION_REPO}/target/release/wayscriber-configurator" \
        env -u VERSION \
            PATH="${PACKAGE_VERSION_FAKE_BIN}:${PATH}" \
            bash "${PACKAGE_VERSION_REPO}/tools/package.sh" \
                --version 0.9.22 \
                --formats tar \
                --artifact-root "${PACKAGE_MISSING_CONFIGURATOR_ARTIFACTS}" \
                --skip-build \
                --no-strip
)

PACKAGE_NO_CONFIGURATOR_ARTIFACTS="${WORK_DIR}/package-no-configurator-artifacts"
(
    cd "${WORK_DIR}"
    env -u VERSION \
        PATH="${PACKAGE_VERSION_FAKE_BIN}:${PATH}" \
        bash "${PACKAGE_VERSION_REPO}/tools/package.sh" \
            --version 0.9.22 \
            --formats deb \
            --artifact-root "${PACKAGE_NO_CONFIGURATOR_ARTIFACTS}" \
            --skip-build \
            --no-strip \
            --no-configurator >/dev/null
)
[[ -f "${PACKAGE_NO_CONFIGURATOR_ARTIFACTS}/wayscriber-amd64.deb" ]] || {
    echo "--no-configurator did not produce the requested main package" >&2
    exit 1
}
[[ ! -e "${PACKAGE_NO_CONFIGURATOR_ARTIFACTS}/wayscriber-configurator-amd64.deb" ]] || {
    echo "--no-configurator unexpectedly produced a configurator package" >&2
    exit 1
}

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
    --config-dir "${WORK_DIR}/missing-config" \
    --no-configurator >/dev/null

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

# The configurator AUR channel is required by default. The hosted clone step
# must fail before the updater runs rather than silently publish two channels.
assert_not_contains "${RELEASE_WORKFLOW}" \
    "git clone ssh://aur@aur.archlinux.org/wayscriber-configurator.git aur-wayscriber-configurator || true"

AUR_SOURCE_SHA='abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789'
AUR_FAKE_BIN="${WORK_DIR}/aur-fake-bin"
mkdir -p "${AUR_FAKE_BIN}"
cat > "${AUR_FAKE_BIN}/curl" <<'EOF'
#!/usr/bin/env bash
echo 'AUR_TEST_NETWORK_ACCESS' >&2
exit 97
EOF
chmod +x "${AUR_FAKE_BIN}/curl"

write_source_clone() {
    local dir="$1" pkgver="$2" pkgrel="$3"
    mkdir -p "${dir}"
    cat > "${dir}/PKGBUILD" <<EOF
pkgname=wayscriber
pkgver=${pkgver}
pkgrel=${pkgrel}
install=wayscriber.install
depends=(
    'gcc-libs'
    'wl-clipboard'
)
makedepends=(
    'git'
)
source=("old.tar.gz::https://example.invalid/old.tar.gz")
sha256sums=('old')
package() {
    cd "\$pkgname"
}
EOF
    cat > "${dir}/.SRCINFO" <<EOF
pkgbase = wayscriber
	pkgver = ${pkgver}
	pkgrel = ${pkgrel}
	install = wayscriber.install
	depends = gcc-libs
	depends = wl-clipboard
	makedepends = git
	source = old.tar.gz::https://example.invalid/old.tar.gz
	sha256sums = old

pkgname = wayscriber
EOF
    touch "${dir}/wayscriber.install"
    git -C "${dir}" init -q
    git -C "${dir}" add PKGBUILD .SRCINFO wayscriber.install
}

write_configurator_clone() {
    local dir="$1" pkgver="$2" pkgrel="$3"
    mkdir -p "${dir}"
    cat > "${dir}/PKGBUILD" <<EOF
pkgname=wayscriber-configurator
pkgver=${pkgver}
pkgrel=${pkgrel}
depends=(
    'gcc-libs'
)
makedepends=(
    'git'
)
source=("old.tar.gz::https://example.invalid/old.tar.gz")
sha256sums=('old')
build() {
    cd wayscriber
}
EOF
    cat > "${dir}/.SRCINFO" <<EOF
pkgbase = wayscriber-configurator
	pkgver = ${pkgver}
	pkgrel = ${pkgrel}
	depends = gcc-libs
	makedepends = git
	source = old.tar.gz::https://example.invalid/old.tar.gz
	sha256sums = old

pkgname = wayscriber-configurator
EOF
    git -C "${dir}" init -q
    git -C "${dir}" add PKGBUILD .SRCINFO
}

run_aur_updater() {
    local cwd="$1"
    shift
    (
        cd "${cwd}"
        PATH="${AUR_FAKE_BIN}:${PATH}" \
            bash "${REPO_ROOT}/tools/update-aur-from-manifest.sh" \
                --version 9.9.9 \
                --manifest "${MANIFEST}" \
                "$@"
    )
}

expect_aur_failure() {
    local expected="$1" cwd="$2"
    shift 2
    local output="${WORK_DIR}/aur-failure-output"

    set +e
    run_aur_updater "${cwd}" "$@" >"${output}" 2>&1
    local status=$?
    set -e
    if [[ ${status} -eq 0 ]]; then
        echo "Expected AUR updater failure containing: ${expected}" >&2
        exit 1
    fi
    assert_contains "${output}" "${expected}"
}

# Relative clone paths used to be resolved after pushd, turning `dir/PKGBUILD`
# into `dir/dir/PKGBUILD` and resetting same-version hotfixes to pkgrel=1.
AUR_SOURCE_HOTFIX="${WORK_DIR}/aur-source-hotfix"
write_source_clone "${AUR_SOURCE_HOTFIX}" 9.9.9 3
run_aur_updater "${WORK_DIR}" \
    --source-dir aur-source-hotfix \
    --bin-dir missing-bin \
    --config-dir missing-config \
    --no-configurator \
    --source-sha256 "${AUR_SOURCE_SHA}" >/dev/null
assert_contains "${AUR_SOURCE_HOTFIX}/PKGBUILD" "pkgrel=4"
assert_contains "${AUR_SOURCE_HOTFIX}/.SRCINFO" "pkgrel = 4"

AUR_CONFIG_HOTFIX="${WORK_DIR}/aur-config-hotfix"
write_configurator_clone "${AUR_CONFIG_HOTFIX}" 9.9.9 3
run_aur_updater "${WORK_DIR}" \
    --source-dir missing-source \
    --bin-dir missing-bin \
    --config-dir aur-config-hotfix \
    --source-sha256 "${AUR_SOURCE_SHA}" >/dev/null
assert_contains "${AUR_CONFIG_HOTFIX}/PKGBUILD" "pkgrel=4"
assert_contains "${AUR_CONFIG_HOTFIX}/.SRCINFO" "pkgrel = 4"

# A missing required configurator clone aborts before an earlier source channel
# is touched. Skipping it remains available, but only as an explicit decision.
AUR_PREFLIGHT_SOURCE="${WORK_DIR}/aur-preflight-source"
write_source_clone "${AUR_PREFLIGHT_SOURCE}" 9.9.9 3
cp "${AUR_PREFLIGHT_SOURCE}/PKGBUILD" "${WORK_DIR}/aur-preflight-source.before"
expect_aur_failure "wayscriber-configurator AUR clone not found" "${WORK_DIR}" \
    --source-dir aur-preflight-source \
    --bin-dir missing-bin \
    --config-dir missing-config \
    --source-sha256 "${AUR_SOURCE_SHA}"
cmp "${WORK_DIR}/aur-preflight-source.before" "${AUR_PREFLIGHT_SOURCE}/PKGBUILD"

AUR_SKIP_OUTPUT="${WORK_DIR}/aur-skip-output"
run_aur_updater "${WORK_DIR}" \
    --source-dir missing-source \
    --bin-dir missing-bin \
    --config-dir missing-config \
    --no-configurator >"${AUR_SKIP_OUTPUT}" 2>&1
assert_contains "${AUR_SKIP_OUTPUT}" "--no-configurator was passed"

# Download and checksum validation also run before mutation. The fake curl
# makes accidental network use deterministic and proves the clone stays intact.
AUR_CHECKSUM_SOURCE="${WORK_DIR}/aur-checksum-source"
write_source_clone "${AUR_CHECKSUM_SOURCE}" 9.9.8 2
cp "${AUR_CHECKSUM_SOURCE}/PKGBUILD" "${WORK_DIR}/aur-checksum-source.before"
expect_aur_failure "Failed to download the source archive" "${WORK_DIR}" \
    --source-dir aur-checksum-source \
    --bin-dir missing-bin \
    --config-dir missing-config \
    --no-configurator
cmp "${WORK_DIR}/aur-checksum-source.before" "${AUR_CHECKSUM_SOURCE}/PKGBUILD"

expect_aur_failure "Source archive checksum is not a sha256 digest" "${WORK_DIR}" \
    --source-dir aur-checksum-source \
    --bin-dir missing-bin \
    --config-dir missing-config \
    --no-configurator \
    --source-sha256 invalid

echo "Release packaging contract checks passed."
