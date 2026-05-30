use log::{debug, info, warn};
use smithay_client_toolkit::shell::{WaylandSurface, wlr_layer::Anchor};
use std::time::Instant;

use super::super::*;
use crate::{
    backend::wayland::session as runtime_session,
    input::state::{OutputFocusAction, UiToastKind},
    notification,
    session::{self, SessionSnapshot},
};

const OUTPUT_BADGE_MAX_LEN: usize = 28;

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

    pub(in crate::backend::wayland) fn persist_session_for_output(
        &mut self,
        output: Option<&wl_output::WlOutput>,
        reason: &str,
    ) {
        let output_identity =
            output.and_then(|surface_output| self.output_identity_for(surface_output));
        let Some(mut save_options) = self.session_options().cloned() else {
            return;
        };
        save_options.set_output_identity(output_identity.as_deref());

        if self.should_skip_protected_session_save(&save_options) {
            return;
        }

        let snapshot = session::snapshot_from_input(&self.input_state, &save_options);
        if self.should_skip_unloaded_contentless_session_save(&save_options, snapshot.as_ref()) {
            return;
        }

        let contentless_clear_boundary = self.session.has_loaded_board_data();
        let saved_board_data = snapshot
            .as_ref()
            .is_some_and(session::SessionSnapshot::has_board_data);
        let save_result = if let Some(snapshot) = snapshot {
            session::save_snapshot_with_report_and_clear_boundary(
                &snapshot,
                &save_options,
                contentless_clear_boundary,
            )
            .map(|report| report.is_some())
        } else if Self::session_persistence_enabled(&save_options) {
            let empty_snapshot = SessionSnapshot {
                active_board_id: self.input_state.board_id().to_string(),
                boards: Vec::new(),
                tool_state: None,
            };
            session::save_snapshot_with_report_and_clear_boundary(
                &empty_snapshot,
                &save_options,
                contentless_clear_boundary,
            )
            .map(|report| report.is_some())
        } else {
            Ok(false)
        };

        match save_result {
            Ok(saved) => {
                if !saved {
                    return;
                }
                if let Some(runtime_options) = self.session_options_mut() {
                    runtime_options.set_output_identity(output_identity.as_deref());
                }
                let _ = self.input_state.take_session_dirty();
                self.session.mark_saved(Instant::now(), saved_board_data);
                info!(
                    "Persisted session before {} (output_identity={:?})",
                    reason,
                    output_identity.as_deref()
                );
            }
            Err(err) => warn!(
                "Failed to persist session before {} (output_identity={:?}): {}",
                reason,
                output_identity.as_deref(),
                err
            ),
        }
    }

    pub(in crate::backend::wayland) fn handle_session_load_outcome(
        &mut self,
        outcome: session::LoadSnapshotOutcome,
        context: &str,
    ) {
        match outcome {
            session::LoadSnapshotOutcome::Loaded(snapshot) => {
                if let Some(options) = self.session_options().cloned() {
                    debug!(
                        "Restoring session {} from {}",
                        context,
                        options.session_file_path().display()
                    );
                    session::apply_snapshot(&mut self.input_state, *snapshot, &options);
                }
            }
            session::LoadSnapshotOutcome::LoadedFromBackup(snapshot) => {
                if let Some(options) = self.session_options().cloned() {
                    warn!(
                        "Restoring session {} from backup {} because the primary session had no board data",
                        context,
                        options.backup_file_path().display()
                    );
                    session::apply_snapshot(&mut self.input_state, *snapshot, &options);
                    self.input_state.set_ui_toast(
                        UiToastKind::Warning,
                        "Restored drawings from the session backup; the primary session had no board data.",
                    );
                }
            }
            session::LoadSnapshotOutcome::LoadedFromRecovery(snapshot) => {
                if let Some(options) = self.session_options().cloned() {
                    debug!(
                        "Restoring session {} from recovery artifact {}",
                        context,
                        options.recovery_file_path().display()
                    );
                    session::apply_snapshot(&mut self.input_state, *snapshot, &options);
                    self.input_state.set_ui_toast(
                        UiToastKind::Warning,
                        "Restored session from recovery file; normal save previously exceeded the size limit.",
                    );
                }
            }
            session::LoadSnapshotOutcome::Empty => {
                if let Some(options) = self.session_options() {
                    debug!(
                        "No session data found for {} ({})",
                        options.session_file_path().display(),
                        context
                    );
                }
            }
            session::LoadSnapshotOutcome::ExpandedTooLarge {
                path,
                max_expanded_size,
            } => {
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
        &self,
        options: &session::SessionOptions,
        snapshot: Option<&SessionSnapshot>,
    ) -> bool {
        let skip = runtime_session::should_skip_unloaded_contentless_save(
            self.session.has_loaded_board_data(),
            self.session.is_dirty(),
            self.input_state.is_session_dirty(),
            snapshot.is_some_and(SessionSnapshot::has_board_data),
            runtime_session::has_session_artifact(options),
        );
        if skip {
            info!(
                "Skipping session save to {} because no session was loaded, no session changes were recorded, and the current snapshot has no board data",
                options.session_file_path().display()
            );
        }
        skip
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
            self.input_state.set_ui_toast(
                UiToastKind::Info,
                "Multi-monitor focus is disabled (ui.multi_monitor_enabled=false)",
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
            self.input_state.set_ui_toast(
                UiToastKind::Info,
                "Cannot switch outputs while capture, frozen mode, or zoom mode is active",
            );
            self.input_state.trigger_blocked_feedback();
            return;
        }

        let outputs = self.sorted_known_outputs();
        if outputs.len() <= 1 {
            self.input_state
                .set_ui_toast(UiToastKind::Info, "Only one output is available");
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

        if self.has_seen_surface_enter() {
            self.persist_session_for_output(surface_current_output.as_ref(), "output switch");
        }

        if self.surface.is_xdg_window() {
            if !self.xdg_fullscreen() {
                self.input_state.set_ui_toast(
                    UiToastKind::Info,
                    "Enable ui.xdg_fullscreen to switch outputs on xdg fallback",
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
        self.buffer_damage.mark_all_full();
        self.set_toolbar_needs_recreate(true);
    }
}
