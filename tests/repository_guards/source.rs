//! Reading the repository's Rust sources for the guards beside this module.
//!
//! The guards read spelling, not compiled items. The Rust-source guards work on
//! a [`Tree`]: the checked-out files, or a copy with an edit applied, which is
//! how their regression cases show that each forbidden escape fails. A corpus
//! of small sources can also be checked as its own tree.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex, PoisonError};

/// Rust sources keyed by repository-relative path.
#[derive(Clone)]
pub struct Tree {
    files: BTreeMap<PathBuf, String>,
}

/// Checkouts this test binary has read, by directory list. The files do not
/// change while the guards run, so each test starts from a copy.
static CHECKOUTS: LazyLock<Mutex<HashMap<Vec<String>, Tree>>> = LazyLock::new(Mutex::default);

impl Tree {
    /// Every `.rs` file under `directories`, read from this checkout.
    pub fn checkout(directories: &[&str]) -> Self {
        let key = directories
            .iter()
            .map(|directory| (*directory).to_owned())
            .collect();
        let mut checkouts = CHECKOUTS.lock().unwrap_or_else(PoisonError::into_inner);

        checkouts
            .entry(key)
            .or_insert_with(|| Self::read_checkout(directories))
            .clone()
    }

    fn read_checkout(directories: &[&str]) -> Self {
        let root = repository_root();
        let mut files = BTreeMap::new();

        for path in directories
            .iter()
            .flat_map(|directory| files_under(directory))
        {
            if path.extension().is_some_and(|extension| extension == "rs") {
                let source = fs::read_to_string(root.join(&path))
                    .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
                files.insert(path, source);
            }
        }

        Self { files }
    }

    pub fn empty() -> Self {
        Self {
            files: BTreeMap::new(),
        }
    }

    pub fn with(mut self, path: &str, source: &str) -> Self {
        self.files.insert(PathBuf::from(path), source.to_owned());
        self
    }

    pub fn without(mut self, path: &str) -> Self {
        assert!(
            self.files.remove(Path::new(path)).is_some(),
            "{path} is not in the tree"
        );
        self
    }

    /// Replaces the only occurrence of `from`, so an edit cannot silently miss.
    pub fn replacing(mut self, path: &str, from: &str, to: &str) -> Self {
        let source = self.files.get_mut(Path::new(path)).expect("edited file");
        assert_eq!(source.matches(from).count(), 1, "{path}: `{from}`");
        *source = source.replacen(from, to, 1);
        self
    }

    pub fn appending(mut self, path: &str, addition: &str) -> Self {
        self.files
            .get_mut(Path::new(path))
            .expect("edited file")
            .push_str(addition);
        self
    }

    pub fn read(&self, path: &Path) -> Option<&str> {
        self.files.get(path).map(String::as_str)
    }

    pub fn contains(&self, path: &Path) -> bool {
        self.files.contains_key(path)
    }

    /// Every path, in sorted order.
    pub fn paths(&self) -> impl Iterator<Item = &Path> {
        self.files.keys().map(PathBuf::as_path)
    }

    /// Paths under `prefix`, in sorted order.
    pub fn paths_under<'a>(&'a self, prefix: &'a str) -> impl Iterator<Item = &'a Path> + 'a {
        self.files
            .keys()
            .map(PathBuf::as_path)
            .filter(move |path| path.starts_with(prefix))
    }
}

pub fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Repository-relative paths of every file under `directory`, sorted. A
/// symbolic link is listed as a file and never followed.
pub fn files_under(directory: &str) -> Vec<PathBuf> {
    let root = repository_root();
    let mut files = Vec::new();

    collect_files(&root, &root.join(directory), &mut files);
    files
}

fn collect_files(root: &Path, directory: &Path, files: &mut Vec<PathBuf>) {
    let mut entries: Vec<_> = fs::read_dir(directory)
        .unwrap_or_else(|error| panic!("read {}: {error}", directory.display()))
        .map(|entry| entry.expect("directory entry").path())
        .collect();
    entries.sort();

    for path in entries {
        let kind = fs::symlink_metadata(&path)
            .unwrap_or_else(|error| panic!("stat {}: {error}", path.display()))
            .file_type();
        if kind.is_dir() {
            collect_files(root, &path, files);
        } else {
            files.push(path.strip_prefix(root).expect("inside the root").to_owned());
        }
    }
}

/// Blanks comments, string and byte-string literals (raw or not), and char
/// literals, keeping every other byte, every newline, and every offset in
/// place. Block comments nest as they do in Rust, and a lifetime is not a
/// char literal.
pub fn mask_non_code(source: &str) -> String {
    let bytes = source.as_bytes();
    let mut masked = bytes.to_vec();
    let mut index = 0;

    while index < bytes.len() {
        let end = match bytes[index] {
            b'/' if bytes.get(index + 1) == Some(&b'/') => line_comment_end(bytes, index),
            b'/' if bytes.get(index + 1) == Some(&b'*') => block_comment_end(bytes, index),
            b'r' | b'b' if !follows_identifier(bytes, index) => raw_string_end(bytes, index),
            b'"' => Some(string_end(bytes, index)),
            b'\'' => char_literal_end(source, index),
            _ => None,
        };

        match end {
            Some(end) => {
                for byte in &mut masked[index..end] {
                    if *byte != b'\n' {
                        *byte = b' ';
                    }
                }
                index = end;
            }
            None => index += 1,
        }
    }

    String::from_utf8(masked).expect("whole UTF-8 sequences are blanked")
}

fn follows_identifier(bytes: &[u8], index: usize) -> bool {
    index > 0 && is_identifier_byte(bytes[index - 1])
}

fn is_identifier_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_' || byte >= 0x80
}

fn line_comment_end(bytes: &[u8], start: usize) -> Option<usize> {
    Some(
        bytes[start..]
            .iter()
            .position(|byte| *byte == b'\n')
            .map_or(bytes.len(), |offset| start + offset),
    )
}

fn block_comment_end(bytes: &[u8], start: usize) -> Option<usize> {
    let mut depth = 0;
    let mut index = start;

    while index + 1 < bytes.len() {
        match (bytes[index], bytes[index + 1]) {
            (b'/', b'*') => {
                depth += 1;
                index += 2;
            }
            (b'*', b'/') => {
                depth -= 1;
                index += 2;
                if depth == 0 {
                    return Some(index);
                }
            }
            _ => index += 1,
        }
    }

    Some(bytes.len())
}

/// `r"…"`, `r#"…"#`, `br##"…"##`; `r#name` is a raw identifier, not a string.
fn raw_string_end(bytes: &[u8], start: usize) -> Option<usize> {
    let mut index = start;
    if bytes[index] == b'b' {
        if bytes.get(index + 1) == Some(&b'"') {
            return Some(string_end(bytes, index + 1));
        }
        index += 1;
    }
    if bytes.get(index) != Some(&b'r') {
        return None;
    }
    index += 1;

    let hashes = bytes[index..]
        .iter()
        .take_while(|byte| **byte == b'#')
        .count();
    index += hashes;
    if bytes.get(index) != Some(&b'"') {
        return None;
    }
    index += 1;

    while index < bytes.len() {
        if bytes[index] == b'"'
            && bytes[index + 1..]
                .iter()
                .take(hashes)
                .all(|byte| *byte == b'#')
        {
            let end = index + 1 + hashes;
            if end <= bytes.len() {
                return Some(end);
            }
        }
        index += 1;
    }

    Some(bytes.len())
}

fn string_end(bytes: &[u8], start: usize) -> usize {
    let mut index = start + 1;

    while index < bytes.len() {
        match bytes[index] {
            b'\\' => index += 2,
            b'"' => return index + 1,
            _ => index += 1,
        }
    }

    bytes.len()
}

/// `'x'`, `'\n'`, `'\u{1F600}'`; anything else starting with `'` is a lifetime.
fn char_literal_end(source: &str, start: usize) -> Option<usize> {
    let rest = &source[start + 1..];
    let mut characters = rest.char_indices();
    let (_, first) = characters.next()?;

    if first == '\\' {
        let escaped = rest[1..].chars().next()?;
        let from = 1 + escaped.len_utf8();
        let closing = from + rest[from..].find(['\'', '\n'])?;
        return (rest.as_bytes()[closing] == b'\'').then_some(start + 1 + closing + 1);
    }

    let (offset, second) = characters.next()?;
    (second == '\'' && first != '\n').then_some(start + 1 + offset + 1)
}

/// One-based line of `offset`.
pub fn line_of(source: &str, offset: usize) -> usize {
    source[..offset].matches('\n').count() + 1
}

/// Offset just past the `}` that closes the block opening at `opening`.
pub fn block_end(masked: &str, opening: usize) -> Option<usize> {
    let mut depth = 0usize;

    for (index, byte) in masked.bytes().enumerate().skip(opening) {
        match byte {
            b'{' => depth += 1,
            b'}' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(index + 1);
                }
            }
            _ => {}
        }
    }

    None
}

/// Offset just past the top-level `;` that ends the item continuing at `from`.
pub fn item_end(masked: &str, from: usize) -> Option<usize> {
    let mut depth = 0i64;

    for (index, byte) in masked.bytes().enumerate().skip(from) {
        match byte {
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => depth -= 1,
            b';' if depth == 0 => return Some(index + 1),
            _ => {}
        }
    }

    None
}

/// A character of an identifier, as `\w` reads one.
pub fn is_word_char(character: char) -> bool {
    character.is_alphanumeric() || character == '_'
}

/// Every maximal run of word characters, with its offset.
pub fn words(text: &str) -> impl Iterator<Item = (usize, &str)> {
    let mut start = None;
    let mut found = Vec::new();

    for (index, character) in text.char_indices() {
        let word = is_word_char(character);
        match (word, start) {
            (true, None) => start = Some(index),
            (false, Some(from)) => {
                found.push((from, &text[from..index]));
                start = None;
            }
            _ => {}
        }
    }
    if let Some(from) = start {
        found.push((from, &text[from..]));
    }

    found.into_iter()
}

/// Offsets where `name` occurs as a whole word.
pub fn word_hits<'a>(text: &'a str, name: &'a str) -> impl Iterator<Item = usize> + 'a {
    words(text)
        .filter(move |(_, word)| *word == name)
        .map(|(offset, _)| offset)
}

/// [`word_hits`] for each of `names`, from one pass over `text`.
pub fn word_index<'n>(text: &str, names: &[&'n str]) -> HashMap<&'n str, Vec<usize>> {
    let mut index: HashMap<&'n str, Vec<usize>> = HashMap::new();

    for (offset, word) in words(text) {
        if let Some(name) = names.iter().find(|name| **name == word) {
            index.entry(name).or_default().push(offset);
        }
    }

    index
}

pub fn within(spans: &[(usize, usize)], offset: usize) -> bool {
    spans
        .iter()
        .any(|(start, end)| *start <= offset && offset < *end)
}

/// A cursor over masked source for recognising item headers.
pub struct Cursor<'a> {
    pub text: &'a str,
    pub at: usize,
}

impl<'a> Cursor<'a> {
    pub fn new(text: &'a str, at: usize) -> Self {
        Self { text, at }
    }

    fn rest(&self) -> &'a str {
        &self.text[self.at..]
    }

    /// Spaces and tabs only, like `[ \t]*`.
    pub fn skip_blanks(&mut self) {
        let skipped = self.rest().len() - self.rest().trim_start_matches([' ', '\t']).len();
        self.at += skipped;
    }

    /// Any whitespace, like `\s*`; returns how much was skipped.
    pub fn skip_whitespace(&mut self) -> usize {
        let skipped = self.rest().len() - self.rest().trim_start().len();
        self.at += skipped;
        skipped
    }

    /// `keyword` as a whole word.
    pub fn keyword(&mut self, keyword: &str) -> bool {
        let rest = self.rest();
        let matched = rest.starts_with(keyword)
            && !rest[keyword.len()..]
                .chars()
                .next()
                .is_some_and(is_word_char);
        if matched {
            self.at += keyword.len();
        }
        matched
    }

    /// `keyword` followed by at least one whitespace character, like `kw\s+`.
    pub fn keyword_then_space(&mut self, keyword: &str) -> bool {
        let saved = self.at;
        if self.keyword(keyword) && self.skip_whitespace() > 0 {
            return true;
        }
        self.at = saved;
        false
    }

    pub fn literal(&mut self, literal: &str) -> bool {
        let matched = self.rest().starts_with(literal);
        if matched {
            self.at += literal.len();
        }
        matched
    }

    /// A run of word characters, like `\w+`.
    pub fn word(&mut self) -> Option<&'a str> {
        let rest = self.rest();
        let length = rest
            .char_indices()
            .find(|(_, character)| !is_word_char(*character))
            .map_or(rest.len(), |(index, _)| index);
        (length > 0).then(|| {
            self.at += length;
            &rest[..length]
        })
    }

    /// `pub`, optionally restricted, then whitespace, like
    /// `pub(?:\s*\([^)]*\))?\s+`. Returns the visibility text.
    pub fn visibility(&mut self) -> Option<&'a str> {
        let start = self.at;
        if !self.keyword("pub") {
            return None;
        }

        let after_pub = self.at;
        self.skip_whitespace();
        if self.literal("(") {
            match self.rest().find(')') {
                Some(close) => self.at += close + 1,
                None => self.at = after_pub,
            }
        } else {
            self.at = after_pub;
        }

        let visibility_end = self.at;
        if self.skip_whitespace() == 0 {
            self.at = start;
            return None;
        }
        Some(&self.text[start..visibility_end])
    }

    /// Function qualifiers, like `(?:const\s+|async\s+|unsafe\s+|extern\s+"[^"]*"\s+)*`.
    pub fn function_qualifiers(&mut self) {
        loop {
            if self.keyword_then_space("const")
                || self.keyword_then_space("async")
                || self.keyword_then_space("unsafe")
            {
                continue;
            }

            let saved = self.at;
            if self.keyword_then_space("extern")
                && self.literal("\"")
                && let Some(close) = self.rest().find('"')
            {
                self.at += close + 1;
                if self.skip_whitespace() > 0 {
                    continue;
                }
            }
            self.at = saved;
            return;
        }
    }
}

/// Offsets of every line start: 0 and each offset after a newline.
pub fn line_starts(text: &str) -> impl Iterator<Item = usize> + '_ {
    std::iter::once(0).chain(text.match_indices('\n').map(|(index, _)| index + 1))
}

/// The attribute that compiles an item only for tests.
pub const CFG_TEST: &str = "#[cfg(test)]";

/// Which files Rust compiles only under `cfg(test)`. A directory called `tests`
/// inside `src/` is production code unless the `mod` item that brings it in
/// says otherwise, so the declarations on the module chain decide. A
/// `#[cfg(test)]` module's `#[path]` makes the file it names test code too,
/// unless ordinary `mod` items also reach that file, so an alias cannot exempt
/// production code. The `#[path]` must follow `#[cfg(test)]` and is read
/// relative to the declaring file's directory, as Rust reads it outside inline
/// modules.
///
/// This catches accidents, not deliberate evasion. Known limits: declarations
/// inside an inline `mod` block are read as the file's own; a
/// `#[cfg(not(test))]` and `#[cfg(test)]` pair of modules with one name counts
/// as test code; and an alias of a file that only a production `#[path]`
/// reaches exempts that file.
#[derive(Default)]
pub struct TestSources {
    /// Each declaring file's child modules, read once.
    declarations: HashMap<PathBuf, Declarations>,
    /// Files named by a `cfg(test)` module's `#[path]`, found on first use.
    pathed: Option<BTreeSet<PathBuf>>,
}

/// The child modules one file declares.
struct Declarations {
    /// Declared under `#[cfg(test)]`.
    test: BTreeSet<String>,
    /// Declared without it.
    production: BTreeSet<String>,
}

impl TestSources {
    pub fn is_test(&mut self, tree: &Tree, path: &Path) -> bool {
        let chain = module_chain(tree, path);

        self.chain_has_test_link(tree, &chain)
            || (self.pathed(tree).contains(path) && !self.chain_is_production(tree, &chain))
    }

    fn pathed(&mut self, tree: &Tree) -> &BTreeSet<PathBuf> {
        self.pathed.get_or_insert_with(|| {
            let mut pathed = BTreeSet::new();
            for file in tree.paths() {
                let text = tree.read(file).expect("listed path");
                if !text.contains("#[path") {
                    continue;
                }

                let directory = file.parent().unwrap_or(Path::new(""));
                for module in test_modules(text, &mask_non_code(text)) {
                    pathed.extend(module.path.map(|relative| directory.join(relative)));
                }
            }
            pathed
        })
    }

    /// Whether some module on the chain is declared under `#[cfg(test)]`.
    fn chain_has_test_link(&mut self, tree: &Tree, chain: &[(Option<PathBuf>, String)]) -> bool {
        for (declaring, segment) in chain {
            let Some(file) = declaring else {
                return false;
            };
            if self.declarations(tree, file).test.contains(segment) {
                return true;
            }
        }

        false
    }

    /// Whether ordinary `mod` items declare every module on the chain.
    fn chain_is_production(&mut self, tree: &Tree, chain: &[(Option<PathBuf>, String)]) -> bool {
        for (declaring, segment) in chain {
            let Some(file) = declaring else {
                return false;
            };
            if !self.declarations(tree, file).production.contains(segment) {
                return false;
            }
        }

        !chain.is_empty()
    }

    fn declarations(&mut self, tree: &Tree, file: &Path) -> &Declarations {
        self.declarations.entry(file.to_owned()).or_insert_with(|| {
            let text = tree.read(file).expect("declaring file");
            let masked = mask_non_code(text);
            let test: BTreeSet<String> = test_modules(text, &masked)
                .into_iter()
                .map(|module| module.name)
                .collect();
            let production = declared_modules(&masked)
                .into_iter()
                .filter(|name| !test.contains(name))
                .collect();

            Declarations { test, production }
        })
    }
}

/// The crate's module chain down to `path`: each module name with the file
/// that would declare it, or `None` once no file can.
fn module_chain(tree: &Tree, path: &Path) -> Vec<(Option<PathBuf>, String)> {
    let Some(root) = crate_source_root(tree, path) else {
        return Vec::new();
    };
    let base = root
        .parent()
        .expect("crate root has a directory")
        .to_owned();
    let mut segments: Vec<String> = path
        .strip_prefix(&base)
        .expect("under its crate root")
        .iter()
        .map(|part| part.to_string_lossy().into_owned())
        .collect();
    if segments.last().is_some_and(|last| last == "mod.rs") {
        segments.pop();
    } else if let Some(last) = segments.last_mut() {
        *last = last.trim_end_matches(".rs").to_owned();
    }

    let mut chain = Vec::with_capacity(segments.len());
    let mut declaring = Some(root);
    let mut module = base;
    for segment in segments {
        module = module.join(&segment);
        let next = module_file(tree, &module);
        chain.push((declaring, segment));
        declaring = next;
    }

    chain
}

/// Every module a file declares with `mod name;` or `mod name { ... }`.
fn declared_modules(masked: &str) -> BTreeSet<String> {
    let mut modules = BTreeSet::new();

    for (offset, word) in words(masked) {
        if word != "mod" {
            continue;
        }

        let mut cursor = Cursor::new(masked, offset + word.len());
        if cursor.skip_whitespace() == 0 {
            continue;
        }
        let Some(name) = cursor.word() else {
            continue;
        };
        cursor.skip_whitespace();
        if masked[cursor.at..].starts_with([';', '{']) {
            modules.insert(name.to_owned());
        }
    }

    modules
}

/// A child module declared under `#[cfg(test)]`, with its `#[path]` if any.
struct TestModule {
    name: String,
    path: Option<String>,
}

/// Child modules compiled only under `cfg(test)`: `#[cfg(test)]`, any further
/// attributes, an optional visibility, then `mod name` and `;` or `{`.
/// `masked` is `text` with comments and literals blanked, at the same offsets.
fn test_modules(text: &str, masked: &str) -> Vec<TestModule> {
    let mut modules = Vec::new();

    for (start, _) in masked.match_indices(CFG_TEST) {
        let mut cursor = Cursor::new(masked, start + CFG_TEST.len());
        let mut path = None;
        cursor.skip_whitespace();
        while cursor.literal("#[") {
            let Some(close) = masked[cursor.at..].find(']') else {
                break;
            };
            path = path.or_else(|| path_attribute(&text[cursor.at..cursor.at + close]));
            cursor.at += close + 1;
            cursor.skip_whitespace();
        }
        cursor.visibility();

        if !cursor.keyword_then_space("mod") {
            continue;
        }
        let Some(name) = cursor.word() else {
            continue;
        };
        cursor.skip_whitespace();
        if masked[cursor.at..].starts_with([';', '{']) {
            modules.push(TestModule {
                name: name.to_owned(),
                path,
            });
        }
    }

    modules
}

/// The file in `path = "..."`, the inside of a `#[path]` attribute.
fn path_attribute(attribute: &str) -> Option<String> {
    let value = attribute
        .trim_start()
        .strip_prefix("path")?
        .trim_start()
        .strip_prefix('=')?
        .trim();

    value
        .strip_prefix('"')?
        .strip_suffix('"')
        .map(str::to_owned)
}

/// `lib.rs`, else `main.rs`, of the crate whose sources hold `path`.
fn crate_source_root(tree: &Tree, path: &Path) -> Option<PathBuf> {
    let base = if path.starts_with("src") {
        Path::new("src")
    } else if path.starts_with("configurator/src") {
        Path::new("configurator/src")
    } else {
        return None;
    };

    ["lib.rs", "main.rs"]
        .iter()
        .map(|name| base.join(name))
        .find(|candidate| tree.contains(candidate))
}

/// The file holding `module`'s own items: `foo.rs` or `foo/mod.rs`.
fn module_file(tree: &Tree, module: &Path) -> Option<PathBuf> {
    [module.with_extension("rs"), module.join("mod.rs")]
        .into_iter()
        .find(|candidate| tree.contains(candidate))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn masking_keeps_offsets_and_hides_only_non_code() {
        let source = "a // b {\nc /* d /* e */ f */ g\n\"h { \\\" i\" r##\"j \"# k\"## \
                      b\"l\" br#\"m\"# 'n' '\\'' '\\u{7B}' 'o: r#p {";
        let masked = mask_non_code(source);

        assert_eq!(masked.len(), source.len());
        assert_eq!(masked.matches('\n').count(), source.matches('\n').count());
        for hidden in ["b {", "d", "e", "f", "h {", "i", "j", "k", "l", "m", "n"] {
            assert!(!masked.contains(hidden), "{hidden:?} in {masked:?}");
        }
        for kept in ["a ", "c ", " g", "'o: r#p {"] {
            assert!(masked.contains(kept), "{kept:?} missing from {masked:?}");
        }
    }
}
