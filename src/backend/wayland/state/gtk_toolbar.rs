//! Backend-side glue for the GTK toolbar frontend: spawn decision,
//! feedback draining, and state pushes. See `crate::toolbar_gtk` for the
//! threading model.

use wayland_client::{Connection, QueueHandle};

use super::WaylandState;
use crate::toolbar_gtk::select::{
    GtkPreconditions, ToolbarFrontend, requested_backend, resolve_frontend,
};
use crate::toolbar_gtk::{GtkToolbarBridge, GtkToolbarFeedback, GtkToolbarUpdate};

fn gtk_toolbar_feedback_blocked(input_state: &crate::input::InputState) -> bool {
    input_state.command_palette_is_engaged()
}

fn acknowledge_blocked_gtk_drag_feedback(
    top_seq: &mut u64,
    side_seq: &mut u64,
    feedback: &GtkToolbarFeedback,
) {
    match feedback {
        GtkToolbarFeedback::SetTopOffset { seq, .. } => {
            *top_seq = (*top_seq).max(*seq);
        }
        GtkToolbarFeedback::SetSideOffset { seq, .. } => {
            *side_seq = (*side_seq).max(*seq);
        }
        GtkToolbarFeedback::Event { .. }
        | GtkToolbarFeedback::CaptureSuppressionReady { .. }
        | GtkToolbarFeedback::CaptureSuppressionFailed { .. } => {}
    }
}

fn gtk_toolbar_feedback_is_blocked(
    modal_engaged: bool,
    top_drag_blocked: &mut bool,
    side_drag_blocked: &mut bool,
    feedback: &GtkToolbarFeedback,
) -> bool {
    match feedback {
        GtkToolbarFeedback::CaptureSuppressionReady { .. }
        | GtkToolbarFeedback::CaptureSuppressionFailed { .. } => false,
        GtkToolbarFeedback::Event { .. } => modal_engaged,
        GtkToolbarFeedback::SetTopOffset { phase, .. } => {
            let blocked = modal_engaged || *top_drag_blocked;
            if blocked {
                *top_drag_blocked = !phase.is_end();
            }
            blocked
        }
        GtkToolbarFeedback::SetSideOffset { phase, .. } => {
            let blocked = modal_engaged || *side_drag_blocked;
            if blocked {
                *side_drag_blocked = !phase.is_end();
            }
            blocked
        }
    }
}

impl WaylandState {
    /// True while the GTK frontend owns the toolbars (built-in bars stay
    /// unmapped).
    pub(in crate::backend::wayland) fn gtk_toolbars_active(&self) -> bool {
        self.gtk_toolbar.is_some()
    }

    /// Spawns the GTK toolbar thread when the resolved frontend is GTK.
    pub(in crate::backend::wayland) fn spawn_gtk_toolbar_if_selected(
        &mut self,
        runtime_wake: crate::backend::wayland::RuntimeWakeHandle,
    ) {
        let request = requested_backend(&self.config);
        let preconditions = GtkPreconditions {
            feature_compiled: cfg!(feature = "toolbar-gtk"),
            layer_shell: self.layer_shell.is_some(),
            force_inline: super::force_inline_toolbars_requested(&self.config),
            main_surface_uses_overlay_layer: self.data.main_surface_uses_overlay_layer,
        };
        match resolve_frontend(request, preconditions) {
            ToolbarFrontend::Gtk => {
                self.gtk_toolbar = GtkToolbarBridge::spawn(runtime_wake);
                if self.gtk_toolbar.is_some() {
                    log::info!("GTK toolbars enabled; built-in toolbar surfaces stay unmapped");
                } else {
                    log::warn!("GTK toolbar thread failed to start; using built-in toolbars");
                }
            }
            ToolbarFrontend::Builtin(blocker) => {
                if let Some(reason) = blocker {
                    log::warn!(
                        "GTK toolbars requested but unavailable ({}); using built-in toolbars",
                        reason.describe()
                    );
                }
            }
        }
    }

    /// Drains pending GTK toolbar feedback into the shared toolbar-event
    /// path, and falls back to the built-in bars if the GTK thread died.
    pub(in crate::backend::wayland) fn process_gtk_toolbar(
        &mut self,
        conn: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        let (pending, failed) = {
            let Some(bridge) = self.gtk_toolbar.as_ref() else {
                return;
            };
            bridge.drain_feedback()
        };
        for feedback in pending {
            // GTK uses a separate connection and bypasses the built-in
            // pointer modal gate. A drag first observed under the modal stays
            // blocked through its matching drag end even if Escape closes the
            // modal first. Acknowledge rejected sequences so the authoritative
            // backend offsets pushed later in this pass snap GTK back and do
            // not become stale.
            if gtk_toolbar_feedback_is_blocked(
                gtk_toolbar_feedback_blocked(&self.input_state),
                &mut self.data.gtk_top_drag_blocked,
                &mut self.data.gtk_side_drag_blocked,
                &feedback,
            ) {
                acknowledge_blocked_gtk_drag_feedback(
                    &mut self.data.gtk_top_offset_seq,
                    &mut self.data.gtk_side_offset_seq,
                    &feedback,
                );
                // If a modal opened after an accepted drag start, the blocked
                // end still has to close the preview lifecycle. Keep the last
                // accepted position rather than applying motion produced while
                // the modal owned input.
                match feedback {
                    GtkToolbarFeedback::SetTopOffset {
                        surface_size,
                        phase,
                        ..
                    } if phase.is_end()
                        && self.data.gtk_drag_preview
                            == Some(crate::toolbar_gtk::GtkToolbarKind::Top) =>
                    {
                        self.apply_gtk_top_offset(
                            self.data.toolbar_top_offset,
                            self.data.toolbar_top_offset_y,
                            surface_size,
                            phase,
                        );
                    }
                    GtkToolbarFeedback::SetSideOffset {
                        surface_size,
                        phase,
                        ..
                    } if phase.is_end()
                        && self.data.gtk_drag_preview
                            == Some(crate::toolbar_gtk::GtkToolbarKind::Side) =>
                    {
                        self.apply_gtk_side_offset(
                            self.data.toolbar_side_offset_x,
                            self.data.toolbar_side_offset,
                            surface_size,
                            phase,
                        );
                    }
                    _ => {}
                }
                continue;
            }
            match feedback {
                GtkToolbarFeedback::CaptureSuppressionReady { generation } => {
                    self.acknowledge_gtk_capture_suppression(generation);
                }
                GtkToolbarFeedback::CaptureSuppressionFailed { generation, error } => {
                    self.reject_gtk_capture_suppression(generation, &error);
                }
                GtkToolbarFeedback::Event {
                    event,
                    rebind_requested,
                } => {
                    self.handle_toolbar_event_with_rebind(
                        event,
                        rebind_requested,
                        Some(conn),
                        Some(qh),
                    );
                }
                GtkToolbarFeedback::SetTopOffset {
                    x,
                    y,
                    surface_size,
                    seq,
                    phase,
                } => {
                    super::drag_log(format!(
                        "gtk top receive seq={seq} phase={phase:?} offset=({x:.3},{y:.3}) surface={}x{}",
                        surface_size.width, surface_size.height,
                    ));
                    self.data.gtk_top_offset_seq = seq;
                    self.apply_gtk_top_offset(x, y, surface_size, phase);
                }
                GtkToolbarFeedback::SetSideOffset {
                    x,
                    y,
                    surface_size,
                    seq,
                    phase,
                } => {
                    super::drag_log(format!(
                        "gtk side receive seq={seq} phase={phase:?} offset=({x:.3},{y:.3}) surface={}x{}",
                        surface_size.width, surface_size.height,
                    ));
                    self.data.gtk_side_offset_seq = seq;
                    self.apply_gtk_side_offset(x, y, surface_size, phase);
                }
            }
        }
        // Feedback committed before the terminal transition remains accepted
        // input. Apply the drained batch before dropping the failed bridge so
        // actions and final drag offsets are not lost during failover.
        if failed {
            self.cancel_overlay_capture_waiting_for_gtk();
            self.cancel_gtk_toolbar_drag_lifecycle();
            self.gtk_toolbar = None;
        }
    }

    /// Pushes the current toolbar state to the GTK thread; the bridge
    /// deduplicates unchanged updates.
    pub(in crate::backend::wayland) fn push_gtk_toolbar_update(&mut self) {
        if self.gtk_toolbar.is_none() {
            return;
        }
        let snapshot = self.toolbar_snapshot();
        // Capture suppression keeps normally visible layer surfaces mapped
        // but transparent, avoiding compositor-owned close-animation
        // snapshots. Other suppression and light passthrough still unmap.
        let capture_suppressed = self.data.overlay_suppression.requires_capture_barrier();
        let unmap_suppressed = self.overlay_passthrough_requested() && !capture_suppressed;
        let update = GtkToolbarUpdate {
            top_visible: self.input_state.toolbar_top_visible() && !unmap_suppressed,
            side_visible: self.input_state.toolbar_side_visible() && !unmap_suppressed,
            top_offset: (self.data.toolbar_top_offset, self.data.toolbar_top_offset_y),
            side_offset: (
                self.data.toolbar_side_offset_x,
                self.data.toolbar_side_offset,
            ),
            top_offset_seq: self.data.gtk_top_offset_seq,
            side_offset_seq: self.data.gtk_side_offset_seq,
            top_base_x: self.gtk_top_base_x(&snapshot),
            output_name: self
                .surface
                .current_output()
                .and_then(|output| self.output_state.info(&output))
                .and_then(|info| info.name),
            rebind_modifier: self.config.ui.toolbar.rebind_modifier,
            rebind_modifier_active: self.config.ui.toolbar.rebind_modifier.matches(
                self.input_state.modifiers.ctrl,
                self.input_state.modifiers.shift,
                self.input_state.modifiers.alt,
            ),
            modal_engaged: gtk_toolbar_feedback_blocked(&self.input_state),
            drag_preview: self.data.gtk_drag_preview,
            capture_suppressed,
            capture_suppression_generation: self
                .data
                .overlay_capture_barrier
                .gtk_paint_generation(),
            snapshot,
        };
        if let Some(generation) = update.capture_suppression_generation {
            log::info!(
                "capture.preflight id={generation} component=backend phase=gtk-update-queued reason={:?} top_visible={} side_visible={} output={:?}",
                self.data.overlay_suppression,
                update.top_visible,
                update.side_visible,
                update.output_name
            );
        }
        if let Some(bridge) = self.gtk_toolbar.as_mut() {
            bridge.maybe_send(update);
        }
    }
}

#[cfg(test)]
mod modal_tests {
    use super::*;
    use crate::config::Action;
    use crate::input::state::test_support::make_test_input_state;
    use crate::toolbar_gtk::GtkToolbarDragPhase;

    const TEST_SURFACE_SIZE: crate::toolbar_gtk::GtkToolbarSurfaceSize =
        crate::toolbar_gtk::GtkToolbarSurfaceSize {
            width: 260,
            height: 789,
        };

    #[test]
    fn command_palette_and_shortcut_capture_block_all_gtk_feedback() {
        let mut input_state = make_test_input_state();
        assert!(!gtk_toolbar_feedback_blocked(&input_state));

        input_state.toggle_command_palette();
        assert!(gtk_toolbar_feedback_blocked(&input_state));

        input_state.toggle_command_palette();
        assert!(input_state.begin_keybinding_capture(Action::Undo));
        assert!(gtk_toolbar_feedback_blocked(&input_state));
    }

    #[test]
    fn blocked_drag_feedback_advances_only_the_originating_sequence() {
        let mut top_seq = 4;
        let mut side_seq = 7;

        acknowledge_blocked_gtk_drag_feedback(
            &mut top_seq,
            &mut side_seq,
            &GtkToolbarFeedback::SetTopOffset {
                x: 100.0,
                y: 50.0,
                surface_size: TEST_SURFACE_SIZE,
                seq: 9,
                phase: GtkToolbarDragPhase::End,
            },
        );
        assert_eq!((top_seq, side_seq), (9, 7));

        acknowledge_blocked_gtk_drag_feedback(
            &mut top_seq,
            &mut side_seq,
            &GtkToolbarFeedback::SetSideOffset {
                x: 25.0,
                y: 75.0,
                surface_size: TEST_SURFACE_SIZE,
                seq: 11,
                phase: GtkToolbarDragPhase::Move,
            },
        );
        assert_eq!((top_seq, side_seq), (9, 11));
    }

    #[test]
    fn blocked_drag_feedback_never_regresses_a_sequence() {
        let mut top_seq = 9;
        let mut side_seq = 11;

        acknowledge_blocked_gtk_drag_feedback(
            &mut top_seq,
            &mut side_seq,
            &GtkToolbarFeedback::SetTopOffset {
                x: 0.0,
                y: 0.0,
                surface_size: TEST_SURFACE_SIZE,
                seq: 8,
                phase: GtkToolbarDragPhase::Move,
            },
        );
        assert_eq!((top_seq, side_seq), (9, 11));
    }

    #[test]
    fn drag_started_under_modal_stays_blocked_until_done() {
        let mut top_blocked = false;
        let mut side_blocked = false;
        let top_update = |phase| GtkToolbarFeedback::SetTopOffset {
            x: 10.0,
            y: 20.0,
            surface_size: TEST_SURFACE_SIZE,
            seq: 1,
            phase,
        };

        assert!(gtk_toolbar_feedback_is_blocked(
            true,
            &mut top_blocked,
            &mut side_blocked,
            &top_update(GtkToolbarDragPhase::Start),
        ));
        assert!(top_blocked);

        assert!(gtk_toolbar_feedback_is_blocked(
            false,
            &mut top_blocked,
            &mut side_blocked,
            &top_update(GtkToolbarDragPhase::Move),
        ));
        assert!(top_blocked);

        assert!(gtk_toolbar_feedback_is_blocked(
            false,
            &mut top_blocked,
            &mut side_blocked,
            &top_update(GtkToolbarDragPhase::End),
        ));
        assert!(!top_blocked);

        assert!(!gtk_toolbar_feedback_is_blocked(
            false,
            &mut top_blocked,
            &mut side_blocked,
            &top_update(GtkToolbarDragPhase::Start),
        ));
    }

    #[test]
    fn blocked_drag_latches_are_independent_per_bar() {
        let mut top_blocked = false;
        let mut side_blocked = false;
        let top = GtkToolbarFeedback::SetTopOffset {
            x: 0.0,
            y: 0.0,
            surface_size: TEST_SURFACE_SIZE,
            seq: 1,
            phase: GtkToolbarDragPhase::Start,
        };
        let side = GtkToolbarFeedback::SetSideOffset {
            x: 0.0,
            y: 0.0,
            surface_size: TEST_SURFACE_SIZE,
            seq: 1,
            phase: GtkToolbarDragPhase::Start,
        };

        assert!(gtk_toolbar_feedback_is_blocked(
            true,
            &mut top_blocked,
            &mut side_blocked,
            &top,
        ));
        assert!(!gtk_toolbar_feedback_is_blocked(
            false,
            &mut top_blocked,
            &mut side_blocked,
            &side,
        ));
        assert_eq!((top_blocked, side_blocked), (true, false));
    }

    #[test]
    fn capture_suppression_ack_bypasses_modal_feedback_blocking() {
        let mut top_blocked = true;
        let mut side_blocked = false;

        assert!(!gtk_toolbar_feedback_is_blocked(
            true,
            &mut top_blocked,
            &mut side_blocked,
            &GtkToolbarFeedback::CaptureSuppressionReady { generation: 7 },
        ));
        assert_eq!((top_blocked, side_blocked), (true, false));
    }
}
