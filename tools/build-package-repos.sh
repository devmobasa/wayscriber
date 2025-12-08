#!/usr/bin/env bash
# Assemble Debian/Ubuntu (apt) and Fedora/RHEL (dnf/yum) repositories from the
# already-built packages under dist/.
# - Requires: apt-ftparchive, createrepo_c, gzip, rpmsign (for RPM signing), gpg.
# - Optional: GPG_PRIVATE_KEY_B64/GPG_PASSPHRASE/GPG_KEY_ID to sign Release/repodata
#             (and RPMs if SIGN_RPMS=1).
#
# Environment variables:
#   ARTIFACT_ROOT   Path containing built packages      [dist]
#   OUTPUT_ROOT     Where to stage the repos            [dist/repos]
#   DEB_SUITE       apt suite/codename (e.g., stable)   [stable]
#   DEB_COMPONENT   apt component (e.g., main)          [main]
#   DEB_ARCH        apt architecture                    [amd64]
#   RPM_ARCH        rpm architecture                    [x86_64]
#   REPO_ORIGIN     Origin field for apt Release        [Wayscriber]
#   REPO_LABEL      Label field for apt Release         [Wayscriber]
#   SIGN_RPMS       1 to rpmsign packages if key exists [1]
#   GPG_PRIVATE_KEY_B64  base64-encoded private key (ASCII-armored)
#   GPG_PASSPHRASE       passphrase for the private key (optional)
#   GPG_KEY_ID           explicit key id (optional; auto-detected otherwise)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

ARTIFACT_ROOT="${ARTIFACT_ROOT:-${REPO_ROOT}/dist}"
OUTPUT_ROOT="${OUTPUT_ROOT:-${REPO_ROOT}/dist/repos}"
DEB_SUITE="${DEB_SUITE:-stable}"
DEB_COMPONENT="${DEB_COMPONENT:-main}"
DEB_ARCH="${DEB_ARCH:-amd64}"
RPM_ARCH="${RPM_ARCH:-x86_64}"
REPO_ORIGIN="${REPO_ORIGIN:-Wayscriber}"
REPO_LABEL="${REPO_LABEL:-Wayscriber}"
SIGN_RPMS="${SIGN_RPMS:-1}"

log()  { echo "[build-repos] $*" >&2; }
warn() { echo "[WARN] $*" >&2; }
die()  { echo "[ERROR] $*" >&2; exit 1; }
need_cmd() { command -v "$1" >/dev/null 2>&1 || die "Missing required command: $1"; }

need_cmd apt-ftparchive
need_cmd createrepo_c
need_cmd gzip
need_cmd find
if [[ -n "${GPG_PRIVATE_KEY_B64:-}" ]]; then
    need_cmd gpg
fi
if [[ "${SIGN_RPMS}" == "1" ]]; then
    need_cmd rpmsign
fi

mkdir -p "${OUTPUT_ROOT}"

DEB_MAIN="${ARTIFACT_ROOT}/wayscriber-amd64.deb"
DEB_CFG="${ARTIFACT_ROOT}/wayscriber-configurator-amd64.deb"
RPM_MAIN="${ARTIFACT_ROOT}/wayscriber-${RPM_ARCH}.rpm"
RPM_CFG="${ARTIFACT_ROOT}/wayscriber-configurator-${RPM_ARCH}.rpm"

[[ -f "${DEB_MAIN}" ]] || die "Missing deb package at ${DEB_MAIN}"
[[ -f "${RPM_MAIN}" ]] || die "Missing rpm package at ${RPM_MAIN}"

GNUPGHOME_TMP=""
ACTIVE_KEY=""

setup_gpg() {
    [[ -n "${GPG_PRIVATE_KEY_B64:-}" ]] || { warn "GPG_PRIVATE_KEY_B64 not set; repos will be unsigned"; return; }

    GNUPGHOME_TMP="$(mktemp -d)"
    export GNUPGHOME="${GNUPGHOME_TMP}"

    echo "${GPG_PRIVATE_KEY_B64}" | base64 --decode | gpg --batch --import >/dev/null 2>&1

    if [[ -z "${GPG_KEY_ID:-}" ]]; then
        ACTIVE_KEY="$(gpg --batch --list-secret-keys --with-colons | awk -F: '/^sec/ {print $5; exit}')"
    else
        ACTIVE_KEY="${GPG_KEY_ID}"
    fi

    [[ -n "${ACTIVE_KEY}" ]] || die "Failed to detect imported GPG key id"
    log "Using GPG key: ${ACTIVE_KEY}"

    gpg --batch --yes --pinentry-mode loopback ${GPG_PASSPHRASE:+--passphrase "$GPG_PASSPHRASE"} \
        --export --armor "${ACTIVE_KEY}" > "${OUTPUT_ROOT}/WAYSCRIBER-GPG-KEY.asc"
    gpg --batch --yes --pinentry-mode loopback ${GPG_PASSPHRASE:+--passphrase "$GPG_PASSPHRASE"} \
        --export "${ACTIVE_KEY}" | gpg --dearmor > "${OUTPUT_ROOT}/WAYSCRIBER-GPG-KEY.gpg"
    log "Exported public key: ${OUTPUT_ROOT}/WAYSCRIBER-GPG-KEY.asc"

    cat > "${HOME}/.rpmmacros" <<EOF
%_signature gpg
%_gpg_name ${ACTIVE_KEY}
%_gpg_path ${GNUPGHOME}
%__gpg $(command -v gpg)
%__gpg_sign_cmd %{__gpg} --batch --pinentry-mode loopback --passphrase-fd 3 --no-armor --detach-sign --sign --local-user "%{_gpg_name}" --output %{__signature_filename} %{__plaintext_filename}
EOF
}

cleanup() {
    if [[ -n "${GNUPGHOME_TMP}" && -d "${GNUPGHOME_TMP}" ]]; then
        rm -rf "${GNUPGHOME_TMP}"
    fi
}
trap cleanup EXIT

sign_rpm() {
    local rpm_path="$1"
    [[ -n "${ACTIVE_KEY}" ]] || { warn "Skipping rpm signing (no GPG key)"; return; }
    [[ "${SIGN_RPMS}" == "1" ]] || { warn "Skipping rpm signing (SIGN_RPMS=0)"; return; }

    if rpmsign --addsign "${rpm_path}" 3<<<"${GPG_PASSPHRASE:-}"; then
        log "Signed rpm: ${rpm_path}"
    else
        warn "rpmsign failed for ${rpm_path}; package left unsigned"
    fi
}

build_apt_repo() {
    local apt_root="${OUTPUT_ROOT}/apt"
    local pool_dir="${apt_root}/pool/main/w/wayscriber"
    local dist_rel="dists/${DEB_SUITE}/${DEB_COMPONENT}/binary-${DEB_ARCH}"
    local dist_dir="${apt_root}/${dist_rel}"

    mkdir -p "${pool_dir}" "${dist_dir}"
    cp "${DEB_MAIN}" "${pool_dir}/"
    [[ -f "${DEB_CFG}" ]] && cp "${DEB_CFG}" "${pool_dir}/"
    [[ -f "${OUTPUT_ROOT}/WAYSCRIBER-GPG-KEY.asc" ]] && cp "${OUTPUT_ROOT}/WAYSCRIBER-GPG-KEY.asc" "${apt_root}/"
    [[ -f "${OUTPUT_ROOT}/WAYSCRIBER-GPG-KEY.gpg" ]] && cp "${OUTPUT_ROOT}/WAYSCRIBER-GPG-KEY.gpg" "${apt_root}/"

    pushd "${apt_root}" >/dev/null
    apt-ftparchive packages pool > "${dist_rel}/Packages"
    gzip -kf "${dist_rel}/Packages"

    apt-ftparchive \
        -o APT::FTPArchive::Release::Origin="${REPO_ORIGIN}" \
        -o APT::FTPArchive::Release::Label="${REPO_LABEL}" \
        -o APT::FTPArchive::Release::Suite="${DEB_SUITE}" \
        -o APT::FTPArchive::Release::Codename="${DEB_SUITE}" \
        -o APT::FTPArchive::Release::Architectures="${DEB_ARCH}" \
        -o APT::FTPArchive::Release::Components="${DEB_COMPONENT}" \
        release "dists/${DEB_SUITE}" > "dists/${DEB_SUITE}/Release"

    if [[ -n "${ACTIVE_KEY}" ]]; then
        gpg --batch --yes --pinentry-mode loopback ${GPG_PASSPHRASE:+--passphrase "$GPG_PASSPHRASE"} \
            --local-user "${ACTIVE_KEY}" \
            --clearsign -o "dists/${DEB_SUITE}/InRelease" "dists/${DEB_SUITE}/Release"
        gpg --batch --yes --pinentry-mode loopback ${GPG_PASSPHRASE:+--passphrase "$GPG_PASSPHRASE"} \
            --local-user "${ACTIVE_KEY}" \
            -abs -o "dists/${DEB_SUITE}/Release.gpg" "dists/${DEB_SUITE}/Release"
    else
        warn "Apt Release left unsigned (no GPG key)"
    fi
    popd >/dev/null

    log "Apt repo staged under ${apt_root}"
}

build_rpm_repo() {
    local rpm_root="${OUTPUT_ROOT}/rpm"
    mkdir -p "${rpm_root}"

    cp "${RPM_MAIN}" "${rpm_root}/"
    [[ -f "${RPM_CFG}" ]] && cp "${RPM_CFG}" "${rpm_root}/"
    [[ -f "${OUTPUT_ROOT}/WAYSCRIBER-GPG-KEY.asc" ]] && cp "${OUTPUT_ROOT}/WAYSCRIBER-GPG-KEY.asc" "${rpm_root}/RPM-GPG-KEY-wayscriber.asc"

    if [[ -n "${ACTIVE_KEY}" && "${SIGN_RPMS}" == "1" ]]; then
        for rpm in "${rpm_root}"/*.rpm; do
            sign_rpm "${rpm}"
        done
    else
        warn "RPM signing skipped (no key or SIGN_RPMS=0)"
    fi

    createrepo_c --update "${rpm_root}"

    if [[ -n "${ACTIVE_KEY}" ]]; then
        gpg --batch --yes --pinentry-mode loopback ${GPG_PASSPHRASE:+--passphrase "$GPG_PASSPHRASE"} \
            --local-user "${ACTIVE_KEY}" \
            --detach-sign --armor -o "${rpm_root}/repodata/repomd.xml.asc" "${rpm_root}/repodata/repomd.xml"
    else
        warn "repomd.xml left unsigned (no GPG key)"
    fi

    log "RPM repo staged under ${rpm_root}"
}

setup_gpg
build_apt_repo
build_rpm_repo

log "Repository build complete. Contents:"
find "${OUTPUT_ROOT}" -maxdepth 3 -type f -print
