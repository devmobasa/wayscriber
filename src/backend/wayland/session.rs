//! Session persistence bookkeeping for per-output snapshots.
//!
//! Tracks the current session options and whether a snapshot has been loaded
//! so WaylandState can coordinate persistence without storing extra fields.

use anyhow::{Result, anyhow};

use crate::input::InputState;
use crate::session::{self as stored_session, LoadSnapshotOutcome, SessionOptions};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// Tracks session persistence state and bookkeeping for per-output snapshots.
pub struct SessionState {
    options: Option<SessionOptions>,
    loaded: bool,
    loaded_board_data: bool,
    dirty: bool,
    dirty_since: Option<Instant>,
    last_dirty_at: Option<Instant>,
    last_save_at: Option<Instant>,
    autosave_retry_at: Option<Instant>,
    autosave_deferred_until: Option<Instant>,
    notified_failure: bool,
    notified_near_limit_paths: HashSet<PathBuf>,
    notified_trimmed_history: bool,
    notified_visible_only: bool,
    protected_session_paths: HashSet<PathBuf>,
    notified_expanded_load_paths: HashSet<PathBuf>,
}

impl SessionState {
    /// Creates a new session state wrapper using the supplied options.
    pub fn new(options: Option<SessionOptions>) -> Self {
        Self {
            options,
            loaded: false,
            loaded_board_data: false,
            dirty: false,
            dirty_since: None,
            last_dirty_at: None,
            last_save_at: None,
            autosave_retry_at: None,
            autosave_deferred_until: None,
            notified_failure: false,
            notified_near_limit_paths: HashSet::new(),
            notified_trimmed_history: false,
            notified_visible_only: false,
            protected_session_paths: HashSet::new(),
            notified_expanded_load_paths: HashSet::new(),
        }
    }

    /// Returns immutable access to the session options, if present.
    pub fn options(&self) -> Option<&SessionOptions> {
        self.options.as_ref()
    }

    /// Returns mutable access to the session options, if present.
    pub fn options_mut(&mut self) -> Option<&mut SessionOptions> {
        self.options.as_mut()
    }

    /// Returns true if a session snapshot has already been loaded this run.
    pub fn is_loaded(&self) -> bool {
        self.loaded
    }

    /// Marks the session as loaded and records whether board data is now on disk.
    pub fn mark_loaded(&mut self, loaded_board_data: bool) {
        self.loaded = true;
        self.loaded_board_data = loaded_board_data;
    }

    pub fn has_loaded_board_data(&self) -> bool {
        self.loaded_board_data
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    pub fn record_input_dirty(&mut self, now: Instant, input_dirty: bool) {
        if !input_dirty {
            return;
        }
        if !self.dirty {
            self.dirty_since = Some(now);
        }
        self.dirty = true;
        self.last_dirty_at = Some(now);
    }

    pub fn mark_saved(&mut self, now: Instant, saved_board_data: bool) {
        self.dirty = false;
        self.dirty_since = None;
        self.last_dirty_at = None;
        self.last_save_at = Some(now);
        self.autosave_retry_at = None;
        self.autosave_deferred_until = None;
        self.notified_failure = false;
        self.loaded_board_data = saved_board_data;
    }

    pub fn mark_clean_after_load(&mut self) {
        self.dirty = false;
        self.dirty_since = None;
        self.last_dirty_at = None;
        self.autosave_retry_at = None;
        self.autosave_deferred_until = None;
    }

    fn commit_runtime_open(&mut self, options: SessionOptions, loaded_board_data: bool) {
        self.options = Some(options);
        self.loaded = true;
        self.loaded_board_data = loaded_board_data;
        self.dirty = false;
        self.dirty_since = None;
        self.last_dirty_at = None;
        self.last_save_at = None;
        self.autosave_retry_at = None;
        self.autosave_deferred_until = None;
        self.notified_failure = false;
    }

    fn commit_runtime_save_as(
        &mut self,
        options: SessionOptions,
        now: Instant,
        saved_board_data: bool,
    ) {
        self.options = Some(options);
        self.loaded = true;
        self.loaded_board_data = saved_board_data;
        self.dirty = false;
        self.dirty_since = None;
        self.last_dirty_at = None;
        self.last_save_at = Some(now);
        self.autosave_retry_at = None;
        self.autosave_deferred_until = None;
        self.notified_failure = false;
    }

    fn commit_runtime_clear(&mut self, now: Instant) {
        self.loaded = true;
        self.loaded_board_data = false;
        self.dirty = false;
        self.dirty_since = None;
        self.last_dirty_at = None;
        self.last_save_at = Some(now);
        self.autosave_retry_at = None;
        self.autosave_deferred_until = None;
        self.notified_failure = false;
    }

    pub fn mark_autosave_failure(&mut self, now: Instant, backoff: Duration) -> bool {
        self.autosave_retry_at = Some(now + backoff);
        if self.notified_failure {
            false
        } else {
            self.notified_failure = true;
            true
        }
    }

    pub fn defer_autosave(&mut self, now: Instant, delay: Duration) {
        let until = now + delay;
        self.autosave_deferred_until = Some(match self.autosave_deferred_until {
            Some(current) => current.max(until),
            None => until,
        });
    }

    pub fn mark_near_limit_notified(&mut self, path: &Path) -> bool {
        self.notified_near_limit_paths.insert(path.to_path_buf())
    }

    pub fn mark_trimmed_history_notified(&mut self) -> bool {
        if self.notified_trimmed_history {
            false
        } else {
            self.notified_trimmed_history = true;
            true
        }
    }

    pub fn mark_visible_only_notified(&mut self) -> bool {
        if self.notified_visible_only {
            false
        } else {
            self.notified_visible_only = true;
            true
        }
    }

    pub fn protect_session_path(&mut self, path: PathBuf) {
        self.protected_session_paths.insert(path);
    }

    pub fn mark_expanded_load_notified(&mut self, path: &Path) -> bool {
        self.notified_expanded_load_paths.insert(path.to_path_buf())
    }

    pub fn should_skip_save_for_protected_path(&self, path: &Path, input_dirty: bool) -> bool {
        self.protected_session_paths.contains(path) && !self.dirty && !input_dirty
    }

    pub fn autosave_due(&self, now: Instant, options: &SessionOptions) -> bool {
        if !autosave_active(options) || !self.dirty {
            return false;
        }
        if let Some(retry_at) = self.autosave_retry_at
            && now < retry_at
        {
            return false;
        }
        if let Some(deferred_until) = self.autosave_deferred_until
            && now < deferred_until
        {
            return false;
        }
        let Some(last_dirty_at) = self.last_dirty_at else {
            return false;
        };
        let debounce_due = now >= last_dirty_at + options.autosave_idle;
        let dirty_since = self.dirty_since.unwrap_or(last_dirty_at);
        let base = match self.last_save_at {
            Some(last_save) if last_save > dirty_since => last_save,
            Some(_) | None => dirty_since,
        };
        let periodic_due = now >= base + options.autosave_interval;
        debounce_due || periodic_due
    }

    pub fn autosave_timeout(&self, now: Instant, options: &SessionOptions) -> Option<Duration> {
        if !autosave_active(options) || !self.dirty {
            return None;
        }
        let last_dirty_at = self.last_dirty_at?;
        let debounce_due = last_dirty_at + options.autosave_idle;
        let dirty_since = self.dirty_since.unwrap_or(last_dirty_at);
        let base = match self.last_save_at {
            Some(last_save) if last_save > dirty_since => last_save,
            Some(_) | None => dirty_since,
        };
        let periodic_due = base + options.autosave_interval;
        let next_due = if debounce_due <= periodic_due {
            debounce_due
        } else {
            periodic_due
        };
        let mut next_time = next_due;
        if let Some(retry_at) = self.autosave_retry_at {
            next_time = next_time.max(retry_at);
        }
        if let Some(deferred_until) = self.autosave_deferred_until {
            next_time = next_time.max(deferred_until);
        }
        Some(next_time.saturating_duration_since(now))
    }
}

fn autosave_active(options: &SessionOptions) -> bool {
    options.autosave_enabled
        && (options.any_enabled() || options.restore_tool_state || options.persist_history)
}

mod runtime;

pub(in crate::backend::wayland) use runtime::{
    RuntimeClearSessionReport, RuntimeClearToolStateReport, RuntimeOpenSessionReport,
    RuntimeSaveAsSessionReport, clear_current_session_runtime, clear_saved_tool_state_runtime,
    open_named_session_runtime, save_named_session_as_requires_overwrite,
    save_named_session_as_runtime,
};
pub(super) use runtime::{has_session_artifact, should_skip_unloaded_contentless_save};

#[cfg(test)]
mod tests;
