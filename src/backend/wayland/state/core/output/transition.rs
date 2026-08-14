use super::*;

impl WaylandState {
    pub(in crate::backend::wayland) fn begin_session_output_transition(
        &mut self,
        physical_output_identity: Option<String>,
        reason: &str,
    ) {
        let Some(current_options) = self.session_options().cloned() else {
            return;
        };
        let mut staged_options = current_options.clone();
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
        let interaction_active = session_save::should_defer_for_interaction(self);
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

        let retry_at = Instant::now() + session_save::interaction_defer_interval();
        match start {
            OutputTransitionStart::IgnoreCurrentTarget => {
                if self
                    .session
                    .cancel_output_transition_for_live_source(input_dirty)
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
                self.notify_output_transition_deferred();
            }
            OutputTransitionStart::LoadInitial => {
                if let Err(err) = self.load_configured_session_for_options(
                    staged_options.clone(),
                    "initial output load",
                ) {
                    warn!("Failed to load initial output session: {err:#}");
                    self.session.stage_output_transition(
                        staged_options,
                        physical_output_identity,
                        output_transition_retry_at(self.output_transition_failure_backoff()),
                    );
                    self.notify_output_transition_deferred();
                }
            }
            OutputTransitionStart::ResolveTransition => {
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
        if session_save::should_defer_for_interaction(self) {
            self.session
                .defer_output_transition(now, session_save::interaction_defer_interval());
            log::debug!("Deferring pending output transition while interaction is active");
            return Ok(true);
        }

        let Some(pending) = self.session.take_pending_output_transition() else {
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
        let interaction_active = session_save::should_defer_for_interaction(self);
        if !live_source_reconciliation_ready(
            true,
            self.session.pending_output_transition().is_some(),
            interaction_active,
            self.persistence.is_healthy(),
        ) {
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
        if session_save::should_defer_for_interaction(self) {
            return Err(anyhow::anyhow!(
                "output transition became ineligible because an interaction started"
            ));
        }
        if let Some(pending) = self.session.pending_output_transition()
            && pending.source_epoch != self.session.target_epoch()
        {
            return Err(anyhow::anyhow!("stale output transition source epoch"));
        }
        session_save::persistence_barrier(self)?;
        let current_options = self
            .session_options()
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("output transition has no active session options"))?;
        self.persist_current_session_for_transition(&current_options, reason)?;

        let outcome = session_save::run_persistence_operation(
            self,
            PersistenceOperation::LoadConfigured {
                options: staged_options.clone(),
            },
        )?;
        let PersistenceOutcome::Load(load_outcome) = outcome else {
            return Err(anyhow::anyhow!("unexpected output-load worker outcome"));
        };
        let loaded_board_data = load_outcome.has_board_data();
        self.handle_session_load_outcome_for_options(load_outcome, &staged_options, "output load")?;
        self.session
            .commit_output_options(staged_options, loaded_board_data);
        info!(
            "Committed logical session output transition after {} (physical_output_identity={:?}, epoch={})",
            reason,
            physical_output_identity,
            self.session.target_epoch()
        );
        Ok(())
    }

    fn persist_current_session_for_transition(
        &mut self,
        options: &session::SessionOptions,
        reason: &str,
    ) -> anyhow::Result<()> {
        if self.should_skip_protected_session_save(options) {
            return Ok(());
        }
        let snapshot = self.input_state.snapshot_for_persistence(options);
        if self.should_skip_unloaded_contentless_session_save(options, snapshot.as_ref())? {
            return Ok(());
        }
        let snapshot = if let Some(snapshot) = snapshot {
            snapshot
        } else if Self::session_persistence_enabled(options) {
            SessionSnapshot {
                active_board_id: self.input_state.board_id().to_string(),
                boards: Vec::new(),
                tool_state: None,
            }
        } else {
            return Ok(());
        };
        let outcome = session_save::run_persistence_operation(
            self,
            PersistenceOperation::Save {
                snapshot,
                options: options.clone(),
                strategy: SaveStrategy::Normal,
                contentless_clear_boundary: self.session.has_loaded_board_data(),
            },
        )?;
        let PersistenceOutcome::Save(save) = outcome else {
            return Err(anyhow::anyhow!("unexpected output-save worker outcome"));
        };
        if !save.committed() {
            return Err(anyhow::anyhow!(
                "required session save before {reason} produced no committed write"
            ));
        }
        self.session
            .mark_saved(Instant::now(), save.committed_board_data);
        info!("Persisted active logical target before {reason}");
        Ok(())
    }
}
