#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
REPO_ROOT=$(cd -- "$SCRIPT_DIR/.." && pwd)

if command -v python3 >/dev/null 2>&1; then
    PYTHON=python3
elif command -v python >/dev/null 2>&1; then
    PYTHON=python
else
    echo "report=wayscriber-code-health"
    echo "status=error"
    echo "error=python_not_found"
    exit 0
fi

set +e
"$PYTHON" - "$REPO_ROOT" <<'PY'
from __future__ import annotations

import re
import subprocess
import sys
from pathlib import Path

repo_root = Path(sys.argv[1]).resolve()


def run_git_ls_files() -> tuple[list[Path], str | None, str | None]:
    try:
        result = subprocess.run(
            ["git", "-C", str(repo_root), "ls-files", "-co", "--exclude-standard", "--", "*.rs"],
            check=False,
            capture_output=True,
            text=True,
        )
    except OSError as exc:
        return [], f"git_unavailable\t{exc}", None
    stderr = result.stderr.strip()
    if result.returncode != 0:
        return [], f"git_ls_files_failed\t{stderr}", None
    warning = f"git_ls_files_stderr\t{stderr}" if stderr else None
    files: list[Path] = []
    seen: set[str] = set()
    for line in result.stdout.splitlines():
        if not line:
            continue
        relative_path = Path(line)
        if not (repo_root / relative_path).is_file():
            continue
        path_key = relative_path.as_posix()
        if path_key in seen:
            continue
        seen.add(path_key)
        files.append(relative_path)
    return files, None, warning


def read_text(relative_path: Path) -> str:
    return (repo_root / relative_path).read_text(encoding="utf-8", errors="replace")


def physical_line_count(text: str) -> int:
    if not text:
        return 0
    return text.count("\n") + (0 if text.endswith("\n") else 1)


def is_test_path(relative_path: Path) -> bool:
    parts = relative_path.as_posix().split("/")
    name = relative_path.name
    return (
        "tests" in parts
        or name in {"tests.rs", "test_helpers.rs", "test_support.rs"}
        or name.startswith("test_")
        or name.endswith("_tests.rs")
    )


def scrub_rust_code(text: str) -> str:
    output: list[str] = []
    index = 0
    length = len(text)
    state = "code"
    block_depth = 0
    raw_hashes = ""

    def blank(char: str) -> str:
        return "\n" if char == "\n" else " "

    while index < length:
        char = text[index]
        next_char = text[index + 1] if index + 1 < length else ""

        if state == "code":
            if char == "/" and next_char == "/":
                output.extend("  ")
                index += 2
                state = "line_comment"
                continue
            if char == "/" and next_char == "*":
                output.extend("  ")
                index += 2
                state = "block_comment"
                block_depth = 1
                continue
            if char == '"':
                output.append(" ")
                index += 1
                state = "string"
                continue
            if char == "r" or (char == "b" and next_char == "r"):
                raw_start = index + (2 if char == "b" else 1)
                hash_end = raw_start
                while hash_end < length and text[hash_end] == "#":
                    hash_end += 1
                if hash_end < length and text[hash_end] == '"':
                    output.extend(" " * (hash_end - index + 1))
                    index = hash_end + 1
                    raw_hashes = text[raw_start:hash_end]
                    state = "raw_string"
                    continue
            output.append(char)
            index += 1
            continue

        if state == "line_comment":
            output.append(blank(char))
            index += 1
            if char == "\n":
                state = "code"
            continue

        if state == "block_comment":
            if char == "/" and next_char == "*":
                output.extend("  ")
                index += 2
                block_depth += 1
                continue
            if char == "*" and next_char == "/":
                output.extend("  ")
                index += 2
                block_depth -= 1
                if block_depth == 0:
                    state = "code"
                continue
            output.append(blank(char))
            index += 1
            continue

        if state == "string":
            if char == "\\" and next_char:
                output.append(blank(char))
                output.append(blank(next_char))
                index += 2
                continue
            output.append(blank(char))
            index += 1
            if char == '"':
                state = "code"
            continue

        if state == "raw_string":
            output.append(blank(char))
            index += 1
            if char == '"' and text.startswith(raw_hashes, index):
                output.extend(" " * len(raw_hashes))
                index += len(raw_hashes)
                state = "code"
            continue

    return "".join(output)


cfg_attr_start_pattern = re.compile(r"#\s*\[\s*cfg\s*\(", re.MULTILINE)


def preserve_newlines_as_spaces(text: str) -> str:
    return "".join("\n" if char == "\n" else " " for char in text)


def find_attribute_end(code: str, start: int) -> int:
    index = start
    bracket_depth = 0
    while index < len(code):
        char = code[index]
        if char == "[":
            bracket_depth += 1
        elif char == "]":
            bracket_depth -= 1
            if bracket_depth == 0:
                return index + 1
        index += 1
    return start


def skip_attributes_and_whitespace(code: str, start: int) -> int:
    index = start
    while index < len(code):
        while index < len(code) and code[index].isspace():
            index += 1
        if code.startswith("#[", index):
            next_index = find_attribute_end(code, index + 1)
            if next_index <= index:
                return index
            index = next_index
            continue
        return index
    return index


def is_cfg_test_only_attribute(attribute: str) -> bool:
    compact = re.sub(r"\s+", "", attribute)
    if compact == "#[cfg(test)]":
        return True
    if "not(test)" in compact:
        return False
    return compact.startswith("#[cfg(all(") and re.search(r"\btest\b", attribute) is not None


def find_item_end(code: str, start: int) -> int:
    paren_depth = 0
    bracket_depth = 0
    index = start
    while index < len(code):
        char = code[index]
        if char == "(":
            paren_depth += 1
        elif char == ")" and paren_depth > 0:
            paren_depth -= 1
        elif char == "[":
            bracket_depth += 1
        elif char == "]" and bracket_depth > 0:
            bracket_depth -= 1
        elif char == ";" and paren_depth == 0 and bracket_depth == 0:
            return index + 1
        elif char == "{" and paren_depth == 0 and bracket_depth == 0:
            body_end = find_matching_brace(code, index)
            return len(code) if body_end is None else body_end + 1
        index += 1
    return len(code)


def strip_cfg_test_code(code: str) -> str:
    chars = list(code)
    for match in cfg_attr_start_pattern.finditer(code):
        attribute_end = find_attribute_end(code, match.start() + 1)
        attribute = code[match.start() : attribute_end]
        if not is_cfg_test_only_attribute(attribute):
            continue
        item_start = skip_attributes_and_whitespace(code, attribute_end)
        item_end = find_item_end(code, item_start)
        replacement = preserve_newlines_as_spaces(code[match.start() : item_end])
        chars[match.start() : item_end] = replacement
    return "".join(chars)


fn_name_pattern = re.compile(
    r"\bfn\s+([A-Za-z_][A-Za-z0-9_]*)\s*(?:<[^>{;]*>)?\s*\(",
    re.MULTILINE,
)


def line_number_at(text: str, index: int) -> int:
    return text.count("\n", 0, index) + 1


def find_function_body(code: str, start: int) -> int | None:
    paren_depth = 1
    bracket_depth = 0
    index = start
    while index < len(code):
        char = code[index]
        if char == "(":
            paren_depth += 1
        elif char == ")" and paren_depth > 0:
            paren_depth -= 1
        elif char == "[":
            bracket_depth += 1
        elif char == "]" and bracket_depth > 0:
            bracket_depth -= 1
        elif char == "{" and paren_depth == 0 and bracket_depth == 0:
            return index
        elif char == ";" and paren_depth == 0 and bracket_depth == 0:
            return None
        index += 1
    return None


def find_matching_brace(code: str, body_start: int) -> int | None:
    depth = 0
    for index in range(body_start, len(code)):
        char = code[index]
        if char == "{":
            depth += 1
        elif char == "}":
            depth -= 1
            if depth == 0:
                return index
    return None


def long_functions(relative_path: Path, code: str) -> list[tuple[int, int, str, str]]:
    findings: list[tuple[int, int, str, str]] = []
    for match in fn_name_pattern.finditer(code):
        body_start = find_function_body(code, match.end())
        if body_start is None:
            continue
        body_end = find_matching_brace(code, body_start)
        if body_end is None:
            continue
        start_line = line_number_at(code, match.start())
        end_line = line_number_at(code, body_end)
        lines = end_line - start_line + 1
        if lines > 120:
            findings.append((lines, start_line, relative_path.as_posix(), match.group(1)))
    return findings


prod_patterns = {
    "unwrap": re.compile(r"\.\s*unwrap\s*\("),
    "expect": re.compile(r"\.\s*expect\s*\("),
    "panic": re.compile(r"\bpanic\s*!"),
    "unsafe": re.compile(r"\bunsafe\b"),
}
allow_dead_code_pattern = re.compile(r"#\s*\[\s*allow\s*\([^)]*\bdead_code\b[^)]*\)\s*\]")
allow_unused_imports_pattern = re.compile(
    r"#\s*\[\s*allow\s*\([^)]*\bunused_imports\b[^)]*\)\s*\]"
)
direct_fs_write_pattern = re.compile(r"(?<![\w:])(?:std::fs::write|fs::write)\s*\(")


rust_files, discovery_error, discovery_warning = run_git_ls_files()
read_errors: list[str] = []
file_lines: list[tuple[int, str]] = []
long_function_findings: list[tuple[int, int, str, str]] = []
production_counts = {name: 0 for name in prod_patterns}
allow_dead_code = 0
allow_unused_imports = 0
direct_fs_write_files: list[str] = []
total_lines = 0

for relative_path in rust_files:
    try:
        text = read_text(relative_path)
    except OSError as exc:
        read_errors.append(f"{relative_path.as_posix()}\t{exc}")
        continue

    lines = physical_line_count(text)
    total_lines += lines
    file_lines.append((lines, relative_path.as_posix()))
    code = scrub_rust_code(text)
    production_code = strip_cfg_test_code(code)
    long_function_findings.extend(long_functions(relative_path, code))

    allow_dead_code += len(allow_dead_code_pattern.findall(text))
    allow_unused_imports += len(allow_unused_imports_pattern.findall(text))

    if is_test_path(relative_path):
        continue

    for name, pattern in prod_patterns.items():
        production_counts[name] += len(pattern.findall(production_code))
    if direct_fs_write_pattern.search(production_code):
        direct_fs_write_files.append(relative_path.as_posix())

files_over_500 = sorted((item for item in file_lines if item[0] > 500), reverse=True)
long_function_findings.sort(reverse=True)
direct_fs_write_files.sort()

report_errors: list[str] = []
report_warnings: list[str] = []
if discovery_error:
    report_errors.append("discovery")
if discovery_warning:
    report_warnings.append("discovery")
if read_errors:
    report_errors.append("read")

print("report=wayscriber-code-health")
if report_errors:
    print("status=error")
elif report_warnings:
    print("status=warning")
else:
    print("status=ok")
if report_errors:
    print(f"errors={','.join(report_errors)}")
if report_warnings:
    print(f"warnings={','.join(report_warnings)}")
if discovery_error:
    error_name, _, error_detail = discovery_error.partition("\t")
    print(f"error={error_name}")
    if error_detail:
        print(f"error_detail={error_detail}")
if discovery_warning:
    warning_name, _, warning_detail = discovery_warning.partition("\t")
    print(f"warning={warning_name}")
    if warning_detail:
        print(f"warning_detail={warning_detail}")
print(f"repo_root={repo_root}")
print(f"rust_files={len(rust_files)}")
print(f"rust_physical_lines={total_lines}")
print(f"files_over_500={len(files_over_500)}")
print(f"functions_over_120={len(long_function_findings)}")
print(f"production_unwrap={production_counts['unwrap']}")
print(f"production_expect={production_counts['expect']}")
print(f"production_panic={production_counts['panic']}")
print(f"production_unsafe={production_counts['unsafe']}")
print(f"allow_dead_code={allow_dead_code}")
print(f"allow_unused_imports={allow_unused_imports}")
print(f"direct_fs_write_files={len(direct_fs_write_files)}")
print(f"read_errors={len(read_errors)}")


def print_section(name: str, rows: list[str]) -> None:
    print()
    print(f"{name}:")
    if not rows:
        print("  none")
        return
    for row in rows:
        print(f"  {row}")


print_section(
    "files_over_500",
    [f"{lines}\t{path}" for lines, path in files_over_500],
)
print_section(
    "functions_over_120",
    [f"{lines}\t{path}:{line}\t{name}" for lines, line, path, name in long_function_findings],
)
print_section("direct_fs_write_files", direct_fs_write_files)
print_section("read_errors", read_errors)
PY
status=$?
set -e

if [ "$status" -ne 0 ]; then
    echo "report=wayscriber-code-health"
    echo "status=error"
    echo "error=python_report_failed"
    echo "python_exit=$status"
fi

exit 0
