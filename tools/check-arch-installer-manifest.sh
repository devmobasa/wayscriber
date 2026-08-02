#!/usr/bin/env bash

set -euo pipefail

usage() {
    cat <<'EOF'
Usage: check-arch-installer-manifest.sh --installer FILE --archive FILE

Compare the statically declared manifest in arch-install.sh with a packaged
Wayscriber release archive. The installer is parsed as data and is not run.
EOF
}

die() {
    echo "error: $*" >&2
    exit 1
}

INSTALLER=""
ARCHIVE=""

while [[ $# -gt 0 ]]; do
    case "$1" in
        --installer)
            [[ $# -ge 2 ]] || die "--installer requires a file"
            INSTALLER="$2"
            shift 2
            ;;
        --archive)
            [[ $# -ge 2 ]] || die "--archive requires a file"
            ARCHIVE="$2"
            shift 2
            ;;
        --help|-h)
            usage
            exit 0
            ;;
        *)
            die "unknown argument: $1"
            ;;
    esac
done

[[ -n "$INSTALLER" ]] || die "--installer is required"
[[ -n "$ARCHIVE" ]] || die "--archive is required"
[[ -f "$INSTALLER" ]] || die "installer not found: $INSTALLER"
[[ -f "$ARCHIVE" ]] || die "archive not found: $ARCHIVE"

for command_name in awk cut diff find grep mkdir mktemp rm sort stat tar uniq; do
    command -v "$command_name" >/dev/null 2>&1 \
        || die "$command_name is required"
done

WORK_DIR="$(mktemp -d "${TMPDIR:-/tmp}/wayscriber-arch-manifest.XXXXXX")"
trap 'rm -rf "$WORK_DIR"' EXIT

BEGIN_MARKER="# ARCH_INSTALL_MANIFEST_BEGIN"
END_MARKER="# ARCH_INSTALL_MANIFEST_END"
MANIFEST="$WORK_DIR/manifest.txt"

BEGIN_COUNT="$(awk -v marker="$BEGIN_MARKER" '$0 == marker { count++ } END { print count + 0 }' "$INSTALLER")"
END_COUNT="$(awk -v marker="$END_MARKER" '$0 == marker { count++ } END { print count + 0 }' "$INSTALLER")"
[[ "$BEGIN_COUNT" == "1" && "$END_COUNT" == "1" ]] \
    || die "installer must contain one static manifest block"

if ! awk -v begin="$BEGIN_MARKER" -v end="$END_MARKER" '
    function trim(value) {
        sub(/^[[:space:]]*/, "", value)
        sub(/[[:space:]]*$/, "", value)
        return value
    }

    function reject() {
        printf "unsupported installer manifest syntax at line %d: %s\n", NR, $0 > "/dev/stderr"
        rejected = 1
        exit 1
    }

    BEGIN {
        quote = sprintf("%c", 39)
        expected_printf = "printf " quote "%s\\n" quote " \\"
        state = "outside"
    }

    $0 == begin {
        if (state != "outside") reject()
        state = "function"
        next
    }

    $0 == end {
        if (state != "end") reject()
        state = "done"
        next
    }

    state == "outside" || state == "done" { next }

    state == "function" {
        if (trim($0) != "release_manifest() {") reject()
        state = "printf"
        next
    }

    state == "printf" {
        if (trim($0) != expected_printf) reject()
        state = "entry"
        next
    }

    state == "entry" {
        line = trim($0)
        continued = (line ~ /\\$/)
        if (continued) {
            sub(/[[:space:]]*\\$/, "", line)
            line = trim(line)
        }
        if (substr(line, 1, 1) != quote || substr(line, length(line), 1) != quote) reject()
        line = substr(line, 2, length(line) - 2)
        if (line !~ /^[0-7][0-7][0-7][0-7] [-A-Za-z0-9._\/+]+$/) reject()
        print line
        entries++
        if (!continued) state = "close"
        next
    }

    state == "close" {
        if (trim($0) != "}") reject()
        state = "end"
        next
    }

    { reject() }

    END {
        if (rejected) exit 1
        if (state != "done" || entries == 0) {
            print "installer manifest block is incomplete" > "/dev/stderr"
            exit 1
        }
    }
' "$INSTALLER" > "$MANIFEST"; then
    die "installer manifest block is malformed or uses unsupported syntax"
fi

[[ -s "$MANIFEST" ]] || die "installer manifest is empty or malformed"

while IFS=' ' read -r mode relative_path; do
    [[ "$mode" =~ ^0[0-7]{3}$ ]] \
        || die "invalid manifest mode: $mode"
    [[ "$relative_path" =~ ^[-A-Za-z0-9._/+]+$ ]] \
        || die "invalid manifest path: $relative_path"
    [[ "$relative_path" != /* && "/$relative_path/" != *"/../"* ]] \
        || die "unsafe manifest path: $relative_path"
done < "$MANIFEST"

cut -d' ' -f2 "$MANIFEST" | LC_ALL=C sort > "$WORK_DIR/expected-paths.txt"
DUPLICATE_PATHS="$(uniq -d "$WORK_DIR/expected-paths.txt")"
[[ -z "$DUPLICATE_PATHS" ]] \
    || die "installer manifest contains duplicate paths: $DUPLICATE_PATHS"

tar -tzf "$ARCHIVE" > "$WORK_DIR/archive-paths.txt"
ARCHIVE_ROOT="$(awk -F/ 'NF > 0 && $1 != "" { print $1; exit }' "$WORK_DIR/archive-paths.txt")"
[[ "$ARCHIVE_ROOT" =~ ^wayscriber-v[0-9]+\.[0-9]+\.[0-9]+(\.[0-9]+)?-linux-x86_64$ ]] \
    || die "archive has an unexpected top-level directory: ${ARCHIVE_ROOT:-<none>}"

awk -v root="$ARCHIVE_ROOT" '
    {
        expected_root = ($0 == root || $0 == root "/" ||
            $0 == root "/usr" || $0 == root "/usr/" ||
            index($0, root "/usr/") == 1)
        unsafe = ($0 ~ /^\// || $0 ~ /(^|\/)\.\.(\/|$)/)
        if (!expected_root || unsafe) bad = 1
    }
    END { exit bad }
' "$WORK_DIR/archive-paths.txt" \
    || die "archive contains an unexpected or unsafe path"

mkdir -p "$WORK_DIR/stage"
tar -xzf "$ARCHIVE" -C "$WORK_DIR/stage"
STAGED_ROOT="$WORK_DIR/stage/$ARCHIVE_ROOT"
[[ -d "$STAGED_ROOT/usr" ]] || die "archive does not contain usr/"

if (
    cd "$STAGED_ROOT"
    LC_ALL=C find . -mindepth 1 ! -regex '\./[-A-Za-z0-9._/+]*' -print -quit | grep -q .
); then
    die "archive contains a path with unsupported characters"
fi
find "$STAGED_ROOT" ! -type d ! -type f -print -quit | grep -q . \
    && die "archive contains a symbolic link or special file"
find "$STAGED_ROOT" -type f -links +1 -print -quit | grep -q . \
    && die "archive contains a hard-linked file"

find "$STAGED_ROOT/usr" -type f -printf '%P\n' | LC_ALL=C sort \
    > "$WORK_DIR/archive-file-paths.txt"

if ! diff -u "$WORK_DIR/expected-paths.txt" "$WORK_DIR/archive-file-paths.txt"; then
    die "installer manifest and release archive file paths differ"
fi

while IFS=' ' read -r expected_mode relative_path; do
    actual_mode="$(stat -c '%a' "$STAGED_ROOT/usr/$relative_path")"
    [[ "$actual_mode" == "${expected_mode#0}" ]] \
        || die "mode mismatch for usr/$relative_path: expected $expected_mode, found $actual_mode"
done < "$MANIFEST"

SERVICE_FILE="$STAGED_ROOT/usr/lib/systemd/user/wayscriber.service"
[[ -f "$SERVICE_FILE" ]] || die "archive does not contain the Wayscriber user service"
awk '
    $0 == "ExecStart=/usr/bin/wayscriber --daemon" ||
    $0 == "ExecStart=\"/usr/bin/wayscriber\" --daemon" {
        matches++
        next
    }
    index($0, "/usr/bin/wayscriber") { unexpected = 1 }
    END { exit !(matches == 1 && !unexpected) }
' "$SERVICE_FILE" \
    || die "release user service is incompatible with the direct installer rewrite"

echo "Arch installer manifest matches ${ARCHIVE##*/}."
