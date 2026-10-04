//! Production code outside the reviewed writers cannot write `config.toml`.
//!
//! `config.toml` is an authored input. It changes only through an explicit
//! user edit action, never as a side effect of running Wayscriber. Two kinds of
//! writer are allowed: the configurator's **Save**, which writes the whole
//! edited draft; and the overlay's **narrow editors** in `src/config/io.rs`, one
//! per explicit gesture, each of which rewrites only its own key and backs the
//! file up first. Everything else reads the file and never writes it. The
//! capability is one `use` away, so this checks for its absence.
//!
//! Six things are enforced:
//!
//! 1. The write primitives are named nowhere outside `src/config/` and the
//!    configurator's Save adapter.
//! 2. Each narrow editor's production call sites are pinned by name in
//!    [`NARROW_WRITERS`]; a new caller is a new place the file can change from.
//! 3. The write-capable surface of `document.rs` and `io.rs` is pinned. Both are
//!    exempt from the primitive scan because they *are* the write, so every
//!    exported function that can reach a write, directly or through the file's
//!    private helpers, must be listed. In those files and `mod.rs`, a write
//!    reachable through a trait fails whatever its name and visibility, and so
//!    does a trait declaring a method that can write.
//! 4. `src/config/mod.rs` is a re-export list: a writer's name may appear only
//!    inside a `use` item, and no function declared there may reach a writer.
//! 5. Names are pinned. The config module re-exports editors but never a
//!    primitive; no production file renames a write-capable name on import or
//!    re-export, or casts one with `as`; inside `src/config/` the editors are
//!    named only in `io.rs` and `mod.rs`; and the editors' path-taking twins are
//!    named in production nowhere.
//! 6. A write may not leave those files as a value: a write-capable name in a
//!    `const` or `static` initializer fails, and a `pub use` in `src/config/`
//!    fails when it re-exports a `fn`, `const`, or `static` declared in
//!    `document.rs` or `io.rs` other than the reviewed editors. Types are
//!    deliberately outside this rule.
//!
//! Scope and limits: a name-level guardrail over `src/` and
//! `configurator/src/`, not a proof. It cannot catch a brand-new write built
//! directly on `durable_io::write_text_atomic` under a different name, a macro
//! defined in `src/config/` that expands to a write, a composed identifier, or a
//! writer stored in a value at runtime and consumed within one writer file's
//! private functions. The behavioural proof is the loader immutability fixture
//! in `src/config/tests/immutability.rs` plus the per-flow "only this key
//! changed" tests beside each gesture. Test sources are exempt, and whether a
//! file is one is read from the `#[cfg(test)]` on the `mod` item that brings it
//! in, not from the shape of its path.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};

use super::source::{
    CFG_TEST, Cursor, TestSources, Tree, block_end, is_word_char, item_end, line_of, line_starts,
    mask_non_code, within, word_hits, word_index, words,
};

const DOCUMENT_SOURCE: &str = "src/config/document.rs";
const IO_SOURCE: &str = "src/config/io.rs";
const CONFIG_MODULE: &str = "src/config/mod.rs";
const CONFIGURATOR_ADAPTER: &str = "configurator/src/app/io.rs";

/// The files that implement the single durable write, plus the configurator
/// adapter that performs it. `src/config/document/merge.rs` rewrites a TOML
/// tree in memory and never touches the filesystem, so it is not here.
const WRITE_ALLOWLIST: [&str; 3] = [DOCUMENT_SOURCE, IO_SOURCE, CONFIGURATOR_ADAPTER];

/// Every name that reaches the filesystem on the config path.
const WRITE_PRIMITIVES: [&str; 4] = [
    "save_with_backup",
    "write_config_text_atomic",
    "create_config_backup",
    "prepare_config_parent",
];

/// The whole of the application's durable config write. What is pinned is
/// reachability, not spelling: anything that reaches `merge_and_write` writes.
const DOCUMENT_WRITE_SURFACE: [&str; 1] = ["save_with_backup"];
const DOCUMENT_WRITE_STEP: &str = "merge_and_write";

/// The three narrow editors, their `#[cfg(test)]` path-taking twins, and the
/// primitives, which `pub(super)` confines to the config module.
const IO_EDITOR_SURFACE: [&str; 6] = [
    "persist_keybinding_edit",
    "persist_keybinding_edit_at",
    "persist_preset_slot",
    "persist_preset_slot_at",
    "persist_quick_color",
    "persist_quick_color_at",
];

/// The overlay's config-edit worker calls every editor, off the dispatch thread.
const EDIT_WORKER: &str = "src/backend/wayland/config_edits.rs";

/// One entry per explicit user gesture that may change `config.toml`, mapped
/// to the production files allowed to invoke it. Adding a caller widens where
/// the file can change from; it needs an explicit gesture, an honest
/// in-memory fallback on failure, and an "only this key changed" test.
const NARROW_WRITERS: [(&str, &[&str]); 3] = [
    ("persist_keybinding_edit", &[EDIT_WORKER]),
    ("persist_preset_slot", &[EDIT_WORKER]),
    ("persist_quick_color", &[EDIT_WORKER]),
];

/// Where the editors may be named inside the config module.
const EDITOR_HOME: [&str; 2] = [IO_SOURCE, CONFIG_MODULE];

/// Sources whose former write authority this replaces. If the walk stops
/// reaching them, it proves nothing.
const EXPECTED_SCANNED: [&str; 11] = [
    DOCUMENT_SOURCE,
    IO_SOURCE,
    CONFIGURATOR_ADAPTER,
    "src/backend/wayland/state.rs",
    "src/daemon/tray/runtime.rs",
    "src/backend/wayland/backend/state_init/config.rs",
    "configurator/src/app/update/config.rs",
    EDIT_WORKER,
    "src/backend/wayland/state/keybindings.rs",
    "src/backend/wayland/state/toolbar/events/presets.rs",
    "src/backend/wayland/state/toolbar/events/quick_colors.rs",
];

/// Names left in a `use` item that are not names it imports.
const USE_KEYWORDS: [&str; 7] = ["use", "pub", "crate", "self", "super", "as", "in"];

type Spans = Vec<(usize, usize)>;

fn narrow_writer_names() -> impl Iterator<Item = &'static str> {
    NARROW_WRITERS.iter().map(|(name, _)| *name)
}

fn path_taking_writers() -> BTreeSet<String> {
    narrow_writer_names()
        .map(|name| format!("{name}_at"))
        .collect()
}

fn io_write_surface() -> BTreeSet<&'static str> {
    IO_EDITOR_SURFACE
        .into_iter()
        .chain(WRITE_PRIMITIVES)
        .collect()
}

fn names(items: impl IntoIterator<Item = &'static str>) -> BTreeSet<String> {
    items.into_iter().map(str::to_owned).collect()
}

/// Ranges covered by inline `#[cfg(test)]` items, and any tracking failure.
fn cfg_test_spans(masked: &str) -> (Spans, Vec<String>) {
    let mut spans = Vec::new();
    let mut problems = Vec::new();

    for (start, _) in masked.match_indices(CFG_TEST) {
        let after = start + CFG_TEST.len();
        match masked[after..]
            .find(['{', ';'])
            .map(|offset| after + offset)
        {
            None => problems.push("a `#[cfg(test)]` item has neither a body nor a `;`".into()),
            Some(index) if masked.as_bytes()[index] == b';' => {}
            Some(index) => match block_end(masked, index) {
                Some(end) => spans.push((start, end)),
                None => problems.push("a `#[cfg(test)]` block never closes".into()),
            },
        }
    }

    (spans, problems)
}

struct FunctionItem {
    name: String,
    visibility: Option<String>,
    start: usize,
    end: usize,
}

struct ConstItem {
    keyword: String,
    name: String,
    start: usize,
    end: usize,
}

struct UseItem {
    start: usize,
    end: usize,
    exported: bool,
}

/// Every function with a body outside `exclude`, at any indentation and
/// visibility, including methods.
fn function_items(masked: &str, exclude: &[(usize, usize)]) -> Vec<FunctionItem> {
    let mut items = Vec::new();

    for start in line_starts(masked) {
        if within(exclude, start) {
            continue;
        }

        let mut cursor = Cursor::new(masked, start);
        cursor.skip_blanks();
        let visibility = cursor.visibility().map(str::to_owned);
        cursor.function_qualifiers();
        if !cursor.keyword_then_space("fn") {
            continue;
        }
        let Some(name) = cursor.word() else {
            continue;
        };

        let Some(opening) = masked[cursor.at..]
            .find(['{', ';'])
            .map(|offset| cursor.at + offset)
        else {
            continue;
        };
        if masked.as_bytes()[opening] == b';' {
            continue;
        }
        if let Some(end) = block_end(masked, opening) {
            items.push(FunctionItem {
                name: name.to_owned(),
                visibility,
                start,
                end,
            });
        }
    }

    items
}

/// Every `const` or `static` item outside `exclude`, including associated ones.
fn const_items(masked: &str, exclude: &[(usize, usize)]) -> Vec<ConstItem> {
    let mut items = Vec::new();

    for start in line_starts(masked) {
        if within(exclude, start) {
            continue;
        }

        let mut cursor = Cursor::new(masked, start);
        cursor.skip_blanks();
        cursor.visibility();
        let keyword = if cursor.keyword_then_space("const") {
            "const"
        } else if cursor.keyword_then_space("static") {
            "static"
        } else {
            continue;
        };
        cursor.keyword_then_space("mut");
        let Some(name) = cursor.word() else {
            continue;
        };
        cursor.skip_whitespace();
        if !cursor.literal(":") {
            continue;
        }

        if let Some(end) = item_end(masked, cursor.at) {
            items.push(ConstItem {
                keyword: keyword.to_owned(),
                name: name.to_owned(),
                start,
                end,
            });
        }
    }

    items
}

fn use_items(masked: &str) -> Vec<UseItem> {
    let mut items = Vec::new();

    for start in line_starts(masked) {
        let mut cursor = Cursor::new(masked, start);
        cursor.skip_blanks();
        let exported = cursor.visibility().is_some();
        if !cursor.keyword("use") {
            continue;
        }

        if let Some(semicolon) = masked[cursor.at..].find(';') {
            items.push(UseItem {
                start,
                end: cursor.at + semicolon + 1,
                exported,
            });
        }
    }

    items
}

/// The names a `use` item brings into scope, without their paths.
fn use_leaf_names(text: &str) -> BTreeSet<String> {
    words(text)
        .filter(|(offset, word)| {
            let after = text[offset + word.len()..].trim_start();
            !after.starts_with("::") && !USE_KEYWORDS.contains(word)
        })
        .map(|(_, word)| word.to_owned())
        .collect()
}

/// `name` followed by `as alias` inside `text`.
fn renamed_alias<'a>(text: &'a str, name: &str) -> Option<&'a str> {
    word_hits(text, name).find_map(|offset| {
        let mut cursor = Cursor::new(text, offset + name.len());
        (cursor.skip_whitespace() > 0 && cursor.keyword_then_space("as"))
            .then(|| cursor.word())
            .flatten()
    })
}

/// Names that reach one of `seeds`, directly or through this file's functions.
fn write_capable_functions(
    masked: &str,
    items: &[FunctionItem],
    seeds: &BTreeSet<String>,
) -> BTreeSet<String> {
    let mut references: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for item in items {
        references.insert(
            &item.name,
            words(&masked[item.start..item.end])
                .map(|(_, word)| word)
                .collect(),
        );
    }

    let mut capable: BTreeSet<String> = references
        .iter()
        .filter(|(_, tokens)| tokens.iter().any(|token| seeds.contains(*token)))
        .map(|(name, _)| (*name).to_owned())
        .collect();
    loop {
        let reached: Vec<String> = references
            .iter()
            .filter(|(name, tokens)| {
                !capable.contains(**name) && tokens.iter().any(|token| capable.contains(*token))
            })
            .map(|(name, _)| (*name).to_owned())
            .collect();
        if reached.is_empty() {
            return capable;
        }
        capable.extend(reached);
    }
}

/// `impl Trait for Type` headers (not an inherent impl, not `for<'a>`), and
/// `trait` declarations: where a `fn` is callable without a `pub` of its own.
fn trait_spans(masked: &str, exclude: &[(usize, usize)]) -> Spans {
    let mut spans = Vec::new();

    for start in line_starts(masked) {
        let mut cursor = Cursor::new(masked, start);
        cursor.skip_blanks();
        cursor.keyword_then_space("unsafe");
        if !cursor.keyword("impl") {
            continue;
        }
        let Some((header, opening)) = header_until_brace(masked, cursor.at) else {
            continue;
        };
        if is_trait_impl(header)
            && !within(exclude, start)
            && let Some(end) = block_end(masked, opening)
        {
            spans.push((start, end));
        }
    }

    for declaration in trait_declarations(masked, exclude) {
        spans.push((declaration.start, declaration.end));
    }

    spans
}

/// Text up to the first `{`, unless a `;` comes first, like `[^{;]*\{`.
fn header_until_brace(masked: &str, from: usize) -> Option<(&str, usize)> {
    let offset = masked[from..].find(['{', ';'])?;
    let opening = from + offset;
    (masked.as_bytes()[opening] == b'{').then(|| (&masked[from..opening], opening))
}

fn is_trait_impl(header: &str) -> bool {
    word_hits(header, "for").any(|offset| !header[offset + 3..].trim_start().starts_with('<'))
}

struct TraitDeclaration {
    name: String,
    start: usize,
    end: usize,
    methods: BTreeSet<String>,
}

fn trait_declarations(masked: &str, exclude: &[(usize, usize)]) -> Vec<TraitDeclaration> {
    let mut declarations = Vec::new();

    for start in line_starts(masked) {
        if within(exclude, start) {
            continue;
        }

        let mut cursor = Cursor::new(masked, start);
        cursor.skip_blanks();
        cursor.visibility();
        cursor.keyword_then_space("unsafe");
        if !cursor.keyword_then_space("trait") {
            continue;
        }
        let Some(name) = cursor.word() else {
            continue;
        };
        let Some((_, opening)) = header_until_brace(masked, cursor.at) else {
            continue;
        };
        let Some(end) = block_end(masked, opening) else {
            continue;
        };

        declarations.push(TraitDeclaration {
            name: name.to_owned(),
            start,
            end,
            methods: trait_methods(&masked[opening + 1..end]),
        });
    }

    declarations
}

/// Method signatures in a trait body, with or without bodies.
fn trait_methods(body: &str) -> BTreeSet<String> {
    line_starts(body)
        .filter_map(|start| {
            let mut cursor = Cursor::new(body, start);
            cursor.skip_blanks();
            cursor.function_qualifiers();
            cursor
                .keyword_then_space("fn")
                .then(|| cursor.word().map(str::to_owned))
                .flatten()
        })
        .collect()
}

struct Source<'a> {
    path: &'a str,
    text: &'a str,
    masked: String,
    test_spans: Spans,
}

impl<'a> Source<'a> {
    fn read(tree: &'a Tree, path: &'a str) -> (Self, Vec<String>) {
        let text = tree
            .read(Path::new(path))
            .unwrap_or_else(|| panic!("{path} is missing"));
        let masked = mask_non_code(text);
        let (test_spans, problems) = cfg_test_spans(&masked);
        let problems = problems
            .into_iter()
            .map(|problem| format!("{path}: {problem}"))
            .collect();

        (
            Self {
                path,
                text,
                masked,
                test_spans,
            },
            problems,
        )
    }

    fn line(&self, offset: usize) -> usize {
        line_of(self.text, offset)
    }
}

struct Audit<'a> {
    tree: &'a Tree,
    failures: Vec<String>,
    test_sources: TestSources,
}

impl<'a> Audit<'a> {
    fn fail(&mut self, failure: String) {
        self.failures.push(failure);
    }

    fn is_test(&mut self, path: &Path) -> bool {
        self.test_sources.is_test(self.tree, path)
    }

    fn sites(&mut self) -> BTreeSet<PathBuf> {
        let mut scanned = BTreeSet::new();
        let twins = path_taking_writers();
        let editors: Vec<String> = narrow_writer_names()
            .map(str::to_owned)
            .chain(twins.iter().cloned())
            .collect();
        let mut callers: HashMap<String, BTreeSet<PathBuf>> = editors
            .iter()
            .map(|name| (name.clone(), BTreeSet::new()))
            .collect();
        let tracked: Vec<&str> = editors
            .iter()
            .map(String::as_str)
            .chain(WRITE_PRIMITIVES)
            .collect();
        let paths: Vec<PathBuf> = self
            .tree
            .paths_under("src")
            .chain(self.tree.paths_under("configurator/src"))
            .map(Path::to_path_buf)
            .collect();

        for path in paths {
            scanned.insert(path.clone());
            let display = path.to_string_lossy().into_owned();
            let text = self.tree.read(&path).expect("listed path");
            let masked = mask_non_code(text);
            let (spans, problems) = cfg_test_spans(&masked);
            let is_test = self.is_test(&path);
            let hits = word_index(&masked, &tracked);
            if !is_test {
                for problem in problems {
                    self.fail(format!("{display}: {problem}"));
                }
                self.renamed_imports(&display, text, &masked, &spans);
                self.writer_casts(&display, text, &masked, &spans, &hits);
            }

            let production_hits = |name: &str| {
                hits.get(name)
                    .into_iter()
                    .flatten()
                    .copied()
                    .filter(|hit| !within(&spans, *hit))
            };
            let in_config_module = path.starts_with("src/config");
            let in_editor_home = EDITOR_HOME.iter().any(|home| path == Path::new(home));
            for name in &editors {
                for hit in production_hits(name) {
                    if is_test {
                        continue;
                    }
                    if !in_config_module {
                        callers.get_mut(name).expect("editor").insert(path.clone());
                    } else if !in_editor_home {
                        self.fail(format!(
                            "{display}:{}: names the narrow config writer `{name}` inside the \
                             config module; the editors are declared in io.rs and leave through \
                             mod.rs, so a wrapper here would carry the capability out under a \
                             name the call-site pins never see",
                            line_of(text, hit)
                        ));
                    }
                }
            }

            let allowed = WRITE_ALLOWLIST
                .iter()
                .any(|allowed| path == Path::new(allowed));
            if allowed || is_test {
                continue;
            }
            let lines: Vec<&str> = text.lines().collect();
            for primitive in WRITE_PRIMITIVES {
                for hit in production_hits(primitive) {
                    let number = line_of(text, hit);
                    let line = lines.get(number - 1).map_or("", |line| line.trim());
                    self.fail(format!(
                        "{display}:{number}: config write capability `{primitive}` outside the \
                         reviewed writers: {line}"
                    ));
                }
            }
        }

        for (name, expected) in NARROW_WRITERS {
            let expected: BTreeSet<PathBuf> = expected.iter().map(PathBuf::from).collect();
            let found = &callers[name];
            for unexpected in found.difference(&expected) {
                self.fail(format!(
                    "{}: unreviewed caller of the narrow config writer `{name}`; record it in \
                     NARROW_WRITERS if this gesture should be able to change config.toml",
                    unexpected.display()
                ));
            }
            for missing in expected.difference(found) {
                self.fail(format!(
                    "{}: expected to call `{name}` but does not; the pinned call site moved, \
                     so this check no longer describes the code",
                    missing.display()
                ));
            }
        }
        for name in &twins {
            for caller in &callers[name] {
                self.fail(format!(
                    "{}: production code names `{name}`; the path-taking twins take the file \
                     to write from their caller and exist for the suites, so a gesture that \
                     needs one is a new writer to review, not an implementation detail",
                    caller.display()
                ));
            }
        }

        scanned
    }

    /// Every `use ... as ...` renaming a name that can write the file: a `pub
    /// use` rename hands the capability on under an unpinned name, and a plain
    /// rename conceals the call in the file that makes it.
    fn renamed_imports(&mut self, display: &str, text: &str, masked: &str, spans: &Spans) {
        for item in use_items(masked) {
            if within(spans, item.start) {
                continue;
            }

            let item_text = &masked[item.start..item.end];
            for name in io_write_surface() {
                let Some(alias) = renamed_alias(item_text, name) else {
                    continue;
                };
                let verb = if item.exported {
                    "re-exports"
                } else {
                    "imports"
                };
                self.fail(format!(
                    "{display}:{}: {verb} the config writer `{name}` as `{alias}`; the writers \
                     are pinned by name, so they travel under their own or not at all",
                    line_of(text, item.start)
                ));
            }
        }
    }

    /// `writer as T` outside a `use` item, such as a cast to a function pointer,
    /// turns the writer into a value that the name pins cannot follow.
    fn writer_casts(
        &mut self,
        display: &str,
        text: &str,
        masked: &str,
        spans: &Spans,
        hits: &HashMap<&str, Vec<usize>>,
    ) {
        let uses: Spans = use_items(masked)
            .iter()
            .map(|item| (item.start, item.end))
            .collect();

        for name in io_write_surface() {
            for &hit in hits.get(name).into_iter().flatten() {
                if within(spans, hit) || within(&uses, hit) {
                    continue;
                }

                let mut cursor = Cursor::new(masked, hit + name.len());
                if cursor.skip_whitespace() > 0 && cursor.keyword_then_space("as") {
                    self.fail(format!(
                        "{display}:{}: casts the config writer `{name}` with `as`; a writer \
                         turned into a value travels under no name, so the writers are only \
                         ever called by name",
                        line_of(text, hit)
                    ));
                }
            }
        }
    }

    /// No write leaves an implementing file as a value.
    fn const_initializers(&mut self, source: &Source, capable: &BTreeSet<String>) {
        for item in const_items(&source.masked, &source.test_spans) {
            let named: BTreeSet<&str> = words(&source.masked[item.start..item.end])
                .map(|(_, word)| word)
                .filter(|word| capable.contains(*word))
                .collect();
            for name in named {
                self.fail(format!(
                    "{}:{}: `{} {}` names `{name}`, which can write config.toml; a writer \
                     stored as a value declares no function for the surface pins to read, so \
                     the writers here stay functions",
                    source.path,
                    source.line(item.start),
                    item.keyword,
                    item.name
                ));
            }
        }
    }

    /// No write leaves an implementing file through a trait.
    fn trait_writers(
        &mut self,
        source: &Source,
        items: &[FunctionItem],
        capable: &BTreeSet<String>,
    ) {
        let spans = trait_spans(&source.masked, &source.test_spans);
        for item in items {
            if capable.contains(&item.name) && within(&spans, item.start) {
                self.fail(format!(
                    "{}:{}: `fn {}` can write config.toml from inside a trait; a trait's \
                     methods are callable wherever the trait is and carry no visibility of \
                     their own, so the writers here stay inherent or free functions",
                    source.path,
                    source.line(item.start),
                    item.name
                ));
            }
        }
        self.trait_declarations(source, capable);
    }

    /// A trait declared here may not name a method that can write, even
    /// without a body: the offer itself is the capability.
    fn trait_declarations(&mut self, source: &Source, capable: &BTreeSet<String>) {
        for declaration in trait_declarations(&source.masked, &source.test_spans) {
            for method in declaration.methods.intersection(capable) {
                self.fail(format!(
                    "{}:{}: `trait {}` declares `{method}`, which can write config.toml; a \
                     trait carries the capability to every caller that can name it, so the \
                     writers here are not offered through one",
                    source.path,
                    source.line(declaration.start),
                    declaration.name
                ));
            }
        }
    }

    fn io_write_surface(&mut self, source: &Source) {
        let items = function_items(&source.masked, &source.test_spans);
        if !items
            .iter()
            .any(|item| item.name == "persist_keybinding_edit")
        {
            self.fail(format!(
                "{IO_SOURCE}: the function scan found no narrow editor; its shape assumption \
                 about the file no longer holds"
            ));
            return;
        }

        let primitives = names(WRITE_PRIMITIVES);
        let capable = write_capable_functions(&source.masked, &items, &primitives);
        self.trait_writers(source, &items, &capable);
        self.const_initializers(source, &capable.union(&primitives).cloned().collect());

        let surface = io_write_surface();
        for item in &items {
            let Some(visibility) = &item.visibility else {
                continue;
            };
            if capable.contains(&item.name) && !surface.contains(item.name.as_str()) {
                self.fail(format!(
                    "{IO_SOURCE}:{}: `{visibility} fn {}` can write config.toml but is not one \
                     of the reviewed editors; record it in IO_WRITE_SURFACE if this is a new \
                     user gesture",
                    source.line(item.start),
                    item.name
                ));
            }
        }
    }

    fn document_write_surface(&mut self, source: &Source) {
        let items = function_items(&source.masked, &source.test_spans);
        let seeds: BTreeSet<String> = names(WRITE_PRIMITIVES)
            .into_iter()
            .chain([DOCUMENT_WRITE_STEP.to_owned()])
            .collect();
        if !items.iter().any(|item| item.name == "save_with_backup") {
            self.fail(format!(
                "{DOCUMENT_SOURCE}: the function scan found no document save; its shape \
                 assumption about the file no longer holds"
            ));
            return;
        }
        if !items.iter().any(|item| seeds.contains(&item.name)) {
            self.fail(format!(
                "{DOCUMENT_SOURCE}: the function scan found none of the write steps ({}); its \
                 shape assumption about the file no longer holds",
                seeds.iter().cloned().collect::<Vec<_>>().join(", ")
            ));
            return;
        }

        let capable = write_capable_functions(&source.masked, &items, &seeds);
        self.trait_writers(source, &items, &capable);
        self.const_initializers(source, &capable.union(&seeds).cloned().collect());

        for item in &items {
            let Some(visibility) = &item.visibility else {
                continue;
            };
            if capable.contains(&item.name) && !DOCUMENT_WRITE_SURFACE.contains(&item.name.as_str())
            {
                self.fail(format!(
                    "{DOCUMENT_SOURCE}:{}: `{visibility} fn {}` can write config.toml but is \
                     not the reviewed document save; the application has exactly one durable \
                     writer, so record it in DOCUMENT_WRITE_SURFACE only if that changed",
                    source.line(item.start),
                    item.name
                ));
            }
        }
    }

    /// `src/config/mod.rs` names the writers only where it re-exports them.
    fn config_module_surface(&mut self) {
        let (source, problems) = Source::read(self.tree, CONFIG_MODULE);
        self.failures.extend(problems);
        let use_spans: Spans = use_items(&source.masked)
            .iter()
            .map(|item| (item.start, item.end))
            .collect();
        let items = function_items(&source.masked, &source.test_spans);
        let surface: BTreeSet<String> = io_write_surface().into_iter().map(str::to_owned).collect();

        for name in &surface {
            for hit in word_hits(&source.masked, name) {
                if within(&source.test_spans, hit) || within(&use_spans, hit) {
                    continue;
                }
                let place = enclosing_function(&items, hit).map_or_else(
                    || "outside any `use` item".to_owned(),
                    |item| format!("inside `fn {}`", item.name),
                );
                self.fail(format!(
                    "{CONFIG_MODULE}:{}: names the config writer `{name}` {place}; this file \
                     re-exports the editors and does nothing else, so anything here that can \
                     call one carries the capability out under a name the call-site pins never \
                     see",
                    source.line(hit)
                ));
            }
        }

        let capable = write_capable_functions(&source.masked, &items, &surface);
        for item in &items {
            if capable.contains(&item.name) {
                self.fail(format!(
                    "{CONFIG_MODULE}:{}: `fn {}` can reach a config writer; the config \
                     module's own file declares no functions, so this is an unreviewed writer \
                     with a name of its own",
                    source.line(item.start),
                    item.name
                ));
            }
        }
        self.trait_declarations(&source, &capable);
    }

    /// The config module re-exports the editors, never a primitive, and never
    /// any other `fn`, `const`, or `static` declared in a writer file.
    fn module_reexports(&mut self) {
        let values = writer_value_items(self.tree);
        let mut paths: Vec<PathBuf> = self
            .tree
            .paths_under("src/config")
            .map(Path::to_path_buf)
            .collect();
        paths.retain(|path| !self.is_test(path));
        paths.sort_by_key(|path| (path != Path::new(CONFIG_MODULE), path.clone()));

        for path in paths {
            let display = path.to_string_lossy().into_owned();
            let (source, _) = Source::read(self.tree, &display);
            for item in use_items(&source.masked) {
                if within(&source.test_spans, item.start) || !item.exported {
                    continue;
                }

                let item_text = &source.masked[item.start..item.end];
                let line = source.line(item.start);
                for primitive in WRITE_PRIMITIVES {
                    if word_hits(item_text, primitive).next().is_some() {
                        self.fail(format!(
                            "{display}:{line}: re-exports the write primitive `{primitive}`; \
                             the primitives stay inside the config module and only the narrow \
                             editors leave it"
                        ));
                    }
                }
                for name in use_leaf_names(item_text) {
                    if narrow_writer_names().any(|editor| editor == name) {
                        continue;
                    }
                    let Some((declared, keyword)) = values.get(&name) else {
                        continue;
                    };
                    self.fail(format!(
                        "{display}:{line}: re-exports `{name}`, a `{keyword}` declared in \
                         {declared}; the write lives in that file, and the surface pins there \
                         read functions — so a value leaving it carries whatever it holds past \
                         them. Only the reviewed editors leave the module"
                    ));
                }
            }
        }
    }

    /// The implementing files may not widen the capability they own.
    fn write_surface(&mut self) {
        let (document, problems) = Source::read(self.tree, DOCUMENT_SOURCE);
        self.failures.extend(problems);
        self.document_write_surface(&document);
        if !document.text.contains("pub fn save_with_backup") {
            self.fail(format!(
                "{DOCUMENT_SOURCE}: `save_with_backup` is gone or renamed; this check no longer \
                 describes the code"
            ));
        }

        let (io, problems) = Source::read(self.tree, IO_SOURCE);
        for primitive in [
            "create_config_backup",
            "write_config_text_atomic",
            "prepare_config_parent",
        ] {
            if !io.text.contains(&format!("pub(super) fn {primitive}")) {
                self.fail(format!(
                    "{IO_SOURCE}: `{primitive}` is no longer `pub(super)`; the write primitives \
                     must stay inside the config module"
                ));
            }
        }
        // Each editor builds its update on `document.config()`; basing it on
        // `authored_config()` would hand the merge gate every clamped value.
        for name in narrow_writer_names() {
            if !io.text.contains(&format!("pub fn {name}")) {
                self.fail(format!(
                    "{IO_SOURCE}: narrow config writer `{name}` is gone or no longer declared \
                     here; this check no longer describes the code"
                ));
            }
        }
        for name in path_taking_writers() {
            if !has_cfg_test_twin(io.text, &name) {
                self.fail(format!(
                    "{IO_SOURCE}: `{name}` is no longer a `#[cfg(test)] pub(crate) fn`; the \
                     path-taking twins exist for the suites, and an ungated one is a config \
                     write at a caller-chosen path available to the whole crate"
                ));
            }
        }
        self.failures.extend(problems);
        for hit in word_hits(&io.masked, "authored_config") {
            if !within(&io.test_spans, hit) {
                self.fail(format!(
                    "{IO_SOURCE}:{}: a narrow writer reads `authored_config()`; the edit base \
                     must be `document.config()` so the merge gate writes only the edited key",
                    io.line(hit)
                ));
            }
        }

        self.io_write_surface(&io);
        self.config_module_surface();
        self.module_reexports();
    }
}

/// `#[cfg(test)]`, whitespace, then `pub(crate) fn name` ending at a word boundary.
fn has_cfg_test_twin(text: &str, name: &str) -> bool {
    let declaration = format!("pub(crate) fn {name}");

    text.match_indices(CFG_TEST).any(|(start, _)| {
        let rest = text[start + CFG_TEST.len()..].trim_start();
        rest.starts_with(&declaration)
            && !rest[declaration.len()..]
                .chars()
                .next()
                .is_some_and(is_word_char)
    })
}

fn enclosing_function(items: &[FunctionItem], offset: usize) -> Option<&FunctionItem> {
    items
        .iter()
        .filter(|item| item.start <= offset && offset < item.end)
        .max_by_key(|item| item.start)
}

/// Every `fn`, `const`, and `static` declared in the two writer files: where a
/// name came from and as what, which a value cannot disguise.
fn writer_value_items(tree: &Tree) -> HashMap<String, (&'static str, String)> {
    let mut items = HashMap::new();

    for path in [DOCUMENT_SOURCE, IO_SOURCE] {
        let (source, _) = Source::read(tree, path);
        for function in function_items(&source.masked, &source.test_spans) {
            items
                .entry(function.name)
                .or_insert((path, "fn".to_owned()));
        }
        for constant in const_items(&source.masked, &source.test_spans) {
            items.insert(constant.name, (path, constant.keyword));
        }
    }

    items
}

fn audit(tree: &Tree) -> Vec<String> {
    let mut audit = Audit {
        tree,
        failures: Vec::new(),
        test_sources: TestSources::default(),
    };

    let scanned = audit.sites();
    let missing: Vec<&str> = EXPECTED_SCANNED
        .into_iter()
        .filter(|path| !scanned.contains(Path::new(path)))
        .collect();
    if !missing.is_empty() {
        audit.fail(format!(
            "the walk missed expected sources, so it proves nothing: {}",
            missing.join(", ")
        ));
    }
    audit.write_surface();

    audit.failures
}

fn checkout() -> Tree {
    Tree::checkout(&["src", "configurator/src"])
}

fn assert_fails(tree: Tree, expected: &str) {
    let failures = audit(&tree);

    assert!(
        failures.iter().any(|failure| failure.contains(expected)),
        "expected a failure containing {expected:?}, got {failures:#?}"
    );
}

#[test]
fn only_the_reviewed_writers_can_write_config_toml() {
    let failures = audit(&checkout());

    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn a_primitive_outside_the_reviewed_writers_fails() {
    assert_fails(
        checkout().appending(
            "src/daemon/core.rs",
            "\nfn leak(document: &ConfigDocument) { document.save_with_backup(config()); }\n",
        ),
        "config write capability `save_with_backup` outside the reviewed writers",
    );
}

#[test]
fn only_a_cfg_test_module_makes_a_tests_directory_test_code() {
    let tree = |declaration: &str| {
        checkout()
            .appending("src/lib.rs", "\nmod probe;\n")
            .with("src/probe.rs", declaration)
            .with(
                "src/probe/tests/leak.rs",
                "fn f() { create_config_backup(path); }\n",
            )
    };

    assert_fails(
        tree("mod tests;\n"),
        "src/probe/tests/leak.rs:1: config write capability `create_config_backup`",
    );
    let failures = audit(&tree("#[cfg(test)]\nmod tests;\n"));
    assert!(failures.is_empty(), "{failures:#?}");
}

#[test]
fn editor_calls_are_pinned_to_their_reviewed_callers() {
    assert_fails(
        checkout().appending(
            "src/backend/wayland/state.rs",
            "\nfn sneak() { crate::config::persist_quick_color(0, color()); }\n",
        ),
        "src/backend/wayland/state.rs: unreviewed caller of the narrow config writer \
         `persist_quick_color`",
    );
    assert_fails(
        checkout()
            .replacing(EDIT_WORKER, "\n    persist_quick_color,\n", "\n")
            .replacing(
                EDIT_WORKER,
                "persist_quick_color(edit.index, edit.color)",
                "unreachable!()",
            ),
        "expected to call `persist_quick_color` but does not",
    );
}

#[test]
fn path_taking_twins_stay_out_of_production() {
    assert_fails(
        checkout().appending(
            "src/daemon/core.rs",
            "\nfn f() { persist_preset_slot_at(path, 0, None); }\n",
        ),
        "src/daemon/core.rs: production code names `persist_preset_slot_at`",
    );
    assert_fails(
        checkout().replacing(
            IO_SOURCE,
            "#[cfg(test)]\npub(crate) fn persist_quick_color_at(",
            "pub(crate) fn persist_quick_color_at(",
        ),
        "`persist_quick_color_at` is no longer a `#[cfg(test)] pub(crate) fn`",
    );
}

#[test]
fn editors_are_not_wrapped_elsewhere_in_the_config_module() {
    assert_fails(
        checkout().appending(
            "src/config/types/mod.rs",
            "\npub fn wrapped() { super::io::persist_keybinding_edit(action(), &[]); }\n",
        ),
        "names the narrow config writer `persist_keybinding_edit` inside the config module",
    );
}

#[test]
fn casting_a_writer_to_a_value_fails() {
    // The edit worker may name the editor, but not hand it on as a value.
    assert_fails(
        checkout().appending(
            EDIT_WORKER,
            "\nfn leak() -> fn() { persist_quick_color as fn() }\n",
        ),
        "casts the config writer `persist_quick_color`",
    );
}

#[test]
fn renaming_a_writer_on_import_or_reexport_fails() {
    assert_fails(
        checkout().appending(
            EDIT_WORKER,
            "\nuse crate::config::persist_preset_slot as save;\n",
        ),
        "imports the config writer `persist_preset_slot` as `save`",
    );
    assert_fails(
        checkout().appending(
            CONFIG_MODULE,
            "\npub use io::persist_keybinding_edit as concealed_edit;\n",
        ),
        "re-exports the config writer `persist_keybinding_edit` as `concealed_edit`",
    );
}

#[test]
fn a_new_document_entry_that_reaches_the_write_fails() {
    assert_fails(
        checkout().replacing(
            DOCUMENT_SOURCE,
            "    fn merge_and_write(",
            "    pub fn persist_any_config(&self, config: &Config) {\n        \
             self.merge_and_write(&self.config, config, &mut || {});\n    }\n\n    \
             fn merge_and_write(",
        ),
        "`pub fn persist_any_config` can write config.toml but is not the reviewed document save",
    );
}

#[test]
fn a_new_io_entry_that_reaches_a_primitive_fails() {
    assert_fails(
        checkout().appending(
            IO_SOURCE,
            "\nfn helper(path: &Path) -> Result<()> { prepare_config_parent(path) }\n\
             pub fn persist_everything(path: &Path) -> Result<()> { helper(path) }\n",
        ),
        "`pub fn persist_everything` can write config.toml but is not one of the reviewed editors",
    );
}

#[test]
fn a_write_reachable_through_a_trait_fails() {
    assert_fails(
        checkout().appending(
            DOCUMENT_SOURCE,
            "\nimpl Persist for ConfigDocument {\n    fn persist(&self, config: &Config) {\n        \
             self.merge_and_write(&self.config, config, &mut || {});\n    }\n}\n",
        ),
        "`fn persist` can write config.toml from inside a trait",
    );
    assert_fails(
        checkout().appending(
            IO_SOURCE,
            "\npub trait Concealed {\n    fn persist_quick_color(&self);\n}\n",
        ),
        "`trait Concealed` declares `persist_quick_color`",
    );
}

#[test]
fn a_writer_stored_as_a_value_fails_on_both_halves() {
    assert_fails(
        checkout().appending(
            DOCUMENT_SOURCE,
            "\npub const PERSIST_ANY_CONFIG: fn(&ConfigDocument, Config) -> \
             Result<ConfigDocumentSaveOutcome> = ConfigDocument::save_with_backup;\n",
        ),
        "`const PERSIST_ANY_CONFIG` names `save_with_backup`",
    );
    assert_fails(
        checkout()
            .appending(IO_SOURCE, "\npub static WRITE_CONFIG: () = ();\n")
            .appending(CONFIG_MODULE, "\npub use io::WRITE_CONFIG;\n"),
        "re-exports `WRITE_CONFIG`, a `static` declared in src/config/io.rs",
    );
}

#[test]
fn the_config_module_stays_a_reexport_list() {
    assert_fails(
        checkout().appending(
            CONFIG_MODULE,
            "\npub fn concealed_edit() { io::persist_keybinding_edit(action(), &[]); }\n",
        ),
        "names the config writer `persist_keybinding_edit` inside `fn concealed_edit`",
    );
    assert_fails(
        checkout().appending(
            CONFIG_MODULE,
            "\npub fn indirect() { concealed(); }\n\
             fn concealed() { io::persist_quick_color(0, color()); }\n",
        ),
        "`fn indirect` can reach a config writer",
    );
}

#[test]
fn primitives_never_leave_the_config_module() {
    assert_fails(
        checkout().appending(CONFIG_MODULE, "\npub use io::create_config_backup;\n"),
        "re-exports the write primitive `create_config_backup`",
    );
    assert_fails(
        checkout().replacing(
            IO_SOURCE,
            "pub(super) fn prepare_config_parent(",
            "pub(crate) fn prepare_config_parent(",
        ),
        "`prepare_config_parent` is no longer `pub(super)`",
    );
}

#[test]
fn editors_build_on_the_loaded_config() {
    assert_fails(
        checkout().appending(
            IO_SOURCE,
            "\nfn base(document: &ConfigDocument) -> &Config { document.authored_config() }\n",
        ),
        "a narrow writer reads `authored_config()`",
    );
}

#[test]
fn the_walk_must_reach_every_former_writer() {
    assert_fails(
        checkout().without("src/daemon/tray/runtime.rs"),
        "the walk missed expected sources, so it proves nothing: src/daemon/tray/runtime.rs",
    );
}
