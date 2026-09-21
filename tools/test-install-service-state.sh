#!/usr/bin/env bash
# Exercise the production shell service probe with a disposable systemctl.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SOURCE="${1:-$ROOT/tools/install.sh}"
FIXTURE="$(mktemp -d)"
trap 'rm -r -- "$FIXTURE"' EXIT
mkdir -p "$FIXTURE/bin"

cat > "$FIXTURE/bin/systemctl" <<'EOF'
#!/usr/bin/env bash
printf '%s\n' "$*" >> "$SERVICE_CALL_LOG"
case "$SERVICE_SCENARIO" in
    no-bus)
        echo 'Failed to connect to user scope bus via local transport: No such file or directory' >&2
        exit 1
        ;;
    query-error)
        echo 'Failed to inspect unit' >&2
        exit 1
        ;;
    *)
        printf '%s\n' "$SERVICE_SCENARIO"
        ;;
esac
EOF
chmod +x "$FIXTURE/bin/systemctl"

die() {
    echo "$*" >&2
    exit 1
}

# The installer executes top-level installation work when sourced. Extract its
# actual probe, keeping every exercised branch identical to production.
eval "$(sed -n '/^service_state() {$/,/^}/p' "$SOURCE")"

expect_state() {
    local scenario="$1" expected="$2" actual
    : > "$FIXTURE/calls"
    actual="$(SERVICE_SCENARIO="$scenario" SERVICE_CALL_LOG="$FIXTURE/calls" PATH="$FIXTURE/bin:$PATH" service_state)"
    [ "$actual" = "$expected" ] || {
        echo "Expected $scenario to yield $expected, got $actual" >&2
        exit 1
    }
    [ "$(wc -l < "$FIXTURE/calls")" -eq 1 ] || {
        echo "Service probe made more than one systemctl call" >&2
        exit 1
    }
    grep -Fxq -- '--user show -p ActiveState --value wayscriber.service' "$FIXTURE/calls"
}

expect_failure() {
    local scenario="$1" expected="$2" output
    : > "$FIXTURE/calls"
    if output="$(SERVICE_SCENARIO="$scenario" SERVICE_CALL_LOG="$FIXTURE/calls" PATH="$FIXTURE/bin:$PATH" service_state 2>&1)"; then
        echo "Expected $scenario to be refused" >&2
        exit 1
    fi
    [[ "$output" == *"$expected"* ]] || {
        echo "Unexpected $scenario error: $output" >&2
        exit 1
    }
}

expect_state no-bus unavailable
expect_state inactive inactive
expect_state failed failed
expect_state active active
expect_failure activating 'retry after it settles'
expect_failure query-error 'Could not inspect wayscriber.service'
echo 'Shell service-state fixture passed.'
