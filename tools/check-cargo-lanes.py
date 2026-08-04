#!/usr/bin/env python3
"""Fail when the configurator feature graph, a libadwaita floor, or a Cargo entry point drifts.

Two independent claims are checked here.

Feature and floor guard
  Live `cargo metadata` must still show the declared feature edges
  (`default -> tablet-input`, `tablet-input -> wayscriber/tablet-input`,
  `adw-modern -> libadwaita/v1_7`, direct libadwaita base features `["v1_4"]`),
  the `default` closure must not reach `adw-modern`, every lane must resolve the
  exact libadwaita floor it declares (this catches a transitive dependency
  quietly enabling a newer `v1_*`), and every declared configurator feature must
  appear in at least one lane's resolved feature closure. Resolution is
  metadata-only, so the modern lane is verified on machines with no libadwaita
  1.7 runtime.

Entry-point contract
  Each entry point in `tools/cargo-lanes.json` must invoke the driver exactly
  once per routed consumer, must no longer contain the raw commands the manifest
  replaced, and must still contain the allowlisted non-lane Cargo commands. A raw
  Cargo command that is neither the driver nor allowlisted fails the check.

`--self-test` replays stored metadata-shaped and entry-point fixtures instead of
reading the working tree, proving each rule rejects what it claims to reject.
"""

from __future__ import annotations

import json
import re
import shlex
import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from cargo_lanes import REPO_ROOT, Lane, Manifest, ManifestError, load_manifest


FIXTURE_ROOT = REPO_ROOT / "tools" / "fixtures" / "cargo-lanes"
FEATURE_CASES = FIXTURE_ROOT / "feature-cases.json"
ENTRY_POINT_CASES = FIXTURE_ROOT / "entry-point-cases.json"
MANIFEST_CASES = FIXTURE_ROOT / "manifest-cases.json"

CONFIGURATOR_PACKAGE = "wayscriber-configurator"
LIBADWAITA_PACKAGE = "libadwaita"
MODERN_FEATURE = "adw-modern"

# The pinned feature edges. Equality is intentional: an extra element here is
# exactly the drift the guard exists to catch, so widening one of these lists is
# a channel decision that belongs in a review, not a quiet manifest edit.
EXPECTED_FEATURE_EDGES: dict[str, tuple[str, ...]] = {
    "default": ("tablet-input",),
    "tablet-input": ("wayscriber/tablet-input",),
    MODERN_FEATURE: ("libadwaita/v1_7",),
}
EXPECTED_LIBADWAITA_BASE_FEATURES: tuple[str, ...] = ("v1_4",)

VERSION_FEATURE = re.compile(r"^v(\d+)_(\d+)$")

# Where one line stops being one command. Splitting here is what makes the
# scan below read the *head* of every command on the line rather than the head
# of the line.
SHELL_OPERATOR = re.compile(r"\$\(|\|\||&&|[;&|(){}`]")

# A leading `NAME=value` token is the shell setting a variable for the command
# that follows, not the command. Matching only the name side is deliberate: the
# value may be quoted and contain anything, and a regex that tries to describe
# the value stops matching the moment it does.
ENV_ASSIGNMENT = re.compile(r"^[A-Za-z_][A-Za-z0-9_]*=")

# Commands that run another command. Their own tokens are skipped so the head
# read afterwards is the program that actually runs.
COMMAND_WRAPPERS = frozenset(
    {
        "sudo",
        "env",
        "nice",
        "ionice",
        "time",
        "timeout",
        "xvfb-run",
        "dbus-run-session",
        "stdbuf",
        "nohup",
        "setarch",
        "runuser",
        "su",
        "doas",
    }
)

# The wrappers that take a bare number before the command: `timeout 300 cargo
# test`, `nice 10 cargo build`. Consuming it is what keeps the number from
# being read as the program.
NUMERIC_OPERAND_WRAPPERS = frozenset({"timeout", "nice", "ionice"})
NUMERIC_OPERAND = re.compile(r"^\d+(?:\.\d+)?[smhd]?$")

CARGO_PROGRAM = "cargo"

# Loader entry points must reach the manifest through the shared module, not
# merely mention a consumer name.
LOADER_IMPORT = re.compile(r"^\s*(?:import\s+cargo_lanes\b|from\s+cargo_lanes\s+import\s)")


class GuardError(RuntimeError):
    """A prerequisite of the guard could not be satisfied."""


# --------------------------------------------------------------------------- #
# Cargo metadata access
# --------------------------------------------------------------------------- #


def run_cargo_metadata(extra_args: list[str]) -> dict:
    command = ["cargo", "metadata", "--locked", "--format-version", "1", *extra_args]
    try:
        result = subprocess.run(
            command,
            cwd=REPO_ROOT,
            check=False,
            capture_output=True,
            text=True,
        )
    except OSError as error:
        raise GuardError(f"could not run cargo metadata: {error}") from error
    if result.returncode != 0:
        detail = result.stderr.strip() or f"exit status {result.returncode}"
        raise GuardError(f"`{' '.join(command)}` failed:\n{detail}")
    try:
        return json.loads(result.stdout)
    except json.JSONDecodeError as error:
        raise GuardError(f"could not parse `{' '.join(command)}` output: {error}") from error


def metadata_args_for_lane(
    lane: Lane, manifest_paths: dict[str, str], workspace_root: str
) -> list[str]:
    """Translate a lane's Cargo arguments into the `cargo metadata` equivalents.

    `cargo metadata` has no `-p`/`--workspace`; the package selection is made by
    pointing `--manifest-path` at the manifest whose feature set should resolve.
    Unknown arguments fail loudly rather than being dropped, because a silently
    dropped selector would make the floor assertion meaningless.
    """
    translated: list[str] = []
    index = 0
    while index < len(lane.args):
        argument = lane.args[index]
        if argument in ("-p", "--package"):
            index += 1
            if index >= len(lane.args):
                raise GuardError(f"lane `{lane.name}`: `{argument}` has no package name")
            name = lane.args[index]
            manifest_path = manifest_paths.get(name)
            if manifest_path is None:
                raise GuardError(
                    f"lane `{lane.name}`: package `{name}` is not a workspace member"
                )
            translated += ["--manifest-path", manifest_path]
        elif argument == "--workspace":
            translated += ["--manifest-path", f"{workspace_root}/Cargo.toml"]
        elif argument in ("--all-features", "--no-default-features"):
            translated.append(argument)
        elif argument == "--features":
            index += 1
            if index >= len(lane.args):
                raise GuardError(f"lane `{lane.name}`: `--features` has no value")
            translated += ["--features", lane.args[index]]
        else:
            raise GuardError(
                f"lane `{lane.name}`: argument `{argument}` has no `cargo metadata` "
                "translation; teach metadata_args_for_lane about it before using it in a lane"
            )
        index += 1
    return translated


def workspace_manifest_paths(document: dict) -> tuple[dict[str, str], str]:
    packages = document.get("packages")
    if not isinstance(packages, list) or not packages:
        raise GuardError("cargo metadata returned no packages")
    manifest_paths = {
        package["name"]: package["manifest_path"]
        for package in packages
        if isinstance(package, dict) and "name" in package and "manifest_path" in package
    }
    workspace_root = document.get("workspace_root")
    if not isinstance(workspace_root, str) or not workspace_root:
        raise GuardError("cargo metadata returned no workspace_root")
    return manifest_paths, workspace_root


# --------------------------------------------------------------------------- #
# Metadata-shaped readers (shared by the live run and the fixtures)
# --------------------------------------------------------------------------- #


def find_package(document: dict, name: str) -> dict:
    packages = document.get("packages")
    if not isinstance(packages, list):
        raise GuardError("metadata document has no `packages` list")
    matches = [
        package
        for package in packages
        if isinstance(package, dict) and package.get("name") == name
    ]
    if not matches:
        raise GuardError(f"metadata document does not describe package `{name}`")
    return matches[0]


def resolved_features(document: dict, name: str) -> set[str]:
    """Every feature the resolver enabled on `name` in this metadata document."""
    packages = document.get("packages")
    resolve = document.get("resolve")
    if not isinstance(packages, list) or not isinstance(resolve, dict):
        raise GuardError(f"metadata document for `{name}` has no resolved dependency graph")
    ids = {
        package["id"]
        for package in packages
        if isinstance(package, dict) and package.get("name") == name and "id" in package
    }
    if not ids:
        raise GuardError(f"metadata document does not describe package `{name}`")
    nodes = resolve.get("nodes")
    if not isinstance(nodes, list):
        raise GuardError(f"metadata document for `{name}` has no `resolve.nodes`")
    features: set[str] = set()
    matched = False
    for node in nodes:
        if not isinstance(node, dict) or node.get("id") not in ids:
            continue
        matched = True
        node_features = node.get("features")
        if not isinstance(node_features, list):
            raise GuardError(f"resolve node for `{name}` has no `features` list")
        features.update(str(feature) for feature in node_features)
    if not matched:
        raise GuardError(f"metadata document has no resolve node for `{name}`")
    return features


def feature_closure(features: dict[str, list[str]], root: str) -> set[str]:
    """Features of the same package that enabling `root` turns on."""
    reached: set[str] = set()
    pending = [root]
    while pending:
        name = pending.pop()
        if name in reached or name not in features:
            continue
        reached.add(name)
        for edge in features[name]:
            if "/" in edge:
                continue
            pending.append(edge.removeprefix("dep:"))
    return reached


def version_key(feature: str) -> tuple[int, int]:
    match = VERSION_FEATURE.match(feature)
    if match is None:
        raise GuardError(f"`{feature}` is not a `v<major>_<minor>` feature name")
    return int(match.group(1)), int(match.group(2))


# --------------------------------------------------------------------------- #
# Rule: declared feature edges
# --------------------------------------------------------------------------- #


def check_feature_edges(no_deps_document: dict) -> list[str]:
    errors: list[str] = []
    package = find_package(no_deps_document, CONFIGURATOR_PACKAGE)
    features = package.get("features")
    if not isinstance(features, dict):
        return [f"{CONFIGURATOR_PACKAGE}: cargo metadata reported no `features` table"]

    for name, expected in EXPECTED_FEATURE_EDGES.items():
        declared = features.get(name)
        if declared is None:
            errors.append(
                f"configurator/Cargo.toml: feature `{name}` is gone; the strategy requires "
                f"`{name} = {list(expected)}`"
            )
            continue
        if sorted(str(edge) for edge in declared) != sorted(expected):
            errors.append(
                f"configurator/Cargo.toml: feature `{name}` enables {sorted(declared)}, "
                f"expected exactly {sorted(expected)}; changing this is a channel decision, "
                "not a routine edit"
            )

    dependencies = package.get("dependencies")
    if not isinstance(dependencies, list):
        errors.append(f"{CONFIGURATOR_PACKAGE}: cargo metadata reported no `dependencies` list")
        return errors

    libadwaita = [
        dependency
        for dependency in dependencies
        if isinstance(dependency, dict)
        and dependency.get("name") == LIBADWAITA_PACKAGE
        and dependency.get("kind") is None
    ]
    if not libadwaita:
        errors.append(
            f"configurator/Cargo.toml: no direct normal `{LIBADWAITA_PACKAGE}` dependency found"
        )
    else:
        base_features = [str(feature) for feature in libadwaita[0].get("features", [])]
        if sorted(base_features) != sorted(EXPECTED_LIBADWAITA_BASE_FEATURES):
            errors.append(
                f"configurator/Cargo.toml: the `{LIBADWAITA_PACKAGE}` dependency declares "
                f"features {sorted(base_features)}, expected exactly "
                f"{sorted(EXPECTED_LIBADWAITA_BASE_FEATURES)}; the baseline deb/rpm floor "
                "is what this pin protects"
            )

    plain_features = {
        name: [str(edge) for edge in edges]
        for name, edges in features.items()
        if isinstance(edges, list)
    }
    if MODERN_FEATURE in feature_closure(plain_features, "default"):
        errors.append(
            f"configurator/Cargo.toml: the `default` feature closure reaches `{MODERN_FEATURE}`; "
            "the modern libadwaita channel must stay opt-in or the deb/rpm baseline build "
            "starts requiring libadwaita 1.7"
        )
    return errors


# --------------------------------------------------------------------------- #
# Rule: resolved libadwaita floor per lane
# --------------------------------------------------------------------------- #


def check_lane_floors(manifest: Manifest, lane_documents: dict[str, dict]) -> list[str]:
    errors: list[str] = []
    for name, lane in sorted(manifest.lanes.items()):
        if lane.libadwaita_floor is None:
            continue
        document = lane_documents.get(name)
        if document is None:
            errors.append(f"lane `{name}`: no resolved metadata available for the floor check")
            continue
        features = resolved_features(document, LIBADWAITA_PACKAGE)
        version_features = sorted(
            (feature for feature in features if VERSION_FEATURE.match(feature)),
            key=version_key,
        )
        if not version_features:
            errors.append(
                f"lane `{name}`: the resolved `{LIBADWAITA_PACKAGE}` node enables no `v1_*` "
                "feature; the API floor is no longer expressed as a feature"
            )
            continue
        highest = version_features[-1]
        if highest != lane.libadwaita_floor:
            errors.append(
                f"lane `{name}`: the resolved `{LIBADWAITA_PACKAGE}` node enables `{highest}` "
                f"but the lane declares floor `{lane.libadwaita_floor}` "
                f"(enabled: {', '.join(version_features)}). Something in the dependency graph "
                "moved the API floor; find the crate that enables it before touching "
                "tools/cargo-lanes.json"
            )
    return errors


# --------------------------------------------------------------------------- #
# Rule: every declared configurator feature is routed to a lane
# --------------------------------------------------------------------------- #


def check_feature_routing(
    manifest: Manifest, no_deps_document: dict, lane_documents: dict[str, dict]
) -> list[str]:
    package = find_package(no_deps_document, CONFIGURATOR_PACKAGE)
    features = package.get("features")
    if not isinstance(features, dict):
        return [f"{CONFIGURATOR_PACKAGE}: cargo metadata reported no `features` table"]

    covering_lanes = sorted(
        name for name, lane in manifest.lanes.items() if lane.covers_configurator
    )
    covered: set[str] = set()
    errors: list[str] = []
    for name in covering_lanes:
        document = lane_documents.get(name)
        if document is None:
            errors.append(f"lane `{name}`: no resolved metadata available for the routing check")
            continue
        covered |= resolved_features(document, CONFIGURATOR_PACKAGE)

    unrouted = sorted(set(features) - covered)
    if unrouted:
        errors.append(
            f"configurator feature(s) {', '.join(unrouted)} are declared but no lane's resolved "
            f"feature closure enables them (lanes that compile the configurator: "
            f"{', '.join(covering_lanes)}). Add the feature to a lane in tools/cargo-lanes.json "
            "or delete it; an unrouted feature is never compiled or tested"
        )
    return errors


# --------------------------------------------------------------------------- #
# Rule: entry-point contract
# --------------------------------------------------------------------------- #


def non_comment_lines(text: str) -> list[tuple[int, str]]:
    lines: list[tuple[int, str]] = []
    for number, line in enumerate(text.splitlines(), start=1):
        stripped = line.strip()
        if not stripped or stripped.startswith("#"):
            continue
        lines.append((number, line))
    return lines


def normalized_command(line: str) -> str:
    """Strip the shell/YAML scaffolding around a command so it can be compared."""
    text = line.strip()
    if text.startswith("- "):
        text = text[2:].strip()
    if text.startswith("run:"):
        text = text[len("run:") :].strip()
    return text


def program_name(token: str) -> str:
    """The basename of a command token, so `/usr/bin/cargo` reads as `cargo`."""
    return token.rsplit("/", maxsplit=1)[-1]


def command_tokens(segment: str) -> list[str]:
    """Split one command segment into shell words.

    Cutting the line on operators first can land inside a quoted string, and
    `shlex` refuses an unbalanced quote. A whitespace split still exposes the
    head token, which is the only thing this scan reads.
    """
    try:
        return shlex.split(segment)
    except ValueError:
        return segment.split()


def cargo_invocation(segment: str) -> str | None:
    """The Cargo command this segment runs, or `None` if it runs something else.

    Environment assignments and wrapper programs are consumed first, so
    `RUSTFLAGS="-A warnings" cargo test`, `sudo cargo ...`, `env cargo ...`,
    `timeout 300 cargo ...`, `xvfb-run cargo ...` and `/usr/bin/cargo ...` all
    answer with the Cargo command they hide. The answer keeps the tokens from
    the head onward, so an allowlisted command still compares equal after the
    prefix is stripped, while a path-qualified `cargo` stays visibly different
    from the allowlisted spelling.
    """
    tokens = command_tokens(segment)
    index = 0
    while index < len(tokens):
        token = tokens[index]
        if ENV_ASSIGNMENT.match(token):
            index += 1
            continue
        name = program_name(token)
        if name not in COMMAND_WRAPPERS:
            break
        index += 1
        while index < len(tokens) and tokens[index].startswith("-"):
            index += 1
        # Every bare number, not just the first: `timeout -k 10 300 cargo test`
        # spends one on the option above and one on the duration, and stopping
        # after the first would read `300` as the program.
        while (
            name in NUMERIC_OPERAND_WRAPPERS
            and index < len(tokens)
            and NUMERIC_OPERAND.match(tokens[index])
        ):
            index += 1

    if index >= len(tokens) or program_name(tokens[index]) != CARGO_PROGRAM:
        return None
    return " ".join(tokens[index:])


def cargo_invocations(command: str) -> list[str]:
    """Every Cargo command one line runs, in order."""
    found: list[str] = []
    for segment in SHELL_OPERATOR.split(command):
        if not segment.strip():
            continue
        invocation = cargo_invocation(segment)
        if invocation is not None:
            found.append(invocation)
    return found


def check_entry_point(manifest: Manifest, path: str, text: str) -> list[str]:
    entry = manifest.entry_points[path]
    errors: list[str] = []
    lines = non_comment_lines(text)
    driver_pattern = re.compile(
        re.escape(manifest.driver) + r"\s+(?P<consumer>[A-Za-z0-9_.-]+)"
    )

    invocations: dict[str, list[int]] = {}
    for number, line in lines:
        for match in driver_pattern.finditer(line):
            invocations.setdefault(match.group("consumer"), []).append(number)

    for consumer in entry.driver_consumers:
        found = invocations.get(consumer, [])
        if len(found) != 1:
            where = ", ".join(f"line {number}" for number in found) or "nowhere"
            errors.append(
                f"{path}: expected exactly one `{manifest.driver} {consumer}` invocation, "
                f"found {len(found)} ({where})"
            )
    for consumer, found in sorted(invocations.items()):
        if consumer in entry.driver_consumers:
            continue
        if consumer not in manifest.consumers:
            errors.append(
                f"{path}:{found[0]}: invokes `{manifest.driver} {consumer}`, which is not a "
                "consumer declared in tools/cargo-lanes.json"
            )
        else:
            errors.append(
                f"{path}:{found[0]}: invokes consumer `{consumer}`, which the manifest does not "
                f"route to this entry point (routed here: "
                f"{', '.join(entry.driver_consumers) or 'none'})"
            )

    if entry.loader_consumers and not any(LOADER_IMPORT.match(line) for _, line in lines):
        errors.append(
            f"{path}: expected the file to import tools/cargo_lanes.py "
            "(`import cargo_lanes` or `from cargo_lanes import ...`); the routed consumer "
            f"name(s) {', '.join(entry.loader_consumers)} can sit in an inlined vector list "
            "long after the loader is gone, and then manifest edits stop reaching this gate"
        )
    for consumer in entry.loader_consumers:
        quoted = (f'"{consumer}"', f"'{consumer}'")
        if not any(token in line for _, line in lines for token in quoted):
            errors.append(
                f"{path}: expected the file to consume the `{consumer}` operations through "
                "tools/cargo_lanes.py, but its name never appears"
            )

    for command in entry.removed_commands:
        hits = [number for number, line in lines if command in line]
        if hits:
            where = ", ".join(f"line {number}" for number in hits)
            errors.append(
                f"{path}: the raw command `{command}` is back ({where}); it was replaced by the "
                "lane manifest and must not be reintroduced outside tools/cargo-lanes.json"
            )

    allowed = manifest.allowed_for(path)
    allowed_commands = {entry_command.command for entry_command in allowed}
    counted: dict[str, int] = {command: 0 for command in allowed_commands}
    # One normalization answers both questions. An allowlisted command that
    # picks up an environment prefix is still that command and must still count
    # exactly once, and a raw command that hides behind the same prefix is
    # still raw.
    for number, line in lines:
        for invocation in cargo_invocations(normalized_command(line)):
            if invocation in counted:
                counted[invocation] += 1
                continue
            errors.append(
                f"{path}:{number}: raw Cargo command `{invocation}` is neither a lane operation "
                "nor an allowlisted non-lane command; move it into tools/cargo-lanes.json or "
                "record it in allowed_non_lane_cargo with a reason"
            )
    for entry_command in allowed:
        actual = counted[entry_command.command]
        if actual != entry_command.occurrences:
            errors.append(
                f"{path}: expected {entry_command.occurrences} occurrence(s) of the allowlisted "
                f"command `{entry_command.command}`, found {actual}. Reason it is allowlisted: "
                f"{entry_command.reason}"
            )
    return errors


# --------------------------------------------------------------------------- #
# Live run
# --------------------------------------------------------------------------- #


def collect_lane_documents(manifest: Manifest, no_deps_document: dict) -> dict[str, dict]:
    manifest_paths, workspace_root = workspace_manifest_paths(no_deps_document)
    documents: dict[str, dict] = {}
    for name, lane in sorted(manifest.lanes.items()):
        if lane.libadwaita_floor is None and not lane.covers_configurator:
            continue
        extra = metadata_args_for_lane(lane, manifest_paths, workspace_root)
        documents[name] = run_cargo_metadata(extra)
    return documents


def run_live_checks(manifest: Manifest) -> list[str]:
    no_deps_document = run_cargo_metadata(["--no-deps"])
    lane_documents = collect_lane_documents(manifest, no_deps_document)

    errors = check_feature_edges(no_deps_document)
    errors += check_lane_floors(manifest, lane_documents)
    errors += check_feature_routing(manifest, no_deps_document, lane_documents)

    for path in sorted(manifest.entry_points):
        absolute = REPO_ROOT / path
        try:
            text = absolute.read_text(encoding="utf-8")
        except OSError as error:
            errors.append(f"{path}: declared as an entry point but unreadable: {error}")
            continue
        errors += check_entry_point(manifest, path, text)
    return errors


# --------------------------------------------------------------------------- #
# Self test
# --------------------------------------------------------------------------- #


def load_fixture_document(relative: str) -> dict:
    path = FIXTURE_ROOT / relative
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except OSError as error:
        raise GuardError(f"could not read fixture {path}: {error}") from error
    except json.JSONDecodeError as error:
        raise GuardError(f"fixture {path} is not valid JSON: {error}") from error


def load_cases(path: Path) -> list[dict]:
    try:
        document = json.loads(path.read_text(encoding="utf-8"))
    except OSError as error:
        raise GuardError(f"could not read fixture index {path}: {error}") from error
    except json.JSONDecodeError as error:
        raise GuardError(f"fixture index {path} is not valid JSON: {error}") from error
    cases = document.get("cases")
    if not isinstance(cases, list) or not cases:
        raise GuardError(f"fixture index {path} declares no cases")
    return cases


def judge_case(name: str, case: dict, produced: list[str]) -> list[str]:
    expectation = case.get("expect")
    if expectation == "pass":
        if produced:
            return [
                f"self-test case `{name}` was expected to pass but reported:\n"
                + "\n".join(f"    {message}" for message in produced)
            ]
        return []
    if expectation != "fail":
        return [f"self-test case `{name}`: `expect` must be \"pass\" or \"fail\""]
    if not produced:
        return [
            f"self-test case `{name}` was expected to fail but every rule accepted it; "
            f"the rule it targets ({case.get('why', 'unstated')}) is not enforced"
        ]
    needle = case.get("expect_error_contains")
    if not isinstance(needle, str) or not needle:
        return [f"self-test case `{name}`: a failing case must declare `expect_error_contains`"]
    if not any(needle in message for message in produced):
        return [
            f"self-test case `{name}` failed for the wrong reason; expected a message containing "
            f"{needle!r} but got:\n" + "\n".join(f"    {message}" for message in produced)
        ]
    return []


def run_feature_fixtures(manifest: Manifest) -> list[str]:
    failures: list[str] = []
    for case in load_cases(FEATURE_CASES):
        name = str(case.get("name", "<unnamed>"))
        packages_reference = case.get("packages")
        lane_references = case.get("lanes")
        if not isinstance(packages_reference, str) or not isinstance(lane_references, dict):
            failures.append(f"self-test case `{name}`: needs `packages` and `lanes` references")
            continue
        no_deps_document = load_fixture_document(packages_reference)
        lane_documents = {
            str(lane): load_fixture_document(str(reference))
            for lane, reference in lane_references.items()
        }
        missing = sorted(
            lane
            for lane, spec in manifest.lanes.items()
            if (spec.covers_configurator or spec.libadwaita_floor is not None)
            and lane not in lane_documents
        )
        if missing:
            failures.append(
                f"self-test case `{name}`: no fixture metadata for lane(s) {', '.join(missing)}; "
                "every lane that compiles the configurator needs one"
            )
            continue
        produced = check_feature_edges(no_deps_document)
        produced += check_lane_floors(manifest, lane_documents)
        produced += check_feature_routing(manifest, no_deps_document, lane_documents)
        failures += judge_case(name, case, produced)
    return failures


def run_entry_point_fixtures(manifest: Manifest) -> list[str]:
    failures: list[str] = []
    for case in load_cases(ENTRY_POINT_CASES):
        name = str(case.get("name", "<unnamed>"))
        entry_point = case.get("entry_point")
        fixture_file = case.get("file")
        if not isinstance(entry_point, str) or entry_point not in manifest.entry_points:
            failures.append(
                f"self-test case `{name}`: `entry_point` must name a manifest entry point"
            )
            continue
        if not isinstance(fixture_file, str):
            failures.append(f"self-test case `{name}`: `file` is missing")
            continue
        path = FIXTURE_ROOT / fixture_file
        try:
            text = path.read_text(encoding="utf-8")
        except OSError as error:
            failures.append(f"self-test case `{name}`: could not read {path}: {error}")
            continue
        failures += judge_case(name, case, check_entry_point(manifest, entry_point, text))
    return failures


def run_manifest_fixtures() -> list[str]:
    """Replay whole manifests through the loader that validates the live one.

    The schema rules have no other fixture shape: they reject a document, not a
    metadata graph or an entry-point text, so each case here is a manifest file
    the loader must accept or refuse for the stated reason.
    """
    failures: list[str] = []
    for case in load_cases(MANIFEST_CASES):
        name = str(case.get("name", "<unnamed>"))
        fixture_file = case.get("file")
        if not isinstance(fixture_file, str):
            failures.append(f"self-test case `{name}`: `file` is missing")
            continue
        try:
            load_manifest(FIXTURE_ROOT / fixture_file)
            produced: list[str] = []
        except ManifestError as error:
            produced = [str(error)]
        failures += judge_case(name, case, produced)
    return failures


def run_self_test(manifest: Manifest) -> list[str]:
    return (
        run_feature_fixtures(manifest)
        + run_entry_point_fixtures(manifest)
        + run_manifest_fixtures()
    )


# --------------------------------------------------------------------------- #


def main(argv: list[str]) -> int:
    self_test = False
    for argument in argv:
        if argument == "--self-test":
            self_test = True
        else:
            print(
                f"usage: ./tools/check-cargo-lanes.py [--self-test]\nunknown argument {argument!r}",
                file=sys.stderr,
            )
            return 2

    try:
        manifest = load_manifest()
        errors = run_self_test(manifest) if self_test else run_live_checks(manifest)
    except (ManifestError, GuardError) as error:
        label = "self-test" if self_test else "check"
        print(f"Cargo lane {label} failed: {error}", file=sys.stderr)
        return 2

    if errors:
        label = "self-test" if self_test else "check"
        print(f"Cargo lane {label} failed:", file=sys.stderr)
        for message in errors:
            print(f"- {message}", file=sys.stderr)
        return 1

    if self_test:
        feature_cases = len(load_cases(FEATURE_CASES))
        entry_cases = len(load_cases(ENTRY_POINT_CASES))
        manifest_cases = len(load_cases(MANIFEST_CASES))
        print(
            f"Cargo lane self-test OK: {feature_cases} feature/floor fixture(s), "
            f"{entry_cases} entry-point fixture(s), and {manifest_cases} manifest "
            "fixture(s) behaved as declared."
        )
        return 0

    floors = sorted(
        f"{name}={lane.libadwaita_floor}"
        for name, lane in manifest.lanes.items()
        if lane.libadwaita_floor is not None
    )
    print(
        f"Cargo lane check OK: {len(manifest.lanes)} lane(s), "
        f"{len(manifest.consumers)} consumer(s), "
        f"{len(manifest.entry_points)} entry point(s); resolved libadwaita floors "
        f"{', '.join(floors)}."
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
