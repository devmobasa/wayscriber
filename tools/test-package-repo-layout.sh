#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
WORK_DIR="$(mktemp -d)"

cleanup() {
    rm -rf "${WORK_DIR}"
}
trap cleanup EXIT

ARTIFACT_ROOT="${WORK_DIR}/dist"
OUTPUT_ROOT="${WORK_DIR}/repo-out"
FAKE_BIN="${WORK_DIR}/bin"

mkdir -p "${ARTIFACT_ROOT}" "${OUTPUT_ROOT}" "${FAKE_BIN}"
touch "${ARTIFACT_ROOT}/wayscriber-amd64.deb"
touch "${ARTIFACT_ROOT}/wayscriber-x86_64.rpm"
printf '%s\n' 'fake public key' > "${OUTPUT_ROOT}/WAYSCRIBER-GPG-KEY.asc"

cat > "${FAKE_BIN}/apt-ftparchive" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
printf '%s\n' "fake apt-ftparchive output"
EOF
chmod +x "${FAKE_BIN}/apt-ftparchive"

cat > "${FAKE_BIN}/createrepo_c" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
repo_root=""
for arg in "$@"; do
    repo_root="${arg}"
done
mkdir -p "${repo_root}/repodata"
printf '%s\n' '<repomd />' > "${repo_root}/repodata/repomd.xml"
EOF
chmod +x "${FAKE_BIN}/createrepo_c"

PATH="${FAKE_BIN}:${PATH}" \
ARTIFACT_ROOT="${ARTIFACT_ROOT}" \
OUTPUT_ROOT="${OUTPUT_ROOT}" \
SIGN_RPMS=0 \
    bash "${REPO_ROOT}/tools/build-package-repos.sh"

cmp "${OUTPUT_ROOT}/WAYSCRIBER-GPG-KEY.asc" "${OUTPUT_ROOT}/rpm/RPM-GPG-KEY-wayscriber.asc"
cmp "${OUTPUT_ROOT}/WAYSCRIBER-GPG-KEY.asc" "${OUTPUT_ROOT}/rpm/RPM-GPG-KEY-wayscriber"
