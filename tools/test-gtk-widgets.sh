#!/usr/bin/env bash
# A private headless compositor: never connects to the user's desktop/display.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."
command -v weston >/dev/null
GTK_TEST_RUNTIME=$(mktemp -d)
chmod 700 "$GTK_TEST_RUNTIME"
cleanup() {
    if [[ -n "${GTK_TEST_WESTON_PID:-}" ]]; then
        kill "$GTK_TEST_WESTON_PID" 2>/dev/null || true
        wait "$GTK_TEST_WESTON_PID" 2>/dev/null || true
    fi
    rm -rf "$GTK_TEST_RUNTIME"
}
trap cleanup EXIT
export XDG_RUNTIME_DIR="$GTK_TEST_RUNTIME"
export WAYLAND_DISPLAY=wayscriber-widget-tests
unset DISPLAY
export GDK_BACKEND=wayland
export GTK_A11Y=test
export WAYSCRIBER_REQUIRE_GTK_TESTS=1
weston --backend=headless-backend.so --renderer=pixman --no-config \
    --socket="$WAYLAND_DISPLAY" --idle-time=0 \
    --log="$GTK_TEST_RUNTIME/weston.log" >"$GTK_TEST_RUNTIME/weston-output.log" 2>&1 &
GTK_TEST_WESTON_PID=$!
for ((attempt=0; attempt<100; attempt++)); do
    [[ -S "$XDG_RUNTIME_DIR/$WAYLAND_DISPLAY" ]] && break
    if ! kill -0 "$GTK_TEST_WESTON_PID" 2>/dev/null; then
        cat "$GTK_TEST_RUNTIME"/*.log
        exit 1
    fi
    sleep 0.1
done
[[ -S "$XDG_RUNTIME_DIR/$WAYLAND_DISPLAY" ]] || { cat "$GTK_TEST_RUNTIME"/*.log; exit 1; }
dbus-run-session -- cargo test --locked -p wayscriber --all-features --lib \
    toolbar_gtk:: -- --test-threads=1 --nocapture 2>&1 | tee "$GTK_TEST_RUNTIME/tests.log"
grep -Fq 'EXECUTED: GTK focus and slider assertions' "$GTK_TEST_RUNTIME/tests.log"
grep -Fq 'EXECUTED: GTK widget contract assertions' "$GTK_TEST_RUNTIME/tests.log"
