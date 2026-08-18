use std::collections::HashSet;
use std::env;
use std::path::{Path, PathBuf};

use log::warn;

use crate::env_vars::PATH_ENV;
use crate::paths::home_dir;

/// Prefixes this project actually installs to, including ones the packaged
/// user unit PATH (`/usr/local/bin:/usr/bin:/bin`) does not search.
///
/// `PATH` is scanned separately. Do not add cargo/nix/opt folders here unless
/// a Wayscriber installer writes them; those still show up when they are on
/// `PATH`.
fn well_known_wayscriber_binaries() -> Vec<PathBuf> {
    let mut paths = vec![
        PathBuf::from("/usr/bin/wayscriber"),
        PathBuf::from("/usr/local/bin/wayscriber"),
    ];
    if let Some(home) = home_dir() {
        paths.push(home.join(".local").join("bin").join("wayscriber"));
    }
    paths
}

fn canonicalize_file(path: &Path) -> Option<PathBuf> {
    if !path.is_file() {
        return None;
    }
    path.canonicalize().ok()
}

/// Other `wayscriber` files on `PATH` or in well-known prefixes besides this process.
///
/// Overlay spawn follows `current_exe` first. A second copy is how `--version` or
/// `--about` on a different path can disagree with the running daemon.
pub(crate) fn other_installed_wayscriber_binaries(
    current_exe: &Path,
    path_env: Option<&str>,
    extra_candidates: &[&Path],
) -> Vec<PathBuf> {
    let Some(current) = canonicalize_file(current_exe) else {
        return Vec::new();
    };

    let mut seen = HashSet::new();
    seen.insert(current);
    let mut others = Vec::new();

    let mut consider = |candidate: &Path| {
        let Some(canonical) = canonicalize_file(candidate) else {
            return;
        };
        if seen.insert(canonical.clone()) {
            others.push(canonical);
        }
    };

    if let Some(path_env) = path_env {
        for dir in env::split_paths(path_env) {
            consider(&dir.join("wayscriber"));
        }
    }
    for candidate in extra_candidates {
        consider(candidate);
    }

    others
}

pub(super) fn warn_if_other_wayscriber_binaries() {
    let Ok(exe) = env::current_exe() else {
        return;
    };
    let path_env = env::var(PATH_ENV).ok();
    let extras = well_known_wayscriber_binaries();
    let extra_refs: Vec<&Path> = extras.iter().map(PathBuf::as_path).collect();
    let others = other_installed_wayscriber_binaries(&exe, path_env.as_deref(), &extra_refs);
    if others.is_empty() {
        return;
    }
    let other_list = others
        .iter()
        .map(|path| path.display().to_string())
        .collect::<Vec<_>>()
        .join(", ");
    warn!(
        "Another wayscriber binary exists besides this daemon ({}). Overlay spawn follows this process file; inspecting a different path with --version or --about can disagree. Other copies: {}",
        exe.display(),
        other_list
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::symlink;

    fn write_file(path: &Path) {
        fs::write(path, b"wayscriber-test-binary").unwrap();
    }

    #[test]
    fn no_conflict_when_path_and_extras_are_the_same_file() {
        let tmp = crate::test_temp::tempdir().unwrap();
        let binary = tmp.path().join("wayscriber");
        write_file(&binary);
        let path_env = tmp.path().display().to_string();

        assert!(
            other_installed_wayscriber_binaries(&binary, Some(&path_env), &[&binary]).is_empty()
        );
    }

    #[test]
    fn reports_a_second_file_on_path() {
        let tmp = crate::test_temp::tempdir().unwrap();
        let current_dir = tmp.path().join("current");
        let other_dir = tmp.path().join("other");
        fs::create_dir(&current_dir).unwrap();
        fs::create_dir(&other_dir).unwrap();
        let current = current_dir.join("wayscriber");
        let other = other_dir.join("wayscriber");
        write_file(&current);
        write_file(&other);
        let path_env = format!("{}:{}", other_dir.display(), current_dir.display());

        let others = other_installed_wayscriber_binaries(&current, Some(&path_env), &[]);
        assert_eq!(others, vec![other.canonicalize().unwrap()]);
    }

    #[test]
    fn reports_a_well_known_copy_even_when_path_matches_current() {
        let tmp = crate::test_temp::tempdir().unwrap();
        let current_dir = tmp.path().join("local");
        let usr_dir = tmp.path().join("usr");
        fs::create_dir(&current_dir).unwrap();
        fs::create_dir(&usr_dir).unwrap();
        let current = current_dir.join("wayscriber");
        let usr = usr_dir.join("wayscriber");
        write_file(&current);
        write_file(&usr);
        let path_env = current_dir.display().to_string();

        let others = other_installed_wayscriber_binaries(&current, Some(&path_env), &[&usr]);
        assert_eq!(others, vec![usr.canonicalize().unwrap()]);
    }

    #[test]
    fn symlink_to_the_same_file_is_not_a_conflict() {
        let tmp = crate::test_temp::tempdir().unwrap();
        let current = tmp.path().join("wayscriber");
        let alias = tmp.path().join("alias-wayscriber");
        write_file(&current);
        symlink(&current, &alias).unwrap();

        assert!(other_installed_wayscriber_binaries(&current, None, &[&alias]).is_empty());
    }

    #[test]
    fn missing_current_exe_yields_no_others() {
        let tmp = crate::test_temp::tempdir().unwrap();
        let missing = tmp.path().join("missing");
        let other = tmp.path().join("wayscriber");
        write_file(&other);

        assert!(other_installed_wayscriber_binaries(&missing, None, &[&other]).is_empty());
    }

    #[test]
    fn well_known_includes_resolved_user_local_bin() {
        let tmp = crate::test_temp::tempdir().unwrap();
        crate::test_env::with_env_var(
            crate::env_vars::HOME_ENV,
            Some(tmp.path().as_os_str()),
            || {
                let paths = well_known_wayscriber_binaries();
                assert!(paths.contains(&PathBuf::from("/usr/bin/wayscriber")));
                assert!(paths.contains(&PathBuf::from("/usr/local/bin/wayscriber")));
                assert!(paths.contains(&tmp.path().join(".local").join("bin").join("wayscriber")));
            },
        );
    }
}
