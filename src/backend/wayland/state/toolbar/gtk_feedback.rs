//! Applies drag-to-move feedback from the GTK toolbar frontend.
//!
//! The GTK bars move their own layer surfaces live during a drag; the
//! backend only mirrors the offsets so snapshots, clamping, and config
//! persistence keep working exactly like a built-in drag.

use super::super::WaylandState;

impl WaylandState {
    /// Base X the GTK top strip must use, matching the backend clamp and
    /// overlap-push math (`inline_top_base_x` is toolbar-module private).
    pub(in crate::backend::wayland) fn gtk_top_base_x(
        &self,
        snapshot: &crate::ui::toolbar::ToolbarSnapshot,
    ) -> f64 {
        self.inline_top_base_x(snapshot)
    }

    pub(in crate::backend::wayland) fn apply_gtk_top_offset(&mut self, x: f64, y: f64, done: bool) {
        self.data.toolbar_top_offset = x;
        self.data.toolbar_top_offset_y = y;
        self.finish_gtk_offset_change(done);
    }

    pub(in crate::backend::wayland) fn apply_gtk_side_offset(
        &mut self,
        x: f64,
        y: f64,
        done: bool,
    ) {
        self.data.toolbar_side_offset_x = x;
        self.data.toolbar_side_offset = y;
        self.finish_gtk_offset_change(done);
    }

    /// On drag end: clamp into the screen like the built-in drag path and
    /// persist the result. Intermediate positions are mirrored unclamped;
    /// the GTK side already keeps the bar on the monitor visually.
    fn finish_gtk_offset_change(&mut self, done: bool) {
        if !done {
            return;
        }
        let snapshot = self.toolbar_snapshot();
        self.clamp_toolbar_offsets(&snapshot);
        self.save_toolbar_pin_config();
        self.input_state.needs_redraw = true;
    }
}
