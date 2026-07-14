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

#[derive(Debug, Clone, Copy)]
pub(in crate::backend::wayland) struct DirtyWindow {
    generation: u64,
    dirty_since: Instant,
    last_dirty_at: Instant,
}

#[derive(Debug, Clone, Copy)]
struct InFlightAutosave {
    request_id: RequestId,
    window: DirtyWindow,
}

#[derive(Debug, Clone)]
pub(in crate::backend::wayland) struct PendingOutputTransition {
    pub(in crate::backend::wayland) source_epoch: u64,
    pub(in crate::backend::wayland) staged_options: SessionOptions,
    pub(in crate::backend::wayland) physical_output_identity: Option<String>,
    pub(in crate::backend::wayland) retry_at: Instant,
    pub(in crate::backend::wayland) failure_notified: bool,
}

/// Tracks session persistence state and bookkeeping for per-output snapshots.
pub struct SessionState {
    options: Option<SessionOptions>,
    loaded: bool,
    loaded_board_data: bool,
    target_epoch: u64,
    edit_generation: u64,
    dirty: bool,
    dirty_since: Option<Instant>,
    last_dirty_at: Option<Instant>,
    last_save_at: Option<Instant>,
    autosave_retry_at: Option<Instant>,
    autosave_deferred_until: Option<Instant>,
    in_flight_autosave: Option<InFlightAutosave>,
    pending_output_transition: Option<PendingOutputTransition>,
    live_source_resolution_pending: bool,
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
            target_epoch: 0,
            edit_generation: 0,
            dirty: false,
            dirty_since: None,
            last_dirty_at: None,
            last_save_at: None,
            autosave_retry_at: None,
            autosave_deferred_until: None,
            in_flight_autosave: None,
            pending_output_transition: None,
            live_source_resolution_pending: false,
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
    #[allow(dead_code)]
    pub fn options_mut(&mut self) -> Option<&mut SessionOptions> {
        self.options.as_mut()
    }

    /// Returns true if the active logical session source has been resolved this run.
    pub fn is_loaded(&self) -> bool {
        self.loaded
    }

    /// Marks the session as loaded and records whether board data is now on disk.
    #[allow(dead_code)]
    pub fn mark_loaded(&mut self, loaded_board_data: bool) {
        self.loaded = true;
        self.loaded_board_data = loaded_board_data;
        self.live_source_resolution_pending = false;
    }

    pub fn has_loaded_board_data(&self) -> bool {
        self.loaded_board_data
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty || self.in_flight_autosave.is_some()
    }

    pub(in crate::backend::wayland) fn target_epoch(&self) -> u64 {
        self.target_epoch
    }

    pub(in crate::backend::wayland) fn edit_generation(&self) -> u64 {
        self.edit_generation
    }

    pub fn record_input_dirty(&mut self, now: Instant, input_dirty: bool) {
        if !input_dirty {
            return;
        }
        if self.live_source_resolution_pending {
            self.loaded = true;
            self.live_source_resolution_pending = false;
        }
        self.edit_generation = self.edit_generation.wrapping_add(1);
        if !self.dirty {
            self.dirty_since = Some(now);
        }
        self.dirty = true;
        self.last_dirty_at = Some(now);
    }

    pub fn mark_saved(&mut self, now: Instant, saved_board_data: bool) {
        self.in_flight_autosave = None;
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
        self.in_flight_autosave = None;
        self.dirty = false;
        self.dirty_since = None;
        self.last_dirty_at = None;
        self.autosave_retry_at = None;
        self.autosave_deferred_until = None;
        self.live_source_resolution_pending = false;
    }

    pub(in crate::backend::wayland) fn commit_runtime_open(
        &mut self,
        options: SessionOptions,
        loaded_board_data: bool,
    ) {
        self.advance_target_epoch();
        self.options = Some(options);
        self.loaded = true;
        self.loaded_board_data = loaded_board_data;
        self.dirty = false;
        self.dirty_since = None;
        self.last_dirty_at = None;
        self.last_save_at = None;
        self.autosave_retry_at = None;
        self.autosave_deferred_until = None;
        self.in_flight_autosave = None;
        self.notified_failure = false;
    }

    pub(in crate::backend::wayland) fn commit_runtime_save_as(
        &mut self,
        options: SessionOptions,
        now: Instant,
        saved_board_data: bool,
    ) {
        self.advance_target_epoch();
        self.options = Some(options);
        self.loaded = true;
        self.loaded_board_data = saved_board_data;
        self.dirty = false;
        self.dirty_since = None;
        self.last_dirty_at = None;
        self.last_save_at = Some(now);
        self.autosave_retry_at = None;
        self.autosave_deferred_until = None;
        self.in_flight_autosave = None;
        self.notified_failure = false;
    }

    pub(in crate::backend::wayland) fn commit_runtime_clear(&mut self, now: Instant) {
        self.loaded = true;
        self.loaded_board_data = false;
        self.dirty = false;
        self.dirty_since = None;
        self.last_dirty_at = None;
        self.last_save_at = Some(now);
        self.autosave_retry_at = None;
        self.autosave_deferred_until = None;
        self.in_flight_autosave = None;
        self.live_source_resolution_pending = false;
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
        if !autosave_active(options) || !self.dirty || self.in_flight_autosave.is_some() {
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
        if !autosave_active(options) || !self.dirty || self.in_flight_autosave.is_some() {
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

    pub(in crate::backend::wayland) fn prepare_autosave_submission(&self) -> Result<DirtyWindow> {
        if self.in_flight_autosave.is_some() {
            return Err(anyhow!("an autosave ticket is already in flight"));
        }
        let (Some(dirty_since), Some(last_dirty_at)) = (self.dirty_since, self.last_dirty_at)
        else {
            return Err(anyhow!(
                "cannot submit autosave without a pending dirty window"
            ));
        };
        Ok(DirtyWindow {
            generation: self.edit_generation,
            dirty_since,
            last_dirty_at,
        })
    }

    pub(in crate::backend::wayland) fn commit_autosave_submission(
        &mut self,
        request_id: RequestId,
        window: DirtyWindow,
    ) {
        debug_assert!(self.in_flight_autosave.is_none());
        self.in_flight_autosave = Some(InFlightAutosave { request_id, window });
        self.dirty = false;
        self.dirty_since = None;
        self.last_dirty_at = None;
        self.autosave_deferred_until = None;
    }

    pub(in crate::backend::wayland) fn complete_autosave(
        &mut self,
        request_id: RequestId,
        now: Instant,
        result: &Result<persistence::SaveCompletion>,
    ) -> Result<bool> {
        let Some(ticket) = self.in_flight_autosave.take() else {
            return Err(anyhow!(
                "autosave completion arrived without an in-flight ticket"
            ));
        };
        if ticket.request_id != request_id || request_id.target_epoch != self.target_epoch {
            self.merge_dirty_window(ticket.window);
            return Err(anyhow!(
                "autosave completion does not own the active ticket/target epoch"
            ));
        }
        match result {
            Ok(save) if save.committed() => {
                self.last_save_at = Some(now);
                self.autosave_retry_at = None;
                self.notified_failure = false;
                self.loaded_board_data = save.committed_board_data;
                Ok(true)
            }
            Ok(_) | Err(_) => {
                self.merge_dirty_window(ticket.window);
                Ok(false)
            }
        }
    }

    pub(in crate::backend::wayland) fn restore_in_flight_autosave(&mut self) -> bool {
        if let Some(ticket) = self.in_flight_autosave.take() {
            self.merge_dirty_window(ticket.window);
            true
        } else {
            false
        }
    }

    fn merge_dirty_window(&mut self, window: DirtyWindow) {
        self.dirty = true;
        self.edit_generation = self.edit_generation.max(window.generation);
        self.dirty_since = Some(self.dirty_since.map_or(window.dirty_since, |current| {
            current.min(window.dirty_since)
        }));
        self.last_dirty_at = Some(self.last_dirty_at.map_or(window.last_dirty_at, |current| {
            current.max(window.last_dirty_at)
        }));
    }

    pub(in crate::backend::wayland) fn stage_output_transition(
        &mut self,
        staged_options: SessionOptions,
        physical_output_identity: Option<String>,
        retry_at: Instant,
    ) {
        let failure_notified = self
            .pending_output_transition
            .as_ref()
            .is_some_and(|pending| {
                pending.source_epoch == self.target_epoch
                    && pending.physical_output_identity == physical_output_identity
            });
        self.pending_output_transition = Some(PendingOutputTransition {
            source_epoch: self.target_epoch,
            staged_options,
            physical_output_identity,
            retry_at,
            failure_notified,
        });
    }

    pub(in crate::backend::wayland) fn pending_output_transition(
        &self,
    ) -> Option<&PendingOutputTransition> {
        self.pending_output_transition.as_ref()
    }

    pub(in crate::backend::wayland) fn take_pending_output_transition(
        &mut self,
    ) -> Option<PendingOutputTransition> {
        self.pending_output_transition.take()
    }

    pub(in crate::backend::wayland) fn cancel_pending_output_transition(
        &mut self,
    ) -> Option<PendingOutputTransition> {
        self.pending_output_transition.take()
    }

    pub(in crate::backend::wayland) fn has_pending_live_source_resolution(&self) -> bool {
        self.live_source_resolution_pending
    }

    /// Cancels a superseded destination while retaining dirty live source data.
    ///
    /// A dirty source becomes authoritative for this run so a later configure
    /// fallback cannot reload over it. A clean unloaded source remains pending
    /// until the controller can perform its initial load. The dirty window and
    /// target epoch remain unchanged.
    pub(in crate::backend::wayland) fn cancel_output_transition_for_live_source(
        &mut self,
        input_dirty: bool,
    ) -> Option<PendingOutputTransition> {
        let pending = self.pending_output_transition.take();
        if pending.is_some() {
            if self.is_dirty() || input_dirty {
                self.loaded = true;
                self.live_source_resolution_pending = false;
            } else {
                self.live_source_resolution_pending = !self.loaded;
            }
        }
        pending
    }

    /// Resolves provisional protection after an output transition was canceled.
    ///
    /// Returns true while an interaction still blocks resolution. A committed
    /// mutation makes the live source authoritative; a clean idle source remains
    /// unloaded so the controller can perform the configured initial load.
    pub(in crate::backend::wayland) fn resolve_live_source_resolution(
        &mut self,
        input_dirty: bool,
        interaction_active: bool,
    ) -> bool {
        if !self.live_source_resolution_pending {
            return false;
        }
        if self.is_dirty() || input_dirty {
            self.loaded = true;
            self.live_source_resolution_pending = false;
            return false;
        }
        if interaction_active {
            return true;
        }
        self.live_source_resolution_pending = false;
        false
    }

    pub(in crate::backend::wayland) fn output_transition_timeout(
        &self,
        now: Instant,
    ) -> Option<Duration> {
        self.pending_output_transition
            .as_ref()
            .map(|transition| transition.retry_at.saturating_duration_since(now))
    }

    pub(in crate::backend::wayland) fn defer_output_transition(
        &mut self,
        now: Instant,
        delay: Duration,
    ) {
        if let Some(transition) = self.pending_output_transition.as_mut() {
            transition.retry_at = now + delay;
        }
    }

    pub(in crate::backend::wayland) fn mark_output_transition_notified(&mut self) -> bool {
        let Some(transition) = self.pending_output_transition.as_mut() else {
            return false;
        };
        if transition.failure_notified {
            false
        } else {
            transition.failure_notified = true;
            true
        }
    }

    pub(in crate::backend::wayland) fn commit_output_options(
        &mut self,
        options: SessionOptions,
        loaded_board_data: bool,
    ) {
        self.advance_target_epoch();
        self.options = Some(options);
        self.loaded = true;
        self.loaded_board_data = loaded_board_data;
        self.mark_clean_after_load();
    }

    fn advance_target_epoch(&mut self) {
        self.target_epoch = self.target_epoch.wrapping_add(1);
        self.live_source_resolution_pending = false;
        if self
            .pending_output_transition
            .as_ref()
            .is_some_and(|pending| pending.source_epoch != self.target_epoch)
        {
            self.pending_output_transition = None;
        }
    }
}

fn autosave_active(options: &SessionOptions) -> bool {
    options.autosave_enabled
        && (options.any_enabled() || options.restore_tool_state || options.persist_history)
}

mod persistence;
mod runtime;

pub(in crate::backend::wayland) use persistence::{
    PersistenceCompletion, PersistenceController, PersistenceOperation, PersistenceOutcome,
    RequestId, SaveCompletion, SaveStrategy, SubmitFailure,
};

pub(in crate::backend::wayland) use runtime::{
    RuntimeClearSessionReport, RuntimeClearToolStateReport, RuntimeOpenSessionReport,
    RuntimeSaveAsSessionReport,
};
#[cfg(test)]
pub(super) use runtime::{
    clear_current_session_runtime, clear_saved_tool_state_runtime, open_named_session_runtime,
    save_named_session_as_requires_overwrite, save_named_session_as_runtime,
};
pub(super) use runtime::{has_session_artifact, should_skip_unloaded_contentless_save};

#[cfg(test)]
mod tests;
