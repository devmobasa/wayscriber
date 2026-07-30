#!/usr/bin/env python3
"""Fail when production code outside the reviewed writers can write `config.toml`.

`config.toml` is an authored input. It changes only through an explicit user
edit action, never as a side effect of running Wayscriber. Two kinds of writer
are allowed:

* the configurator's **Save**, which writes the whole edited draft; and
* the overlay's **narrow editors** in `src/config/io.rs`, one per explicit
  gesture, each of which rewrites only its own key and backs the file up first.

Everything else — startup, shutdown, the daemon, the tray, validation, migration
preview, and every incidental preference toggle — reads the file and never
writes it. The capability is one `use` away, so this checks for its absence
instead of remembering it.

Six things are enforced. First, the write primitives are named nowhere outside
`src/config/` and the configurator's Save adapter. Second, each narrow editor's
production call sites are pinned by name: a new caller of one of them is a new
place `config.toml` can change, which is a review decision rather than an
implementation detail, so it has to be recorded in `NARROW_WRITERS` below.
Third, the write-capable surface of the two implementing files —
`src/config/document.rs` and `src/config/io.rs` — is pinned the same way. Both
are exempt from the primitive scan because they *are* the write, so their own
exported functions are enumerated instead: every one that can reach a write —
directly or through the file's own private helpers — has to be recorded in
`DOCUMENT_WRITE_SURFACE` or `IO_WRITE_SURFACE`. The walk is what makes the pin
about the capability rather than about the spelling: a `pub fn
persist_any_config(..) { self.merge_and_write(..) }` in `document.rs` reaches
the file exactly as far as `save_with_backup` does, and is caught by name and
line even though nothing about its name says "save".

Visibility is not the whole of the surface either, so it is not the whole of
that pin. A method inside `impl SomeTrait for ConfigDocument` carries no `pub`
of its own, and neither does a trait's default method: the trait's visibility is
what makes them callable, and the trait is one `pub use` away from the rest of
the crate. In the three implementing files — `document.rs`, `io.rs`, and
`mod.rs` — a write reachable through a trait therefore fails whatever it is
called and whatever visibility it was given, and so does a `trait` declared
there whose methods reach a write, including the bodiless signature whose impl
sits elsewhere in the file. The reviewed writers are inherent functions and free
functions, which is the only shape the surfaces above enumerate.

Fourth, `src/config/mod.rs` is held to being a re-export list. It is the other
file exempt from caller collection, because it is where the editors leave the
module, so a writer's name may appear there inside a `use` or `pub use` item and
nowhere else, and no function declared there may reach a writer — directly,
through `crate::config::io::`, or through another wrapper in the same file. The
same write-capability walk `io.rs` gets runs over it, so `pub fn
concealed_edit(..) { io::persist_keybinding_edit(..) }` fails by name and line
instead of being invisible to every other check and callable from anywhere.

Fifth, because everything above matches call sites by *name*, the names
themselves are pinned. Any file in `src/config/` may re-export the editors but
never a primitive. No production file anywhere may re-export or import a
write-capable name under a different one: both `pub use
io::persist_keybinding_edit as concealed_edit;` and, in the calling file,
`use crate::config::persist_preset_slot as save;` would leave a call site the
pins cannot recognise, so the rename itself fails. Inside `src/config/` the
editors are named only where they are declared (`io.rs`) and where they leave
(`mod.rs`), so a wrapper in a third file cannot re-offer the capability under a
name of its own. And the editors' path-taking twins, which exist for the suites,
are named in production nowhere at all.

Sixth, a write may not leave one of those files as a *value*. Everything above
reads functions — the surfaces enumerate them, and the capability walk follows
calls between them — and a `pub const PERSIST_ANY_CONFIG: fn(..) =
ConfigDocument::save_with_backup;` declares no function at all. It is the same
capability with a type in place of a body, exported, callable by anyone who can
name it, and it used to walk out through a matching `pub use document::
PERSIST_ANY_CONFIG;` with every pin here reading past both halves: not a
primitive, not a function, not a rename. Both halves now fail. In the three
implementing files a write-capable name inside a `const` or `static`
initializer fails by name and line; and a `pub use` anywhere in `src/config/`
fails when it re-exports a `fn`, `const`, or `static` *declared* in
`document.rs` or `io.rs` and not among the three reviewed editors. The second
half asks what a name is and where it came from rather than whether it is on a
list of known writers, which is what makes it hold for a value nothing here has
heard of. Types are deliberately outside it: `ConfigDocument` and
`ConfigEditOutcome` are most of what the module re-exports, they carry no
capability, and the methods on them are already pinned by the surfaces.

Scope and limits: this is a name-level guardrail over `src/` and
`configurator/src/`, not the proof. It catches a new caller of the config write
primitives; it cannot catch a brand-new write built directly on
`durable_io::write_text_atomic` under a different name. Renaming is checked at
`use` items, which is where a rename has to be written; a call reached some
other way still names the function and is caught by the pins (a module alias,
`use crate::config::io as elsewhere;` followed by `elsewhere::persist_quick_color
(..)`, spells the writer at the call site). Three known holes remain: a macro
defined inside `src/config/` that expands to a write, whose invocation names only
the macro; any path that composes the identifier rather than writing it, such
as `concat_idents!`; and a writer put into a value at *runtime* rather than in a
`const` — a struct field assigned `save_with_backup` inside some function body,
handed out later through that struct. The last one is narrower than it sounds:
the assignment spells the writer's name inside a function, which is exactly what
the capability walk reads, so the enclosing function becomes write-capable and
the surface pins fail it if it is visible outside the file. What is genuinely
uncovered is a value built and consumed entirely within one writer file's
private functions, which reaches no further than those functions already do. The behavioural proof is the loader immutability fixture in
`src/config/tests/immutability.rs` plus the per-flow "only this key changed"
tests beside each gesture that queues a write. Test sources are exempt, and
whether a file is one is read from the `#[cfg(test)]` on the `mod` item that
brings it in rather than from the shape of its path — a directory called
`tests` under `src/` is production code unless the compiler is told otherwise.
`src/daemon/tests.rs` keeps its own
in-tree version of this check for the daemon subtree, which stays useful because
it runs under `cargo test` rather than only in the full lint gate.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parent.parent

DOCUMENT_SOURCE = Path("src/config/document.rs")
IO_SOURCE = Path("src/config/io.rs")
CONFIG_MODULE = Path("src/config/mod.rs")
CONFIGURATOR_ADAPTER = Path("configurator/src/app/io.rs")

# The files that implement the single durable write, plus the configurator
# adapter that performs it. `src/config/document/merge.rs` is deliberately
# absent: it rewrites a TOML tree in memory and never touches the filesystem, so
# it names none of the primitives below.
WRITE_ALLOWLIST = {
    DOCUMENT_SOURCE,  # ConfigDocument::save_with_backup, the only durable write
    IO_SOURCE,  # the atomic write, the timestamped .bak copy, the parent mkdir
    CONFIGURATOR_ADAPTER,  # the configurator's explicit Save
}

# Every name that reaches the filesystem on the config path.
WRITE_PRIMITIVES = (
    "save_with_backup",
    "write_config_text_atomic",
    "create_config_backup",
    "prepare_config_parent",
)

WRITE_PATTERNS = tuple(
    re.compile(rf"\b{re.escape(name)}\b") for name in WRITE_PRIMITIVES
)

# The write-capable surface `src/config/document.rs` is allowed to expose.
#
# One entry, and it is the whole of the application's durable config write.
# The file is exempt from the primitive scan below — it owns the merge, the
# backup, and the rename — so a second exported entry point here would be a
# second way for `config.toml` to change with nothing else in this check to
# notice it. Matching on the *name* is not enough: an entry point called
# anything at all reaches the file if it can reach `merge_and_write`, so what is
# pinned is the reachability.
DOCUMENT_WRITE_SURFACE = {
    "save_with_backup",
}

# What "can write" means inside `document.rs`: the merge-and-rename step every
# save funnels through, plus the primitives it calls. The walk is a fixpoint, so
# naming the step is belt and braces — a helper that called the primitives
# directly would be caught by them — but it keeps the pin meaningful if the
# primitives ever move behind another name inside this file.
DOCUMENT_WRITE_SEEDS = {
    "merge_and_write",
    *WRITE_PRIMITIVES,
}

# The write-capable surface `src/config/io.rs` is allowed to expose, by name.
#
# The file is exempt from the primitive scan above — it *is* the primitive — so
# without this a new `pub fn` there could wrap the atomic write under any name,
# be re-exported, and be called from anywhere with nothing to notice it. Each
# entry is a reviewed capability:
#
# * the three narrow editors, one per explicit user gesture;
# * their path-taking twins, the same gesture without the process environment,
#   which the suites drive and which are `#[cfg(test)]`-gated so a production
#   build has no such function at all; and
# * the three primitives themselves, which `pub(super)` already confines to the
#   config module and which `document.rs` is the only caller of.
#
# Anything else in `io.rs` that can reach a primitive — directly or through the
# file's own private helpers — is a new way for `config.toml` to change, which
# is a review decision rather than an implementation detail.
IO_WRITE_SURFACE = {
    "persist_keybinding_edit",
    "persist_keybinding_edit_at",
    "persist_preset_slot",
    "persist_preset_slot_at",
    "persist_quick_color",
    "persist_quick_color_at",
    *WRITE_PRIMITIVES,
}

# The one file that calls the editors. The writes run on a worker thread rather
# than on the overlay's dispatch thread — a parse, a file copy, a rename, and two
# fsyncs are not work for the thread that reads input and paints — so the three
# gestures hand a typed edit to `config_edits.rs` and route its completion when
# it comes back. That makes this module the caller of record for all three.
EDIT_WORKER = Path("src/backend/wayland/config_edits.rs")

# Where each gesture is still decided, and where its "only this key changed"
# test lives. Not writers any more, but named so the walk below can assert it
# still reaches them: a check that stopped seeing these files would stop proving
# anything about the gestures.
SHORTCUT_CALL_SITE = Path("src/backend/wayland/state/keybindings.rs")
PRESET_CALL_SITE = Path("src/backend/wayland/state/toolbar/events/presets.rs")
QUICK_COLOR_CALL_SITE = Path("src/backend/wayland/state/toolbar/events/quick_colors.rs")

# The overlay's narrow editors: one entry per explicit user gesture that may
# change `config.toml`, mapped to the production files allowed to invoke it.
# Each is declared in `src/config/io.rs`, writes exactly its own key on top of
# `ConfigDocument::config()`, and backs the file up through `save_with_backup`.
#
# Adding a caller here widens where the file can change from. Do it only with
# the same scrutiny the gestures themselves got: the edit must be explicit, the
# failure must degrade to an in-memory change with honest wording, and there
# must be an "only this key changed" test beside the gesture it belongs to.
NARROW_WRITERS = {
    "persist_keybinding_edit": {EDIT_WORKER},
    "persist_preset_slot": {EDIT_WORKER},
    "persist_quick_color": {EDIT_WORKER},
}

# The editors' path-taking twins: the same gesture against an explicit file,
# which the suites drive and production has no use for. They are `#[cfg(test)]
# pub(crate)`, so a production build has no such function to call at all; this
# scan is the second line, and says *why* rather than reporting an unresolved
# name. No production file may name one — it would be a config write at a path
# nobody reviewed, and the word boundary that pins the editors above ends before
# the `_at`, so `NARROW_WRITERS` would never match the call.
PATH_TAKING_WRITERS = {f"{name}_at" for name in NARROW_WRITERS}

# The only value items the config module hands out of the two writer files. The
# three editors, which are the reviewed capability; everything else those files
# export is a type, and a type is not a way to call anything the pins here have
# not already read.
REEXPORTABLE_WRITER_VALUES = set(NARROW_WRITERS)

# The gate that keeps that first line in place. Dropping the `#[cfg(test)]` would
# hand every module in the crate a config write at a caller-chosen path again.
CFG_TEST_TWIN = {
    name: re.compile(rf"\#\[cfg\(test\)\]\s*pub\(crate\) fn {re.escape(name)}\b")
    for name in sorted(PATH_TAKING_WRITERS)
}

# Where the editors may be named inside the config module: declared in `io.rs`,
# re-exported from `mod.rs`. The module is exempt from caller collection because
# it owns the writers, so a wrapper anywhere else in it would be an unreviewed
# writer with a name of its own. Both files earn the exemption by being pinned
# on their own terms instead — `audit_io_write_surface` for the declarations and
# `audit_config_module_surface` for the re-exports.
EDITOR_HOME = {IO_SOURCE, CONFIG_MODULE}

# Subtrees whose former write authority this check replaces. If the walk stops
# reaching them, it proves nothing, so their presence is asserted rather than
# assumed.
EXPECTED_SCANNED = {
    DOCUMENT_SOURCE,
    IO_SOURCE,
    CONFIGURATOR_ADAPTER,
    Path("src/backend/wayland/state.rs"),  # the overlay that owned the writer
    Path("src/daemon/tray/runtime.rs"),  # the tray that owned the resume toggle
    Path("src/backend/wayland/backend/state_init/config.rs"),  # startup load
    Path("configurator/src/app/update/config.rs"),  # Save's message handling
    EDIT_WORKER,  # the overlay's off-dispatch config-edit worker
    SHORTCUT_CALL_SITE,  # the overlay's shortcut editor
    PRESET_CALL_SITE,  # the overlay's preset slots
    QUICK_COLOR_CALL_SITE,  # the overlay's quick-color palette
}

# String bodies, char literals, and comments carry no capability, and a `{` in
# one would desynchronise the `#[cfg(test)]` block tracking below. Blanked in
# place so byte offsets and line numbers keep pointing at the real source.
MASKED_SPANS = re.compile(
    r"""
      //[^\n]*                              # line comment
    | /\*.*?\*/                             # block comment
    | (?<![A-Za-z0-9_])b?r(?P<hashes>\#*)"  # raw string, byte or not
        .*?
      "(?P=hashes)
    | b?"(?:\\.|[^"\\])*"                   # string, byte or not
    | '(?:\\.|[^'\\])'                      # char literal, never a lifetime
    """,
    re.VERBOSE | re.DOTALL,
)

CFG_TEST = re.compile(r"\#\[cfg\(test\)\]")

# `#[cfg(test)] mod foo;`, the declaration that makes a whole file test-only.
# Any further attributes and the visibility sit between the two. An inline
# `#[cfg(test)] mod tests { .. }` counts the same way: its own children are files
# in the directory beside it, and they are just as test-only.
CFG_TEST_MOD = re.compile(
    r"\#\[cfg\(test\)\]\s*(?:\#\[[^\]]*\]\s*)*"
    r"(?:pub(?:\s*\([^)]*\))?\s+)?mod\s+(?P<name>\w+)\s*[;{]"
)

# A function item, at any indentation and any visibility. Methods inside `impl`
# blocks match too: a `pub fn` on `Config` reaches as far as a free one.
FUNCTION_ITEM = re.compile(
    r"(?m)^[ \t]*(?P<visibility>pub(?:\s*\([^)]*\))?\s+)?"
    r"(?:const\s+|async\s+|unsafe\s+|extern\s+\"[^\"]*\"\s+)*"
    r"fn\s+(?P<name>\w+)"
)

# A `const` or `static` item, at any indentation and any visibility, including
# an associated one inside an `impl`. The `: ` is what tells it from a `const
# fn`, whose name is followed by its parameter list.
CONST_ITEM = re.compile(
    r"(?m)^[ \t]*(?P<visibility>pub(?:\s*\([^)]*\))?\s+)?"
    r"(?P<keyword>const|static)\s+(?:mut\s+)?(?P<name>\w+)\s*:"
)

USE_ITEM = re.compile(r"(?m)^[ \t]*(?:pub(?:\s*\([^)]*\))?\s+)?use\b[^;]*;", re.DOTALL)

# A path prefix inside a `use` item. Masking these leaves the names the item
# actually brings into scope: `pub use document::{ConfigDocument, X};` names two
# things, and `document` is not one of them — it is the module they came from,
# and it collides with method names all over the writer files.
USE_PATH_SEGMENT = re.compile(r"\b\w+\s*::")

# What is left in a `use` item after that masking and is not a name it imports.
USE_KEYWORDS = frozenset({"use", "pub", "crate", "self", "super", "as", "in"})

# An `impl` block header, up to the `{` that opens its body. What tells a trait
# impl from an inherent one is the `for`; the `for<'a>` of a higher-ranked bound
# is not it, hence the lookahead.
IMPL_BLOCK = re.compile(r"(?m)^[ \t]*(?:unsafe\s+)?impl\b(?P<header>[^{;]*)\{")

IMPL_FOR = re.compile(r"\bfor\b(?!\s*<)")

# A `trait` declaration: the surface it hands out, and the bodies of any default
# methods that travel with it.
TRAIT_BLOCK = re.compile(
    r"(?m)^[ \t]*(?P<visibility>pub(?:\s*\([^)]*\))?\s+)?(?:unsafe\s+)?"
    r"trait\s+(?P<name>\w+)[^{;]*\{"
)

# A method signature inside a trait declaration, body or no body. The bodiless
# ones are why this exists: `function_items` skips them, and they are exactly
# how a trait offers a write whose implementation sits elsewhere in the file.
TRAIT_METHOD = re.compile(
    r"(?m)^[ \t]*(?:const\s+|async\s+|unsafe\s+|extern\s+\"[^\"]*\"\s+)*fn\s+(?P<name>\w+)"
)

# `<name> as <alias>`, for every name that can write once it is in scope. The
# rest of this check recognises a write by the name at the call site, so a
# rename is the one way to put a call beyond it — whether the rename hands the
# capability on (`pub use`) or only conceals the call in the file that makes it
# (a plain `use`). The rename is what fails; the alias is named in the message
# so the reader can find the call it was hiding.
RENAME_PATTERNS = {
    name: re.compile(rf"\b{re.escape(name)}\s+as\s+(?P<alias>\w+)")
    for name in sorted(IO_WRITE_SURFACE)
}

IDENTIFIER = re.compile(r"\b\w+\b")


def blank(match: re.Match[str]) -> str:
    return "".join("\n" if character == "\n" else " " for character in match.group(0))


def mask_source(source: str) -> str:
    return MASKED_SPANS.sub(blank, source)


def block_end(masked: str, opening: int) -> int | None:
    """Offset just past the `}` closing the block that starts at `opening`."""
    depth = 0
    for index in range(opening, len(masked)):
        if masked[index] == "{":
            depth += 1
        elif masked[index] == "}":
            depth -= 1
            if depth == 0:
                return index + 1
    return None


def cfg_test_spans(masked: str) -> tuple[list[tuple[int, int]], list[str]]:
    """Offset ranges covered by `#[cfg(test)]` items, and any tracking failure.

    Inline test modules live in production files, so exempting whole files by
    name is not enough; the attributed item is what has to be exempt.
    """
    spans: list[tuple[int, int]] = []
    problems: list[str] = []
    for attribute in CFG_TEST.finditer(masked):
        index = attribute.end()
        while index < len(masked) and masked[index] not in "{;":
            index += 1
        if index >= len(masked):
            problems.append("a `#[cfg(test)]` item has neither a body nor a `;`")
            continue
        if masked[index] == ";":
            # `#[cfg(test)] mod tests;` and `#[cfg(test)] use ...;` bring in no
            # inline code; the module file is exempt by its own path.
            continue
        end = block_end(masked, index)
        if end is None:
            problems.append("a `#[cfg(test)]` block never closes")
            continue
        spans.append((attribute.start(), end))
    return spans, problems


def crate_source_root(relative: Path) -> Path | None:
    """The crate root file whose `mod` items begin `relative`'s module chain."""
    if relative.parts[:1] == ("src",):
        base = Path("src")
    elif relative.parts[:2] == ("configurator", "src"):
        base = Path("configurator/src")
    else:
        return None
    for name in ("lib.rs", "main.rs"):
        candidate = base / name
        if (ROOT / candidate).is_file():
            return candidate
    return None


def module_file(module: Path) -> Path | None:
    """The file that holds `module`'s own items, `foo.rs` or `foo/mod.rs`."""
    for candidate in (module.with_suffix(".rs"), module / "mod.rs"):
        if (ROOT / candidate).is_file():
            return candidate
    return None


_CFG_TEST_MODULES: dict[Path, frozenset[str]] = {}


def cfg_test_modules(declaring: Path) -> frozenset[str]:
    """Child modules `declaring` compiles only under `cfg(test)`."""
    cached = _CFG_TEST_MODULES.get(declaring)
    if cached is not None:
        return cached
    masked = mask_source((ROOT / declaring).read_text())
    names = frozenset(match.group("name") for match in CFG_TEST_MOD.finditer(masked))
    _CFG_TEST_MODULES[declaring] = names
    return names


def is_test_source(relative: Path) -> bool:
    """Whether Rust compiles this file only under `cfg(test)`.

    The shape of the path is not evidence. A directory called `tests` inside
    `src/` is ordinary production code unless the `mod` item that brings it in
    says otherwise, and a file dropped into one would otherwise be exempt from
    every check here — a config write nobody reviewed, in a file that only looks
    like a test. What the compiler reads is the declaration, so that is what
    this reads: any module on the chain gated with `#[cfg(test)]` makes the file
    below it test-only, which is exactly how `mod tests;` earns its exemption.
    """
    root = crate_source_root(relative)
    if root is None:
        return False
    base = root.parent
    segments = list(relative.relative_to(base).parts)
    if segments[-1] == "mod.rs":
        segments.pop()
    else:
        segments[-1] = segments[-1].removesuffix(".rs")

    declaring: Path | None = root
    module = base
    for segment in segments:
        if declaring is None:
            return False
        if segment in cfg_test_modules(declaring):
            return True
        module = module / segment
        declaring = module_file(module)
    return False


def rust_sources() -> list[Path]:
    roots = (ROOT / "src", ROOT / "configurator" / "src")
    return sorted(path for root in roots for path in root.rglob("*.rs"))


def line_of(source: str, offset: int) -> int:
    return source.count("\n", 0, offset) + 1


def audit_sites() -> tuple[list[str], set[Path]]:
    failures: list[str] = []
    scanned: set[Path] = set()
    editor_names = (*NARROW_WRITERS, *sorted(PATH_TAKING_WRITERS))
    editor_callers: dict[str, set[Path]] = {name: set() for name in editor_names}
    editor_patterns = {
        name: re.compile(rf"\b{re.escape(name)}\b") for name in editor_names
    }
    for absolute in rust_sources():
        relative = absolute.relative_to(ROOT)
        scanned.add(relative)
        source = absolute.read_text()
        masked = mask_source(source)
        spans, problems = cfg_test_spans(masked)
        is_test = is_test_source(relative)
        if not is_test:
            failures.extend(f"{relative}: {problem}" for problem in problems)
            failures.extend(renamed_imports(relative, source, masked, spans))

        # Narrow-editor call sites are pinned outside the config module, which
        # declares and re-exports them. A rogue write inside `src/config/` is
        # still caught: only `io.rs` and `document.rs` are allowlisted for the
        # primitives, so every other file there is scanned for them below, and
        # the editors themselves are confined to `EDITOR_HOME` here.
        in_config_module = relative.parts[:2] == ("src", "config")
        if not is_test and not in_config_module:
            for name, pattern in editor_patterns.items():
                for hit in pattern.finditer(masked):
                    if any(start <= hit.start() < end for start, end in spans):
                        continue
                    editor_callers[name].add(relative)
        elif not is_test and relative not in EDITOR_HOME:
            for name, pattern in editor_patterns.items():
                for hit in pattern.finditer(masked):
                    if any(start <= hit.start() < end for start, end in spans):
                        continue
                    failures.append(
                        f"{relative}:{line_of(source, hit.start())}: names the "
                        f"narrow config writer `{name}` inside the config module; "
                        "the editors are declared in io.rs and leave through "
                        "mod.rs, so a wrapper here would carry the capability out "
                        "under a name the call-site pins never see"
                    )

        if relative in WRITE_ALLOWLIST or is_test:
            continue
        lines = source.splitlines()
        for pattern in WRITE_PATTERNS:
            for hit in pattern.finditer(masked):
                if any(start <= hit.start() < end for start, end in spans):
                    continue
                number = line_of(source, hit.start())
                text = lines[number - 1].strip() if number <= len(lines) else ""
                failures.append(
                    f"{relative}:{number}: config write capability "
                    f"`{hit.group(0)}` outside the reviewed writers: {text}"
                )

    for name, expected in NARROW_WRITERS.items():
        found = editor_callers[name]
        for unexpected in sorted(found - expected):
            failures.append(
                f"{unexpected}: unreviewed caller of the narrow config writer "
                f"`{name}`; record it in NARROW_WRITERS if this gesture should "
                "be able to change config.toml"
            )
        for missing in sorted(expected - found):
            failures.append(
                f"{missing}: expected to call `{name}` but does not; the pinned "
                "call site moved, so this check no longer describes the code"
            )
    for name in sorted(PATH_TAKING_WRITERS):
        for caller in sorted(editor_callers[name]):
            failures.append(
                f"{caller}: production code names `{name}`; the path-taking "
                "twins take the file to write from their caller and exist for "
                "the suites, so a gesture that needs one is a new writer to "
                "review, not an implementation detail"
            )
    return failures, scanned


def renamed_imports(
    relative: Path, source: str, masked: str, spans: list[tuple[int, int]]
) -> list[str]:
    """Every `use ... as ...` that renames a name capable of writing the file.

    Both shapes matter and neither is legitimate here. A `pub use` rename hands
    the capability to the whole crate under a name nothing pins, and a plain
    `use` rename conceals the call in the file that makes it — the call-site
    pinning above matches `persist_preset_slot(..)`, never `save(..)`.
    """
    failures: list[str] = []
    for item in USE_ITEM.finditer(masked):
        if any(start <= item.start() < end for start, end in spans):
            continue
        exported = item.group(0).lstrip().startswith("pub")
        for name, pattern in RENAME_PATTERNS.items():
            rename = pattern.search(item.group(0))
            if rename is None:
                continue
            failures.append(
                f"{relative}:{line_of(source, item.start())}: "
                f"{'re-exports' if exported else 'imports'} the config writer "
                f"`{name}` as `{rename.group('alias')}`; the writers are pinned "
                "by name, so they travel under their own or not at all"
            )
    return failures


class FunctionItem:
    """One `fn` in a file, with the source span of its body."""

    def __init__(self, name: str, visibility: str | None, start: int, end: int) -> None:
        self.name = name
        self.visibility = visibility
        self.start = start
        self.end = end

    @property
    def is_exported(self) -> bool:
        return self.visibility is not None


class ConstItem:
    """One `const` or `static` in a file, with the span of its declaration."""

    def __init__(self, keyword: str, name: str, start: int, end: int) -> None:
        self.keyword = keyword
        self.name = name
        self.start = start
        self.end = end


def item_end(masked: str, opening: int) -> int | None:
    """Offset just past the `;` that ends the item starting at `opening`."""
    depth = 0
    for index in range(opening, len(masked)):
        character = masked[index]
        if character in "([{":
            depth += 1
        elif character in ")]}":
            depth -= 1
        elif character == ";" and depth == 0:
            return index + 1
    return None


def const_items(masked: str, exclude: list[tuple[int, int]]) -> list[ConstItem]:
    """Every `const` or `static` defined outside `exclude`, in source order."""
    items: list[ConstItem] = []
    for match in CONST_ITEM.finditer(masked):
        if any(start <= match.start() < end for start, end in exclude):
            continue
        end = item_end(masked, match.end())
        if end is None:
            continue
        items.append(
            ConstItem(match.group("keyword"), match.group("name"), match.start(), end)
        )
    return items


def audit_const_initializers(
    relative: Path,
    source: str,
    masked: str,
    capable: set[str],
    exclude: list[tuple[int, int]],
) -> list[str]:
    """No write leaves an implementing file as a value.

    The surface pins read `fn` items and walk the calls between them, so what
    they see is functions. A `pub const PERSIST_ANY_CONFIG: fn(..) =
    ConfigDocument::save_with_backup;` declares no function at all: it is the
    same capability with a type instead of a body, exported, callable by anyone
    who can name it, and invisible to every walk here. The initializer is where
    the writer's name has to appear for that to work, so that is what this
    reads.
    """
    failures: list[str] = []
    for item in const_items(masked, exclude):
        body = masked[item.start : item.end]
        for name in sorted({token for token in IDENTIFIER.findall(body)} & capable):
            failures.append(
                f"{relative}:{line_of(source, item.start)}: `{item.keyword} "
                f"{item.name}` names `{name}`, which can write config.toml; a "
                "writer stored as a value declares no function for the surface "
                "pins to read, so the writers here stay functions"
            )
    return failures


def function_items(masked: str, exclude: list[tuple[int, int]]) -> list[FunctionItem]:
    """Every function defined outside `exclude`, in source order."""
    items: list[FunctionItem] = []
    for match in FUNCTION_ITEM.finditer(masked):
        if any(start <= match.start() < end for start, end in exclude):
            continue
        index = match.end()
        while index < len(masked) and masked[index] not in "{;":
            index += 1
        if index >= len(masked) or masked[index] == ";":
            # A signature without a body: a trait item or an `extern` block.
            continue
        end = block_end(masked, index)
        if end is None:
            continue
        visibility = match.group("visibility")
        items.append(
            FunctionItem(
                match.group("name"),
                visibility.strip() if visibility else None,
                match.start(),
                end,
            )
        )
    return items


def write_capable_functions(
    masked: str, items: list[FunctionItem], seeds: set[str] | tuple[str, ...]
) -> set[str]:
    """Names that reach one of `seeds`, directly or through this file.

    Best-effort by design: it follows plain name references inside each body,
    which is what a wrapper in this file looks like. It cannot see through a
    function pointer stored in a struct, and it does not need to — the point is
    that a *new* write path has to be named here to be reviewed.
    """
    bodies = {item.name: masked[item.start : item.end] for item in items}
    references = {
        name: {token for token in IDENTIFIER.findall(body)}
        for name, body in bodies.items()
    }
    capable = {name for name, tokens in references.items() if tokens & set(seeds)}
    # Fixpoint: a caller of a write-capable function is write-capable too.
    changed = True
    while changed:
        changed = False
        for name, tokens in references.items():
            if name in capable:
                continue
            if tokens & capable:
                capable.add(name)
                changed = True
    return capable


def trait_spans(masked: str, exclude: list[tuple[int, int]]) -> list[tuple[int, int]]:
    """Ranges where a `fn` is callable without a `pub` of its own.

    A method in `impl Trait for Type` is reachable wherever the trait is, and a
    trait's own default method travels with the trait. Neither carries a
    visibility — the trait's is what counts — so `is_exported` reads `False` for
    both, and the surface pins would let a write walk out of the file behind one.
    """
    spans: list[tuple[int, int]] = []
    blocks = [
        match
        for match in IMPL_BLOCK.finditer(masked)
        if IMPL_FOR.search(match.group("header")) is not None
    ]
    # An inherent `impl Type` is deliberately not here: its methods carry their
    # own visibility, and the surface pins already read it.
    blocks.extend(TRAIT_BLOCK.finditer(masked))
    for match in blocks:
        if any(start <= match.start() < end for start, end in exclude):
            continue
        end = block_end(masked, match.end() - 1)
        if end is not None:
            spans.append((match.start(), end))
    return spans


def trait_declarations(
    masked: str, exclude: list[tuple[int, int]]
) -> list[tuple[str, int, set[str]]]:
    """Each `trait` block outside `exclude`: name, offset, and declared methods."""
    declarations: list[tuple[str, int, set[str]]] = []
    for match in TRAIT_BLOCK.finditer(masked):
        if any(start <= match.start() < end for start, end in exclude):
            continue
        end = block_end(masked, match.end() - 1)
        if end is None:
            continue
        body = masked[match.end() : end]
        methods = {item.group("name") for item in TRAIT_METHOD.finditer(body)}
        declarations.append((match.group("name"), match.start(), methods))
    return declarations


def audit_trait_writers(
    relative: Path,
    source: str,
    masked: str,
    items: list[FunctionItem],
    capable: set[str],
    exclude: list[tuple[int, int]],
) -> list[str]:
    """No write leaves an implementing file through a trait.

    Everything else here reads the `pub` an item carries, and a trait method
    carries none. An `impl SomeTrait for ConfigDocument` whose method calls the
    merge step is callable from anywhere the trait is in scope, and the trait
    itself is one `pub use` away — so the file would have a second durable
    writer, exported, under a name no pin here ever sees. The surfaces above
    enumerate inherent and free functions; a write reachable through a trait is
    out of bounds whatever it is called and whatever visibility it was given.
    """
    failures: list[str] = []
    spans = trait_spans(masked, exclude)
    for item in items:
        if item.name not in capable:
            continue
        if not any(start <= item.start < end for start, end in spans):
            continue
        failures.append(
            f"{relative}:{line_of(source, item.start)}: `fn {item.name}` can write "
            "config.toml from inside a trait; a trait's methods are callable "
            "wherever the trait is and carry no visibility of their own, so the "
            "writers here stay inherent or free functions"
        )
    failures.extend(audit_trait_declarations(relative, source, masked, capable, exclude))
    return failures


def audit_trait_declarations(
    relative: Path,
    source: str,
    masked: str,
    capable: set[str],
    exclude: list[tuple[int, int]],
) -> list[str]:
    """A trait declared here may not name a method that can write.

    Separate from the walk above because a trait's method signatures need no
    bodies: `fn persist_any_config(&self);` is the whole export, and the impl
    that fills it in is an ordinary block elsewhere in the file. What is pinned
    is the offer — a trait whose surface can write is a write capability handed
    to every caller that can name the trait.
    """
    failures: list[str] = []
    for name, offset, methods in trait_declarations(masked, exclude):
        for method in sorted(methods & capable):
            failures.append(
                f"{relative}:{line_of(source, offset)}: `trait {name}` declares "
                f"`{method}`, which can write config.toml; a trait carries the "
                "capability to every caller that can name it, so the writers "
                "here are not offered through one"
            )
    return failures


def audit_io_write_surface(io_source: str, masked_io: str, test_spans) -> list[str]:
    """`io.rs` is exempt from the primitive scan, so its own surface is pinned.

    Every function here that can reach a write primitive and is visible outside
    the file has to be one of the reviewed editors. A new `pub fn` wrapping the
    atomic write would otherwise be re-exportable and callable from anywhere,
    with the rest of this check none the wiser.
    """
    failures: list[str] = []
    items = function_items(masked_io, test_spans)
    if not any(item.name == "persist_keybinding_edit" for item in items):
        return [
            f"{IO_SOURCE}: the function scan found no narrow editor; its shape "
            "assumption about the file no longer holds"
        ]

    capable = write_capable_functions(masked_io, items, WRITE_PRIMITIVES)
    failures.extend(
        audit_trait_writers(IO_SOURCE, io_source, masked_io, items, capable, test_spans)
    )
    failures.extend(
        audit_const_initializers(
            IO_SOURCE,
            io_source,
            masked_io,
            capable | set(WRITE_PRIMITIVES),
            test_spans,
        )
    )
    for item in items:
        if not item.is_exported or item.name not in capable:
            continue
        if item.name in IO_WRITE_SURFACE:
            continue
        failures.append(
            f"{IO_SOURCE}:{line_of(io_source, item.start)}: `{item.visibility} fn "
            f"{item.name}` can write config.toml but is not one of the reviewed "
            "editors; record it in IO_WRITE_SURFACE if this is a new user gesture"
        )
    return failures


def audit_document_write_surface(
    document_source: str, masked_document: str, test_spans
) -> list[str]:
    """`document.rs` is exempt from the primitive scan, so its surface is pinned.

    The same walk `io.rs` gets, for the same reason: this file owns the merge,
    the backup, and the rename, so matching on a name — anything containing
    "save", say — pins the spelling rather than the capability. A `pub fn
    persist_any_config(..) { self.merge_and_write(..) }` writes `config.toml`
    just as `save_with_backup` does, and would leave the application with two
    durable writers, one of them unreviewed and callable from anywhere the
    document type is.
    """
    failures: list[str] = []
    items = function_items(masked_document, test_spans)
    if not any(item.name == "save_with_backup" for item in items):
        return [
            f"{DOCUMENT_SOURCE}: the function scan found no document save; its "
            "shape assumption about the file no longer holds"
        ]
    if not any(item.name in DOCUMENT_WRITE_SEEDS for item in items):
        return [
            f"{DOCUMENT_SOURCE}: the function scan found none of the write steps "
            f"({', '.join(sorted(DOCUMENT_WRITE_SEEDS))}); its shape assumption "
            "about the file no longer holds"
        ]

    capable = write_capable_functions(masked_document, items, DOCUMENT_WRITE_SEEDS)
    failures.extend(
        audit_trait_writers(
            DOCUMENT_SOURCE, document_source, masked_document, items, capable, test_spans
        )
    )
    failures.extend(
        audit_const_initializers(
            DOCUMENT_SOURCE,
            document_source,
            masked_document,
            capable | set(DOCUMENT_WRITE_SEEDS),
            test_spans,
        )
    )
    for item in items:
        if not item.is_exported or item.name not in capable:
            continue
        if item.name in DOCUMENT_WRITE_SURFACE:
            continue
        failures.append(
            f"{DOCUMENT_SOURCE}:{line_of(document_source, item.start)}: "
            f"`{item.visibility} fn {item.name}` can write config.toml but is not "
            "the reviewed document save; the application has exactly one durable "
            "writer, so record it in DOCUMENT_WRITE_SURFACE only if that changed"
        )
    return failures


def enclosing_function(items: list[FunctionItem], offset: int) -> FunctionItem | None:
    """The innermost function whose body contains `offset`."""
    innermost: FunctionItem | None = None
    for item in items:
        if item.start <= offset < item.end and (
            innermost is None or item.start > innermost.start
        ):
            innermost = item
    return innermost


def writer_value_items() -> dict[str, tuple[Path, str]]:
    """Every `fn`, `const`, and `static` declared in the two writer files.

    Names, not capabilities. The surface pins above answer what can *write*, by
    walking calls between functions, and they are the right tool for a function.
    They are the wrong tool for everything else a name can be: a `const` holding
    a function pointer declares no function, so no walk reaches it, and it
    leaves the module under a name nothing here has ever heard of. What can be
    said about such a name honestly is where it was declared — and a value
    declared in a file that owns the durable write is not something the config
    module hands out without a reason on the record.

    Types are deliberately absent. `ConfigDocument` and `ConfigEditOutcome` are
    most of what the module re-exports, they carry no capability of their own,
    and the methods on them are pinned by the surface audits.
    """
    items: dict[str, tuple[Path, str]] = {}
    for relative in (DOCUMENT_SOURCE, IO_SOURCE):
        masked = mask_source((ROOT / relative).read_text())
        spans, _ = cfg_test_spans(masked)
        for function in function_items(masked, spans):
            items.setdefault(function.name, (relative, "fn"))
        for constant in const_items(masked, spans):
            items[constant.name] = (relative, constant.keyword)
    return items


def use_leaf_names(item: str) -> set[str]:
    """The names a `use` item brings into scope, without their paths."""
    leaves = USE_PATH_SEGMENT.sub(lambda match: " " * len(match.group(0)), item)
    return {
        token for token in IDENTIFIER.findall(leaves) if token not in USE_KEYWORDS
    }


def audit_config_module_surface() -> list[str]:
    """`src/config/mod.rs` may name the writers only where it re-exports them.

    Everything else here recognises a write by the name at the call site, and
    this file is exempt from that collection because it is where the editors
    leave the module. The write-capability walk reads `io.rs` and nothing else,
    so without this a `pub fn concealed_edit(..) { io::persist_keybinding_edit(..) }`
    sitting here would be invisible to every check and callable from anywhere in
    the crate.

    The tightest honest rule is the one the file already lives by: it is a
    re-export list. A writer's name may appear inside a `use` or `pub use` item
    and nowhere else, and no function declared here may reach a writer at all —
    directly, through `crate::config::io::`, or through another wrapper in this
    file. Either failure names the function and the line.
    """
    source = (ROOT / CONFIG_MODULE).read_text()
    masked = mask_source(source)
    spans, problems = cfg_test_spans(masked)
    failures = [f"{CONFIG_MODULE}: {problem}" for problem in problems]
    use_spans = [(item.start(), item.end()) for item in USE_ITEM.finditer(masked)]
    items = function_items(masked, spans)

    for name in sorted(IO_WRITE_SURFACE):
        for hit in re.finditer(rf"\b{re.escape(name)}\b", masked):
            if any(start <= hit.start() < end for start, end in spans):
                continue
            if any(start <= hit.start() < end for start, end in use_spans):
                continue
            enclosing = enclosing_function(items, hit.start())
            where = (
                f"inside `fn {enclosing.name}`"
                if enclosing is not None
                else "outside any `use` item"
            )
            failures.append(
                f"{CONFIG_MODULE}:{line_of(source, hit.start())}: names the config "
                f"writer `{name}` {where}; this file re-exports the editors and "
                "does nothing else, so anything here that can call one carries the "
                "capability out under a name the call-site pins never see"
            )

    capable = write_capable_functions(masked, items, IO_WRITE_SURFACE)
    for item in items:
        if item.name not in capable:
            continue
        failures.append(
            f"{CONFIG_MODULE}:{line_of(source, item.start)}: `fn {item.name}` can "
            "reach a config writer; the config module's own file declares no "
            "functions, so this is an unreviewed writer with a name of its own"
        )
    # The loop above reads every function, whatever visibility it carries, so a
    # trait's default method is already in it. What is left is the offer with no
    # body: a `trait` here declaring a method something else in the file
    # implements.
    failures.extend(
        audit_trait_declarations(CONFIG_MODULE, source, masked, capable, spans)
    )
    return failures


def audit_module_reexports() -> list[str]:
    """The config module may re-export the editors, never the primitives.

    A `pub use` of the atomic write, the backup copy, or the document save would
    hand the capability to the whole crate under a path this check's call-site
    pinning says nothing about.

    Naming the primitives is not enough on its own, because a re-export need not
    name a writer to carry one. `pub use document::PERSIST_ANY_CONFIG;` names a
    `const` whose initializer is the document save, and every pin here reads
    past it: it is not a primitive, not a function, and not a rename. So the
    second half of this asks a question the shape of the export cannot dodge —
    which file declared the name, and as what. A `fn`, `const`, or `static` from
    one of the two files that own the durable write leaves the module only if
    the review that let the editors out covers it too; a type is not part of
    this, and types are most of what the module re-exports.

    Every production file in the module is scanned, not just `mod.rs`: a `pub
    use` in a submodule travels exactly as far, since the submodule is itself
    re-exported. Renaming is caught for all of `src/` by `renamed_imports`.
    """
    failures: list[str] = []
    values = writer_value_items()
    for relative in config_module_sources():
        source = (ROOT / relative).read_text()
        masked = mask_source(source)
        spans, _ = cfg_test_spans(masked)
        for item in USE_ITEM.finditer(masked):
            if any(start <= item.start() < end for start, end in spans):
                continue
            if not item.group(0).lstrip().startswith("pub"):
                continue
            for primitive in WRITE_PRIMITIVES:
                if re.search(rf"\b{re.escape(primitive)}\b", item.group(0)):
                    failures.append(
                        f"{relative}:{line_of(source, item.start())}: re-exports the "
                        f"write primitive `{primitive}`; the primitives stay inside the "
                        "config module and only the narrow editors leave it"
                    )
            for name in sorted(use_leaf_names(item.group(0))):
                if name in REEXPORTABLE_WRITER_VALUES or name not in values:
                    continue
                declared, keyword = values[name]
                failures.append(
                    f"{relative}:{line_of(source, item.start())}: re-exports "
                    f"`{name}`, a `{keyword}` declared in {declared}; the write "
                    "lives in that file, and the surface pins there read "
                    "functions — so a value leaving it carries whatever it holds "
                    "past them. Only the reviewed editors leave the module"
                )
    return failures


def config_module_sources() -> list[Path]:
    """Production files in `src/config/`, `mod.rs` first for readable output."""
    root = ROOT / "src" / "config"
    paths = [
        path.relative_to(ROOT)
        for path in root.rglob("*.rs")
        if not is_test_source(path.relative_to(ROOT))
    ]
    return sorted(paths, key=lambda path: (path != CONFIG_MODULE, path))


def audit_write_surface() -> list[str]:
    """The implementing files may not widen the capability they own."""
    failures: list[str] = []

    document = (ROOT / DOCUMENT_SOURCE).read_text()
    masked_document = mask_source(document)
    document_test_spans, document_problems = cfg_test_spans(masked_document)
    failures.extend(f"{DOCUMENT_SOURCE}: {problem}" for problem in document_problems)
    failures.extend(
        audit_document_write_surface(document, masked_document, document_test_spans)
    )
    if "pub fn save_with_backup" not in document:
        failures.append(
            f"{DOCUMENT_SOURCE}: `save_with_backup` is gone or renamed; "
            "this check no longer describes the code"
        )

    io_source = (ROOT / IO_SOURCE).read_text()
    for primitive in ("create_config_backup", "write_config_text_atomic", "prepare_config_parent"):
        if f"pub(super) fn {primitive}" not in io_source:
            failures.append(
                f"{IO_SOURCE}: `{primitive}` is no longer `pub(super)`; the write "
                "primitives must stay inside the config module"
            )

    # Each narrow editor must still be declared here, and must still build its
    # `updated` config on `document.config()` — the base that makes the merge
    # gate write one key. Basing it on `authored_config()` would hand the gate
    # every value the loader clamped or resolved as if the user had typed it.
    for name in NARROW_WRITERS:
        if f"pub fn {name}" not in io_source:
            failures.append(
                f"{IO_SOURCE}: narrow config writer `{name}` is gone or no longer "
                "declared here; this check no longer describes the code"
            )
    for name, pattern in CFG_TEST_TWIN.items():
        if pattern.search(io_source) is None:
            failures.append(
                f"{IO_SOURCE}: `{name}` is no longer a `#[cfg(test)] pub(crate) fn`; "
                "the path-taking twins exist for the suites, and an ungated one is a "
                "config write at a caller-chosen path available to the whole crate"
            )
    # Production code only: the inline suite reads `authored_config()` on
    # purpose, to prove the loader had something to repair in its fixture.
    masked_io = mask_source(io_source)
    test_spans, problems = cfg_test_spans(masked_io)
    failures.extend(f"{IO_SOURCE}: {problem}" for problem in problems)
    for hit in re.finditer(r"\bauthored_config\b", masked_io):
        if any(start <= hit.start() < end for start, end in test_spans):
            continue
        failures.append(
            f"{IO_SOURCE}:{line_of(io_source, hit.start())}: a narrow writer reads "
            "`authored_config()`; the edit base must be `document.config()` so the "
            "merge gate writes only the edited key"
        )
    failures.extend(audit_io_write_surface(io_source, masked_io, test_spans))
    failures.extend(audit_config_module_surface())
    failures.extend(audit_module_reexports())
    return failures


def main() -> int:
    failures, scanned = audit_sites()
    missing = sorted(str(path) for path in EXPECTED_SCANNED - scanned)
    if missing:
        failures.append("the walk missed expected sources, so it proves nothing: " + ", ".join(missing))
    failures.extend(audit_write_surface())

    if failures:
        print("config-writer audit failed:", file=sys.stderr)
        for failure in failures:
            print(f"  {failure}", file=sys.stderr)
        return 1
    print(f"config-writer audit passed ({len(scanned)} sources)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
