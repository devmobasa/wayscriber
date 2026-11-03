use std::collections::HashSet;
use std::env;
use std::ffi::OsStr;
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

/// Whether `candidate` is the wrapper that Nix's `wrapProgram` put in front of
/// `current`, both canonical.
///
/// `wrapProgram`, which `wrapGAppsHook4` runs, moves the binary to
/// `.<name>-wrapped` in the same directory, appending `_` for each further
/// wrap, and installs a wrapper under the original name that execs it. The
/// two files are one install, so the wrapper is not another copy.
fn is_nix_wrapper_of(candidate: &Path, current: &Path) -> bool {
    if candidate.parent() != current.parent() {
        return false;
    }
    let (Some(wrapper_name), Some(current_name)) = (
        candidate.file_name().and_then(OsStr::to_str),
        current.file_name().and_then(OsStr::to_str),
    ) else {
        return false;
    };

    current_name
        .strip_prefix('.')
        .and_then(|hidden| hidden.trim_end_matches('_').strip_suffix("-wrapped"))
        .is_some_and(|wrapped| wrapped == wrapper_name)
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
    seen.insert(current.clone());
    let mut others = Vec::new();

    let mut consider = |candidate: &Path| {
        let Some(canonical) = canonicalize_file(candidate) else {
            return;
        };
        if is_nix_wrapper_of(&canonical, &current) {
            return;
        }
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
    use crate::{env_vars, test_env, test_temp};

    use super::*;
    use std::fs;
    use std::os::unix::fs::symlink;

    fn write_file(path: &Path) {
        fs::write(path, b"wayscriber-test-binary").unwrap();
    }

    #[test]
    fn no_conflict_when_path_and_extras_are_the_same_file() {
        let tmp = test_temp::tempdir().unwrap();
        let binary = tmp.path().join("wayscriber");
        write_file(&binary);
        let path_env = tmp.path().display().to_string();

        assert!(
            other_installed_wayscriber_binaries(&binary, Some(&path_env), &[&binary]).is_empty()
        );
    }

    #[test]
    fn reports_a_second_file_on_path() {
        let tmp = test_temp::tempdir().unwrap();
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
        let tmp = test_temp::tempdir().unwrap();
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
        let tmp = test_temp::tempdir().unwrap();
        let current = tmp.path().join("wayscriber");
        let alias = tmp.path().join("alias-wayscriber");
        write_file(&current);
        symlink(&current, &alias).unwrap();

        assert!(other_installed_wayscriber_binaries(&current, None, &[&alias]).is_empty());
    }

    /// A Nix build's `bin/` after `wrapProgram`: the binary hidden as `payload`
    /// and the wrapper that execs it under the original name.
    fn nix_wrapped_install(bin: &Path, payload: &str) -> (PathBuf, PathBuf) {
        fs::create_dir_all(bin).unwrap();
        let wrapper = bin.join("wayscriber");
        let current = bin.join(payload);
        write_file(&wrapper);
        write_file(&current);
        (wrapper, current)
    }

    #[test]
    fn a_nix_wrapper_and_the_binary_it_runs_are_one_install() {
        let tmp = test_temp::tempdir().unwrap();
        let bin = tmp.path().join("store-wayscriber").join("bin");
        let (_, current) = nix_wrapped_install(&bin, ".wayscriber-wrapped");
        let path_env = bin.display().to_string();

        let others = other_installed_wayscriber_binaries(&current, Some(&path_env), &[]);

        assert!(
            others.is_empty(),
            "the wrapper beside the running binary is the same install: {others:?}"
        );
    }

    #[test]
    fn a_profile_link_to_the_nix_wrapper_is_the_same_install() {
        let tmp = test_temp::tempdir().unwrap();
        let bin = tmp.path().join("store-wayscriber").join("bin");
        let (wrapper, current) = nix_wrapped_install(&bin, ".wayscriber-wrapped");
        let profile_bin = tmp.path().join("profile").join("bin");
        fs::create_dir_all(&profile_bin).unwrap();
        symlink(&wrapper, profile_bin.join("wayscriber")).unwrap();
        let path_env = profile_bin.display().to_string();

        let others = other_installed_wayscriber_binaries(&current, Some(&path_env), &[]);

        assert!(
            others.is_empty(),
            "a profile entry resolves to the same wrapper: {others:?}"
        );
    }

    #[test]
    fn a_twice_wrapped_binary_is_still_one_install() {
        let tmp = test_temp::tempdir().unwrap();
        let bin = tmp.path().join("store-wayscriber").join("bin");
        let (_, current) = nix_wrapped_install(&bin, ".wayscriber-wrapped_");
        let path_env = bin.display().to_string();

        let others = other_installed_wayscriber_binaries(&current, Some(&path_env), &[]);

        assert!(
            others.is_empty(),
            "each further wrap appends `_` to the hidden name: {others:?}"
        );
    }

    #[test]
    fn another_nix_build_is_still_reported() {
        let tmp = test_temp::tempdir().unwrap();
        let running_bin = tmp.path().join("store-wayscriber-new").join("bin");
        let older_bin = tmp.path().join("store-wayscriber-old").join("bin");
        let (_, current) = nix_wrapped_install(&running_bin, ".wayscriber-wrapped");
        let (older_wrapper, _) = nix_wrapped_install(&older_bin, ".wayscriber-wrapped");
        let usr = tmp.path().join("usr-wayscriber");
        write_file(&usr);
        let path_env = format!("{}:{}", older_bin.display(), running_bin.display());

        let others = other_installed_wayscriber_binaries(&current, Some(&path_env), &[&usr]);

        assert_eq!(
            others,
            vec![
                older_wrapper.canonicalize().unwrap(),
                usr.canonicalize().unwrap()
            ],
            "a wrapper from another build directory and a system copy are separate installs"
        );
    }

    #[test]
    fn a_hidden_binary_of_another_name_does_not_claim_the_wrapper() {
        let tmp = test_temp::tempdir().unwrap();
        let bin = tmp.path().join("store-other").join("bin");
        let (wrapper, current) = nix_wrapped_install(&bin, ".other-wrapped");
        let path_env = bin.display().to_string();

        let others = other_installed_wayscriber_binaries(&current, Some(&path_env), &[]);

        assert_eq!(
            others,
            vec![wrapper.canonicalize().unwrap()],
            "only `.wayscriber-wrapped` is the binary behind `wayscriber`"
        );
    }

    #[test]
    fn missing_current_exe_yields_no_others() {
        let tmp = test_temp::tempdir().unwrap();
        let missing = tmp.path().join("missing");
        let other = tmp.path().join("wayscriber");
        write_file(&other);

        assert!(other_installed_wayscriber_binaries(&missing, None, &[&other]).is_empty());
    }

    #[test]
    fn well_known_includes_resolved_user_local_bin() {
        let tmp = test_temp::tempdir().unwrap();
        test_env::with_env_var(env_vars::HOME_ENV, Some(tmp.path().as_os_str()), || {
            let paths = well_known_wayscriber_binaries();
            assert!(paths.contains(&PathBuf::from("/usr/bin/wayscriber")));
            assert!(paths.contains(&PathBuf::from("/usr/local/bin/wayscriber")));
            assert!(paths.contains(&tmp.path().join(".local").join("bin").join("wayscriber")));
        });
    }
}
