#!/usr/bin/env bash
# Pin the standalone installer's cohort name to the same binary fixture as C#.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
eval "$(sed -n '/^cohort_hash() {$/,/^}/p' "$ROOT/tools/install.sh")"

app="$ROOT/tools/fixtures/install-cohort-app.txt"
broker="$ROOT/tools/fixtures/install-cohort-broker.txt"
expected="$(cat "$ROOT/tools/fixtures/install-cohort-hash.txt")"
actual="$(cohort_hash "$app" "$broker")"
[ "$actual" = "$expected" ] || {
    echo "Shell installer cohort hash: expected $expected, got $actual" >&2
    exit 1
}
[ "$(cohort_hash "$broker" "$app")" != "$expected" ] || {
    echo "Shell installer cohort hash ignored app/broker order" >&2
    exit 1
}
echo 'Shell installer cohort-hash fixture passed.'
