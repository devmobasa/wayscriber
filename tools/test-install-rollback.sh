#!/usr/bin/env bash
# Drive the installer's rollback trap with disposable selectors and a fake service.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
FIXTURE="$(mktemp -d)"
sleep 60 &
SERVICE_PID=$!
trap 'kill "$SERVICE_PID" 2>/dev/null || true; wait "$SERVICE_PID" 2>/dev/null || true; rm -r -- "$FIXTURE"' EXIT
mkdir -p "$FIXTURE/bin"

cat > "$FIXTURE/bin/systemctl" <<'EOF'
#!/usr/bin/env bash
printf '%s\n' "$*" >> "$FAKE_SERVICE_CALLS"
case "$*" in
    '--user stop wayscriber.service')
        echo inactive > "$FAKE_SERVICE_STATE"
        ;;
    '--user start wayscriber.service')
        [ "${FAKE_FAIL_START:-0}" -eq 0 ] || exit 1
        echo active > "$FAKE_SERVICE_STATE"
        ;;
    '--user is-active --quiet wayscriber.service')
        [ "$(cat "$FAKE_SERVICE_STATE")" = active ]
        ;;
    '--user show -p MainPID --value wayscriber.service')
        printf '%s\n' "$FAKE_SERVICE_PID"
        ;;
    *)
        echo "Unexpected systemctl call: $*" >&2
        exit 1
        ;;
esac
EOF
chmod +x "$FIXTURE/bin/systemctl"

# The top-level installer cannot be sourced without installing. Exercise its
# actual EXIT-trap function after arranging the state immediately after a new
# selector has been published.
eval "$(sed -n '/^cleanup_install_stage() {$/,/^}/p' "$ROOT/tools/install.sh")"

run_case() {
    local name="$1" old_kind="$2" fail_start="$3" dir old_hash result
    dir="$FIXTURE/$name"
    mkdir -p "$dir/old" "$dir/new"
    cp /usr/bin/sleep "$dir/old/wayscriber"
    cp /usr/bin/true "$dir/new/wayscriber"
    old_hash="$(sha256sum "$dir/old/wayscriber" | cut -d' ' -f1)"
    echo active > "$dir/service-state"
    : > "$dir/service-calls"
    if [ "$old_kind" = symlink ]; then
        ln -s old/wayscriber "$dir/wayscriber"
        PREVIOUS_LINK=old/wayscriber
        PREVIOUS_BACKUP=""
    else
        cp "$dir/old/wayscriber" "$dir/.wayscriber-previous"
        PREVIOUS_LINK=""
        PREVIOUS_BACKUP="$dir/.wayscriber-previous"
    fi
    ln -s new/wayscriber "$dir/.wayscriber-link"
    mv -Tf -- "$dir/.wayscriber-link" "$dir/wayscriber"

    if (
        export PATH="$FIXTURE/bin:$PATH"
        export FAKE_SERVICE_STATE="$dir/service-state"
        export FAKE_SERVICE_CALLS="$dir/service-calls"
        export FAKE_SERVICE_PID="$SERVICE_PID"
        export FAKE_FAIL_START="$fail_start"
        INSTALL_DIR="$dir"
        INSTALLED_BINARY="$dir/wayscriber"
        STAGE_DIR=""
        LINK_STAGE=""
        ROLLBACK_LINK=""
        PREVIOUS_HASH="$old_hash"
        SELECTOR_PENDING=1
        SERVICE_WAS_ACTIVE=1
        SUDO=""
        cleanup_install_stage
    ) > "$dir/rollback.log" 2>&1; then
        result=0
    else
        result=$?
    fi

    [ "$(sha256sum "$dir/wayscriber" | cut -d' ' -f1)" = "$old_hash" ]
    [ -f "$dir/new/wayscriber" ]
    if [ "$old_kind" = symlink ]; then
        [ "$(readlink "$dir/wayscriber")" = old/wayscriber ]
    else
        [ ! -L "$dir/wayscriber" ]
    fi
    if [ "$fail_start" -eq 0 ]; then
        [ "$result" -eq 0 ]
        [ "$(cat "$dir/service-state")" = active ]
    else
        [ "$result" -ne 0 ]
        grep -Fq 'Automatic rollback failed' "$dir/rollback.log"
        [ "$(cat "$dir/service-state")" = inactive ]
    fi
    grep -Fxq -- '--user stop wayscriber.service' "$dir/service-calls"
    grep -Fxq -- '--user start wayscriber.service' "$dir/service-calls"
}

run_case old-link symlink 0
run_case old-file regular 0
run_case failed-restart symlink 1
echo 'Shell installer rollback fixture passed.'
