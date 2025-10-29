//! Session persistence bookkeeping for per-output snapshots.
//!
//! Tracks the current session options and whether a snapshot has been loaded
//! so WaylandState can coordinate persistence without storing extra fields.

use crate::session::SessionOptions;

/// Tracks session persistence state and bookkeeping for per-output snapshots.
pub struct SessionState {
    options: Option<SessionOptions>,
    loaded: bool,
}

impl SessionState {
    /// Creates a new session state wrapper using the supplied options.
    pub fn new(options: Option<SessionOptions>) -> Self {
        Self {
            options,
            loaded: false,
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
}
