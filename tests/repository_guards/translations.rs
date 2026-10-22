//! Every string marked for translation is covered by the shipped catalogs.
//!
//! `i18n::tr()` and `i18n::trf()` draw a string by exact lookup and fall back
//! to the English source, so a missing catalog entry is invisible at runtime:
//! the UI simply stays English in a language that should have translated it.
//! That is the escape this guard closes. It reads every `tr(…)` and `trf(…)`
//! call in the crate's own sources, and every catalog under `locales/`, and
//! fails when a marked string has no entry in a catalog, when an entry repeats
//! its own key, or when the number of `{}` placeholders in an entry does not
//! match the key's, which would make `trf` drop or misplace a value.
//!
//! `trf` fills anonymous `{}` placeholders with its arguments in order, so the
//! guard also fails a `trf` template that spells any other placeholder
//! (`{name}`, `{0}`), or whose argument slice holds a different number of
//! values than the template has placeholders. A translation may move its
//! placeholders within the sentence; it cannot swap two values, because the
//! first `{}` always receives the first argument.
//!
//! The guard reads spelling, not compiled items. It reads a string literal
//! (plain or raw, on one line or several) wherever it opens the call, after
//! any whitespace, and `trf(tr("…"), &[…])`. Any other argument cannot be
//! read, so it fails rather than passing unchecked: write the literal at the
//! call. The one exemption is the action accessors, which translate
//! `ActionMeta` fields centrally; the guard audits those fields, and the
//! labels `action_display_label` derives from them, through the metadata
//! table itself. A local translator that tables built for one language bind
//! (`let tr = |source| lookup(language, source)`) keeps the `tr("…")` spelling
//! and is read like any other call.
//!
//! `src/i18n` itself is skipped, because its tests spell strings that exist to
//! exercise lookup rather than to draw anything. Unused catalog entries are
//! not reported.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use wayscriber::domain::Language;
use wayscriber::i18n;

use crate::source::{Tree, files_under, is_word_char, line_of, mask_non_code, repository_root};
use wayscriber::config::{action_display_label, action_meta_iter};

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
/// The action accessors: the only calls allowed an argument the guard cannot
/// read, because `action_meta_strings` audits what they translate. Each is
/// the file and the argument with its whitespace removed.
const ACCESSOR_ARGUMENTS: [(&str, &str); 4] = [
    ("src/config/action_meta/mod.rs", "source_label(action)"),
    (
        "src/config/action_meta/mod.rs",
        "action_meta(action).map(|meta|meta.short_label()).unwrap_or(\"Action\")",
    ),
    (
        "src/config/action_meta/mod.rs",
        "action_meta(action).map(|meta|meta.description).unwrap_or(\"\")",
    ),
    ("src/config/action_meta/mod.rs", "stripped"),
];

/// Marked source strings, each with the call sites that mark it.
type Marks = BTreeMap<String, BTreeSet<Site>>;
/// One catalog: language code, or path, to its entry table.
type Catalogs = BTreeMap<String, BTreeMap<String, String>>;
/// A `path:line` location, ordered by path and then line.
type Site = (PathBuf, usize);

/// What the guard read at one marked call.
#[derive(Debug, PartialEq)]
enum Call {
    /// `tr("…")`, or a `trf` template with the number of values its argument
    /// slice holds.
    Literal {
        text: String,
        arguments: Option<usize>,
    },
    /// An argument the guard cannot read, with its whitespace removed.
    Unreadable { argument: String },
}

/// Everything the marks in a tree say: the strings the catalogs must cover,
/// and every call that breaks the formatting contract or cannot be read.
#[derive(Default)]
struct Reading {
    marks: Marks,
    failures: Vec<String>,
}

fn read_marks(tree: &Tree) -> Reading {
    let mut reading = Reading::default();

    for path in tree.paths() {
        if EXEMPT_PREFIXES.iter().any(|prefix| {
            path == Path::new(prefix) || path.as_os_str().to_string_lossy().starts_with(prefix)
        }) {
            continue;
        }
        let Some(source) = tree.read(path) else {
            continue;
        };

        collect_marks(path, source, &mut reading);
    }

    reading
}

/// Records every marked call in one source. A mark is read only when it is
/// code — not inside a comment or a string — and not the tail of a longer
/// identifier such as `attr(`.
fn collect_marks(path: &Path, source: &str, reading: &mut Reading) {
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

            let formatted = mark == "trf(";
            let site = format!("{}:{}", path.display(), line_of(source, at));
            match read_call(source, &masked, at + mark.len(), formatted) {
                Call::Literal { text, arguments } => {
                    if let Some(arguments) = arguments {
                        check_template(&mut reading.failures, &text, arguments, &site);
                    }
                    reading
                        .marks
                        .entry(text)
                        .or_default()
                        .insert((path.to_owned(), line_of(source, at)));
                }
                Call::Unreadable { argument } => {
                    let exempt = ACCESSOR_ARGUMENTS
                        .iter()
                        .any(|(file, allowed)| path == Path::new(file) && argument == *allowed);
                    if !exempt {
                        reading.failures.push(format!(
                            "{site}: `{}{argument})` is not a literal the guard can read; \
                             write the string at the call",
                            mark
                        ));
                    }
                }
            }
        }
    }
}

/// Reads the call whose arguments start at `open`, just after its `(`.
fn read_call(source: &str, masked: &str, open: usize, formatted: bool) -> Call {
    let unreadable = || Call::Unreadable {
        argument: argument_text(source, masked, open),
    };
    let start = skip_whitespace(source, open);

    let template = if formatted && masked[start..].starts_with("tr(") {
        let inner = skip_whitespace(source, start + "tr(".len());
        let Some((text, end)) = literal(source, inner) else {
            return unreadable();
        };
        let Some(end) = close_call(source, end) else {
            return unreadable();
        };
        Some((text, end))
    } else {
        literal(source, start)
    };
    let Some((text, end)) = template else {
        return unreadable();
    };

    if !formatted {
        return match close_call(source, end) {
            Some(_) => Call::Literal {
                text,
                arguments: None,
            },
            None => unreadable(),
        };
    }

    let comma = skip_whitespace(source, end);
    if source.as_bytes().get(comma) != Some(&b',') {
        return unreadable();
    }
    let ampersand = skip_whitespace(source, comma + 1);
    if source.as_bytes().get(ampersand) != Some(&b'&') {
        return unreadable();
    }
    let bracket = skip_whitespace(source, ampersand + 1);
    match slice_length(masked, bracket) {
        Some(arguments) => Call::Literal {
            text,
            arguments: Some(arguments),
        },
        None => unreadable(),
    }
}

/// Offset just past the `)` that closes a call whose last argument ended at
/// `from`, allowing the trailing comma rustfmt writes after a wrapped argument.
fn close_call(source: &str, from: usize) -> Option<usize> {
    let mut at = skip_whitespace(source, from);
    if source.as_bytes().get(at) == Some(&b',') {
        at = skip_whitespace(source, at + 1);
    }

    (source.as_bytes().get(at) == Some(&b')')).then_some(at + 1)
}

/// The number of values in the slice literal that opens at `bracket`.
fn slice_length(masked: &str, bracket: usize) -> Option<usize> {
    let bytes = masked.as_bytes();
    if bytes.get(bracket) != Some(&b'[') {
        return None;
    }

    let mut depth = 0usize;
    let mut commas = 0;
    let mut last_code = b'[';
    for &byte in &bytes[bracket..] {
        match byte {
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(match last_code {
                        b'[' => 0,
                        b',' => commas,
                        _ => commas + 1,
                    });
                }
            }
            b',' if depth == 1 => commas += 1,
            _ => {}
        }
        if !byte.is_ascii_whitespace() {
            last_code = byte;
        }
    }

    None
}

/// The call's argument text up to its closing `)`, whitespace removed, for
/// the report and the accessor exemption.
fn argument_text(source: &str, masked: &str, open: usize) -> String {
    let mut depth = 1usize;
    let mut end = source.len();
    for (index, byte) in masked.bytes().enumerate().skip(open) {
        match byte {
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => {
                depth -= 1;
                if depth == 0 {
                    end = index;
                    break;
                }
            }
            _ => {}
        }
    }

    let argument: String = source[open..end]
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect();

    // rustfmt's trailing comma after a wrapped argument is not part of it.
    argument
        .strip_suffix(',')
        .map_or(argument.clone(), str::to_owned)
}

fn skip_whitespace(source: &str, from: usize) -> usize {
    let rest = &source[from..];
    from + rest.len() - rest.trim_start().len()
}

/// The value of the string literal that opens at `at`, plain or raw, and the
/// offset after it, or `None` when no literal opens there.
fn literal(source: &str, at: usize) -> Option<(String, usize)> {
    let bytes = source.as_bytes();
    match bytes.get(at)? {
        b'"' => plain_literal(source, at),
        b'r' => raw_literal(source, at),
        _ => None,
    }
}

fn plain_literal(source: &str, quote: usize) -> Option<(String, usize)> {
    let bytes = source.as_bytes();
    let mut value = String::new();
    let mut index = quote + 1;

    while let Some(&byte) = bytes.get(index) {
        match byte {
            b'"' => return Some((value, index + 1)),
            // A line continuation drops the newline and the next line's
            // leading whitespace.
            b'\\' if matches!(bytes.get(index + 1), Some(b'\n' | b'\r')) => {
                index = skip_whitespace(source, index + 1);
            }
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

/// `r"…"` or `r#"…"#`: no escapes, closed by a quote and as many hashes.
fn raw_literal(source: &str, start: usize) -> Option<(String, usize)> {
    let rest = &source[start + 1..];
    let hashes = rest.bytes().take_while(|byte| *byte == b'#').count();
    let body = rest[hashes..].strip_prefix('"')?;
    let closing = format!("\"{}", "#".repeat(hashes));
    let length = body.find(&closing)?;
    let end = start + 1 + hashes + 1 + length + closing.len();

    Some((body[..length].to_owned(), end))
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
    if escaped == b'x' {
        let digits = source.get(backslash + 2..backslash + 4)?;
        let character = u8::from_str_radix(digits, 16).ok()?;
        return Some((char::from(character), backslash + 4));
    }
    if escaped != b'u' || bytes.get(backslash + 2) != Some(&b'{') {
        return None;
    }

    let end = source[backslash + 3..].find('}')? + backslash + 3;
    let character = u32::from_str_radix(&source[backslash + 3..end], 16).ok()?;

    Some((char::from_u32(character)?, end + 1))
}

/// Fails a `trf` template whose placeholders `trf` cannot fill as written.
fn check_template(failures: &mut Vec<String>, template: &str, arguments: usize, site: &str) {
    if let Some(unsupported) = unsupported_placeholder(template) {
        failures.push(format!(
            "{site}: {template:?} spells `{unsupported}`; `trf` fills only anonymous `{{}}` \
             placeholders, in order"
        ));
    }
    let wanted = placeholders(template);
    if wanted != arguments {
        failures.push(format!(
            "{site}: {template:?} has {wanted} placeholders and the call passes {arguments} \
             values"
        ));
    }
}

/// The first brace group other than `{}`, such as `{min}` or `{0}`.
fn unsupported_placeholder(template: &str) -> Option<&str> {
    let mut searched = 0;
    while let Some(found) = template[searched..].find('{') {
        let at = searched + found;
        if template[at..].starts_with("{}") {
            searched = at + 2;
            continue;
        }
        let end = template[at..]
            .find('}')
            .map_or(template.len(), |close| at + close + 1);
        return Some(&template[at..end]);
    }

    None
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
    let reading = read_marks(tree);
    let mut failures = reading.failures;

    for (source, sites) in &reading.marks {
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

/// English strings the action accessors translate, each named by the action
/// and field it comes from: the `ActionMeta` fields, the labels
/// `action_display_label` derives from them, and the accessors' fallback.
///
/// The accessors translate these centrally, so they have no `tr("...")` call
/// for the guard to point at; reading the same fields keeps their coverage as
/// honest as a literal's. The derived labels come from the production
/// function, read in this process, which never selects a language and so
/// draws English source text.
fn action_meta_strings() -> BTreeMap<String, BTreeSet<String>> {
    assert_eq!(
        i18n::language(),
        Language::English,
        "the guard reads source text; nothing in this process may select a language"
    );
    let mut strings: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut record = |text: &str, from: String| {
        if !text.is_empty() {
            strings.entry(text.to_owned()).or_default().insert(from);
        }
    };

    for meta in action_meta_iter() {
        let action = format!("{:?}", meta.action);
        for (field, text) in [
            ("label", meta.label),
            ("short label", meta.short_label()),
            ("description", meta.description),
            ("display label", action_display_label(meta.action)),
        ] {
            record(text, format!("{action} {field}"));
        }
    }
    record("Action", "action accessor fallback".to_owned());

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
fn a_missing_entry_after_a_newline_fails() {
    let source = "fn f() {\n    draw(tr(\n        \"A sentence long enough for rustfmt to wrap\",\n    ));\n}\n";
    let failures = audit(&marked(source), &catalog(&[]));

    assert_eq!(failures.len(), 1, "{failures:?}");
    assert!(
        failures[0].contains("\"A sentence long enough for rustfmt to wrap\" has no entry"),
        "{failures:?}"
    );
    assert!(failures[0].contains("src/ui/surface.rs:2"), "{failures:?}");
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
    let source = "fn f(n: usize) {\n\
                  draw(tr(\"Boards\"));\n\
                  draw(trf(\"Saved {}\", &[&n]));\n\
                  draw(trf(tr(\"Moved {} to {}\"), &[&n, &format!(\"{}\", n)]));\n\
                  draw(tr(r#\"Say \"hi\"\"#));\n\
                  draw(tr(\"Two \\\n      lines\"));\n\
                  }\n";
    let failures = audit(
        &marked(source),
        &catalog(&[
            ("Boards", "画板"),
            ("Saved {}", "已保存 {}"),
            ("Moved {} to {}", "已将 {} 移至 {}"),
            ("Say \"hi\"", "说“你好”"),
            ("Two lines", "两行"),
        ]),
    );

    assert_eq!(failures, Vec::<String>::new());
}

#[test]
fn a_named_placeholder_fails() {
    let source = "fn f(min: u32, max: u32) {\n\
                  draw(trf(\n\
                      tr(\"Enter a whole number between {min} and {max}.\"),\n\
                      &[&min, &max],\n\
                  ));\n\
                  }\n";
    let failures = audit(
        &marked(source),
        &catalog(&[(
            "Enter a whole number between {min} and {max}.",
            "请输入 {min} 到 {max} 之间的整数。",
        )]),
    );

    // The catalog agrees with the key, so only the template check sees it.
    assert_eq!(failures.len(), 2, "{failures:?}");
    assert!(failures[0].contains("spells `{min}`"), "{failures:?}");
    assert!(
        failures[1].contains("has 0 placeholders and the call passes 2 values"),
        "{failures:?}"
    );
}

#[test]
fn a_nested_template_with_the_wrong_argument_count_fails() {
    let failures = audit(
        &marked("fn f(n: usize) { draw(trf(tr(\"{} of {}\"), &[&n])); }"),
        &catalog(&[("{} of {}", "{} / {}")]),
    );

    assert_eq!(failures.len(), 1, "{failures:?}");
    assert!(
        failures[0].contains("has 2 placeholders and the call passes 1 values"),
        "{failures:?}"
    );
}

#[test]
fn an_unreadable_argument_fails() {
    let source = "const SAVED: &str = \"Saved\";\n\
                  fn f(drawing: bool) {\n\
                  draw(tr(SAVED));\n\
                  draw(tr(if drawing { \"Drawing\" } else { \"Passthrough\" }));\n\
                  }\n";
    let failures = audit(
        &marked(source),
        &catalog(&[
            ("Saved", "已保存"),
            ("Drawing", "绘制"),
            ("Passthrough", "穿透"),
        ]),
    );

    assert_eq!(failures.len(), 2, "{failures:?}");
    assert!(
        failures[0].contains("src/ui/surface.rs:3: `tr(SAVED)` is not a literal"),
        "{failures:?}"
    );
    assert!(failures[1].contains("src/ui/surface.rs:4"), "{failures:?}");
}

#[test]
fn only_the_action_accessors_may_translate_an_unreadable_argument() {
    let accessor = "fn label(action: Action) -> &'static str { tr(source_label(action)) }";
    let tree = Tree::empty()
        .with("src/config/action_meta/mod.rs", accessor)
        .with("src/ui/surface.rs", accessor);

    let failures = audit(&tree, &catalog(&[]));

    assert_eq!(failures.len(), 1, "{failures:?}");
    assert!(
        failures[0].starts_with("src/ui/surface.rs:1"),
        "{failures:?}"
    );
}

#[test]
fn a_local_translator_bound_to_one_language_is_read() {
    let source = "fn build(language: Language) -> Vec<&'static str> {\n\
                  let tr = |source: &'static str| crate::i18n::lookup(language, source);\n\
                  vec![tr(\"Move top toolbar\")]\n\
                  }\n";
    let failures = audit(&marked(source), &catalog(&[]));

    assert_eq!(failures.len(), 1, "{failures:?}");
    assert!(
        failures[0].contains("\"Move top toolbar\" has no entry"),
        "{failures:?}"
    );
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
        &marked("fn f() { draw(tr(\"Line\\nbreak \\u{2713}\")); }"),
        &catalog(&[("Line\nbreak \u{2713}", "换行 \u{2713}")]),
    );

    assert_eq!(failures, Vec::<String>::new());
}

#[test]
fn an_action_label_without_an_entry_fails() {
    let strings = BTreeMap::from([(
        "Whiteboard".to_owned(),
        BTreeSet::from(["ToggleWhiteboard display label".to_owned()]),
    )]);
    let failures = audit_action_meta(&strings, &catalog(&[("Boards", "画板")]));

    assert_eq!(failures.len(), 1, "{failures:?}");
    assert!(
        failures[0].contains("\"Whiteboard\" has no entry"),
        "{failures:?}"
    );
    assert!(
        failures[0].contains("ToggleWhiteboard display label"),
        "{failures:?}"
    );
}

#[test]
fn derived_display_labels_are_audited() {
    let strings = action_meta_strings();

    // `Toggle Whiteboard Mode` draws as "Whiteboard", a key no literal spells.
    assert!(
        strings
            .get("Whiteboard")
            .is_some_and(|from| from.contains("ToggleWhiteboard display label")),
        "{:?}",
        strings.get("Whiteboard")
    );
}
