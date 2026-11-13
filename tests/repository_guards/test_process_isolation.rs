//! Integration tests must launch the application through the isolated helper.
//! This is a source guard over process constructors, not a data-flow proof.
//! The helper's sentinel regression independently checks its actual environment.
use std::path::Path;

use super::source::{Tree, mask_non_code};

fn audit(tree: &Tree) -> Vec<String> {
    let mut failures = Vec::new();
    for path in tree.paths_under("tests") {
        if path.starts_with("tests/repository_guards") || path.starts_with("tests/fixtures") {
            continue;
        }
        let source = tree.read(path).expect("listed test source");
        let code = mask_non_code(source);
        // The frozen daemon fixture compiles and runs its own miniature daemon,
        // never the Wayscriber application. The CLI sentinel re-execs its test.
        let reviewed = match path.to_str().expect("UTF-8 repository path") {
            "tests/support/wayscriber.rs" => 2,
            "tests/daemon_v1_fixture.rs" => 5,
            "tests/cli.rs" => source.matches("Command::new(env::current_exe()").count(),
            _ => 0,
        };
        let compact: String = code
            .chars()
            .filter(|character| !character.is_whitespace())
            .collect();
        if compact.matches("Command::new(").count() != reviewed {
            failures.push(format!(
                "{}: application launches must use the isolated wayscriber_cmd helper",
                path.display()
            ));
        }
        if path != Path::new("tests/support/wayscriber.rs")
            && source.contains("CARGO_BIN_EXE_wayscriber")
        {
            failures.push(format!(
                "{}: only the isolated helper may locate the application binary",
                path.display()
            ));
        }
    }
    failures
}

#[test]
fn application_test_processes_use_the_isolated_helper() {
    let failures = audit(&Tree::checkout(&["tests"]));
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn unreviewed_application_launches_are_rejected() {
    let tree = Tree::checkout(&["tests"]);
    assert!(
        audit(&tree).is_empty(),
        "baseline must pass before mutations"
    );
    for (path, addition) in [
        ("tests/cli.rs", "\nfn leak() { Command::new(binary); }\n"),
        (
            "tests/unisolated.rs",
            "fn leak() { Command :: new(env!(\"CARGO_BIN_EXE_wayscriber\")); }",
        ),
        (
            "tests/unisolated.rs",
            "fn leak() { std::process::Command::new(\"wayscriber\"); }",
        ),
    ] {
        let changed = if tree.contains(Path::new(path)) {
            tree.clone().appending(path, addition)
        } else {
            tree.clone().with(path, addition)
        };
        assert!(
            !audit(&changed).is_empty(),
            "unisolated launch in {path} must fail"
        );
    }
}
