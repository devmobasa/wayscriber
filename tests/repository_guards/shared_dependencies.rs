//! Shared layers do not reach up into the runtime crates' owners.
//!
//! A partial source guard: it understands rooted and parent-relative paths and
//! grouped `use` trees, and ignores comments and literals. It does not resolve
//! aliases, macro expansion, re-exports, or Rust's complete module graph. The
//! syntax corpus beside this file pins how it reads each form.

use std::path::Path;

use super::source::{Tree, is_word_char, mask_non_code};

const BOUNDARIES: [(&str, &[&str]); 2] = [
    (
        "src/domain",
        &["config", "input", "draw", "backend", "ui", "session"],
    ),
    ("src/config/validate", &["input", "backend"]),
];

/// Public-path compatibility assertions only.
const EXEMPT: &str = "src/domain/tests.rs";

fn check(tree: &Tree) -> Vec<String> {
    let mut errors = Vec::new();

    for (directory, forbidden) in BOUNDARIES {
        for path in tree.paths_under(directory) {
            if path == Path::new(EXEMPT) {
                continue;
            }

            let source = tree.read(path).expect("listed path");
            if has_upward_path(source, path, forbidden) {
                errors.push(format!(
                    "{}: upward dependency in shared layer",
                    path.display()
                ));
            }
        }
    }

    errors
}

fn tokens(source: &str) -> Vec<String> {
    let code = mask_non_code(source);
    let mut tokens = Vec::new();
    let mut characters = code.char_indices().peekable();

    while let Some((index, character)) = characters.next() {
        if character == ':' && code[index + 1..].starts_with(':') {
            characters.next();
            tokens.push("::".to_owned());
        } else if "{},;*".contains(character) {
            tokens.push(character.to_string());
        } else if character.is_alphabetic() || character == '_' {
            let raw = character == 'r'
                && code[index + 1..].starts_with('#')
                && code[index + 2..]
                    .chars()
                    .next()
                    .is_some_and(|next| next.is_alphabetic() || next == '_');
            let start = if raw { index + 2 } else { index };
            if raw {
                characters.next();
            }

            let mut end = start;
            for (offset, next) in code[start..].char_indices() {
                if !is_word_char(next) {
                    break;
                }
                end = start + offset + next.len_utf8();
            }
            while characters.peek().is_some_and(|(next, _)| *next < end) {
                characters.next();
            }
            tokens.push(code[start..end].to_owned());
        }
    }

    tokens
}

/// The module path of a source file below `src/`, `mod.rs` naming its directory.
fn module_path(path: &Path) -> Vec<String> {
    let mut parts: Vec<String> = path
        .with_extension("")
        .iter()
        .skip(1)
        .map(|part| part.to_string_lossy().into_owned())
        .collect();
    if parts.last().is_some_and(|last| last == "mod") {
        parts.pop();
    }
    parts
}

fn has_upward_path(source: &str, path: &Path, forbidden: &[&str]) -> bool {
    let tokens = tokens(source);
    let module = module_path(path);

    (0..tokens.len().saturating_sub(1)).any(|index| {
        matches!(tokens[index].as_str(), "crate" | "super" | "self")
            && tokens[index + 1] == "::"
            && use_tree(&tokens, index, module.clone(), forbidden).0
    })
}

/// Walks one path or grouped tree from `index`, returning whether it names a
/// forbidden top-level module and where it stopped.
fn use_tree(
    tokens: &[String],
    mut index: usize,
    prefix: Vec<String>,
    forbidden: &[&str],
) -> (bool, usize) {
    let mut path = prefix;

    while index < tokens.len() {
        let token = tokens[index].as_str();
        if matches!(token, "," | ";" | "}" | "as") {
            break;
        }

        if token == "{" {
            index += 1;
            while index < tokens.len() && tokens[index] != "}" {
                let (rejected, next) = use_tree(tokens, index, path.clone(), forbidden);
                if rejected {
                    return (true, next);
                }

                index = next;
                if tokens.get(index).is_some_and(|token| token == "as") {
                    index += 2;
                }
                if tokens.get(index).is_some_and(|token| token == ",") {
                    index += 1;
                } else if tokens.get(index).is_some_and(|token| token != "}") {
                    break;
                }
            }
            return (false, index + 1);
        }

        match token {
            "crate" => path.clear(),
            "super" => {
                path.pop();
            }
            "self" | "::" | "*" => {}
            name => path.push(name.to_owned()),
        }
        if path
            .first()
            .is_some_and(|first| forbidden.contains(&first.as_str()))
        {
            return (true, index);
        }

        index += 1;
        if tokens.get(index).is_none_or(|token| token != "::") {
            break;
        }
        index += 1;
    }

    (false, index)
}

#[test]
fn shared_layers_name_no_upward_crate_paths() {
    let errors = check(&Tree::checkout(&["src"]));

    assert!(errors.is_empty(), "{}", errors.join("\n"));
}

#[test]
fn shared_dependency_syntax_corpus_matches_its_expectations() {
    let corpus: serde_json::Value =
        serde_json::from_str(include_str!("shared_dependency_fixtures.json"))
            .expect("fixture JSON");
    let fixtures = corpus.as_array().expect("fixture array");

    assert!(!fixtures.is_empty());
    for fixture in fixtures {
        let name = fixture["name"].as_str().expect("name");
        let tree = Tree::empty().with(
            fixture["path"].as_str().expect("path"),
            fixture["source"].as_str().expect("source"),
        );
        let rejected = !check(&tree).is_empty();

        assert_eq!(
            rejected,
            fixture["reject"].as_bool().expect("reject"),
            "{name}"
        );
    }
}
