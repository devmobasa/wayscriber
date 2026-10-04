//! Every process-creation site is in the reviewed ownership map.
//!
//! Production code creates processes only through the process broker; the
//! configurator, a separate process, keeps three reviewed direct sites. The
//! broker's raw-clone child stub runs between `clone` and `execve`, so it may
//! reach only the approved syscalls and nothing that allocates, locks, or logs.
//! Lines are read with `//` comments cut, and strings are read as written, so a
//! shell command spelled inside a literal is still a process site. Integration
//! tests and files compiled only under `cfg(test)` are test code; a `tests`
//! directory in `src/` counts only when its `mod` item says `#[cfg(test)]`.

use std::path::Path;

use super::source::{TestSources, Tree, is_word_char};

const BROKER_ROOT: &str = "src/process_broker";
const BROKER_BOOTSTRAP: &str = "src/process_broker/bootstrap.rs";

const DIRECT_PRODUCTION_ALLOWLIST: [&str; 3] = [
    // The configurator is a separate process with its own reviewed sites.
    "configurator/src/app/session_catalog.rs",
    "configurator/src/app/daemon_setup/command.rs",
    "configurator/src/app/daemon_setup/service.rs",
];

const STUB_START: &str = "    if pid == 0 {";
const STUB_END: &str = "    drop(child_socket);";
const STUB_BANNED: [&str; 11] = [
    "format!(",
    "log::",
    "panic!(",
    ".unwrap(",
    ".expect(",
    "drop(",
    "Command::",
    "CString::",
    "Vec::",
    "String::",
    "Box::",
];
const STUB_LIBC_CALLS: [&str; 2] = ["syscall", "_exit"];
const STUB_SYSCALLS: [&str; 6] = [
    "fcntl",
    "dup3",
    "setpgid",
    "exit_group",
    "close_range",
    "execve",
];

fn audit_sites(tree: &Tree) -> Vec<String> {
    let mut failures = Vec::new();
    let mut test_sources = TestSources::default();

    for directory in ["src", "configurator/src", "tests"] {
        for path in tree.paths_under(directory) {
            let allowed = path.starts_with(BROKER_ROOT)
                || DIRECT_PRODUCTION_ALLOWLIST
                    .iter()
                    .any(|allowed| path == Path::new(allowed))
                || path.starts_with("tests")
                || test_sources.is_test(tree, path);
            if allowed {
                continue;
            }

            let source = tree.read(path).expect("listed path");
            for (number, line) in source.lines().enumerate() {
                let code = line.split("//").next().unwrap_or_default();
                if is_process_site(code) {
                    failures.push(format!(
                        "{}:{}: unclassified process site: {}",
                        path.display(),
                        number + 1,
                        line.trim()
                    ));
                }
            }
        }
    }

    failures
}

/// Paths that create a process. `Command::new` also covers its
/// `std::process::` spelling.
const PROCESS_PATHS: [&str; 11] = [
    "Command::new",
    "std::process::Child",
    "libc::fork",
    "libc::vfork",
    "libc::posix_spawn",
    "libc::posix_spawnp",
    "libc::pthread_atfork",
    "libc::SYS_clone",
    "libc::SYS_clone3",
    "libc::SYS_fork",
    "libc::SYS_vfork",
];

fn is_process_site(code: &str) -> bool {
    (code.contains("::") && PROCESS_PATHS.iter().any(|path| has_word_path(code, path)))
        || (code.contains("-c") && has_shell_command(code))
}

/// `path` with word boundaries at both ends, like `\bpath\b`.
fn has_word_path(code: &str, path: &str) -> bool {
    code.match_indices(path).any(|(index, _)| {
        let before = code[..index].chars().next_back();
        let after = code[index + path.len()..].chars().next();
        !before.is_some_and(is_word_char) && !after.is_some_and(is_word_char)
    })
}

/// `sh -c`, `bash -c`, or `zsh -c`, like `\b(?:sh|bash|zsh)\s+-c\b`.
fn has_shell_command(code: &str) -> bool {
    ["sh", "bash", "zsh"].iter().any(|shell| {
        code.match_indices(shell).any(|(index, _)| {
            let before = code[..index].chars().next_back();
            let rest = &code[index + shell.len()..];
            let flag = rest.trim_start();
            !before.is_some_and(is_word_char)
                && flag.len() < rest.len()
                && flag.starts_with("-c")
                && !flag[2..].chars().next().is_some_and(is_word_char)
        })
    })
}

fn audit_child_stub(tree: &Tree) -> Vec<String> {
    let source = tree
        .read(Path::new(BROKER_BOOTSTRAP))
        .expect("broker bootstrap source");
    let Some(stub) = source
        .split_once(STUB_START)
        .and_then(|(_, rest)| rest.split_once(STUB_END))
        .map(|(stub, _)| stub)
    else {
        return vec![format!(
            "{BROKER_BOOTSTRAP}: raw-clone child-stub markers changed"
        )];
    };

    let mut failures: Vec<String> = STUB_BANNED
        .iter()
        .filter(|token| stub.contains(*token))
        .map(|token| format!("{BROKER_BOOTSTRAP}: child stub reaches banned token {token:?}"))
        .collect();

    let mut calls = prefixed_names(stub, "libc::", |rest| rest.trim_start().starts_with('('));
    calls.retain(|call| !STUB_LIBC_CALLS.contains(&call.as_str()));
    if !calls.is_empty() {
        failures.push(format!(
            "{BROKER_BOOTSTRAP}: child stub reaches unapproved libc calls: {}",
            calls.join(", ")
        ));
    }

    let mut syscalls = prefixed_names(stub, "libc::SYS_", |_| true);
    syscalls.retain(|syscall| !STUB_SYSCALLS.contains(&syscall.as_str()));
    if !syscalls.is_empty() {
        failures.push(format!(
            "{BROKER_BOOTSTRAP}: child stub reaches unapproved syscalls: {}",
            syscalls.join(", ")
        ));
    }

    failures
}

/// Sorted, distinct `[A-Za-z0-9_]+` names following `prefix` whose remaining
/// text satisfies `accept`.
fn prefixed_names(text: &str, prefix: &str, accept: impl Fn(&str) -> bool) -> Vec<String> {
    let mut names: Vec<String> = text
        .match_indices(prefix)
        .filter_map(|(index, _)| {
            let rest = &text[index + prefix.len()..];
            let length = rest
                .find(|character: char| !(character.is_ascii_alphanumeric() || character == '_'))
                .unwrap_or(rest.len());
            (length > 0 && accept(&rest[length..])).then(|| rest[..length].to_owned())
        })
        .collect();
    names.sort();
    names.dedup();
    names
}

fn audit(tree: &Tree) -> Vec<String> {
    let mut failures = audit_sites(tree);
    failures.extend(audit_child_stub(tree));
    failures
}

fn checkout() -> Tree {
    Tree::checkout(&["src", "configurator/src", "tests"])
}

#[test]
fn every_process_site_is_owned() {
    let failures = audit(&checkout());

    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn unowned_process_sites_fail() {
    let probe = "src/daemon/probe.rs";
    for site in [
        "let child = Command::new(program);",
        "let child: std::process::Child = spawn();",
        "unsafe { libc::fork() };",
        "unsafe { libc::syscall(libc::SYS_clone3, args) };",
        "let line = \"bash -c 'exit'\";",
    ] {
        let failures = audit(&checkout().with(probe, &format!("fn f() {{ {site} }}\n")));

        assert!(
            failures
                .iter()
                .any(|failure| failure.starts_with(&format!("{probe}:1:"))),
            "{site}: {failures:?}"
        );
    }
}

#[test]
fn owned_and_commented_process_sites_pass() {
    let site = "fn f() { Command::new(program); }\n";
    let tree = checkout()
        .with("src/process_broker/probe.rs", site)
        .with("src/daemon/probe/tests.rs", site)
        .with("src/daemon/tests/probe.rs", site)
        .with("tests/probe.rs", site)
        // Off the module chain, a test module's `#[path]` names its file.
        .with("src/daemon/fixtures/pathed.rs", site)
        .with(
            "src/daemon/probe.rs",
            "#[cfg(test)]\nmod tests;\n#[cfg(test)]\n#[path = \"fixtures/pathed.rs\"]\nmod pathed;\n\n\
             fn f() {} // Command::new(program)\n",
        );

    let failures = audit(&tree);

    assert!(failures.is_empty(), "{failures:?}");
}

#[test]
fn only_a_cfg_test_module_makes_a_file_test_code() {
    let site = "fn f() { Command::new(program); }\n";
    let tree = checkout()
        .with(
            "src/daemon/probe.rs",
            "mod tests;\n#[path = \"fixtures/pathed.rs\"]\nmod pathed;\n",
        )
        .with("src/daemon/probe/tests.rs", site)
        .with("src/daemon/fixtures/pathed.rs", site);

    let failures = audit(&tree);

    for path in ["src/daemon/probe/tests.rs", "src/daemon/fixtures/pathed.rs"] {
        assert!(
            failures
                .iter()
                .any(|failure| failure.starts_with(&format!("{path}:1:"))),
            "{path}: {failures:?}"
        );
    }
}

#[test]
fn a_test_path_alias_does_not_exempt_production_code() {
    let tree = checkout()
        .with(
            "src/daemon/probe.rs",
            "#[cfg(test)]\n#[path = \"core.rs\"]\nmod core_alias;\n",
        )
        .appending(
            "src/daemon/core.rs",
            "\nfn leak() { Command::new(program); }\n",
        );

    let failures = audit(&tree);

    assert!(
        failures
            .iter()
            .any(|failure| failure.starts_with("src/daemon/core.rs:")),
        "{failures:?}"
    );
}

#[test]
fn the_raw_clone_child_stub_stays_minimal() {
    for (addition, expected) in [
        ("log::warn!(\"x\");", "banned token \"log::\""),
        ("libc::getpid();", "unapproved libc calls: getpid"),
        (
            "libc::syscall(libc::SYS_write, 1);",
            "unapproved syscalls: write",
        ),
    ] {
        let tree = checkout().replacing(
            BROKER_BOOTSTRAP,
            STUB_END,
            &format!("        {addition}\n{STUB_END}"),
        );
        let failures = audit(&tree);

        assert!(
            failures.iter().any(|failure| failure.contains(expected)),
            "{addition}: {failures:?}"
        );
    }

    let failures = audit(&checkout().replacing(BROKER_BOOTSTRAP, STUB_END, "    drop(socket);"));
    assert!(
        failures
            .iter()
            .any(|failure| failure.contains("markers changed"))
    );
}
