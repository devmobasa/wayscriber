//! Session persistence bookkeeping for per-output snapshots.
//!
//! Tracks the current session options and whether a snapshot has been loaded
//! so WaylandState can coordinate persistence without storing extra fields.

use crate::session::SessionOptions;
use std::time::{Duration, Instant};

/// Tracks session persistence state and bookkeeping for per-output snapshots.
pub struct SessionState {
    options: Option<SessionOptions>,
    loaded: bool,
    dirty: bool,
    dirty_since: Option<Instant>,
    last_dirty_at: Option<Instant>,
    last_save_at: Option<Instant>,
    autosave_retry_at: Option<Instant>,
    notified_failure: bool,
}

impl SessionState {
    /// Creates a new session state wrapper using the supplied options.
    pub fn new(options: Option<SessionOptions>) -> Self {
        Self {
            options,
            loaded: false,
            dirty: false,
            dirty_since: None,
            last_dirty_at: None,
            last_save_at: None,
            autosave_retry_at: None,
            notified_failure: false,
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

    /// Marks the session as loaded and records the identity used.
    pub fn mark_loaded(&mut self) {
        self.loaded = true;
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

    pub fn mark_saved(&mut self, now: Instant) {
        self.dirty = false;
        self.dirty_since = None;
        self.last_dirty_at = None;
        self.last_save_at = Some(now);
        self.autosave_retry_at = None;
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

    pub fn autosave_due(&self, now: Instant, options: &SessionOptions) -> bool {
        if !autosave_active(options) || !self.dirty {
            return false;
        }
        if let Some(retry_at) = self.autosave_retry_at
            && now < retry_at
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
        let next_time = match self.autosave_retry_at {
            Some(retry_at) => std::cmp::max(next_due, retry_at),
            None => next_due,
        };
        Some(next_time.saturating_duration_since(now))
    }
}

fn autosave_active(options: &SessionOptions) -> bool {
    options.autosave_enabled
        && (options.any_enabled() || options.restore_tool_state || options.persist_history)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn autosave_failure_backoff_delays_retry() {
        let mut options = SessionOptions::new(PathBuf::from("/tmp"), "display");
        options.autosave_enabled = true;
        options.persist_transparent = true;
        options.autosave_idle = Duration::from_millis(1);
        options.autosave_interval = Duration::from_millis(1);
        options.autosave_failure_backoff = Duration::from_millis(50);

        let mut state = SessionState::new(Some(options.clone()));
        let now = Instant::now();
        state.record_input_dirty(now, true);
        state.mark_autosave_failure(now, options.autosave_failure_backoff);

        assert!(!state.autosave_due(now, &options));
        assert_eq!(
            state.autosave_timeout(now, &options),
            Some(options.autosave_failure_backoff)
        );

        let later = now + options.autosave_failure_backoff;
        assert!(state.autosave_due(later, &options));
        assert_eq!(
            state.autosave_timeout(later, &options),
            Some(Duration::from_millis(0))
        );
    }
}
