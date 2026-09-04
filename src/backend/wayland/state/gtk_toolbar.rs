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
        || (input_state.region_is_engaged()
            && input_state
                .region_state()
                .purpose()
                .is_some_and(|purpose| purpose.is_capture()))
}

fn gtk_toolbar_top_visible(
    requested: bool,
    unmap_suppressed: bool,
    capture_picker_suppressed: bool,
) -> bool {
    requested && !unmap_suppressed && !capture_picker_suppressed
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
            layer_shell: self.protocol.layer_shell().is_some(),
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
            if self
                .toolbar_drag
                .gtk_note_feedback(gtk_toolbar_feedback_blocked(&self.input_state), &feedback)
            {
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
                        && self.toolbar_drag.gtk_preview_kind()
                            == Some(crate::toolbar_gtk::GtkToolbarKind::Top) =>
                    {
                        self.toolbar_drag.set_gtk_rebase(None);
                        let offset = self.toolbar_chrome.top_offset();
                        self.apply_gtk_top_offset(offset.0, offset.1, surface_size, phase);
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
                GtkToolbarFeedback::PointerShortcut {
                    button,
                    ctrl,
                    shift,
                    alt,
                    logo,
                } => {
                    if !self.input_state.modal_owns_pointer_shortcuts() {
                        self.try_dispatch_gdk_pointer_shortcut(button, ctrl, shift, alt, logo);
                    }
                }
                GtkToolbarFeedback::TopHover { hovered } => {
                    self.toolbar_chrome.set_gtk_top_hover(hovered);
                }
                GtkToolbarFeedback::SetTopOffset {
                    x,
                    y,
                    surface_size,
                    seq,
                    phase,
                } => {
                    super::drag_log(|| {
                        format!(
                            "gtk top receive seq={seq} phase={phase:?} offset=({x:.3},{y:.3}) surface={}x{}",
                            surface_size.width, surface_size.height,
                        )
                    });
                    self.toolbar_drag.note_gtk_offset_seq(seq);
                    self.apply_gtk_top_offset(x, y, surface_size, phase);
                }
            }
        }
        // Feedback committed before the terminal transition remains accepted
        // input. Apply the drained batch before dropping the failed bridge so
        // actions and final drag offsets are not lost during failover.
        if failed {
            self.cancel_overlay_capture_waiting_for_gtk();
            self.cancel_gtk_toolbar_drag_lifecycle();
            self.toolbar_chrome.set_gtk_top_hover(false);
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
        let capture_picker_suppressed = self.capture_picker_chrome_suppressed();
        let update = GtkToolbarUpdate {
            top_visible: gtk_toolbar_top_visible(
                self.input_state.toolbar_top_visible(),
                unmap_suppressed,
                capture_picker_suppressed,
            ),
            top_offset: self.toolbar_chrome.top_offset(),
            top_offset_seq: self.toolbar_drag.gtk_offset_seq(),
            top_base_x: self.gtk_top_base_x(),
            output_name: self
                .surface
                .current_output()
                .and_then(|output| self.protocol.output().info(&output))
                .and_then(|info| info.name),
            rebind_modifier: self.config.ui.toolbar.rebind_modifier,
            rebind_modifier_active: self.config.ui.toolbar.rebind_modifier.matches(
                self.input_state.modifiers.ctrl,
                self.input_state.modifiers.shift,
                self.input_state.modifiers.alt,
            ),
            modal_engaged: gtk_toolbar_feedback_blocked(&self.input_state),
            drag_preview: self.toolbar_drag.gtk_preview_kind(),
            capture_suppressed,
            capture_suppression_generation: self
                .data
                .overlay_capture_barrier
                .gtk_paint_generation(),
            snapshot,
        };
        if let Some(generation) = update.capture_suppression_generation {
            log::info!(
                "capture.preflight id={generation} component=backend phase=gtk-update-queued reason={:?} top_visible={} output={:?}",
                self.data.overlay_suppression,
                update.top_visible,
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
    fn capture_picker_blocks_feedback_but_ocr_does_not() {
        use crate::input::state::RegionPurposeTag;

        let mut input_state = make_test_input_state();
        input_state.activate_region(RegionPurposeTag::Ocr, 1);
        assert!(!gtk_toolbar_feedback_blocked(&input_state));

        input_state.cancel_region_ui_only();
        input_state.activate_region(RegionPurposeTag::CaptureDeliver, 2);
        assert!(gtk_toolbar_feedback_blocked(&input_state));
    }

    #[test]
    fn capture_picker_hides_gtk_toolbar_without_mutating_requested_visibility() {
        let requested = true;
        assert!(!gtk_toolbar_top_visible(requested, false, true));
        assert!(gtk_toolbar_top_visible(requested, false, false));
        assert!(!gtk_toolbar_top_visible(requested, true, false));
        assert!(requested, "the persisted/live request remains untouched");
    }
}
