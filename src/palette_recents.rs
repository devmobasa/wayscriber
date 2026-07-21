//! Persisted command-palette recents.
//!
//! Stores the most-recently-run palette actions next to `onboarding.toml`
//! (`~/.local/share/wayscriber/palette_recents.toml`) so the palette's
//! "Recent" section survives restarts. This is UI state, not configuration:
//! it never appears in `config.toml` and a corrupt or missing file is always
//! treated as an empty history, never an error.

use crate::domain::Action;
use crate::durable_io::{AtomicWriteOptions, OverwriteMode, PermissionPolicy, SymlinkPolicy};
use crate::paths::data_dir;
use log::warn;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::ErrorKind;
use std::path::PathBuf;

mod worker;

pub(crate) use worker::PaletteRecentsWriter;

const PALETTE_RECENTS_VERSION: u32 = 1;
const PALETTE_RECENTS_FILE: &str = "palette_recents.toml";
const PALETTE_RECENTS_DIR: &str = "wayscriber";

/// Maximum number of persisted (and in-memory) palette recents.
pub const PALETTE_RECENTS_CAP: usize = 8;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct PaletteRecentsFile {
    #[serde(default = "default_version")]
    version: u32,
    /// Snake-case action ids, most recent first.
    #[serde(default)]
    recent: Vec<toml::Value>,
}

fn default_version() -> u32 {
    PALETTE_RECENTS_VERSION
}

/// Load/save gateway for the palette recents file.
pub struct PaletteRecentsStore {
    recents: Vec<Action>,
    /// Whether `recents` is known to match what is durably on disk. Cleared
    /// whenever the desired history changes or a write fails, so a transient
    /// failure is retried on the next drain instead of being masked by the
    /// in-memory value already matching a later identical update.
    persisted: bool,
    path: Option<PathBuf>,
}

impl PaletteRecentsStore {
    pub fn load() -> Self {
        let Some(path) = palette_recents_path() else {
            return Self {
                recents: Vec::new(),
                // No target path: there is nothing to persist, so treat the
                // store as durable and never spin retrying an impossible write.
                persisted: true,
                path: None,
            };
        };
        Self::load_from_path(path)
    }

    fn load_from_path(path: PathBuf) -> Self {
        let (recents, persisted) = match fs::read_to_string(&path) {
            // Loaded (or absent): the in-memory view matches disk, so a later
            // identical `set_recents` can early-return without rewriting.
            Ok(raw) => (parse_recents(&raw), true),
            Err(err) if err.kind() == ErrorKind::NotFound => (Vec::new(), true),
            Err(err) => {
                warn!("Failed to read palette recents {}: {}", path.display(), err);
                // The on-disk state is unknown; start un-persisted so the next
                // update heals the file even if it matches the empty history.
                (Vec::new(), false)
            }
        };
        Self {
            recents,
            persisted,
            path: Some(path),
        }
    }

    pub fn recents(&self) -> &[Action] {
        &self.recents
    }

    /// Replace the stored history (deduped, capped, most-recent-first) and
    /// persist it atomically. Ordering is preserved as given.
    ///
    /// Returns `true` once the history is durable on disk (or when there is no
    /// history file to write) and `false` when the write failed. On failure the
    /// desired history is retained un-persisted so the caller can retry: a later
    /// identical `set_recents` re-attempts the write rather than early-returning
    /// success, which would silently drop the pending state.
    #[must_use = "a false return means the write failed and must be retried"]
    pub fn set_recents(&mut self, recents: &[Action]) -> bool {
        let mut deduped: Vec<Action> = Vec::with_capacity(PALETTE_RECENTS_CAP);
        for action in recents {
            if !deduped.contains(action) {
                deduped.push(*action);
            }
            if deduped.len() >= PALETTE_RECENTS_CAP {
                break;
            }
        }
        if deduped == self.recents && self.persisted {
            return true;
        }
        self.recents = deduped;
        self.persisted = false;
        self.save()
    }

    /// Write the current history atomically. Returns `true` on success (and
    /// marks the store persisted) or `false` on any failure, leaving the store
    /// un-persisted so the write is retried.
    fn save(&mut self) -> bool {
        let Some(path) = self.path.clone() else {
            // No path means nothing to persist; consider it durable.
            self.persisted = true;
            return true;
        };
        if let Some(parent) = path.parent()
            && let Err(err) = fs::create_dir_all(parent)
        {
            warn!(
                "Failed to create palette recents dir {}: {}",
                parent.display(),
                err
            );
            return false;
        }
        let file = PaletteRecentsFile {
            version: PALETTE_RECENTS_VERSION,
            recent: self
                .recents
                .iter()
                .filter_map(|action| toml::Value::try_from(action).ok())
                .collect(),
        };
        let contents = match toml::to_string_pretty(&file) {
            Ok(contents) => contents,
            Err(err) => {
                warn!("Failed to serialize palette recents: {}", err);
                return false;
            }
        };
        if let Err(err) = crate::durable_io::write_text_atomic(
            &path,
            &contents,
            AtomicWriteOptions {
                overwrite: OverwriteMode::Replace,
                permissions: PermissionPolicy::PreserveExistingOrMode(0o644),
                symlink: SymlinkPolicy::Reject,
                sync_file: true,
                sync_parent: true,
            },
        ) {
            warn!(
                "Failed to write palette recents {}: {}",
                path.display(),
                err
            );
            return false;
        }
        self.persisted = true;
        true
    }
}

/// Parse a recents file leniently: a corrupt document or unknown action ids
/// degrade to whatever valid entries remain (or an empty history), never an
/// error and never blocking startup.
fn parse_recents(raw: &str) -> Vec<Action> {
    let file: PaletteRecentsFile = match toml::from_str(raw) {
        Ok(file) => file,
        Err(err) => {
            warn!("Failed to parse palette recents: {}", err);
            return Vec::new();
        }
    };
    let mut recents: Vec<Action> = Vec::new();
    for value in file.recent {
        let Ok(action) = value.try_into::<Action>() else {
            continue;
        };
        if !recents.contains(&action) {
            recents.push(action);
        }
        if recents.len() >= PALETTE_RECENTS_CAP {
            break;
        }
    }
    recents
}

fn palette_recents_path() -> Option<PathBuf> {
    data_dir().map(|dir| dir.join(PALETTE_RECENTS_DIR).join(PALETTE_RECENTS_FILE))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn missing_file_yields_empty_recents() {
        let tmp = crate::test_temp::tempdir().expect("tempdir should succeed");
        let path = tmp
            .path()
            .join(PALETTE_RECENTS_DIR)
            .join(PALETTE_RECENTS_FILE);
        let store = PaletteRecentsStore::load_from_path(path);
        assert!(store.recents().is_empty());
    }

    #[test]
    fn recents_round_trip_preserves_order_dedupes_and_caps() {
        let tmp = crate::test_temp::tempdir().expect("tempdir should succeed");
        let path = tmp
            .path()
            .join(PALETTE_RECENTS_DIR)
            .join(PALETTE_RECENTS_FILE);
        let mut store = PaletteRecentsStore::load_from_path(path.clone());

        let history = vec![
            Action::ToggleHelp,
            Action::CaptureFileFull,
            Action::ToggleHelp, // duplicate must collapse to first position
            Action::Undo,
            Action::ClearCanvas,
            Action::ZoomIn,
            Action::ZoomOut,
            Action::SelectPenTool,
            Action::SelectEraserTool,
            Action::ToggleStatusBar, // overflows the cap
            Action::TogglePresenterMode,
        ];
        assert!(store.set_recents(&history), "write should succeed");
        assert!(path.exists());

        let reloaded = PaletteRecentsStore::load_from_path(path);
        assert_eq!(
            reloaded.recents(),
            &[
                Action::ToggleHelp,
                Action::CaptureFileFull,
                Action::Undo,
                Action::ClearCanvas,
                Action::ZoomIn,
                Action::ZoomOut,
                Action::SelectPenTool,
                Action::SelectEraserTool,
            ]
        );
        assert_eq!(reloaded.recents().len(), PALETTE_RECENTS_CAP);
    }

    #[test]
    fn corrupt_file_degrades_to_empty_without_error() {
        let tmp = crate::test_temp::tempdir().expect("tempdir should succeed");
        let path = tmp
            .path()
            .join(PALETTE_RECENTS_DIR)
            .join(PALETTE_RECENTS_FILE);
        fs::create_dir_all(path.parent().expect("parent")).expect("create dir");
        fs::write(&path, "not = [toml").expect("write invalid toml");

        let store = PaletteRecentsStore::load_from_path(path.clone());
        assert!(store.recents().is_empty());

        // A later save must recover the file in place.
        let mut store = store;
        assert!(
            store.set_recents(&[Action::ToggleHelp]),
            "recovery write should succeed"
        );
        let reloaded = PaletteRecentsStore::load_from_path(path);
        assert_eq!(reloaded.recents(), &[Action::ToggleHelp]);
    }

    #[test]
    fn identical_update_after_no_change_is_a_successful_noop() {
        let tmp = crate::test_temp::tempdir().expect("tempdir should succeed");
        let path = tmp
            .path()
            .join(PALETTE_RECENTS_DIR)
            .join(PALETTE_RECENTS_FILE);
        let mut store = PaletteRecentsStore::load_from_path(path);

        assert!(store.set_recents(&[Action::ToggleHelp]), "first write");
        // Re-issuing the same history once persisted must not rewrite: it is a
        // successful no-op (the store is already durable).
        assert!(store.set_recents(&[Action::ToggleHelp]), "no-op is success");
    }

    #[test]
    fn failed_write_retains_pending_state_and_is_retried() {
        // Dirty-retention-on-failure: a transient write failure must NOT be
        // masked when the in-memory value already matches. A later identical
        // update has to re-attempt the write (and fail again) instead of
        // early-returning success and silently dropping the pending state.
        let tmp = crate::test_temp::tempdir().expect("tempdir should succeed");
        // Put a regular file where the store's parent directory must go, so
        // `create_dir_all` on the parent fails deterministically.
        let blocker = tmp.path().join(PALETTE_RECENTS_DIR);
        fs::write(&blocker, "not a directory").expect("write blocker file");
        let path = blocker.join(PALETTE_RECENTS_FILE);

        let mut store = PaletteRecentsStore::load_from_path(path.clone());

        // First write fails (parent is a file, not a directory).
        assert!(
            !store.set_recents(&[Action::ToggleHelp]),
            "write must report failure when the directory cannot be created"
        );
        // The desired history is retained in memory even though it is not on disk.
        assert_eq!(store.recents(), &[Action::ToggleHelp]);
        // The *same* update must retry the failed write rather than early-return
        // success: the transient failure is not dropped.
        assert!(
            !store.set_recents(&[Action::ToggleHelp]),
            "an identical update after a failed write must retry, not mask the loss"
        );
    }

    #[test]
    fn unknown_action_ids_are_skipped_not_fatal() {
        let tmp = crate::test_temp::tempdir().expect("tempdir should succeed");
        let path = tmp
            .path()
            .join(PALETTE_RECENTS_DIR)
            .join(PALETTE_RECENTS_FILE);
        fs::create_dir_all(path.parent().expect("parent")).expect("create dir");
        fs::write(
            &path,
            "version = 1\nrecent = [\"toggle_help\", \"not_a_real_action\", \"undo\"]\n",
        )
        .expect("write recents");

        let store = PaletteRecentsStore::load_from_path(path);
        assert_eq!(store.recents(), &[Action::ToggleHelp, Action::Undo]);
    }
}
