#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
cd "$REPO_ROOT"

run_check() {
    printf '\nRunning:'
    printf ' %s' "$@"
    printf '\n'
    "$@"
}

run_check bash tools/check-version-consistency.sh
run_check bash tools/test-package-repo-layout.sh
run_check bash tools/test-release-packaging.sh
run_check bash tools/test-aur-desktop-assets.sh

if command -v dotnet >/dev/null 2>&1 && dotnet --version >/dev/null 2>&1; then
    run_check dotnet build tools/wayscriber.cs --disable-build-servers --verbosity quiet
    run_check dotnet build tools/install.cs --disable-build-servers --verbosity quiet
    run_check dotnet build tools/wayscriber.tests.cs --disable-build-servers --verbosity quiet
    for file_app in tools/wayscriber.cs tools/install.cs tools/wayscriber.tests.cs; do
        run_check dotnet format style "$file_app" --no-restore --verify-no-changes
        run_check dotnet format whitespace "$file_app" --no-restore --verify-no-changes
    done
    run_check dotnet run tools/wayscriber.tests.cs --no-build --verbosity quiet
else
    printf '\nSkipping C# repository-tool checks: the SDK selected by global.json is unavailable.\n'
fi
run_check ./tools/check-nixpkgs-recipe.py
run_check ./tools/check-rust-source-coverage.py
run_check ./tools/check-process-sites.py
run_check ./tools/check-config-writers.py
run_check ./tools/check-shared-dependencies.py
run_check cargo fmt --all -- --check
run_check cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
run_check cargo build --locked --workspace --all-features --bins
# Parallel render tests have crashed in the native font stack from context-menu,
# board-picker, and region-capture paths. Serialize the harness while that issue
# remains unresolved; tests that create their own threads still exercise them.
run_check cargo test --locked --workspace --all-features -- --test-threads=1
# Keep these regressions covered in separate processes: the parallel font tests
# can trigger an upstream Cairo/FreeType race. This does not fix that race.
run_isolated_render_test() {
    local output status=0
    local test_name="$2"
    output="$(mktemp)"
    run_check cargo test --locked -p wayscriber "$1" --lib "$test_name" -- --exact --ignored --test-threads=1 \
        | tee "$output" || status=$?
    if [[ "$status" -eq 0 ]] && ! grep -Fq 'test result: ok. 1 passed; 0 failed;' "$output"; then
        echo "Expected exactly one passing isolated render test: $test_name ($1)" >&2
        status=1
    fi
    rm -f "$output"
    return "$status"
}
isolated_render_tests=(
    ui::context_menu::engine_tests::retained_context_menu_owner_preserves_layout_pixels_and_row_hits
    ui::board_picker::tests::retained_board_text_owner_matches_fresh_during_unicode_rename_and_small_layouts
)
for test_name in "${isolated_render_tests[@]}"; do
    run_isolated_render_test --all-features "$test_name"
done
# Linted as strictly as the all-features build: code reachable only behind an
# optional feature leaves its callers dead without it, and building alone does
# not promote that to an error.
run_check cargo clippy --locked --workspace --all-targets --no-default-features -- -D warnings
run_check cargo build --locked --workspace --no-default-features --bins
run_check cargo test --locked --workspace --no-default-features -- --test-threads=1
for test_name in "${isolated_render_tests[@]}"; do
    run_isolated_render_test --no-default-features "$test_name"
done
