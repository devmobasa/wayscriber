#!/usr/bin/env bash
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
bash "$SCRIPT_DIR/aur-desktop-assets.sh" --check
WORK_DIR="$(mktemp -d)"
trap 'rm -rf "$WORK_DIR"' EXIT
mkdir -p "$WORK_DIR/tools"
cp "$SCRIPT_DIR/aur-desktop-assets.sh" "$SCRIPT_DIR/aur-desktop-assets.py" "$WORK_DIR/tools/"
cp -a "$REPO_ROOT/packaging" "$WORK_DIR/"

# All concurrent standalone callers must receive complete, identical JSON.
pids=()
for run in 1 2 3; do
    bash "$WORK_DIR/tools/aur-desktop-assets.sh" "$WORK_DIR" > "$WORK_DIR/$run.json" 2> "$WORK_DIR/$run.log" &
    pids+=("$!")
done
for index in "${!pids[@]}"; do
    if ! wait "${pids[$index]}"; then
        echo "Concurrent cold asset generation failed for caller $((index + 1))" >&2
        cat "$WORK_DIR/$((index + 1)).log" >&2
        exit 1
    fi
done
for run in 1 2 3; do
    jq -e '.source.lines | length > 0' "$WORK_DIR/$run.json" >/dev/null
    cmp "$WORK_DIR/1.json" "$WORK_DIR/$run.json" || {
        echo "Concurrent asset output differs for caller $run" >&2
        exit 1
    }
done

write_manifest() {
    cat > "$WORK_DIR/packaging/package.wayscriber.yaml" <<EOF
contents:
  - src: packaging/wayscriber.desktop
    dst: /usr/share/applications/wayscriber.desktop
    file_info:
      mode: $1
  - src: packaging/icons/wayscriber.svg
    dst: /usr/share/icons/hicolor/scalable/apps/wayscriber.svg
    file_info:
      mode: 0644
EOF
}

expect_failure() {
    if bash "$WORK_DIR/tools/aur-desktop-assets.sh" "$WORK_DIR" > "$WORK_DIR/failure.json" 2> "$WORK_DIR/failure.log"; then
        echo "Expected asset generator failure: $1" >&2
        exit 1
    fi
    grep -Fq "$1" "$WORK_DIR/failure.log" || {
        echo "Expected diagnostic: $1" >&2
        cat "$WORK_DIR/failure.log" >&2
        exit 1
    }
    [[ ! -s "$WORK_DIR/failure.json" ]] || {
        echo 'Failed generation wrote partial JSON' >&2
        exit 1
    }
}

for mode in 0644 420 0o644; do
    write_manifest "$mode"
    bash "$WORK_DIR/tools/aur-desktop-assets.sh" "$WORK_DIR" > "$WORK_DIR/mode-$mode.json"
    cmp "$WORK_DIR/mode-0644.json" "$WORK_DIR/mode-$mode.json" || {
        echo "Equivalent YAML integer mode $mode changed the recipe" >&2
        exit 1
    }
done
for mode in nonsense 0oBAD 999999999999999999999999 '"0644"' '!!str 0644' 644; do
    write_manifest "$mode"
    expect_failure 'package.wayscriber.yaml: desktop asset /usr/share/applications/wayscriber.desktop must have mode 0644'
done
write_manifest 0644
sed -i '/file_info:/,+1d' "$WORK_DIR/packaging/package.wayscriber.yaml"
expect_failure 'package.wayscriber.yaml: desktop asset /usr/share/applications/wayscriber.desktop has no file_info mapping'
printf '{}\n' > "$WORK_DIR/packaging/package.wayscriber.yaml"
expect_failure 'package.wayscriber.yaml: expected one package contents sequence'

echo 'AUR standalone asset concurrency and parser regression checks passed.'
