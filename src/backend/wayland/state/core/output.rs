use crate::input::state::{Toast, ToastPriority};
use log::{debug, info, warn};
use smithay_client_toolkit::shell::{WaylandSurface, wlr_layer::Anchor};
use std::time::{Duration, Instant};

use super::super::*;
use crate::{
    backend::wayland::{
        backend::event_loop::session_save,
        session::{
            self as runtime_session, PersistenceOperation, PersistenceOutcome, SaveStrategy,
        },
    },
    input::state::OutputFocusAction,
    notification,
    session::{self, SessionSnapshot},
};

const OUTPUT_BADGE_MAX_LEN: usize = 28;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OutputTransitionStart {
    IgnoreCurrentTarget,
    KeepPending,
    DeferForInteraction,
    LoadInitial,
    ResolveTransition,
}

fn output_transition_start(
    loaded: bool,
    target_changed: bool,
    matching_pending: bool,
    same_epoch_pending: bool,
    live_source_resolution_pending: bool,
    interaction_active: bool,
) -> OutputTransitionStart {
    let superseding_pending_destination = same_epoch_pending && !matching_pending;
    if !target_changed
        && (loaded || superseding_pending_destination || live_source_resolution_pending)
    {
        OutputTransitionStart::IgnoreCurrentTarget
    } else if matching_pending {
        OutputTransitionStart::KeepPending
    } else if interaction_active {
        OutputTransitionStart::DeferForInteraction
    } else if loaded || same_epoch_pending {
        OutputTransitionStart::ResolveTransition
    } else {
        OutputTransitionStart::LoadInitial
    }
}

fn output_transition_retry_at(backoff: Duration) -> Instant {
    Instant::now() + backoff
}

fn live_source_reconciliation_ready(
    live_source_resolution_pending: bool,
    output_transition_pending: bool,
    interaction_active: bool,
    worker_healthy: bool,
) -> bool {
    live_source_resolution_pending
        && !output_transition_pending
        && !interaction_active
        && worker_healthy
}

fn replace_output_session_snapshot(
    input_state: &mut crate::input::InputState,
    snapshot: Option<SessionSnapshot>,
    options: &session::SessionOptions,
) -> anyhow::Result<()> {
    let snapshot = snapshot.unwrap_or_else(|| SessionSnapshot {
        active_board_id: input_state.board_id().to_string(),
        boards: Vec::new(),
        tool_state: None,
    });
    session::apply_snapshot_replacing_boards(input_state, snapshot, options)
}

impl WaylandState {
    pub(in crate::backend::wayland) fn preferred_fullscreen_output(
        &self,
    ) -> Option<wl_output::WlOutput> {
        if let Some(preferred) = self.preferred_output_identity()
            && let Some(output) = self.output_state.outputs().find(|output| {
                self.output_identity_for(output)
                    .map(|id| id.eq_ignore_ascii_case(preferred))
                    .unwrap_or(false)
            })
        {
            return Some(output);
        }

        self.surface
            .current_output()
            .or_else(|| self.output_state.outputs().next())
    }

    pub(in crate::backend::wayland) fn output_identity_for(
        &self,
        output: &wl_output::WlOutput,
    ) -> Option<String> {
        let info = self.output_state.info(output)?;

        let mut components: Vec<String> = Vec::new();

        if let Some(name) = info.name.filter(|s| !s.is_empty()) {
            components.push(name);
        }

        if !info.make.is_empty() {
            components.push(info.make);
        }

        if !info.model.is_empty() {
            components.push(info.model);
        }

        if components.is_empty() {
            components.push(format!("id{}", info.id));
        }

        Some(components.join("-"))
    }

    fn sorted_known_outputs(&self) -> Vec<wl_output::WlOutput> {
        let mut outputs: Vec<(u32, wl_output::WlOutput)> = self
            .output_state
            .outputs()
            .filter_map(|output| {
                self.output_state
                    .info(&output)
                    .map(|info| (info.id, output))
            })
            .collect();

        outputs.sort_by_key(|(id, _)| *id);
        outputs.into_iter().map(|(_, output)| output).collect()
    }

    fn output_badge_label_for(&self, output: &wl_output::WlOutput) -> Option<String> {
        let info = self.output_state.info(output)?;

        if let Some(name) = info.name.as_deref().filter(|name| !name.is_empty()) {
            return Some(crate::util::truncate_with_ellipsis(
                name,
                OUTPUT_BADGE_MAX_LEN,
            ));
        }

        let label = match (info.make.trim(), info.model.trim()) {
            ("", "") => format!("Output {}", info.id),
            (make, "") => make.to_string(),
            ("", model) => model.to_string(),
            (make, model) => format!("{make} {model}"),
        };

        Some(crate::util::truncate_with_ellipsis(
            &label,
            OUTPUT_BADGE_MAX_LEN,
        ))
    }

    pub(in crate::backend::wayland) fn refresh_active_output_label(&mut self) {
        let next_label = self
            .surface
            .current_output()
            .as_ref()
            .and_then(|output| self.output_badge_label_for(output))
            .or_else(|| {
                self.sorted_known_outputs()
                    .first()
                    .and_then(|output| self.output_badge_label_for(output))
            });

        if self.input_state.active_output_label != next_label {
            self.input_state.active_output_label = next_label;
            self.input_state.needs_redraw = true;
        }
    }

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

        let pending = self
            .session
            .take_pending_output_transition()
            .expect("pending transition checked above");
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
        let snapshot = session::snapshot_from_input(&self.input_state, options);
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

    pub(in crate::backend::wayland) fn load_configured_session_for_options(
        &mut self,
        options: session::SessionOptions,
        context: &str,
    ) -> anyhow::Result<()> {
        let outcome = session_save::run_persistence_operation(
            self,
            PersistenceOperation::LoadConfigured {
                options: options.clone(),
            },
        )?;
        let PersistenceOutcome::Load(load_outcome) = outcome else {
            return Err(anyhow::anyhow!("unexpected configured-load worker outcome"));
        };
        let loaded_board_data = load_outcome.has_board_data();
        self.handle_session_load_outcome_for_options(load_outcome, &options, context)?;
        self.session
            .commit_output_options(options, loaded_board_data);
        Ok(())
    }

    fn notify_output_transition_deferred(&mut self) {
        if !self.session.mark_output_transition_notified() {
            return;
        }
        self.input_state.push_toast(ToastPriority::Info, "output", Toast::warning("Session switch deferred until the active drawing is committed and the current session is saved."));
        self.input_state.needs_redraw = true;
    }

    fn output_transition_failure_backoff(&self) -> Duration {
        self.session_options()
            .map_or(Duration::from_secs(1), |options| {
                options.autosave_failure_backoff
            })
    }

    fn handle_session_load_outcome_for_options(
        &mut self,
        outcome: session::LoadSnapshotOutcome,
        options: &session::SessionOptions,
        context: &str,
    ) -> anyhow::Result<()> {
        match outcome {
            session::LoadSnapshotOutcome::Loaded(snapshot) => {
                debug!(
                    "Restoring session {} from {}",
                    context,
                    options.session_file_path().display()
                );
                replace_output_session_snapshot(&mut self.input_state, Some(*snapshot), options)?;
            }
            session::LoadSnapshotOutcome::LoadedFromBackup(snapshot) => {
                warn!(
                    "Restoring session {} from backup {} because the primary session had no board data",
                    context,
                    options.backup_file_path().display()
                );
                replace_output_session_snapshot(&mut self.input_state, Some(*snapshot), options)?;
                self.input_state.push_toast(ToastPriority::Info, "output", Toast::warning("Restored drawings from the session backup; the primary session had no board data."));
            }
            session::LoadSnapshotOutcome::LoadedFromRecovery(snapshot) => {
                debug!(
                    "Restoring session {} from recovery artifact {}",
                    context,
                    options.recovery_file_path().display()
                );
                replace_output_session_snapshot(&mut self.input_state, Some(*snapshot), options)?;
                self.input_state.push_toast(ToastPriority::Info, "output", Toast::warning("Restored session from recovery file; normal save previously exceeded the size limit."));
            }
            session::LoadSnapshotOutcome::Empty => {
                debug!(
                    "No session data found for {} ({})",
                    options.session_file_path().display(),
                    context
                );
                replace_output_session_snapshot(&mut self.input_state, None, options)?;
            }
            session::LoadSnapshotOutcome::NonRegularArtifact { path } => {
                debug!(
                    "Skipping non-regular session artifact {} for {}",
                    path.display(),
                    context
                );
                replace_output_session_snapshot(&mut self.input_state, None, options)?;
            }
            session::LoadSnapshotOutcome::ExpandedTooLarge {
                path,
                max_expanded_size,
            } => {
                replace_output_session_snapshot(&mut self.input_state, None, options)?;
                self.session.protect_session_path(path.clone());
                if self.session.mark_expanded_load_notified(&path) {
                    notification::send_notification_async(
                        &self.tokio_handle,
                        "Session Too Large to Restore".to_string(),
                        format!(
                            "The saved session was left unchanged because it expands beyond the {} MiB safety cap. Clear the session or move {} if it is no longer needed.",
                            max_expanded_size / 1024 / 1024,
                            path.display()
                        ),
                        Some("dialog-warning".to_string()),
                    );
                }
            }
        }
        self.mark_clean_after_session_load();
        Ok(())
    }

    fn mark_clean_after_session_load(&mut self) {
        self.input_state.clear_session_dirty();
        self.session.mark_clean_after_load();
    }

    fn should_skip_protected_session_save(&self, options: &session::SessionOptions) -> bool {
        let session_path = options.session_file_path();
        let skip = self.session.should_skip_save_for_protected_path(
            &session_path,
            self.input_state.is_session_dirty(),
        );
        if skip {
            info!(
                "Skipping session save to {} because a previous oversized compressed session was left protected and no session changes have been made",
                session_path.display()
            );
        }
        skip
    }

    fn should_skip_unloaded_contentless_session_save(
        &mut self,
        options: &session::SessionOptions,
        snapshot: Option<&SessionSnapshot>,
    ) -> anyhow::Result<bool> {
        let has_board_data = snapshot.is_some_and(SessionSnapshot::has_board_data);
        if has_board_data
            || self.session.has_loaded_board_data()
            || self.session.is_dirty()
            || self.input_state.is_session_dirty()
        {
            return Ok(false);
        }
        let outcome = session_save::run_persistence_operation(
            self,
            PersistenceOperation::HasArtifacts {
                options: options.clone(),
            },
        )?;
        let PersistenceOutcome::HasArtifacts(has_artifacts) = outcome else {
            return Err(anyhow::anyhow!("unexpected artifact-inspection outcome"));
        };
        let skip = runtime_session::should_skip_unloaded_contentless_save(
            self.session.has_loaded_board_data(),
            self.session.is_dirty(),
            self.input_state.is_session_dirty(),
            has_board_data,
            has_artifacts,
        );
        if skip {
            info!(
                "Skipping session save to {} because no session was loaded, no session changes were recorded, and the current snapshot has no board data",
                options.session_file_path().display()
            );
        }
        Ok(skip)
    }

    fn session_persistence_enabled(options: &session::SessionOptions) -> bool {
        options.any_enabled() || options.restore_tool_state || options.persist_history
    }

    pub(in crate::backend::wayland) fn handle_output_focus_action(
        &mut self,
        qh: &QueueHandle<Self>,
        action: OutputFocusAction,
    ) {
        if !self.config.ui.multi_monitor_enabled {
            self.input_state.push_toast(
                ToastPriority::Info,
                "output",
                Toast::info("Multi-monitor focus is disabled (ui.multi_monitor_enabled=false)"),
            );
            self.input_state.trigger_blocked_feedback();
            return;
        }
        if self.capture.is_in_progress()
            || self.frozen.is_in_progress()
            || self.zoom.is_in_progress()
            || self.input_state.frozen_active()
            || self.input_state.zoom_active()
        {
            self.input_state.push_toast(
                ToastPriority::Info,
                "output",
                Toast::info(
                    "Cannot switch outputs while capture, frozen mode, or zoom mode is active",
                ),
            );
            self.input_state.trigger_blocked_feedback();
            return;
        }

        let outputs = self.sorted_known_outputs();
        if outputs.len() <= 1 {
            self.input_state.push_toast(
                ToastPriority::Info,
                "output",
                Toast::info("Only one output is available"),
            );
            self.input_state.trigger_blocked_feedback();
            return;
        }

        let surface_current_output = self.surface.current_output();
        let current_output = surface_current_output
            .clone()
            .or_else(|| self.preferred_fullscreen_output());
        let current_index = current_output
            .as_ref()
            .and_then(|current| outputs.iter().position(|output| output == current))
            .unwrap_or(0);
        let target_index = match action {
            OutputFocusAction::Next => (current_index + 1) % outputs.len(),
            OutputFocusAction::Prev => {
                if current_index == 0 {
                    outputs.len() - 1
                } else {
                    current_index - 1
                }
            }
        };
        let target_output = outputs[target_index].clone();
        let target_label = self
            .output_badge_label_for(&target_output)
            .unwrap_or_else(|| format!("Output {}", target_index + 1));
        let target_identity = self.output_identity_for(&target_output);

        if self.surface.is_xdg_window() {
            if !self.xdg_fullscreen() {
                self.input_state.push_toast(
                    ToastPriority::Info,
                    "output",
                    Toast::info("Enable fullscreen mode before switching outputs in this session."),
                );
                self.input_state.trigger_blocked_feedback();
                return;
            }
            let Some(window) = self.surface.xdg_window().cloned() else {
                warn!("Output switch requested in xdg mode, but no xdg window is active");
                return;
            };
            info!("Switching xdg overlay to {}", target_label);
            window.set_fullscreen(Some(&target_output));
            window.commit();
            self.surface.set_current_output(target_output);
            self.set_has_seen_surface_enter(false);
            self.refresh_active_output_label();
            self.begin_session_output_transition(target_identity, "output switch");
            self.request_xdg_activation(qh);
            self.input_state.needs_redraw = true;
            return;
        }

        if self.layer_shell.is_none() {
            warn!("Output switch requested, but no supported shell is active");
            self.input_state.trigger_blocked_feedback();
            return;
        }

        info!("Switching layer overlay to {}", target_label);
        self.recreate_layer_surface_for_output(qh, &target_output);
        self.surface.set_current_output(target_output);
        self.set_has_seen_surface_enter(false);
        self.refresh_active_output_label();
        self.begin_session_output_transition(target_identity, "output switch");
        self.set_keyboard_focus(false);
        self.set_overlay_ready(false);
        self.input_state.needs_redraw = true;
        self.sync_toolbar_visibility(qh);
    }

    fn recreate_layer_surface_for_output(
        &mut self,
        qh: &QueueHandle<Self>,
        output: &wl_output::WlOutput,
    ) {
        let Some(layer_shell) = self.layer_shell.as_ref() else {
            return;
        };

        let wl_surface = self.compositor_state.create_surface(qh);
        wl_surface.set_buffer_scale(self.surface.scale().max(1));
        let layer = self.main_surface_layer();
        let layer_surface = layer_shell.create_layer_surface(
            qh,
            wl_surface,
            layer,
            Some("wayscriber"),
            Some(output),
        );

        layer_surface.set_anchor(Anchor::all());
        let desired_keyboard_mode = self.desired_keyboard_interactivity();
        layer_surface.set_keyboard_interactivity(desired_keyboard_mode);
        layer_surface.set_size(0, 0);
        layer_surface.set_exclusive_zone(-1);
        layer_surface.commit();

        self.surface.set_layer_surface(layer_surface);
        self.set_current_keyboard_interactivity(Some(desired_keyboard_mode));
        self.force_sync_overlay_interactivity();
        self.buffer_damage
            .mark_all_full(FullDamageReason::LayerSurfaceRecreated);
        self.set_toolbar_needs_recreate(true);
    }
}

#[cfg(test)]
mod tests {
    use super::{
        OutputTransitionStart, live_source_reconciliation_ready, output_transition_retry_at,
        output_transition_start, replace_output_session_snapshot,
    };
    use crate::{
        backend::wayland::session::SessionState,
        draw::{Color, Frame, Shape},
        input::state::test_support::make_test_input_state,
        session::{BoardPagesSnapshot, BoardSnapshot, SessionOptions, SessionSnapshot},
    };
    use std::path::PathBuf;
    use std::time::{Duration, Instant};

    fn add_test_line(input: &mut crate::input::InputState) {
        input.boards.active_frame_mut().add_shape(Shape::Line {
            x1: 0,
            y1: 0,
            x2: 20,
            y2: 20,
            color: Color {
                r: 1.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
            thick: 2.0,
        });
    }

    #[test]
    fn empty_output_load_replaces_source_board_contents() {
        let options = SessionOptions::new(PathBuf::from("/tmp"), "empty-output");
        let mut input = make_test_input_state();
        add_test_line(&mut input);
        assert_eq!(input.boards.active_frame().shapes.len(), 1);

        replace_output_session_snapshot(&mut input, None, &options)
            .expect("empty output replacement");

        assert!(input.boards.active_frame().shapes.is_empty());
    }

    #[test]
    fn partial_output_load_clears_boards_omitted_from_snapshot() {
        let options = SessionOptions::new(PathBuf::from("/tmp"), "partial-output");
        let mut input = make_test_input_state();
        input.switch_board_force("transparent");
        add_test_line(&mut input);
        assert_eq!(input.boards.active_frame().shapes.len(), 1);
        let snapshot = SessionSnapshot {
            active_board_id: "whiteboard".to_string(),
            boards: vec![BoardSnapshot {
                id: "whiteboard".to_string(),
                pages: BoardPagesSnapshot {
                    pages: vec![Frame::new()],
                    active: 0,
                },
            }],
            tool_state: None,
        };

        replace_output_session_snapshot(&mut input, Some(snapshot), &options)
            .expect("partial output replacement");

        input.switch_board_force("transparent");
        assert!(input.boards.active_frame().shapes.is_empty());
    }

    #[test]
    fn failed_output_replacement_preserves_source_board_contents() {
        let options = SessionOptions::new(PathBuf::from("/tmp"), "oversized-output");
        let mut input = make_test_input_state();
        add_test_line(&mut input);
        let boards = (0..=input.boards.max_count())
            .map(|index| BoardSnapshot {
                id: format!("replacement-{index}"),
                pages: BoardPagesSnapshot {
                    pages: vec![Frame::new()],
                    active: 0,
                },
            })
            .collect();
        let snapshot = SessionSnapshot {
            active_board_id: "replacement-0".to_string(),
            boards,
            tool_state: None,
        };

        let err = replace_output_session_snapshot(&mut input, Some(snapshot), &options)
            .expect_err("oversized replacement must fail before mutating live boards");

        assert!(err.to_string().contains("current runtime allows"));
        assert_eq!(input.boards.active_frame().shapes.len(), 1);
    }

    #[test]
    fn configure_retry_keeps_matching_epoch_bound_transition() {
        assert_eq!(
            output_transition_start(false, false, true, true, false, false),
            OutputTransitionStart::KeepPending
        );
    }

    #[test]
    fn initial_and_loaded_transitions_defer_for_active_interaction() {
        assert_eq!(
            output_transition_start(false, true, false, false, false, true),
            OutputTransitionStart::DeferForInteraction
        );
        assert_eq!(
            output_transition_start(true, true, false, false, false, true),
            OutputTransitionStart::DeferForInteraction
        );
    }

    #[test]
    fn transition_start_distinguishes_initial_load_and_loaded_switch() {
        assert_eq!(
            output_transition_start(false, true, false, false, false, false),
            OutputTransitionStart::LoadInitial
        );
        assert_eq!(
            output_transition_start(true, true, false, false, false, false),
            OutputTransitionStart::ResolveTransition
        );
    }

    #[test]
    fn unloaded_dirty_source_with_nonmatching_pending_transition_resolves_before_load() {
        let mut options = SessionOptions::new(PathBuf::from("/tmp"), "source-output");
        options.per_output = true;
        let mut first_target = options.clone();
        first_target.set_output_identity(Some("output-a"));
        let mut session = SessionState::new(Some(options.clone()));
        session.stage_output_transition(first_target, Some("output-a".to_string()), Instant::now());
        session.record_input_dirty(Instant::now(), true);

        let incoming_identity = Some("output-b".to_string());
        let mut incoming = options;
        let changed = incoming.set_output_identity(incoming_identity.as_deref());
        let same_epoch_pending = session
            .pending_output_transition()
            .is_some_and(|pending| pending.source_epoch == session.target_epoch());
        let matching_pending = same_epoch_pending
            && session
                .pending_output_transition()
                .is_some_and(|pending| pending.physical_output_identity == incoming_identity);

        assert!(!session.is_loaded());
        assert!(session.is_dirty());
        assert!(!matching_pending);
        assert_eq!(
            output_transition_start(
                session.is_loaded(),
                changed,
                matching_pending,
                same_epoch_pending,
                false,
                false,
            ),
            OutputTransitionStart::ResolveTransition
        );
        assert!(session.is_dirty());
    }

    #[test]
    fn unloaded_dirty_return_to_current_target_blocks_followup_configure_load() {
        let mut options = SessionOptions::new(PathBuf::from("/tmp"), "source-output");
        options.per_output = true;
        let mut destination = options.clone();
        destination.set_output_identity(Some("output-a"));
        let mut session = SessionState::new(Some(options.clone()));
        session.stage_output_transition(destination, Some("output-a".to_string()), Instant::now());
        session.record_input_dirty(Instant::now(), true);
        let source_epoch = session.target_epoch();
        let edit_generation = session.edit_generation();

        let incoming_identity = None;
        let mut incoming = options.clone();
        let changed = incoming.set_output_identity(incoming_identity);
        let same_epoch_pending = session
            .pending_output_transition()
            .is_some_and(|pending| pending.source_epoch == source_epoch);
        let matching_pending = same_epoch_pending
            && session
                .pending_output_transition()
                .is_some_and(|pending| pending.physical_output_identity.is_none());
        let start = output_transition_start(
            session.is_loaded(),
            changed,
            matching_pending,
            same_epoch_pending,
            false,
            false,
        );

        assert!(!session.is_loaded());
        assert!(!changed);
        assert!(!matching_pending);
        assert_eq!(start, OutputTransitionStart::IgnoreCurrentTarget);
        assert!(
            session
                .cancel_output_transition_for_live_source(false)
                .is_some()
        );
        assert!(session.pending_output_transition().is_none());
        assert!(session.is_loaded());
        assert!(session.is_dirty());
        assert!(session.prepare_autosave_submission().is_ok());
        assert_eq!(session.target_epoch(), source_epoch);
        assert_eq!(session.edit_generation(), edit_generation);

        let mut configure_options = options;
        let configure_changed = configure_options.set_output_identity(None);
        let followup = output_transition_start(
            session.is_loaded(),
            configure_changed,
            false,
            false,
            false,
            false,
        );

        assert!(!configure_changed);
        assert_eq!(followup, OutputTransitionStart::IgnoreCurrentTarget);
        assert_ne!(followup, OutputTransitionStart::LoadInitial);
        assert!(session.is_dirty());
        assert!(session.prepare_autosave_submission().is_ok());
        assert_eq!(session.target_epoch(), source_epoch);
        assert_eq!(session.edit_generation(), edit_generation);
    }

    #[test]
    fn active_stroke_return_resolves_to_dirty_live_source_or_clean_initial_load() {
        let mut options = SessionOptions::new(PathBuf::from("/tmp"), "source-output");
        options.per_output = true;
        let mut destination = options.clone();
        destination.set_output_identity(Some("output-a"));

        let mut committed = SessionState::new(Some(options.clone()));
        committed.stage_output_transition(
            destination.clone(),
            Some("output-a".to_string()),
            Instant::now(),
        );
        let committed_epoch = committed.target_epoch();
        assert_eq!(
            output_transition_start(false, false, false, true, false, true),
            OutputTransitionStart::IgnoreCurrentTarget
        );
        assert!(
            committed
                .cancel_output_transition_for_live_source(false)
                .is_some()
        );
        assert!(!committed.is_loaded());
        assert!(committed.resolve_live_source_resolution(false, true));
        assert_eq!(
            output_transition_start(false, false, false, false, true, true),
            OutputTransitionStart::IgnoreCurrentTarget
        );

        committed.record_input_dirty(Instant::now(), true);
        assert!(committed.is_loaded());
        assert!(committed.is_dirty());
        assert!(!committed.resolve_live_source_resolution(false, false));
        assert_eq!(committed.target_epoch(), committed_epoch);
        assert_eq!(
            output_transition_start(true, false, false, false, false, false),
            OutputTransitionStart::IgnoreCurrentTarget
        );

        let mut configure_first = SessionState::new(Some(options.clone()));
        configure_first.stage_output_transition(
            destination.clone(),
            Some("output-a".to_string()),
            Instant::now(),
        );
        assert!(
            configure_first
                .cancel_output_transition_for_live_source(false)
                .is_some()
        );
        assert!(!configure_first.resolve_live_source_resolution(true, false));
        assert!(configure_first.is_loaded());

        let mut canceled = SessionState::new(Some(options));
        canceled.stage_output_transition(destination, Some("output-a".to_string()), Instant::now());
        let canceled_epoch = canceled.target_epoch();
        assert!(
            canceled
                .cancel_output_transition_for_live_source(false)
                .is_some()
        );
        assert!(!canceled.resolve_live_source_resolution(false, false));
        assert!(!canceled.is_loaded());
        assert!(!canceled.is_dirty());
        assert_eq!(canceled.target_epoch(), canceled_epoch);
        assert_eq!(
            output_transition_start(false, false, false, false, false, false),
            OutputTransitionStart::LoadInitial
        );
    }

    #[test]
    fn live_source_reconciliation_runs_only_when_idle_without_a_destination() {
        assert!(live_source_reconciliation_ready(true, false, false, true));
        assert!(!live_source_reconciliation_ready(false, false, false, true));
        assert!(!live_source_reconciliation_ready(true, true, false, true));
        assert!(!live_source_reconciliation_ready(true, false, true, true));
        assert!(!live_source_reconciliation_ready(true, false, false, false));
    }

    #[test]
    fn clean_unloaded_cancellation_arms_immediate_source_resolution() {
        let options = SessionOptions::new(PathBuf::from("/tmp"), "source-output");
        let mut session = SessionState::new(Some(options.clone()));
        session.stage_output_transition(options, Some("output-a".to_string()), Instant::now());

        assert!(
            session
                .cancel_output_transition_for_live_source(false)
                .is_some()
        );
        assert!(session.has_pending_live_source_resolution());
        assert!(live_source_reconciliation_ready(true, false, false, true));
        assert!(!session.resolve_live_source_resolution(false, false));
        assert!(!session.is_loaded());
        assert!(!session.has_pending_live_source_resolution());
        assert_eq!(
            output_transition_start(false, false, false, false, false, false),
            OutputTransitionStart::LoadInitial
        );
    }

    #[test]
    fn loaded_source_cancellation_does_not_arm_provisional_resolution() {
        let mut session = SessionState::new(None);
        session.mark_loaded(false);
        session.stage_output_transition(
            SessionOptions::new(PathBuf::from("/tmp"), "loaded-destination"),
            Some("output-a".to_string()),
            Instant::now(),
        );

        assert!(
            session
                .cancel_output_transition_for_live_source(false)
                .is_some()
        );
        assert!(session.is_loaded());
        assert!(!session.has_pending_live_source_resolution());
    }

    #[test]
    fn failure_retry_deadline_is_based_on_failure_observation_time() {
        let backoff = Duration::from_millis(50);
        let before_failure_handling = Instant::now();
        let retry_at = output_transition_retry_at(backoff);

        assert!(retry_at >= before_failure_handling + backoff);
    }
}
