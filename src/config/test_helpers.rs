use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use std::env;
use std::ffi::OsString;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

use crate::domain::Language;
use crate::env_vars::{LANGUAGE_OVERRIDE_ENV, XDG_CONFIG_HOME_ENV};
use crate::test_temp::TempDir;
use crate::{i18n, test_env};

/// Runs `f` with a fresh config home, and with the UI language under the
/// test's control: loading a config at startup selects its `[ui] language`,
/// so the `WAYSCRIBER_LANGUAGE` override is cleared for the call and the
/// process language is put back afterwards, including after a panic.
pub(crate) fn with_temp_config_home<F, T>(f: F) -> T
where
    F: FnOnce(&Path) -> T,
{
    let _guard = test_env::lock();
    let temp = TempDir::new().expect("tempdir");
    let _restore = RestoreConfigHome {
        config_home: env::var_os(XDG_CONFIG_HOME_ENV),
        language_override: env::var_os(LANGUAGE_OVERRIDE_ENV),
        language: i18n::language(),
    };
    // SAFETY: tests serialize process environment access and restore the previous value.
    unsafe {
        env::set_var(XDG_CONFIG_HOME_ENV, temp.path());
        env::remove_var(LANGUAGE_OVERRIDE_ENV);
    }

    f(temp.path())
}

/// What [`with_temp_config_home`] changed, put back on drop.
struct RestoreConfigHome {
    config_home: Option<OsString>,
    language_override: Option<OsString>,
    language: Language,
}

impl Drop for RestoreConfigHome {
    fn drop(&mut self) {
        // SAFETY: dropped while the environment lock is still held.
        unsafe {
            for (key, value) in [
                (XDG_CONFIG_HOME_ENV, self.config_home.take()),
                (LANGUAGE_OVERRIDE_ENV, self.language_override.take()),
            ] {
                match value {
                    Some(value) => env::set_var(key, value),
                    None => env::remove_var(key),
                }
            }
        }
        // The override is back, so restore the resolved language directly.
        i18n::restore_language(self.language);
    }
}

/// Everything about a config path that reading it must leave alone.
///
/// `config.toml` is an authored input: outside an explicit user edit action —
/// the configurator's Save, or one of the overlay's three scoped edits — no
/// process may create, replace, truncate, rewrite, touch, chmod, or back it up.
/// The callers of this snapshot are all the other paths, the ones that only
/// read: loading, validating, running, and every gesture that is not one of
/// those edits. Contents
/// alone would not prove that — a rewrite with identical bytes still moves the
/// mtime, a chmod changes nothing visible, and a `.bak` appears next to the
/// file rather than in it — so the whole observable footprint is captured and
/// compared.
#[derive(Debug)]
pub(crate) struct ConfigFileSnapshot {
    path: PathBuf,
    facts: PathFacts,
    /// Every name in the containing directory, sorted: a timestamped backup, a
    /// leftover atomic-write temp file, or a config created out of nothing all
    /// show up here and nowhere else.
    siblings: Vec<String>,
}

#[derive(Debug, PartialEq, Eq)]
struct PathFacts {
    /// The path itself, without following a symlink. `None` when nothing is
    /// there — the state a loader must also leave alone.
    link: Option<EntryFacts>,
    /// The file the path resolves to, with its bytes. For a symlink this is
    /// the target, which is where a write that followed the link would land.
    resolved: Option<(EntryFacts, Vec<u8>)>,
}

#[derive(Debug, PartialEq, Eq)]
struct EntryFacts {
    len: u64,
    /// `None` off unix, where the permission bits are not the same concept.
    mode: Option<u32>,
    modified: Option<SystemTime>,
    /// Where a symlink points, so a relinked config is not mistaken for an
    /// untouched one.
    link_target: Option<PathBuf>,
}

impl ConfigFileSnapshot {
    pub(crate) fn capture(path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        let siblings = path
            .parent()
            .and_then(|parent| fs::read_dir(parent).ok())
            .map(|entries| {
                let mut names = entries
                    .filter_map(Result::ok)
                    .map(|entry| entry.file_name().to_string_lossy().into_owned())
                    .collect::<Vec<_>>();
                names.sort();
                names
            })
            .unwrap_or_default();
        Self {
            facts: PathFacts {
                link: link_facts(&path),
                resolved: resolved_facts(&path),
            },
            siblings,
            path,
        }
    }

    /// Fails with `note` naming the operation that was supposed to be a read.
    pub(crate) fn assert_unchanged(&self, note: &str) {
        let current = Self::capture(&self.path);
        assert_eq!(
            current.facts,
            self.facts,
            "{note} changed {}",
            self.path.display()
        );
        assert_eq!(
            current.siblings,
            self.siblings,
            "{note} added or removed files next to {}",
            self.path.display()
        );
    }
}

fn link_facts(path: &Path) -> Option<EntryFacts> {
    let metadata = fs::symlink_metadata(path).ok()?;
    Some(EntryFacts {
        len: metadata.len(),
        mode: permission_mode(&metadata),
        modified: metadata.modified().ok(),
        link_target: fs::read_link(path).ok(),
    })
}

fn resolved_facts(path: &Path) -> Option<(EntryFacts, Vec<u8>)> {
    let metadata = fs::metadata(path).ok()?;
    if !metadata.is_file() {
        return None;
    }
    let bytes = fs::read(path).ok()?;
    Some((
        EntryFacts {
            len: metadata.len(),
            mode: permission_mode(&metadata),
            modified: metadata.modified().ok(),
            link_target: None,
        },
        bytes,
    ))
}

#[cfg(unix)]
fn permission_mode(metadata: &fs::Metadata) -> Option<u32> {
    Some(metadata.permissions().mode())
}

#[cfg(not(unix))]
fn permission_mode(_metadata: &fs::Metadata) -> Option<u32> {
    None
}
