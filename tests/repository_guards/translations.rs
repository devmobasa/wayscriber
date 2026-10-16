//! Every string marked for translation is covered by the shipped catalogs.
//!
//! `i18n::tr()` and `i18n::trf()` draw a string by exact lookup and fall back
//! to the English source, so a missing catalog entry is invisible at runtime:
//! the UI simply stays English in a language that should have translated it.
//! That is the escape this guard closes. It reads every `tr("...")` and
//! `trf("...")` call in the crate's own sources, and every catalog under
//! `locales/`, and fails when a marked string has no entry in a catalog, when
//! an entry repeats its own key, or when the number of `{}` placeholders in an
//! entry does not match the key's, which would make `trf` drop or misplace a
//! value.
//!
//! The guard reads spelling, not compiled items: it sees literal call
//! arguments and the `ActionMeta` fields the action accessors translate, but
//! nothing built at draw time. Labels derived at draw time by
//! `action_display_label` (`Whiteboard`, `Transparent`, ...) therefore become
//! catalog keys with no source string to point at, so a catalog may hold
//! entries this guard never sees, and unused entries are not reported. A
//! runtime-built string, such as a binding label or a user name, cannot be
//! marked and is never checked. Placeholder order is not checked: `{}` values
//! may be reordered by a translation, which is intended.
//!
//! `src/i18n` itself is skipped, because its tests spell strings that exist to
//! exercise lookup rather than to draw anything.
//!
//! Known limits: a `tr` argument that is not a plain double-quoted literal (a
//! `const`, a `concat!`, or a path to one) is not read; a literal split across
//! lines is not read; `\u{...}` is decoded but no other exotic escape is; and
//! a call whose text is inside a comment or a string is skipped, because the
//! mask decides whether the call is code.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use crate::source::{Tree, files_under, is_word_char, line_of, mask_non_code, repository_root};
use wayscriber::config::action_meta_iter;

/// Sources whose marked literals are checked: the overlay crate and the
/// configurator crate, which draws its chrome through the same `tr`/`trf`.
const SOURCE_DIRECTORIES: [&str; 2] = ["src", "configurator/src"];
/// Catalogs that must cover them.
const CATALOG_DIRECTORY: &str = "locales";
/// Call prefixes that mark a literal for translation, longest first so `trf(`
/// is not read as `tr(` followed by a variable.
const MARKS: [&str; 2] = ["trf(", "tr("];
/// The translation layer's own sources, which spell strings for their tests.
const EXEMPT_PREFIXES: [&str; 2] = ["src/i18n.rs", "src/i18n/"];

/// Marked source strings, each with the call sites that mark it.
type Marks = BTreeMap<String, BTreeSet<Site>>;
/// One catalog: language code, or path, to its entry table.
type Catalogs = BTreeMap<String, BTreeMap<String, String>>;
/// A `path:line` location, ordered by path and then line.
type Site = (PathBuf, usize);

fn marked_literals(tree: &Tree) -> Marks {
    let mut marks = Marks::new();

    for path in tree.paths() {
        if EXEMPT_PREFIXES.iter().any(|prefix| {
            path == Path::new(prefix) || path.as_os_str().to_string_lossy().starts_with(prefix)
        }) {
            continue;
        }
        let Some(source) = tree.read(path) else {
            continue;
        };

        collect_marks(path, source, &mut marks);
    }

    marks
}

/// Records every marked literal in one source. A mark is read only when it is
/// code — not inside a comment or a string — and not the tail of a longer
/// identifier such as `attr(`.
fn collect_marks(path: &Path, source: &str, marks: &mut Marks) {
    let masked = mask_non_code(source);

    for mark in MARKS {
        let mut searched = 0;
        while let Some(found) = source[searched..].find(mark) {
            let at = searched + found;
            searched = at + mark.len();

            let previous = source[..at].chars().next_back();
            if previous.is_some_and(is_word_char) {
                continue;
            }
            if !masked[at..].starts_with(mark) {
                continue;
            }
            let mut quote = at + mark.len();
            while source.as_bytes().get(quote) == Some(&b' ') {
                quote += 1;
            }
            let Some(text) = single_line_literal(source, quote) else {
                continue;
            };
            marks
                .entry(text)
                .or_default()
                .insert((path.to_owned(), line_of(source, at)));
        }
    }
}

/// The value of the double-quoted literal that opens at `quote`, or `None`
/// when it is not a plain single-line string.
fn single_line_literal(source: &str, quote: usize) -> Option<String> {
    let bytes = source.as_bytes();
    if bytes.get(quote) != Some(&b'"') {
        return None;
    }

    let mut value = String::new();
    let mut index = quote + 1;
    while let Some(&byte) = bytes.get(index) {
        match byte {
            b'"' => return Some(value),
            b'\n' => return None,
            b'\\' => {
                let (character, end) = escape(source, index)?;
                value.push(character);
                index = end;
            }
            _ => {
                let character = source[index..].chars().next()?;
                value.push(character);
                index += character.len_utf8();
            }
        }
    }

    None
}

/// Decodes the escape at `backslash`, returning the character and the offset
/// after it.
fn escape(source: &str, backslash: usize) -> Option<(char, usize)> {
    let bytes = source.as_bytes();
    let escaped = *bytes.get(backslash + 1)?;

    let simple = match escaped {
        b'n' => Some('\n'),
        b't' => Some('\t'),
        b'r' => Some('\r'),
        b'0' => Some('\0'),
        b'"' => Some('"'),
        b'\'' => Some('\''),
        b'\\' => Some('\\'),
        _ => None,
    };
    if let Some(character) = simple {
        return Some((character, backslash + 2));
    }
    if escaped != b'u' || bytes.get(backslash + 2) != Some(&b'{') {
        return None;
    }

    let end = source[backslash + 3..].find('}')? + backslash + 3;
    let character = u32::from_str_radix(&source[backslash + 3..end], 16).ok()?;

    Some((char::from_u32(character)?, end + 1))
}

/// The entries one catalog declares, decoded, or the parse failure that names
/// the offending line.
fn catalog_entries(path: &Path, source: &str) -> Result<BTreeMap<String, String>, String> {
    let mut entries = BTreeMap::new();

    for (number, line) in source.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let line_number = number + 1;
        let expected = |detail: &str| format!("{}:{line_number}: {detail}", path.display());
        let Some(rest) = line.strip_prefix('"') else {
            return Err(expected("a catalog line is a quoted key"));
        };
        let Some((key, rest)) = quoted_body(rest) else {
            return Err(expected("an unterminated key"));
        };
        let Some(rest) = rest.trim_start().strip_prefix('=') else {
            return Err(expected("a key is followed by `=`"));
        };
        let Some(rest) = rest.trim_start().strip_prefix('"') else {
            return Err(expected("a value is a quoted string"));
        };
        let Some((value, rest)) = quoted_body(rest) else {
            return Err(expected("an unterminated value"));
        };
        if !rest.trim().is_empty() {
            return Err(expected("trailing text after the value"));
        }
        if entries.insert(key.clone(), value).is_some() {
            return Err(expected("a duplicate key"));
        }
    }

    Ok(entries)
}

/// Reads the body of a double-quoted string from just after its opening quote,
/// returning the decoded value and the rest of the line.
fn quoted_body(rest: &str) -> Option<(String, &str)> {
    let bytes = rest.as_bytes();
    let mut value = String::new();
    let mut index = 0;

    while let Some(&byte) = bytes.get(index) {
        match byte {
            b'"' => return Some((value, &rest[index + 1..])),
            b'\\' => {
                let (character, end) = escape(rest, index)?;
                value.push(character);
                index = end;
            }
            _ => {
                let character = rest[index..].chars().next()?;
                value.push(character);
                index += character.len_utf8();
            }
        }
    }

    None
}

/// The placeholders `trf` substitutes into a string.
fn placeholders(text: &str) -> usize {
    text.match_indices("{}").count()
}

fn audit(tree: &Tree, catalogs: &Catalogs) -> Vec<String> {
    let mut failures = Vec::new();

    for (source, sites) in &marked_literals(tree) {
        let where_marked = sites
            .iter()
            .map(|(path, line)| format!("{}:{line}", path.display()))
            .collect::<Vec<_>>()
            .join(", ");

        check_entry(&mut failures, catalogs, source, &where_marked);
    }

    failures
}

/// Fails every catalog that does not cover one source string.
fn check_entry(failures: &mut Vec<String>, catalogs: &Catalogs, source: &str, where_from: &str) {
    for (language, entries) in catalogs {
        let Some(translated) = entries.get(source) else {
            failures.push(format!(
                "{language}: {source:?} has no entry ({where_from})"
            ));
            continue;
        };
        if translated == source {
            failures.push(format!(
                "{language}: {source:?} repeats its key ({where_from})"
            ));
        }
        let wanted = placeholders(source);
        let found = placeholders(translated);
        if wanted != found {
            failures.push(format!(
                "{language}: {source:?} has {found} placeholders and the key has {wanted} \
                 ({where_from})"
            ));
        }
    }
}

/// English strings that `ActionMeta` fields contribute, each named by the
/// action and field it comes from.
///
/// The action accessors translate these fields centrally, so they have no
/// `tr("...")` call for the guard to point at; reading the same fields keeps
/// their coverage as honest as a literal's.
fn action_meta_strings() -> BTreeMap<String, BTreeSet<String>> {
    let mut strings: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();

    for meta in action_meta_iter() {
        let action = format!("{:?}", meta.action);
        for (field, text) in [
            ("label", meta.label),
            ("short label", meta.short_label()),
            ("description", meta.description),
        ] {
            if text.is_empty() {
                continue;
            }
            strings
                .entry(text.to_owned())
                .or_default()
                .insert(format!("{action} {field}"));
        }
    }

    strings
}

/// Fails every catalog that does not cover the action metadata strings.
fn audit_action_meta(
    strings: &BTreeMap<String, BTreeSet<String>>,
    catalogs: &Catalogs,
) -> Vec<String> {
    let mut failures = Vec::new();

    for (source, fields) in strings {
        let where_from = fields.iter().cloned().collect::<Vec<_>>().join(", ");

        check_entry(&mut failures, catalogs, source, &where_from);
    }

    failures
}

/// Every catalog under `locales/`, read from this checkout.
fn checkout_catalogs() -> Catalogs {
    let root = repository_root();
    let mut catalogs = Catalogs::new();

    for path in files_under(CATALOG_DIRECTORY) {
        if path.extension().is_none_or(|extension| extension != "toml") {
            continue;
        }
        let language = path
            .file_stem()
            .expect("a catalog file name")
            .to_string_lossy()
            .into_owned();
        let source = fs::read_to_string(root.join(&path))
            .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
        match catalog_entries(&path, &source) {
            Ok(entries) => {
                catalogs.insert(language, entries);
            }
            Err(failure) => panic!("{failure}"),
        }
    }

    catalogs
}

#[test]
fn every_marked_string_has_a_catalog_entry() {
    let tree = Tree::checkout(&SOURCE_DIRECTORIES);
    let catalogs = checkout_catalogs();
    assert!(
        !catalogs.is_empty(),
        "no catalogs under {CATALOG_DIRECTORY}/; i18n has nothing to draw"
    );

    let actions = action_meta_strings();
    assert!(
        !actions.is_empty(),
        "no action metadata strings; the guard read nothing"
    );

    let mut failures = audit(&tree, &catalogs);
    failures.extend(audit_action_meta(&actions, &catalogs));

    assert!(
        failures.is_empty(),
        "marked for translation without a complete catalog:\n{}",
        failures.join("\n")
    );
}

/// A source with one marked literal, as a tree a regression case can audit.
fn marked(source: &str) -> Tree {
    Tree::empty().with("src/ui/surface.rs", source)
}

fn catalog(pairs: &[(&str, &str)]) -> Catalogs {
    BTreeMap::from([(
        "test".to_owned(),
        pairs
            .iter()
            .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
            .collect(),
    )])
}

#[test]
fn a_missing_entry_fails() {
    let failures = audit(&marked("fn f() { draw(tr(\"Boards\")); }"), &catalog(&[]));

    assert_eq!(failures.len(), 1, "{failures:?}");
    assert!(
        failures[0].contains("\"Boards\" has no entry"),
        "{failures:?}"
    );
    assert!(failures[0].contains("src/ui/surface.rs:1"), "{failures:?}");
}

#[test]
fn a_repeated_key_or_a_misplaced_placeholder_fails() {
    let failures = audit(
        &marked(
            "fn f(n: usize) { draw(tr(\"Boards\")); draw(trf(\"Saved {} of {}\", &[&n, &n])); }",
        ),
        &catalog(&[("Boards", "Boards"), ("Saved {} of {}", "Saved {}")]),
    );

    assert_eq!(failures.len(), 2, "{failures:?}");
    assert!(failures[0].contains("repeats its key"), "{failures:?}");
    assert!(failures[1].contains("the key has 2"), "{failures:?}");
}

#[test]
fn a_covered_literal_passes() {
    let failures = audit(
        &marked("fn f(n: usize) { draw(tr(\"Boards\")); draw(trf(\"Saved {}\", &[&n])); }"),
        &catalog(&[("Boards", "画板"), ("Saved {}", "已保存 {}")]),
    );

    assert_eq!(failures, Vec::<String>::new());
}

#[test]
fn comments_strings_and_longer_identifiers_are_not_marks() {
    let source = "// tr(\"Commented\")\n\
                  const DOC: &str = \"tr(\\\"Quoted\\\")\";\n\
                  const RAW: &str = r#\"trf(\"Raw\")\"#;\n\
                  fn attr() {}\n\
                  fn f() { attr(\"Attribute\"); }\n";
    let failures = audit(&marked(source), &catalog(&[]));

    assert_eq!(failures, Vec::<String>::new());
}

#[test]
fn an_escaped_literal_is_decoded() {
    let failures = audit(
        &marked("fn f() { draw(tr(\"Line\\nbreak\")); }"),
        &catalog(&[("Line\nbreak", "换行")]),
    );

    assert_eq!(failures, Vec::<String>::new());
}

#[test]
fn an_action_label_without_an_entry_fails() {
    let strings = BTreeMap::from([(
        "Whiteboard".to_owned(),
        BTreeSet::from(["Whiteboard label".to_owned()]),
    )]);
    let failures = audit_action_meta(&strings, &catalog(&[("Boards", "画板")]));

    assert_eq!(failures.len(), 1, "{failures:?}");
    assert!(
        failures[0].contains("\"Whiteboard\" has no entry"),
        "{failures:?}"
    );
    assert!(failures[0].contains("Whiteboard label"), "{failures:?}");
}
