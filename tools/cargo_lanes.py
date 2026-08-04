#!/usr/bin/env python3
"""Load and validate the Cargo lane manifest that drives the build/lint/test matrix.

`tools/cargo-lanes.json` is the single source of truth for which Cargo command
each consumer runs. This module parses it, checks the schema strictly, and hands
back typed lanes, consumers, and entry-point expectations. Callers are
`tools/run-cargo-consumer.py`, `tools/check-cargo-lanes.py`, and
`tools/check-rust-source-coverage.py`.

Stdlib only, Python >= 3.10: it must run on Ubuntu 24.04 and on Arch containers
bootstrapped with nothing but `python`.
"""

from __future__ import annotations

import json
from dataclasses import dataclass
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parent.parent
MANIFEST_PATH = REPO_ROOT / "tools" / "cargo-lanes.json"
SCHEMA_VERSION = 1

# Everything that picks packages or features. A lane owns that selection, so
# after the lane arguments an operation may only carry flags that change how
# Cargo reports or how hard it lints. Appending `--no-default-features` or
# `--features adw-modern` here would change what a lane-labeled command
# compiles while the floor and routing guards keep asserting from `lane.args`.
SELECTOR_ARGUMENTS = frozenset(
    {
        "-p",
        "--package",
        "--workspace",
        "--exclude",
        "--features",
        "--all-features",
        "--no-default-features",
    }
)


class ManifestError(RuntimeError):
    """The lane manifest is missing, unreadable, or does not match the schema."""


@dataclass(frozen=True)
class Lane:
    """A package/feature selection shared by several consumers."""

    name: str
    description: str
    args: tuple[str, ...]
    covers_configurator: bool
    libadwaita_floor: str | None


@dataclass(frozen=True)
class Operation:
    """One Cargo invocation, storing its complete argv."""

    consumer: str
    lane: str
    label: str
    argv: tuple[str, ...]

    @property
    def subcommand(self) -> str:
        return self.argv[1]

    def display(self) -> str:
        return " ".join(self.argv)


@dataclass(frozen=True)
class Consumer:
    """An ordered list of operations owned by one caller."""

    name: str
    description: str
    caller: str
    operations: tuple[Operation, ...]


@dataclass(frozen=True)
class EntryPoint:
    """A file that must invoke the driver, and the raw commands it must not regrow."""

    path: str
    description: str
    driver_consumers: tuple[str, ...]
    loader_consumers: tuple[str, ...]
    removed_commands: tuple[str, ...]


@dataclass(frozen=True)
class AllowedCargoCommand:
    """A raw Cargo command that legitimately stays outside the lane matrix."""

    entry_point: str
    command: str
    occurrences: int
    reason: str


@dataclass(frozen=True)
class Manifest:
    """The whole validated lane contract."""

    path: Path
    driver: str
    lanes: dict[str, Lane]
    consumers: dict[str, Consumer]
    entry_points: dict[str, EntryPoint]
    allowed_non_lane_cargo: tuple[AllowedCargoCommand, ...]

    def consumer(self, name: str) -> Consumer:
        found = self.consumers.get(name)
        if found is None:
            known = ", ".join(sorted(self.consumers))
            raise ManifestError(f"unknown consumer {name!r}; the manifest declares: {known}")
        return found

    def lane(self, name: str) -> Lane:
        found = self.lanes.get(name)
        if found is None:
            known = ", ".join(sorted(self.lanes))
            raise ManifestError(f"unknown lane {name!r}; the manifest declares: {known}")
        return found

    def allowed_for(self, entry_point: str) -> tuple[AllowedCargoCommand, ...]:
        return tuple(
            entry for entry in self.allowed_non_lane_cargo if entry.entry_point == entry_point
        )


def _require_mapping(value: object, where: str) -> dict:
    if not isinstance(value, dict):
        raise ManifestError(f"{where}: expected an object, found {type(value).__name__}")
    return value


def _require_string(value: object, where: str) -> str:
    if not isinstance(value, str) or not value:
        raise ManifestError(f"{where}: expected a non-empty string")
    return value


def _require_bool(value: object, where: str) -> bool:
    if not isinstance(value, bool):
        raise ManifestError(f"{where}: expected true or false")
    return value


def _require_string_list(value: object, where: str) -> tuple[str, ...]:
    if not isinstance(value, list):
        raise ManifestError(f"{where}: expected a list of strings")
    for index, item in enumerate(value):
        if not isinstance(item, str) or not item:
            raise ManifestError(f"{where}[{index}]: expected a non-empty string")
    return tuple(value)


def _require_keys(mapping: dict, allowed: set[str], required: set[str], where: str) -> None:
    unknown = sorted(set(mapping) - allowed)
    if unknown:
        raise ManifestError(f"{where}: unknown key(s): {', '.join(unknown)}")
    missing = sorted(required - set(mapping))
    if missing:
        raise ManifestError(f"{where}: missing key(s): {', '.join(missing)}")


def _parse_lane(name: str, raw: object) -> Lane:
    where = f"lanes.{name}"
    body = _require_mapping(raw, where)
    _require_keys(
        body,
        {"description", "args", "covers_configurator", "libadwaita_floor"},
        {"description", "args", "covers_configurator", "libadwaita_floor"},
        where,
    )
    args = _require_string_list(body["args"], f"{where}.args")
    if not args:
        raise ManifestError(f"{where}.args: a lane must select at least one package or feature")
    floor = body["libadwaita_floor"]
    if floor is not None and (not isinstance(floor, str) or not floor.startswith("v1_")):
        raise ManifestError(f"{where}.libadwaita_floor: expected null or a `v1_*` feature name")
    return Lane(
        name=name,
        description=_require_string(body["description"], f"{where}.description"),
        args=args,
        covers_configurator=_require_bool(
            body["covers_configurator"], f"{where}.covers_configurator"
        ),
        libadwaita_floor=floor,
    )


def _parse_operation(consumer: str, index: int, raw: object, lanes: dict[str, Lane]) -> Operation:
    where = f"consumers.{consumer}.operations[{index}]"
    body = _require_mapping(raw, where)
    _require_keys(body, {"lane", "label", "argv"}, {"lane", "label", "argv"}, where)
    lane_name = _require_string(body["lane"], f"{where}.lane")
    lane = lanes.get(lane_name)
    if lane is None:
        raise ManifestError(f"{where}.lane: unknown lane {lane_name!r}")
    argv = _require_string_list(body["argv"], f"{where}.argv")
    if len(argv) < 2 or argv[0] != "cargo":
        raise ManifestError(f"{where}.argv: expected `cargo <subcommand> ...`")
    lane_span = argv[2 : 2 + len(lane.args)]
    if lane_span != lane.args:
        raise ManifestError(
            f"{where}.argv: lane arguments must follow the subcommand verbatim; "
            f"expected {list(lane.args)} at position 2, found {list(lane_span)}"
        )
    # The tail is checked too, and past a bare `--` as well. Validating only the
    # lane span would let an operation append its own package or feature
    # selection: the command would compile something the lane never describes
    # while every guard that reads `lane.args` kept asserting the lane's story.
    for argument in argv[2 + len(lane.args) :]:
        if argument.split("=", maxsplit=1)[0] not in SELECTOR_ARGUMENTS:
            continue
        raise ManifestError(
            f"{where}.argv: `{argument}` selects packages or features after the lane "
            f"arguments, so this operation no longer compiles what lane {lane_name!r} "
            f"({list(lane.args)}) describes. Add a lane for the selection you want and "
            "label the operation with it"
        )
    return Operation(
        consumer=consumer,
        lane=lane_name,
        label=_require_string(body["label"], f"{where}.label"),
        argv=argv,
    )


def _parse_consumer(name: str, raw: object, lanes: dict[str, Lane]) -> Consumer:
    where = f"consumers.{name}"
    body = _require_mapping(raw, where)
    _require_keys(
        body,
        {"description", "caller", "operations"},
        {"description", "caller", "operations"},
        where,
    )
    raw_operations = body["operations"]
    if not isinstance(raw_operations, list) or not raw_operations:
        raise ManifestError(f"{where}.operations: expected a non-empty list")
    operations = tuple(
        _parse_operation(name, index, item, lanes) for index, item in enumerate(raw_operations)
    )
    return Consumer(
        name=name,
        description=_require_string(body["description"], f"{where}.description"),
        caller=_require_string(body["caller"], f"{where}.caller"),
        operations=operations,
    )


def _parse_entry_point(path: str, raw: object, consumers: set[str]) -> EntryPoint:
    where = f"entry_points.{path}"
    body = _require_mapping(raw, where)
    _require_keys(
        body,
        {"description", "driver_consumers", "loader_consumers", "removed_commands"},
        {"description", "driver_consumers", "loader_consumers", "removed_commands"},
        where,
    )
    driver_consumers = _require_string_list(body["driver_consumers"], f"{where}.driver_consumers")
    loader_consumers = _require_string_list(body["loader_consumers"], f"{where}.loader_consumers")
    for name in (*driver_consumers, *loader_consumers):
        if name not in consumers:
            raise ManifestError(f"{where}: references unknown consumer {name!r}")
    if not driver_consumers and not loader_consumers:
        raise ManifestError(f"{where}: an entry point must consume at least one consumer")
    return EntryPoint(
        path=path,
        description=_require_string(body["description"], f"{where}.description"),
        driver_consumers=driver_consumers,
        loader_consumers=loader_consumers,
        removed_commands=_require_string_list(
            body["removed_commands"], f"{where}.removed_commands"
        ),
    )


def _parse_allowed(index: int, raw: object) -> AllowedCargoCommand:
    where = f"allowed_non_lane_cargo[{index}]"
    body = _require_mapping(raw, where)
    _require_keys(
        body,
        {"entry_point", "command", "occurrences", "reason"},
        {"entry_point", "command", "occurrences", "reason"},
        where,
    )
    occurrences = body["occurrences"]
    if not isinstance(occurrences, int) or isinstance(occurrences, bool) or occurrences < 1:
        raise ManifestError(f"{where}.occurrences: expected a positive integer")
    return AllowedCargoCommand(
        entry_point=_require_string(body["entry_point"], f"{where}.entry_point"),
        command=_require_string(body["command"], f"{where}.command"),
        occurrences=occurrences,
        reason=_require_string(body["reason"], f"{where}.reason"),
    )


def load_manifest(path: Path | None = None) -> Manifest:
    """Read and validate the lane manifest."""
    manifest_path = MANIFEST_PATH if path is None else path
    try:
        raw_text = manifest_path.read_text(encoding="utf-8")
    except OSError as error:
        raise ManifestError(f"could not read {manifest_path}: {error}") from error
    try:
        document = json.loads(raw_text)
    except json.JSONDecodeError as error:
        raise ManifestError(f"{manifest_path}: invalid JSON: {error}") from error

    root = _require_mapping(document, str(manifest_path))
    _require_keys(
        root,
        {
            "schema_version",
            "notes",
            "driver",
            "lanes",
            "consumers",
            "entry_points",
            "allowed_non_lane_cargo",
        },
        {
            "schema_version",
            "driver",
            "lanes",
            "consumers",
            "entry_points",
            "allowed_non_lane_cargo",
        },
        str(manifest_path),
    )
    if root["schema_version"] != SCHEMA_VERSION:
        raise ManifestError(
            f"{manifest_path}: schema_version {root['schema_version']!r} is not supported "
            f"(this loader implements {SCHEMA_VERSION})"
        )

    lanes = {
        name: _parse_lane(name, body)
        for name, body in _require_mapping(root["lanes"], "lanes").items()
    }
    if not lanes:
        raise ManifestError(f"{manifest_path}: no lanes declared")

    consumers = {
        name: _parse_consumer(name, body, lanes)
        for name, body in _require_mapping(root["consumers"], "consumers").items()
    }
    if not consumers:
        raise ManifestError(f"{manifest_path}: no consumers declared")

    entry_points = {
        path_key: _parse_entry_point(path_key, body, set(consumers))
        for path_key, body in _require_mapping(root["entry_points"], "entry_points").items()
    }
    if not entry_points:
        raise ManifestError(f"{manifest_path}: no entry points declared")

    raw_allowed = root["allowed_non_lane_cargo"]
    if not isinstance(raw_allowed, list):
        raise ManifestError("allowed_non_lane_cargo: expected a list")
    allowed = tuple(_parse_allowed(index, item) for index, item in enumerate(raw_allowed))
    for entry in allowed:
        if entry.entry_point not in entry_points:
            raise ManifestError(
                f"allowed_non_lane_cargo: {entry.entry_point!r} is not a declared entry point"
            )

    routed = {name for point in entry_points.values() for name in point.driver_consumers}
    routed |= {name for point in entry_points.values() for name in point.loader_consumers}
    unrouted = sorted(set(consumers) - routed)
    if unrouted:
        raise ManifestError(
            f"{manifest_path}: consumer(s) {', '.join(unrouted)} are declared but no entry point "
            "calls them; wire the caller or delete the consumer"
        )

    return Manifest(
        path=manifest_path,
        driver=_require_string(root["driver"], "driver"),
        lanes=lanes,
        consumers=consumers,
        entry_points=entry_points,
        allowed_non_lane_cargo=allowed,
    )
