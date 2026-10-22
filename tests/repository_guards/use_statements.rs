//! Each `use` statement imports from one module.
//!
//! `use foo::bar::{Bar, Baz};` and `use foo::{self, Foo};` are fine;
//! `use foo::{Foo, bar::Bar};` is not, because a `::` inside the braces names a
//! second module. rustfmt can enforce this with `imports_granularity =
//! "Module"`, but that option is not available on the stable toolchain, so this
//! guard reads the spelling instead. It sees every `use` item in the Rust
//! sources, including function-local ones, but not imports that a macro
//! expands to.

use std::path::Path;

use crate::source::{Tree, mask_non_code};

/// Every directory whose Rust sources the workspace compiles.
const SOURCE_DIRECTORIES: &[&str] = &["src", "configurator/src", "tests", "examples"];

fn check(tree: &Tree) -> Vec<String> {
    let mut errors = Vec::new();

    for path in tree.paths() {
        let source = tree.read(path).expect("listed path");
        let code = mask_non_code(source);
        for (start, statement) in use_statements(&code) {
            if names_a_second_module(statement) {
                errors.push(location(path, &code, start));
            }
        }
    }

    errors
}

fn location(path: &Path, code: &str, start: usize) -> String {
    let line = code[..start].matches('\n').count() + 1;

    format!(
        "{}:{line}: a `use` statement imports from more than one module",
        path.display()
    )
}

/// The `use` statements in masked code, as (offset, text up to the `;`).
fn use_statements(code: &str) -> Vec<(usize, &str)> {
    let mut statements = Vec::new();
    let mut search_from = 0;

    while let Some(found) = code[search_from..].find("use") {
        let start = search_from + found;
        search_from = start + "use".len();
        if !starts_use_item(code, start) {
            continue;
        }

        let Some(length) = code[start..].find(';') else {
            break;
        };
        statements.push((start, &code[start..start + length]));
        search_from = start + length;
    }

    statements
}

/// Whether the `use` at `start` is the keyword that begins a `use` item.
fn starts_use_item(code: &str, start: usize) -> bool {
    let after = &code[start + "use".len()..];
    if !after.starts_with(char::is_whitespace) {
        return false;
    }

    // An item begins after another item or block, an attribute, or a
    // visibility such as `pub` or `pub(crate)`; `reuse` begins with none.
    let before = code[..start].trim_end();
    before.is_empty()
        || before.ends_with([';', '{', '}', ']', ')'])
        || before
            .strip_suffix("pub")
            .is_some_and(|rest| !rest.ends_with(is_word_char))
}

fn is_word_char(character: char) -> bool {
    character.is_alphanumeric() || character == '_'
}

fn names_a_second_module(statement: &str) -> bool {
    let mut depth = 0_usize;
    let mut characters = statement.chars().peekable();

    while let Some(character) = characters.next() {
        match character {
            '{' => depth += 1,
            '}' => depth = depth.saturating_sub(1),
            ':' if depth > 0 && characters.peek() == Some(&':') => return true,
            _ => {}
        }
    }

    false
}

#[test]
fn every_use_statement_imports_from_one_module() {
    let errors = check(&Tree::checkout(SOURCE_DIRECTORIES));

    assert!(errors.is_empty(), "{}", errors.join("\n"));
}

#[test]
fn a_module_path_inside_braces_is_rejected() {
    let escapes = [
        "use foo::{Foo, bar::Bar};\n",
        "use foo::{bar::{Baz, Qux}};\n",
        "pub(crate) use foo::{bar::Bar, Foo};\n",
        "#[cfg(test)]\nuse foo::{\n    Foo,\n    bar::Bar,\n};\n",
        "fn body() {\n    use std::{fs, io::Read};\n}\n",
        "use {std::fs, crate::paths};\n",
    ];

    for escape in escapes {
        let tree = Tree::empty().with("src/escape.rs", escape);

        assert_eq!(check(&tree).len(), 1, "accepted: {escape}");
    }
}

#[test]
fn single_module_imports_and_text_that_only_looks_like_one_are_accepted() {
    let accepted = [
        "use foo::bar::{Bar, Baz};\n",
        "use foo::{self, Foo};\n",
        "pub use foo::bar::Bar;\n",
        "use super::*;\n",
        "use std::io::{self, Read as _};\n",
        "// use foo::{Foo, bar::Bar};\n",
        "const DOC: &str = \"use foo::{Foo, bar::Bar};\";\n",
        "fn reuse() -> usize { let used = 1; used }\n",
        "let pause = ::std::time::Duration::ZERO;\n",
    ];

    for source in accepted {
        let tree = Tree::empty().with("src/accepted.rs", source);

        assert!(check(&tree).is_empty(), "rejected: {source}");
    }
}
