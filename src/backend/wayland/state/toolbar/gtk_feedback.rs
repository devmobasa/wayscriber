//! Applies drag-to-move feedback from the GTK toolbar frontend.
//!
//! The gesture-owning GTK surface stays parked and transparent during a drag.
//! The backend mirrors start-relative positions into an inline preview, then
//! moves the transparent GTK surface to the final clamped margin before it is
//! revealed. This avoids feeding surface movement back into GTK-local gesture
//! coordinates.

use super::super::{WaylandState, drag_log};
use crate::toolbar_gtk::{GtkToolbarDragPhase, GtkToolbarKind, GtkToolbarSurfaceSize};

fn clamp_gtk_surface_offset(
    offset: (f64, f64),
    viewport: (u32, u32),
    surface_size: GtkToolbarSurfaceSize,
    start_margin: (f64, f64),
    end_margin: (f64, f64),
) -> Option<(f64, f64)> {
    if !surface_size.is_configured() || viewport.0 == 0 || viewport.1 == 0 {
        return None;
    }
    Some((
        super::geometry::clamp_floating_axis_offset(
            offset.0,
            viewport.0 as f64,
            surface_size.width as f64,
            start_margin.0,
            end_margin.0,
        )
        .0,
        super::geometry::clamp_floating_axis_offset(
            offset.1,
            viewport.1 as f64,
            surface_size.height as f64,
            start_margin.1,
            end_margin.1,
        )
        .0,
    ))
}

impl WaylandState {
    /// Base X the GTK top strip must use, matching the backend clamp and
    /// overlap-push math (`inline_top_base_x` is toolbar-module private).
    pub(in crate::backend::wayland) fn gtk_top_base_x(
        &self,
        snapshot: &crate::ui::toolbar::ToolbarSnapshot,
    ) -> f64 {
        self.inline_top_base_x(snapshot)
    }

    pub(in crate::backend::wayland) fn apply_gtk_top_offset(
        &mut self,
        x: f64,
        y: f64,
        surface_size: GtkToolbarSurfaceSize,
        phase: GtkToolbarDragPhase,
    ) {
        if phase == GtkToolbarDragPhase::Start {
            self.begin_gtk_toolbar_drag_preview(GtkToolbarKind::Top);
        }
        self.data.toolbar_top_offset = x;
        self.data.toolbar_top_offset_y = y;
        self.mark_gtk_drag_preview_dirty();
        if phase.is_end() {
            self.clamp_gtk_top_offset(surface_size);
            self.finish_gtk_offset_change(crate::backend::wayland::state::MoveDragKind::Top);
        }
    }

    pub(in crate::backend::wayland) fn apply_gtk_side_offset(
        &mut self,
        x: f64,
        y: f64,
        surface_size: GtkToolbarSurfaceSize,
        phase: GtkToolbarDragPhase,
    ) {
        if phase == GtkToolbarDragPhase::Start {
            self.begin_gtk_toolbar_drag_preview(GtkToolbarKind::Side);
        }
        self.data.toolbar_side_offset_x = x;
        self.data.toolbar_side_offset = y;
        self.mark_gtk_drag_preview_dirty();
        if phase.is_end() {
            self.clamp_gtk_side_offset(surface_size);
            self.finish_gtk_offset_change(crate::backend::wayland::state::MoveDragKind::Side);
        }
    }

    fn mark_gtk_drag_preview_dirty(&mut self) {
        if self.data.gtk_drag_preview.is_none() {
            return;
        }
        self.toolbar.mark_dirty();
        self.input_state.dirty_tracker.mark_full();
        self.input_state.needs_redraw = true;
    }

    fn clamp_gtk_top_offset(&mut self, surface_size: GtkToolbarSurfaceSize) {
        let snapshot = self.toolbar_snapshot();
        let base_x = self.inline_top_base_x(&snapshot);
        let before = (self.data.toolbar_top_offset, self.data.toolbar_top_offset_y);
        let Some((x, y)) = clamp_gtk_surface_offset(
            (self.data.toolbar_top_offset, self.data.toolbar_top_offset_y),
            (self.surface.width(), self.surface.height()),
            surface_size,
            (base_x, Self::TOP_BASE_MARGIN_TOP),
            (Self::TOP_MARGIN_RIGHT, Self::TOP_MARGIN_BOTTOM),
        ) else {
            self.clamp_toolbar_offsets(&snapshot);
            return;
        };
        self.data.toolbar_top_offset = x;
        self.data.toolbar_top_offset_y = y;
        drag_log(format!(
            "gtk top final clamp before=({:.3},{:.3}) after=({x:.3},{y:.3}) viewport={}x{} surface={}x{} base=({base_x:.3},{:.3}) end=({:.3},{:.3})",
            before.0,
            before.1,
            self.surface.width(),
            self.surface.height(),
            surface_size.width,
            surface_size.height,
            Self::TOP_BASE_MARGIN_TOP,
            Self::TOP_MARGIN_RIGHT,
            Self::TOP_MARGIN_BOTTOM,
        ));
    }

    fn clamp_gtk_side_offset(&mut self, surface_size: GtkToolbarSurfaceSize) {
        let before = (
            self.data.toolbar_side_offset_x,
            self.data.toolbar_side_offset,
        );
        let Some((x, y)) = clamp_gtk_surface_offset(
            (
                self.data.toolbar_side_offset_x,
                self.data.toolbar_side_offset,
            ),
            (self.surface.width(), self.surface.height()),
            surface_size,
            (Self::SIDE_BASE_MARGIN_LEFT, Self::SIDE_BASE_MARGIN_TOP),
            (Self::SIDE_MARGIN_RIGHT, Self::SIDE_MARGIN_BOTTOM),
        ) else {
            let snapshot = self.toolbar_snapshot();
            self.clamp_toolbar_offsets(&snapshot);
            return;
        };
        self.data.toolbar_side_offset_x = x;
        self.data.toolbar_side_offset = y;
        drag_log(format!(
            "gtk side final clamp before=({:.3},{:.3}) after=({x:.3},{y:.3}) viewport={}x{} surface={}x{} base=({:.3},{:.3}) end=({:.3},{:.3})",
            before.0,
            before.1,
            self.surface.width(),
            self.surface.height(),
            surface_size.width,
            surface_size.height,
            Self::SIDE_BASE_MARGIN_LEFT,
            Self::SIDE_BASE_MARGIN_TOP,
            Self::SIDE_MARGIN_RIGHT,
            Self::SIDE_MARGIN_BOTTOM,
        ));
    }

    /// On drag end, persist the offset accepted against GTK's measured
    /// surface. Intermediate positions are mirrored without disk writes.
    fn finish_gtk_offset_change(&mut self, kind: crate::backend::wayland::state::MoveDragKind) {
        self.reconcile_top_base_after_drag();
        self.data.drag_top_base_x = None;
        self.save_toolbar_position_config(kind);
        self.begin_gtk_toolbar_drag_handoff();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn final_offset_is_clamped_against_the_measured_gtk_surface() {
        let clamped = clamp_gtk_surface_offset(
            (379.25, 600.0),
            (2304, 1296),
            GtkToolbarSurfaceSize {
                width: 260,
                height: 789,
            },
            (24.0, 24.0),
            (0.0, 24.0),
        );

        assert_eq!(clamped, Some((379.25, 459.0)));
    }

    #[test]
    fn final_offset_preserves_fractional_coordinates_inside_the_bounds() {
        let clamped = clamp_gtk_surface_offset(
            (379.25, 458.75),
            (2304, 1296),
            GtkToolbarSurfaceSize {
                width: 260,
                height: 789,
            },
            (24.0, 24.0),
            (0.0, 24.0),
        );

        assert_eq!(clamped, Some((379.25, 458.75)));
    }

    #[test]
    fn missing_surface_measurement_requests_the_modeled_fallback() {
        assert_eq!(
            clamp_gtk_surface_offset(
                (10.0, 20.0),
                (2304, 1296),
                GtkToolbarSurfaceSize {
                    width: 0,
                    height: 0,
                },
                (24.0, 24.0),
                (0.0, 24.0),
            ),
            None
        );
    }
}
