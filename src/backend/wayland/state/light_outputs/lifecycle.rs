use crate::i18n::tr;
use crate::input::state::{InputTextResources, Toast, ToastPriority};
use wayland_client::QueueHandle;

use super::super::WaylandState;

impl WaylandState {
    /// Applies a daemon toggle delivered to a light-capable overlay.
    ///
    /// From light passthrough, the editor moves to the compositor-chosen
    /// output and this output keeps its drawing. Any other state closes the
    /// overlay, as a plain daemon toggle would.
    pub(in crate::backend::wayland) fn toggle_light_output_editor(
        &mut self,
        qh: &QueueHandle<Self>,
    ) {
        if !self.light_output_overlays_enabled() || !self.input_state.light_mode_passthrough() {
            self.input_state.should_exit = true;
            return;
        }
        if self.suppression.suppressed()
            || self.session_transaction.is_some()
            || self.session.pending_output_transition().is_some()
            || self.capture.is_in_progress()
            || self.frozen.is_in_progress()
            || self.zoom.is_in_progress()
            || self.input_state.frozen_active()
            || self.input_state.zoom_active()
        {
            self.input_state.push_toast(
                ToastPriority::Info,
                "light.outputs",
                Toast::info(tr(
                    "Finish the current operation before moving the light-mode editor.",
                )),
            );
            return;
        }

        if !self.recreate_layer_surface_for_output(qh, None) {
            return;
        }

        self.input_state
            .toggle_light_mode_with_resources(InputTextResources {
                ui_engine: self.render.ui_text(),
                measurer: self.render.text_measurer(),
            });
        self.force_sync_overlay_interactivity();
        self.input_state.needs_redraw = true;
        self.sync_toolbar_visibility(qh);
    }
}
