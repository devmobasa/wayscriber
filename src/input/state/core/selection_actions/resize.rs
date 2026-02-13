//! Selection resize functionality.

use crate::draw::frame::ShapeSnapshot;
use crate::draw::{Shape, ShapeId};
use crate::input::InputState;
use crate::input::state::core::base::SelectionHandle;
use crate::util::Rect;
mod resize_helpers;

// Handle size for hit testing (matches render constants)
const HANDLE_SIZE: i32 = 8;
const HANDLE_TOLERANCE: i32 = 4;

impl InputState {
    /// Hit test for selection handles. Returns the handle if mouse is over one.
    pub fn hit_selection_handle(&self, x: i32, y: i32) -> Option<SelectionHandle> {
        let bounds = self.selection_bounds()?;
        let corner_radius = (HANDLE_SIZE / 2) + HANDLE_TOLERANCE;
        let edge_radius = (HANDLE_SIZE * 3 / 4) / 2 + HANDLE_TOLERANCE;

        Self::selection_handle_probes(&bounds, corner_radius, edge_radius)
            .into_iter()
            .find_map(|probe| {
                self.point_near(x, y, probe.x, probe.y, probe.radius)
                    .then_some(probe.handle)
            })
    }

    /// Capture snapshots of selected shapes for resize operation.
    pub(crate) fn capture_resize_selection_snapshots(&self) -> Vec<(ShapeId, ShapeSnapshot)> {
        let ids = self.selected_shape_ids();
        let frame = self.boards.active_frame();
        let mut snapshots = Vec::with_capacity(ids.len());
        for id in ids {
            if let Some(shape) = frame.shape(*id)
                && !shape.locked
            {
                snapshots.push((
                    *id,
                    ShapeSnapshot {
                        shape: shape.shape.clone(),
                        locked: shape.locked,
                    },
                ));
            }
        }
        snapshots
    }

    /// Apply resize transformation to all selected shapes.
    pub(crate) fn apply_selection_resize(
        &mut self,
        handle: SelectionHandle,
        original_bounds: &Rect,
        dx: i32,
        dy: i32,
        snapshots: &[(ShapeId, ShapeSnapshot)],
    ) {
        if snapshots.is_empty() || (dx == 0 && dy == 0) {
            return;
        }

        self.mark_selection_dirty_region(Some(*original_bounds));
        // Calculate scale factors based on handle and delta
        let (scale_x, scale_y, anchor_x, anchor_y) =
            Self::compute_scale_factors(handle, original_bounds, dx, dy);

        // Collect IDs to invalidate after the loop
        let mut ids_to_invalidate = Vec::with_capacity(snapshots.len());

        {
            let frame = self.boards.active_frame_mut();
            for (shape_id, snapshot) in snapshots {
                if let Some(drawn) = frame.shape_mut(*shape_id) {
                    // Apply scaling transformation to the shape
                    drawn.shape = Self::scale_shape(
                        &snapshot.shape,
                        scale_x,
                        scale_y,
                        anchor_x,
                        anchor_y,
                        original_bounds,
                    );
                    ids_to_invalidate.push(*shape_id);
                }
            }
        }

        for shape_id in ids_to_invalidate {
            self.invalidate_hit_cache_for(shape_id);
        }
        self.mark_selection_dirty_region(self.selection_bounds());
    }

    fn scale_shape(
        original: &Shape,
        scale_x: f64,
        scale_y: f64,
        anchor_x: f64,
        anchor_y: f64,
        _original_bounds: &Rect,
    ) -> Shape {
        match original {
            Shape::Rect {
                x,
                y,
                w,
                h,
                fill,
                color,
                thick,
            } => {
                let (nx, ny) = Self::scale_point_i32(*x, *y, anchor_x, anchor_y, scale_x, scale_y);
                let nw = Self::scale_size(*w, scale_x);
                let nh = Self::scale_size(*h, scale_y);
                Shape::Rect {
                    x: nx,
                    y: ny,
                    w: nw.max(1),
                    h: nh.max(1),
                    fill: *fill,
                    color: *color,
                    thick: *thick,
                }
            }
            Shape::Ellipse {
                cx,
                cy,
                rx,
                ry,
                fill,
                color,
                thick,
            } => {
                let (ncx, ncy) =
                    Self::scale_point_i32(*cx, *cy, anchor_x, anchor_y, scale_x, scale_y);
                let nrx = Self::scale_size(*rx, scale_x);
                let nry = Self::scale_size(*ry, scale_y);
                Shape::Ellipse {
                    cx: ncx,
                    cy: ncy,
                    rx: nrx.max(1),
                    ry: nry.max(1),
                    fill: *fill,
                    color: *color,
                    thick: *thick,
                }
            }
            Shape::Line {
                x1,
                y1,
                x2,
                y2,
                color,
                thick,
            } => {
                let (nx1, ny1) =
                    Self::scale_point_i32(*x1, *y1, anchor_x, anchor_y, scale_x, scale_y);
                let (nx2, ny2) =
                    Self::scale_point_i32(*x2, *y2, anchor_x, anchor_y, scale_x, scale_y);
                Shape::Line {
                    x1: nx1,
                    y1: ny1,
                    x2: nx2,
                    y2: ny2,
                    color: *color,
                    thick: *thick,
                }
            }
            Shape::Arrow {
                x1,
                y1,
                x2,
                y2,
                color,
                thick,
                arrow_length,
                arrow_angle,
                head_at_end,
                label,
            } => {
                let (nx1, ny1) =
                    Self::scale_point_i32(*x1, *y1, anchor_x, anchor_y, scale_x, scale_y);
                let (nx2, ny2) =
                    Self::scale_point_i32(*x2, *y2, anchor_x, anchor_y, scale_x, scale_y);
                Shape::Arrow {
                    x1: nx1,
                    y1: ny1,
                    x2: nx2,
                    y2: ny2,
                    color: *color,
                    thick: *thick,
                    arrow_length: *arrow_length,
                    arrow_angle: *arrow_angle,
                    head_at_end: *head_at_end,
                    label: label.clone(),
                }
            }
            Shape::Freehand {
                points,
                color,
                thick,
            } => {
                let scaled_points =
                    Self::scale_points(points, anchor_x, anchor_y, scale_x, scale_y);
                Shape::Freehand {
                    points: scaled_points,
                    color: *color,
                    thick: *thick,
                }
            }
            Shape::FreehandPressure { points, color } => {
                let scaled_points =
                    Self::scale_points_with_pressure(points, anchor_x, anchor_y, scale_x, scale_y);
                Shape::FreehandPressure {
                    points: scaled_points,
                    color: *color,
                }
            }
            Shape::MarkerStroke {
                points,
                color,
                thick,
            } => {
                let scaled_points =
                    Self::scale_points(points, anchor_x, anchor_y, scale_x, scale_y);
                Shape::MarkerStroke {
                    points: scaled_points,
                    color: *color,
                    thick: *thick,
                }
            }
            Shape::StepMarker { x, y, color, label } => {
                let (nx, ny) = Self::scale_point_i32(*x, *y, anchor_x, anchor_y, scale_x, scale_y);
                Shape::StepMarker {
                    x: nx,
                    y: ny,
                    color: *color,
                    label: label.clone(),
                }
            }
            Shape::EraserStroke { points, brush } => {
                let scaled_points =
                    Self::scale_points(points, anchor_x, anchor_y, scale_x, scale_y);
                Shape::EraserStroke {
                    points: scaled_points,
                    brush: brush.clone(),
                }
            }
            // Text and StickyNote: just move position, don't scale content
            Shape::Text {
                x,
                y,
                text,
                color,
                size,
                font_descriptor,
                background_enabled,
                wrap_width,
            } => {
                let (nx, ny) = Self::scale_point_i32(*x, *y, anchor_x, anchor_y, scale_x, scale_y);
                Shape::Text {
                    x: nx,
                    y: ny,
                    text: text.clone(),
                    color: *color,
                    size: *size,
                    font_descriptor: font_descriptor.clone(),
                    background_enabled: *background_enabled,
                    wrap_width: *wrap_width,
                }
            }
            Shape::StickyNote {
                x,
                y,
                text,
                background,
                size,
                font_descriptor,
                wrap_width,
            } => {
                let (nx, ny) = Self::scale_point_i32(*x, *y, anchor_x, anchor_y, scale_x, scale_y);
                Shape::StickyNote {
                    x: nx,
                    y: ny,
                    text: text.clone(),
                    background: *background,
                    size: *size,
                    font_descriptor: font_descriptor.clone(),
                    wrap_width: *wrap_width,
                }
            }
        }
    }

    /// Restore shapes from snapshots (used for cancel).
    pub(crate) fn restore_resize_from_snapshots(&mut self, snapshots: &[(ShapeId, ShapeSnapshot)]) {
        let mut dirty_rects: Vec<Option<Rect>> =
            Vec::with_capacity(snapshots.len().saturating_mul(2));
        let mut ids_to_invalidate = Vec::with_capacity(snapshots.len());

        {
            let frame = self.boards.active_frame_mut();
            for (shape_id, snapshot) in snapshots {
                if let Some(drawn) = frame.shape_mut(*shape_id) {
                    dirty_rects.push(drawn.shape.bounding_box());
                    drawn.shape = snapshot.shape.clone();
                    drawn.locked = snapshot.locked;
                    dirty_rects.push(drawn.shape.bounding_box());
                    ids_to_invalidate.push(*shape_id);
                }
            }
        }

        for rect in dirty_rects {
            self.dirty_tracker.mark_optional_rect(rect);
        }
        for shape_id in ids_to_invalidate {
            self.invalidate_hit_cache_for(shape_id);
        }
        self.needs_redraw = true;
    }
}
