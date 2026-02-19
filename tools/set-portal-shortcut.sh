#!/usr/bin/env bash
set -euo pipefail

SHORTCUT="${1:-<Ctrl><Shift>g}"
APP_ID="${2:-wayscriber}"
DROP_IN_DIR="${HOME}/.config/systemd/user/wayscriber.service.d"
DROP_IN_FILE="${DROP_IN_DIR}/shortcut.conf"

ESCAPED_SHORTCUT="${SHORTCUT//\"/\\\"}"
ESCAPED_APP_ID="${APP_ID//\"/\\\"}"

mkdir -p "${DROP_IN_DIR}"

cat > "${DROP_IN_FILE}" <<EOF
[Service]
Environment="WAYSCRIBER_PORTAL_SHORTCUT=${ESCAPED_SHORTCUT}"
Environment="WAYSCRIBER_PORTAL_APP_ID=${ESCAPED_APP_ID}"
EOF

systemctl --user daemon-reload
systemctl --user restart wayscriber.service

echo "Updated ${DROP_IN_FILE}"
echo "Shortcut set to: ${SHORTCUT}"
echo "Portal app id set to: ${APP_ID}"
echo "wayscriber.service restarted."
