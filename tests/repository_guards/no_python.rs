//! The repository carries no Python.
//!
//! Tools are C# or POSIX shell, and tests and fixtures are Rust. This fails on a
//! Python file or a link to one, a Python project or lock file, a Python
//! shebang, or a Python interpreter or package name in the sources it reads:
//! Rust, C#, MSBuild, shell, Nix, TOML, workflow, build, service, desktop-entry,
//! and packaging files. So neither a script embedded in a string literal, an
//! interpreter launched by the tools, nor a declared interpreter dependency can
//! come back. Markdown is not read, so documentation can still say that the
//! repository uses no Python, and a `python` domain label such as
//! `docs.python.org` is not an interpreter.
//!
//! The walk covers every directory and file at the root except `.git`, build
//! output (`target/` and any root directory holding a `CACHEDIR.TAG`), and the
//! local files named by the root `.gitignore`. Inside an ignored directory such
//! as `packaging/`, the files that `.gitignore` re-includes one by one are read.
//! Extensionless files, such as `PKGBUILD` or a script, are read in full. It
//! walks the checkout rather than asking Git, so it also runs in a Nix build,
//! which has no `.git`.
//!
//! It reads spelling, as a tripwire against Python coming back by accident,
//! not as a sandbox. Known limits: an interpreter reached under another name,
//! such as `pypy3`, `PYTHONPATH`, or `buildPythonApplication`, is not seen; a
//! `python` path segment that is not a domain label, as in
//! `github.com/python/cpython`, is reported, so such a link is reworded; and
//! an untracked directory that is not ignored and holds no `CACHEDIR.TAG`, such
//! as `.direnv/` or `node_modules/`, is read like a source directory.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::source::{files_under, is_word_char, repository_root, words};

/// Never walked, whether or not `.gitignore` names them.
const SKIPPED_DIRECTORIES: [&str; 2] = [".git", "target"];
/// Marks a cache directory, such as a Cargo target directory under any name.
const CACHE_DIRECTORY_TAG: &str = "CACHEDIR.TAG";
const READ_EXTENSIONS: [&str; 11] = [
    "cs", "desktop", "nix", "props", "rs", "service", "sh", "targets", "toml", "yaml", "yml",
];
const PYTHON_EXTENSIONS: [&str; 9] = [
    "ipynb", "pxd", "py", "pyc", "pyd", "pyi", "pyo", "pyw", "pyx",
];
const PYTHON_PROJECT_FILES: [&str; 10] = [
    ".python-version",
    "Pipfile",
    "Pipfile.lock",
    "pdm.lock",
    "poetry.lock",
    "pyproject.toml",
    "requirements.txt",
    "setup.cfg",
    "tox.ini",
    "uv.lock",
];
/// Top-level domains after which a `python` label names a website.
const DOMAIN_SUFFIXES: [&str; 4] = ["com", "io", "net", "org"];
/// This guard names the interpreter to reject it.
const THIS_GUARD: &str = "tests/repository_guards/no_python.rs";

enum Entry {
    /// A file whose contents are read.
    Read(Vec<u8>),
    /// A symbolic link and its target, which is not followed.
    Link(PathBuf),
}

/// One entry of the root `.gitignore` that this guard honors.
#[derive(Debug, PartialEq)]
struct LocalEntry {
    path: PathBuf,
    /// Written with a trailing `/` or `/**`, so it matches only a directory.
    directory_only: bool,
}

/// The local files and directories the root `.gitignore` names. Only plain
/// names, which match at the root, and anchored paths with a `/` inside are
/// read; globs are left out, so this skips no more than Git ignores. A file Git
/// tracks despite a matching entry is skipped too, unless a `!` line names it.
#[derive(Debug, Default, PartialEq)]
struct LocalFiles {
    root_names: Vec<LocalEntry>,
    paths: Vec<LocalEntry>,
    /// Files a `!` line re-includes one by one, such as the packaging recipes.
    included: Vec<PathBuf>,
}

impl LocalFiles {
    fn parse(gitignore: &str) -> Self {
        let mut local = Self::default();

        for line in gitignore.lines().map(str::trim) {
            if let Some(included) = line.strip_prefix('!') {
                let path = included.trim_start_matches('/');
                if !path.is_empty() && !path.ends_with('/') && !path.contains(['*', '?', '[', '\\'])
                {
                    local.included.push(PathBuf::from(path));
                }
                continue;
            }
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            let (pattern, directory_only) = match line.strip_suffix("/**") {
                Some(directory) => (directory, true),
                None => (line.trim_end_matches('/'), line.ends_with('/')),
            };
            let pattern = pattern.trim_start_matches('/');
            if pattern.is_empty() || pattern.contains(['*', '?', '[', '\\']) {
                continue;
            }

            let entry = LocalEntry {
                path: PathBuf::from(pattern),
                directory_only,
            };
            if pattern.contains('/') {
                local.paths.push(entry);
            } else {
                local.root_names.push(entry);
            }
        }

        local
    }

    fn contains(&self, path: &Path, is_directory: bool) -> bool {
        let matches = |entry: &LocalEntry| {
            (path == entry.path && (is_directory || !entry.directory_only))
                || (path != entry.path && path.starts_with(&entry.path))
        };
        let at_root = path.parent() == Some(Path::new(""));

        self.paths.iter().any(matches) || (at_root && self.root_names.iter().any(matches))
    }
}

fn checkout() -> BTreeMap<PathBuf, Entry> {
    let root = repository_root();
    let gitignore = fs::read_to_string(root.join(".gitignore")).unwrap_or_default();
    let local = LocalFiles::parse(&gitignore);
    let mut paths: Vec<PathBuf> = local
        .included
        .iter()
        .filter(|path| root.join(path).is_file())
        .cloned()
        .collect();

    for entry in fs::read_dir(&root).expect("read the repository root") {
        let path = PathBuf::from(entry.expect("root entry").file_name());
        let is_directory = fs::symlink_metadata(root.join(&path)).is_ok_and(|meta| meta.is_dir());
        let is_cache = is_directory && root.join(&path).join(CACHE_DIRECTORY_TAG).is_file();
        if SKIPPED_DIRECTORIES
            .iter()
            .any(|skipped| path == Path::new(skipped))
            || is_cache
            || local.contains(&path, is_directory)
        {
            continue;
        }

        if is_directory {
            let directory = path.to_str().expect("UTF-8 directory name");
            paths.extend(
                files_under(directory)
                    .into_iter()
                    .filter(|file| !local.contains(file, false)),
            );
        } else {
            paths.push(path);
        }
    }

    paths
        .into_iter()
        .map(|path| {
            let entry = read(&root.join(&path));
            (path, entry)
        })
        .collect()
}

fn read(path: &Path) -> Entry {
    let metadata = fs::symlink_metadata(path)
        .unwrap_or_else(|error| panic!("stat {}: {error}", path.display()));

    if metadata.is_symlink() {
        let target = fs::read_link(path)
            .unwrap_or_else(|error| panic!("read link {}: {error}", path.display()));
        return Entry::Link(target);
    }

    Entry::Read(fs::read(path).unwrap_or_else(|error| panic!("read {}: {error}", path.display())))
}

fn audit(entries: &BTreeMap<PathBuf, Entry>) -> Vec<String> {
    let mut failures = Vec::new();

    for (path, entry) in entries {
        let shown = path.display();
        if is_python_name(path) {
            failures.push(format!("{shown}: Python source, project, or lock file"));
            continue;
        }

        let bytes = match entry {
            Entry::Link(target) => {
                if is_python_name(target) || names_python(&target.to_string_lossy()) {
                    failures.push(format!("{shown}: links to Python at {}", target.display()));
                }
                continue;
            }
            Entry::Read(bytes) => bytes,
        };

        let first_line = bytes
            .split(|byte| *byte == b'\n')
            .next()
            .unwrap_or_default();
        if first_line.starts_with(b"#!") && names_python(&String::from_utf8_lossy(first_line)) {
            failures.push(format!("{shown}:1: Python shebang"));
            continue;
        }
        if path == Path::new(THIS_GUARD) || !is_read(path) {
            continue;
        }

        let Ok(text) = std::str::from_utf8(bytes) else {
            // An extensionless file may be binary; a source file must be text.
            if path.extension().is_some() {
                failures.push(format!("{shown}: not UTF-8"));
            }
            continue;
        };
        for (number, line) in text.lines().enumerate() {
            if names_python(line) {
                failures.push(format!(
                    "{shown}:{}: names Python: {}",
                    number + 1,
                    line.trim()
                ));
            }
        }
    }

    failures
}

/// Sources by extension, and every extensionless file, such as `PKGBUILD`,
/// `.SRCINFO`, `Makefile`, `.envrc`, or a script.
fn is_read(path: &Path) -> bool {
    path.extension().is_none() || has_extension(path, &READ_EXTENSIONS)
}

fn is_python_name(path: &Path) -> bool {
    let name = file_name(path);

    has_extension(path, &PYTHON_EXTENSIONS)
        || PYTHON_PROJECT_FILES.contains(&name)
        || (name.starts_with("requirements") && name.ends_with(".txt"))
}

fn file_name(path: &Path) -> &str {
    path.file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
}

fn has_extension(path: &Path, extensions: &[&str]) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extensions.contains(&extension))
}

/// Whether `text` names a Python interpreter or package, qualified or not: a
/// word that is `python`, a version such as `python3` (and so `python3.12`),
/// or either one followed by a capitalized part, as in the nixpkgs names
/// `python3Packages`, `python311Packages`, and `python3Minimal`. A capital
/// `Python` counts only with a version, so prose stays readable, and a
/// `python` domain label such as `docs.python.org` names a website.
fn names_python(text: &str) -> bool {
    words(text).any(|(offset, word)| {
        is_python_word(word) && !is_domain_label(&text[offset + word.len()..])
    })
}

fn is_python_word(word: &str) -> bool {
    let (capitalized, rest) = match (word.strip_prefix("python"), word.strip_prefix("Python")) {
        (Some(rest), _) => (false, rest),
        (None, Some(rest)) => (true, rest),
        (None, None) => return false,
    };
    let version_length = rest.bytes().take_while(u8::is_ascii_digit).count();
    let (version, suffix) = rest.split_at(version_length);

    (suffix.is_empty() || suffix.starts_with(|character: char| character.is_ascii_uppercase()))
        && (!capitalized || !version.is_empty())
}

/// Whether the text after a word continues it into a domain name.
fn is_domain_label(after: &str) -> bool {
    let Some(rest) = after.strip_prefix('.') else {
        return false;
    };
    let label = rest
        .split(|character| !is_word_char(character))
        .next()
        .unwrap_or_default();

    DOMAIN_SUFFIXES.contains(&label)
}

fn read_entry(contents: &str) -> Entry {
    Entry::Read(contents.as_bytes().to_vec())
}

#[test]
fn the_repository_carries_no_python() {
    let failures = audit(&checkout());

    assert!(
        failures.is_empty(),
        "Python found:\n{}",
        failures.join("\n")
    );
}

#[test]
fn python_files_links_shebangs_and_words_fail() {
    let python_file = "Python source, project, or lock file";
    let cases = [
        ("tools/check.py", read_entry("print('x')\n"), python_file),
        ("tests/fixture.pyw", read_entry(""), python_file),
        ("docs/notebook.ipynb", read_entry("{}"), python_file),
        ("src/speedups.pyx", read_entry(""), python_file),
        ("setup.py", read_entry(""), python_file),
        ("pyproject.toml", read_entry(""), python_file),
        ("poetry.lock", read_entry(""), python_file),
        ("requirements-dev.txt", read_entry(""), python_file),
        (
            "tools/lib.py",
            Entry::Link(PathBuf::from("../vendor/lib")),
            python_file,
        ),
        (
            "helper",
            Entry::Link(PathBuf::from("tools/helper.py")),
            "links to Python",
        ),
        (
            "tools/run",
            Entry::Link(PathBuf::from("/usr/bin/python3")),
            "links to Python",
        ),
        (
            "bootstrap",
            read_entry("#!/usr/bin/env python3\n"),
            "Python shebang",
        ),
        (
            "tools/check",
            read_entry("#!/usr/bin/python3.12 -u\n"),
            "Python shebang",
        ),
        (
            "src/overlay/tests.rs",
            read_entry(
                "const SCRIPT: &str = r#\"import os\"#;\nfn f() { Command::new(\"python3\"); }\n",
            ),
            "src/overlay/tests.rs:2: names Python",
        ),
        (
            "tools/csharp/Infrastructure/ToolConstants.cs",
            read_entry("    public const string Python = \"python3\";\n"),
            "ToolConstants.cs:1: names Python",
        ),
        (
            "tools/gate.sh",
            read_entry("run python -c 1\n"),
            "tools/gate.sh:1: names Python",
        ),
        (
            "makefile",
            read_entry("check:\n\tpython3 tools/check\n"),
            "makefile:2: names Python",
        ),
        (
            ".envrc",
            read_entry("export PATH=$PWD/.venv/bin:$PATH # python3\n"),
            ".envrc:1",
        ),
        (
            "tools/helper",
            read_entry("set -eu\nexec python3 \"$@\"\n"),
            "tools/helper:2: names Python",
        ),
        (
            "packaging/wayscriber.service",
            read_entry("ExecStart=/usr/bin/python3 -m wayscriber\n"),
            "wayscriber.service:1: names Python",
        ),
        (
            "packaging/wayscriber.desktop",
            read_entry("Exec=python3 /usr/bin/wayscriber\n"),
            "wayscriber.desktop:1: names Python",
        ),
        (
            "tools/Directory.Build.props",
            read_entry("<Exec Command=\"python3 check\" />\n"),
            "Directory.Build.props:1: names Python",
        ),
        (
            "flake.nix",
            read_entry("  nativeCheckInputs = [ python3 ];\n"),
            "flake.nix:1: names Python",
        ),
        (
            "flake.nix",
            read_entry("  nativeCheckInputs = [ pkgs.python3 ];\n"),
            "flake.nix:1: names Python",
        ),
        (
            "flake.nix",
            read_entry("  nativeCheckInputs = [ pkgs.python3Packages.pytest ];\n"),
            "flake.nix:1: names Python",
        ),
        (
            "flake.nix",
            read_entry("  buildInputs = [ python311Packages.requests python3Minimal ];\n"),
            "flake.nix:1: names Python",
        ),
        (
            "packaging/nixpkgs/package.nix",
            read_entry("  nativeCheckInputs = [ (pkgs.python312.withPackages (p: [ ])) ];\n"),
            "package.nix:1: names Python",
        ),
        (
            "packaging/PKGBUILD",
            read_entry("makedepends=('cargo' 'python')\n"),
            "packaging/PKGBUILD:1: names Python",
        ),
        (
            "src/notes.rs",
            read_entry("// Requires Python3 on the build host.\n"),
            "src/notes.rs:1: names Python",
        ),
        (
            ".github/workflows/ci.yml",
            read_entry("      - run: python3.12 tools/check\n"),
            ".github/workflows/ci.yml:1: names Python",
        ),
    ];

    for (path, entry, expected) in cases {
        let failures = audit(&BTreeMap::from([(PathBuf::from(path), entry)]));

        assert!(
            failures.iter().any(|failure| failure.contains(expected)),
            "{path} should fail with {expected:?}, got {failures:?}"
        );
    }
}

#[test]
fn prose_lookalikes_and_unread_files_pass() {
    let entries = BTreeMap::from([
        (
            PathBuf::from("tools/README.md"),
            read_entry("Uses no python.\n"),
        ),
        (
            PathBuf::from("src/a.rs"),
            read_entry(
                "// Python is not used; see docs.python.org.\n// https://www.python.org/\n\
                 fn pythonic() {}\nconst python_free: bool = true;\n",
            ),
        ),
        (
            PathBuf::from("tools/run.sh"),
            read_entry("#!/usr/bin/env bash\necho python_free\n"),
        ),
        (
            PathBuf::from("tools/current"),
            Entry::Link(PathBuf::from("wayscriber")),
        ),
        (
            PathBuf::from("assets/a.png"),
            Entry::Read(vec![0x89, b'P', b'N', b'G', 0xff]),
        ),
        // An extensionless binary is skipped rather than reported as not UTF-8.
        (
            PathBuf::from(".DS_Store"),
            Entry::Read(vec![0, 0, 0, 1, 0xff]),
        ),
    ]);

    assert_eq!(audit(&entries), Vec::<String>::new());
}

#[test]
fn only_plain_ignored_names_and_paths_count_as_local() {
    let local = LocalFiles::parse(
        "# Local\n/target\n**/*.rs.bk\nCLAUDE.md\nrun.sh\n!/tools/run.sh\ntools/release.sh\n\
         packaging/**\n!packaging/PKGBUILD\n!packaging/icons/*.png\n!packaging/licenses/\nscripts/\ndocs/temp\n",
    );
    let entry = |path: &str, directory_only| LocalEntry {
        path: PathBuf::from(path),
        directory_only,
    };

    assert_eq!(
        local,
        LocalFiles {
            root_names: vec![
                entry("target", false),
                entry("CLAUDE.md", false),
                entry("run.sh", false),
                entry("packaging", true),
                entry("scripts", true),
            ],
            paths: vec![entry("tools/release.sh", false), entry("docs/temp", false)],
            included: ["tools/run.sh", "packaging/PKGBUILD"]
                .map(PathBuf::from)
                .to_vec(),
        }
    );
    assert!(local.contains(Path::new("run.sh"), false));
    assert!(local.contains(Path::new("docs/temp/draft.py"), false));
    assert!(local.contains(Path::new("packaging"), true));
    assert!(local.contains(Path::new("scripts"), true));
    // A root file named like a directory-only entry is not ignored by Git.
    assert!(!local.contains(Path::new("scripts"), false));
    assert!(!local.contains(Path::new("tools/run.sh"), false));
    assert!(!local.contains(Path::new("tools/scripts/run.sh"), false));
}
