#!/usr/bin/env bash
# Build-and-package helper to keep local and CI artifacts identical.
# Produces tar/deb/rpm plus a manifest with SHA256 and sizes.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

ARTIFACT_ROOT="${ARTIFACT_ROOT:-${REPO_ROOT}/dist}"
FORMATS="${FORMATS:-tar,deb,rpm}"
VERSION="${VERSION:-}"
NFPM_CONFIG_MAIN="${NFPM_CONFIG_MAIN:-${REPO_ROOT}/packaging/package.wayscriber.yaml}"
NFPM_CONFIG_CONFIG="${NFPM_CONFIG_CONFIG:-${REPO_ROOT}/packaging/package.configurator.yaml}"
PACKAGE_CONFIGURATOR="${PACKAGE_CONFIGURATOR:-1}"
STRIP_BINARIES="${STRIP_BINARIES:-1}"
SKIP_BUILD="${SKIP_BUILD:-0}"

usage() {
    cat <<'EOF'
package.sh
Builds release binaries and packages them into tar/deb/rpm with a manifest.

Environment/flags:
  --version <ver>          Override crate version (defaults to cargo metadata)
  --formats <list>         Comma list of outputs (tar,deb,rpm) [tar,deb,rpm]
  --artifact-root <path>   Output directory [./dist]
  --strip                  Strip binaries (default)
  --no-strip               Do not strip binaries
  --skip-build             Reuse existing target/ (no cargo build)
  --no-configurator        Skip building/packaging configurator artifacts
EOF
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --version) VERSION="$2"; shift 2 ;;
        --formats) FORMATS="$2"; shift 2 ;;
        --artifact-root) ARTIFACT_ROOT="$2"; shift 2 ;;
        --strip) STRIP_BINARIES=1; shift ;;
        --no-strip) STRIP_BINARIES=0; shift ;;
        --skip-build) SKIP_BUILD=1; shift ;;
        --no-configurator) PACKAGE_CONFIGURATOR=0; shift ;;
        -h|--help) usage; exit 0 ;;
        *) echo "Unknown arg: $1" >&2; usage; exit 1 ;;
    esac
done

need_cmd() {
    command -v "$1" >/dev/null 2>&1 || { echo "Missing required command: $1" >&2; exit 1; }
}

need_cmd cargo
need_cmd tar
need_cmd sha256sum
need_cmd awk
need_cmd jq
if [[ "${FORMATS}" == *deb* || "${FORMATS}" == *rpm* ]]; then
    need_cmd nfpm
    [[ -f "${NFPM_CONFIG_MAIN}" ]] || { echo "Missing nfpm config ${NFPM_CONFIG_MAIN}" >&2; exit 1; }
    if [[ "${PACKAGE_CONFIGURATOR}" == 1 ]]; then
        [[ -f "${NFPM_CONFIG_CONFIG}" ]] || { echo "Missing nfpm config ${NFPM_CONFIG_CONFIG}" >&2; exit 1; }
    fi
fi

if [[ -z "$VERSION" ]]; then
    VERSION="$(cargo metadata --no-deps --format-version 1 \
        | jq -r '.packages[] | select(.name=="wayscriber") | .version' \
        | head -n1)"
fi

mkdir -p "${ARTIFACT_ROOT}"

info() { printf '\033[0;32m[INFO]\033[0m %s\n' "$*"; }
warn() { printf '\033[1;33m[WARN]\033[0m %s\n' "$*"; }

build_binaries() {
    [[ "$SKIP_BUILD" == 1 ]] && { warn "Skipping cargo build (SKIP_BUILD=1)"; return; }
    info "Building release binaries (locked, frozen)"
    (cd "$REPO_ROOT" && cargo build --locked --frozen --release --bins)
    if [[ -d "${REPO_ROOT}/configurator" ]]; then
        (cd "$REPO_ROOT" && cargo build --locked --frozen --release --bins --manifest-path configurator/Cargo.toml)
    fi
}

strip_binaries() {
    [[ "$STRIP_BINARIES" == 1 ]] || { warn "Skipping strip (STRIP_BINARIES=0)"; return; }
    if command -v strip >/dev/null 2>&1; then
        info "Stripping binaries"
        strip "${REPO_ROOT}/target/release/wayscriber" || warn "strip failed for wayscriber"
        if [[ -f "${REPO_ROOT}/target/release/wayscriber-configurator" ]]; then
            strip "${REPO_ROOT}/target/release/wayscriber-configurator" || warn "strip failed for configurator"
        fi
    else
        warn "'strip' not found; binaries will remain unstripped"
    fi
}

package_tar() {
    local dist_dir="${ARTIFACT_ROOT}/wayscriber-v${VERSION}-linux-x86_64"
    local tarball="${ARTIFACT_ROOT}/wayscriber-v${VERSION}-linux-x86_64.tar.gz"

    info "Packaging tarball -> ${tarball}"
    rm -rf "${dist_dir}" "${tarball}"
    mkdir -p "${dist_dir}/usr/bin" \
             "${dist_dir}/usr/lib/systemd/user" \
             "${dist_dir}/usr/share/doc/wayscriber"

    cp "${REPO_ROOT}/target/release/wayscriber" "${dist_dir}/usr/bin/"
    cp "${REPO_ROOT}/packaging/wayscriber.service" "${dist_dir}/usr/lib/systemd/user/"
    cp "${REPO_ROOT}/README.md" "${REPO_ROOT}/config.example.toml" "${dist_dir}/usr/share/doc/wayscriber/"
    [[ -f "${REPO_ROOT}/LICENSE" ]] && cp "${REPO_ROOT}/LICENSE" "${dist_dir}/usr/share/doc/wayscriber/" || true

    tar -C "${ARTIFACT_ROOT}" -czf "${tarball}" "$(basename "${dist_dir}")"
    echo "${tarball}"
}

package_tar_configurator() {
    local dist_dir="${ARTIFACT_ROOT}/wayscriber-configurator-v${VERSION}-linux-x86_64"
    local tarball="${ARTIFACT_ROOT}/wayscriber-configurator-v${VERSION}-linux-x86_64.tar.gz"

    if [[ ! -f "${REPO_ROOT}/target/release/wayscriber-configurator" ]]; then
        warn "Configurator binary not found, skipping configurator tarball"
        return
    fi

    info "Packaging configurator tarball -> ${tarball}"
    rm -rf "${dist_dir}" "${tarball}"
    mkdir -p "${dist_dir}/usr/bin" \
             "${dist_dir}/usr/share/doc/wayscriber-configurator"

    cp "${REPO_ROOT}/target/release/wayscriber-configurator" "${dist_dir}/usr/bin/"
    cp "${REPO_ROOT}/README.md" "${dist_dir}/usr/share/doc/wayscriber-configurator/"
    [[ -f "${REPO_ROOT}/LICENSE" ]] && cp "${REPO_ROOT}/LICENSE" "${dist_dir}/usr/share/doc/wayscriber-configurator/" || true

    tar -C "${ARTIFACT_ROOT}" -czf "${tarball}" "$(basename "${dist_dir}")"
    echo "${tarball}"
}

package_nfpm_main() {
    local fmt="$1"
    local target=""
    if [[ "${fmt}" == "deb" ]]; then
        target="${ARTIFACT_ROOT}/wayscriber-amd64.deb"
    elif [[ "${fmt}" == "rpm" ]]; then
        target="${ARTIFACT_ROOT}/wayscriber-x86_64.rpm"
    else
        warn "Unknown nfpm format '${fmt}' for main package"
        return
    fi
    info "Building ${fmt} (wayscriber) via nfpm -> ${target}"
    rm -f "${target}"
    nfpm pkg --packager "${fmt}" --config "${NFPM_CONFIG_MAIN}" --target "${target}"
    [[ -f "${target}" ]] && echo "${target}"
}

package_nfpm_configurator() {
    local fmt="$1"
    local target=""
    if [[ "${fmt}" == "deb" ]]; then
        target="${ARTIFACT_ROOT}/wayscriber-configurator-amd64.deb"
    elif [[ "${fmt}" == "rpm" ]]; then
        target="${ARTIFACT_ROOT}/wayscriber-configurator-x86_64.rpm"
    else
        warn "Unknown nfpm format '${fmt}' for configurator package"
        return
    fi
    info "Building ${fmt} (wayscriber-configurator) via nfpm -> ${target}"
    rm -f "${target}"
    nfpm pkg --packager "${fmt}" --config "${NFPM_CONFIG_CONFIG}" --target "${target}"
    [[ -f "${target}" ]] && echo "${target}"
}

artifacts=()
add_artifact() {
    local path="$1"
    [[ -f "$path" ]] || return
    artifacts+=("$path")
}

emit_manifest() {
    local manifest_txt="${ARTIFACT_ROOT}/checksums.txt"
    local manifest_json="${ARTIFACT_ROOT}/manifest.json"

    : > "${manifest_txt}"
    local json_entries=()

    for path in "${artifacts[@]}"; do
        local sha size name
        name="$(basename "$path")"
        sha="$(sha256sum "$path" | awk '{print $1}')"
        size="$(stat -c%s "$path")"
        printf "%s  %s\n" "$sha" "$name" >> "${manifest_txt}"
        json_entries+=("{\"name\":\"${name}\",\"sha256\":\"${sha}\",\"size\":${size}}")
    done

    cat > "${manifest_json}" <<EOF
{
  "version": "${VERSION}",
  "artifacts": [
    $(IFS=,; echo "${json_entries[*]}")
  ]
}
EOF
    info "Wrote manifest: ${manifest_txt} and ${manifest_json}"
}

build_binaries
strip_binaries

IFS=',' read -r -a fmt_arr <<< "${FORMATS}"
for fmt in "${fmt_arr[@]}"; do
    case "$fmt" in
        tar)
            add_artifact "$(package_tar)"
            if [[ "${PACKAGE_CONFIGURATOR}" == 1 ]]; then
                cfg_tar="$(package_tar_configurator || true)"
                [[ -n "$cfg_tar" ]] && add_artifact "$cfg_tar"
            fi
            ;;
        deb|rpm)
            add_artifact "$(package_nfpm_main "$fmt")"
            if [[ "${PACKAGE_CONFIGURATOR}" == 1 ]]; then
                cfg_pkg="$(package_nfpm_configurator "$fmt" || true)"
                [[ -n "$cfg_pkg" ]] && add_artifact "$cfg_pkg"
            fi
            ;;
        "")
            ;;
        *)
            warn "Unknown format '${fmt}', skipping"
            ;;
    esac
done

emit_manifest
