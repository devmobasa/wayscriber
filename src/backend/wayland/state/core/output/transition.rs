use super::*;
use crate::backend::wayland::session::{
    SessionCommand, SourceWriteRefused, interaction_defer_interval,
};

impl WaylandState {
    pub(in crate::backend::wayland) fn begin_session_output_transition(
        &mut self,
        physical_output_identity: Option<String>,
        reason: &str,
    ) {
        let Some(mut staged_options) = self.session_options().cloned() else {
            return;
        };
        let changed = staged_options.set_output_identity(physical_output_identity.as_deref());
        let same_epoch_pending = self
            .session
            .pending_output_transition()
            .is_some_and(|pending| pending.source_epoch == self.session.target_epoch());
        let matching_pending = same_epoch_pending
            && self
                .session
                .pending_output_transition()
                .is_some_and(|pending| {
                    pending.physical_output_identity == physical_output_identity
                });
        let interaction_active =
            self.session_transaction.is_some() || session_save::should_defer_for_interaction(self);
        let input_dirty = self.input_state.is_session_dirty();
        let live_source_resolution_pending = self
            .session
            .resolve_live_source_resolution(input_dirty, interaction_active);
        let start = output_transition_start(
            self.session.is_loaded(),
            changed,
            matching_pending,
            same_epoch_pending,
            live_source_resolution_pending,
            interaction_active,
        );

        let retry_at = Instant::now() + interaction_defer_interval();
        match start {
            OutputTransitionStart::IgnoreCurrentTarget => {
                if self
                    .session
                    .cancel_output_transition_for_live_source()
                    .is_some()
                {
                    log::info!(
                        "Canceling pending output transition because the physical output matches the active logical target"
                    );
                }
            }
            OutputTransitionStart::KeepPending => {
                log::debug!(
                    "Keeping existing pending output transition for physical output {:?}",
                    physical_output_identity
                );
            }
            OutputTransitionStart::DeferForInteraction => {
                self.session.stage_output_transition(
                    staged_options,
                    physical_output_identity,
                    retry_at,
                );
                if self.session.is_loaded() {
                    self.notify_output_transition_deferred();
                }
            }
            OutputTransitionStart::LoadInitial if self.session.begin_initial_load() => {
                // This callback blocks one initial load, but earlier callbacks
                // may have delivered ink. Keep that ink instead of replacing it.
                if self.session.is_dirty() || self.input_state.is_session_dirty() {
                    self.session.stage_output_transition(
                        staged_options,
                        physical_output_identity,
                        output_transition_retry_at(self.output_transition_failure_backoff()),
                    );
                    self.input_state.push_toast(
                        ToastPriority::Critical,
                        "session.load",
                        Toast::warning(format!(
                            "Drawings kept on screen: {}",
                            SourceWriteRefused::NotLoaded
                        )),
                    );
                    return;
                }
                let first_output_resolved =
                    !staged_options.per_output || staged_options.output_identity().is_some();
                if let Err(err) = self.load_configured_session_for_options(
                    staged_options.clone(),
                    physical_output_identity.as_deref(),
                    "initial output load",
                ) {
                    warn!("Initial session load failed: {err:#}");
                    self.session.stage_output_transition(
                        staged_options,
                        physical_output_identity,
                        output_transition_retry_at(self.output_transition_failure_backoff()),
                    );
                    self.notify_session_load_failure(&err);
                } else {
                    self.announce_launch_restore(first_output_resolved);
                }
            }
            OutputTransitionStart::LoadInitial | OutputTransitionStart::ResolveTransition => {
                if let Err(err) = self.run_output_transition(
                    staged_options.clone(),
                    physical_output_identity.clone(),
                    reason,
                ) {
                    warn!("Failed to complete session transition for {reason}: {err:#}");
                    let retry_at =
                        output_transition_retry_at(self.output_transition_failure_backoff());
                    self.session.stage_output_transition(
                        staged_options,
                        physical_output_identity,
                        retry_at,
                    );
                    self.notify_output_transition_deferred();
                }
            }
        }
    }

    pub(in crate::backend::wayland) fn retry_pending_output_transition_if_due(
        &mut self,
        now: Instant,
    ) -> anyhow::Result<bool> {
        let Some(pending) = self.session.pending_output_transition() else {
            return Ok(false);
        };
        if now < pending.retry_at {
            return Ok(false);
        }
        if pending.source_epoch != self.session.target_epoch() {
            warn!(
                "Discarding stale output transition owned by epoch {} while active epoch is {}",
                pending.source_epoch,
                self.session.target_epoch()
            );
            self.session.cancel_pending_output_transition();
            return Ok(true);
        }
        if self.session_transaction.is_some() || session_save::should_defer_for_interaction(self) {
            self.session
                .defer_output_transition(now, interaction_defer_interval());
            log::debug!("Deferring pending output transition while interaction is active");
            return Ok(true);
        }

        let Some(pending) = self.session.pending_output_transition().cloned() else {
            return Ok(false);
        };
        if let Err(err) = self.run_output_transition(
            pending.staged_options.clone(),
            pending.physical_output_identity.clone(),
            "deferred output transition",
        ) {
            let retry_at = output_transition_retry_at(self.output_transition_failure_backoff());
            self.session.stage_output_transition(
                pending.staged_options,
                pending.physical_output_identity,
                retry_at,
            );
            return Err(err);
        }
        Ok(true)
    }

    pub(in crate::backend::wayland) fn begin_configure_fallback_session_transition(
        &mut self,
        reason: &str,
    ) {
        if self.session.is_loaded() {
            return;
        }
        let physical_output_identity = self
            .surface
            .current_output()
            .as_ref()
            .and_then(|output| self.output_identity_for(output));
        self.begin_session_output_transition(physical_output_identity, reason);
        self.input_state.needs_redraw = true;
    }

    /// Resolves a canceled return-to-source transition as soon as the interaction
    /// that protected it becomes idle. This is called after protocol dispatch and
    /// from the persistence tick, so a clean initial load does not depend on another
    /// compositor configure event.
    pub(in crate::backend::wayland) fn reconcile_live_source_interaction_if_idle(
        &mut self,
        reason: &str,
    ) -> bool {
        if !self.session.has_pending_live_source_resolution() {
            return false;
        }
        if self.session.is_loaded() {
            let _ = self.session.resolve_live_source_resolution(false, false);
            return false;
        }
        let interaction_active =
            self.session_transaction.is_some() || session_save::should_defer_for_interaction(self);
        if !live_source_reconciliation_ready(
            true,
            self.session.pending_output_transition().is_some(),
            interaction_active,
            self.persistence.is_healthy(),
        ) {
            return false;
        }

        if self.session.is_dirty() || self.input_state.is_session_dirty() {
            if let Some(mut options) = self.session.options().cloned() {
                let identity = self
                    .surface
                    .current_output()
                    .as_ref()
                    .and_then(|output| self.output_identity_for(output));
                options.set_output_identity(identity.as_deref());
                self.session.stage_output_transition(
                    options,
                    identity,
                    Instant::now() + self.output_transition_failure_backoff(),
                );
            }
            return false;
        }

        log::info!(
            "Resolving live source after output-transition cancellation ({reason}, epoch={})",
            self.session.target_epoch()
        );
        self.begin_configure_fallback_session_transition(reason);
        true
    }

    fn run_output_transition(
        &mut self,
        staged_options: session::SessionOptions,
        physical_output_identity: Option<String>,
        reason: &str,
    ) -> anyhow::Result<()> {
        if self.session_transaction.is_some() || session_save::should_defer_for_interaction(self) {
            return Err(anyhow::anyhow!(
                "output transition became ineligible because an interaction started"
            ));
        }
        if let Some(pending) = self.session.pending_output_transition()
            && pending.source_epoch != self.session.target_epoch()
        {
            return Err(anyhow::anyhow!("stale output transition source epoch"));
        }
        // The session is about to be written and then replaced. Close any wheel
        // adjustment first, so its undo entry is part of what gets persisted
        // instead of being dropped with the frame it belonged to.
        self.input_state.flush_spotlight_magnification_gesture();
        self.spotlight.clear_wheel_idle_deadline();
        if !self
            .session
            .pending_output_transition()
            .is_some_and(|pending| {
                pending.physical_output_identity == physical_output_identity
                    && pending.staged_options.session_file_path()
                        == staged_options.session_file_path()
            })
        {
            self.session.stage_output_transition(
                staged_options,
                physical_output_identity.clone(),
                Instant::now(),
            );
        }
        let transition = self
            .session
            .pending_output_transition()
            .expect("staged output transition")
            .clone();
        let remembered = self.unloaded_remembered_session();
        let home = self
            .session_home
            .options_for_output(physical_output_identity.as_deref())
            .map(Box::new);
        let leaves_placeholder = physical_output_identity.is_some()
            && self.session_options().is_some_and(|current| {
                current.per_output
                    && current.output_identity().is_none()
                    && !current.is_named_file()
            });
        self.start_session_command(SessionCommand::Output {
            transition: Box::new(transition),
            remembered,
            home,
        })?;
        if self.session_transaction.is_none() {
            return Ok(());
        }
        if leaves_placeholder {
            // A per-output session loads a placeholder without an output name
            // until the surface enters one. Leaving it is part of startup, so
            // it runs to completion as the first load does: the user never
            // draws into a placeholder whose output session is still loading.
            info!("Resolving the first output session after {reason}");
            return self.finish_session_command_blocking();
        }
        info!("Started nonblocking output transition after {reason}");
        Ok(())
    }
}
