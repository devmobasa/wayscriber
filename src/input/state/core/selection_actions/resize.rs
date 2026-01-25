//! Selection resize functionality.

use crate::draw::frame::ShapeSnapshot;
use crate::draw::{Shape, ShapeId};
use crate::input::InputState;
use crate::input::state::core::base::SelectionHandle;
use crate::util::Rect;

/// Handle size for hit testing (matches render constants)
const HANDLE_SIZE: i32 = 8;
const HANDLE_TOLERANCE: i32 = 4;

impl InputState {
    /// Hit test for selection handles. Returns the handle if mouse is over one.
    pub fn hit_selection_handle(&self, x: i32, y: i32) -> Option<SelectionHandle> {
        let bounds = self.selection_bounds()?;
        let half = HANDLE_SIZE / 2;
        let tol = HANDLE_TOLERANCE;

        // Check corner handles first (they have priority)
        // Top-left
        if self.point_near(x, y, bounds.x, bounds.y, half + tol) {
            return Some(SelectionHandle::TopLeft);
        }
        // Top-right
        if self.point_near(x, y, bounds.x + bounds.width, bounds.y, half + tol) {
            return Some(SelectionHandle::TopRight);
        }
        // Bottom-left
        if self.point_near(x, y, bounds.x, bounds.y + bounds.height, half + tol) {
            return Some(SelectionHandle::BottomLeft);
        }
        // Bottom-right
        if self.point_near(
            x,
            y,
            bounds.x + bounds.width,
            bounds.y + bounds.height,
            half + tol,
        ) {
            return Some(SelectionHandle::BottomRight);
        }

        // Edge handles
        let edge_half = (HANDLE_SIZE * 3 / 4) / 2;
        // Top center
        if self.point_near(x, y, bounds.x + bounds.width / 2, bounds.y, edge_half + tol) {
            return Some(SelectionHandle::Top);
        }
        // Bottom center
        if self.point_near(
            x,
            y,
            bounds.x + bounds.width / 2,
            bounds.y + bounds.height,
            edge_half + tol,
        ) {
            return Some(SelectionHandle::Bottom);
        }
        // Left center
        if self.point_near(
            x,
            y,
            bounds.x,
            bounds.y + bounds.height / 2,
            edge_half + tol,
        ) {
            return Some(SelectionHandle::Left);
        }
        // Right center
        if self.point_near(
            x,
            y,
            bounds.x + bounds.width,
            bounds.y + bounds.height / 2,
            edge_half + tol,
        ) {
            return Some(SelectionHandle::Right);
        }

        None
    }

    fn point_near(&self, x: i32, y: i32, cx: i32, cy: i32, radius: i32) -> bool {
        (x - cx).abs() <= radius && (y - cy).abs() <= radius
    }

    /// Capture snapshots of selected shapes for resize operation.
    pub(crate) fn capture_resize_selection_snapshots(&self) -> Vec<(ShapeId, ShapeSnapshot)> {
        let ids = self.selected_shape_ids();
        let frame = self.boards.active_frame();
        ids.iter()
            .filter_map(|id| {
                frame.shape(*id).and_then(|shape| {
                    if shape.locked {
                        None
                    } else {
                        Some((
                            *id,
                            ShapeSnapshot {
                                shape: shape.shape.clone(),
                                locked: shape.locked,
                            },
                        ))
                    }
                })
            })
            .collect()
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
        self.mark_selection_dirty_region(Some(*original_bounds));
        // Calculate scale factors based on handle and delta
        let (scale_x, scale_y, anchor_x, anchor_y) =
            Self::compute_scale_factors(handle, original_bounds, dx, dy);

        // Collect IDs to invalidate after the loop
        let mut ids_to_invalidate = Vec::new();

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

    fn compute_scale_factors(
        handle: SelectionHandle,
        bounds: &Rect,
        dx: i32,
        dy: i32,
    ) -> (f64, f64, f64, f64) {
        let w = bounds.width as f64;
        let h = bounds.height as f64;
        let x = bounds.x as f64;
        let y = bounds.y as f64;

        match handle {
            SelectionHandle::TopLeft => {
                let new_w = (w - dx as f64).max(10.0);
                let new_h = (h - dy as f64).max(10.0);
                (new_w / w, new_h / h, x + w, y + h)
            }
            SelectionHandle::TopRight => {
                let new_w = (w + dx as f64).max(10.0);
                let new_h = (h - dy as f64).max(10.0);
                (new_w / w, new_h / h, x, y + h)
            }
            SelectionHandle::BottomLeft => {
                let new_w = (w - dx as f64).max(10.0);
                let new_h = (h + dy as f64).max(10.0);
                (new_w / w, new_h / h, x + w, y)
            }
            SelectionHandle::BottomRight => {
                let new_w = (w + dx as f64).max(10.0);
                let new_h = (h + dy as f64).max(10.0);
                (new_w / w, new_h / h, x, y)
            }
            SelectionHandle::Top => {
                let new_h = (h - dy as f64).max(10.0);
                (1.0, new_h / h, x + w / 2.0, y + h)
            }
            SelectionHandle::Bottom => {
                let new_h = (h + dy as f64).max(10.0);
                (1.0, new_h / h, x + w / 2.0, y)
            }
            SelectionHandle::Left => {
                let new_w = (w - dx as f64).max(10.0);
                (new_w / w, 1.0, x + w, y + h / 2.0)
            }
            SelectionHandle::Right => {
                let new_w = (w + dx as f64).max(10.0);
                (new_w / w, 1.0, x, y + h / 2.0)
            }
        }
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
                let (nx, ny) =
                    Self::scale_point(*x as f64, *y as f64, anchor_x, anchor_y, scale_x, scale_y);
                let nw = (*w as f64 * scale_x).round() as i32;
                let nh = (*h as f64 * scale_y).round() as i32;
                Shape::Rect {
                    x: nx.round() as i32,
                    y: ny.round() as i32,
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
                    Self::scale_point(*cx as f64, *cy as f64, anchor_x, anchor_y, scale_x, scale_y);
                let nrx = (*rx as f64 * scale_x).round() as i32;
                let nry = (*ry as f64 * scale_y).round() as i32;
                Shape::Ellipse {
                    cx: ncx.round() as i32,
                    cy: ncy.round() as i32,
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
                    Self::scale_point(*x1 as f64, *y1 as f64, anchor_x, anchor_y, scale_x, scale_y);
                let (nx2, ny2) =
                    Self::scale_point(*x2 as f64, *y2 as f64, anchor_x, anchor_y, scale_x, scale_y);
                Shape::Line {
                    x1: nx1.round() as i32,
                    y1: ny1.round() as i32,
                    x2: nx2.round() as i32,
                    y2: ny2.round() as i32,
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
                    Self::scale_point(*x1 as f64, *y1 as f64, anchor_x, anchor_y, scale_x, scale_y);
                let (nx2, ny2) =
                    Self::scale_point(*x2 as f64, *y2 as f64, anchor_x, anchor_y, scale_x, scale_y);
                Shape::Arrow {
                    x1: nx1.round() as i32,
                    y1: ny1.round() as i32,
                    x2: nx2.round() as i32,
                    y2: ny2.round() as i32,
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
                let scaled_points: Vec<(i32, i32)> = points
                    .iter()
                    .map(|(px, py)| {
                        let (nx, ny) = Self::scale_point(
                            *px as f64, *py as f64, anchor_x, anchor_y, scale_x, scale_y,
                        );
                        (nx.round() as i32, ny.round() as i32)
                    })
                    .collect();
                Shape::Freehand {
                    points: scaled_points,
                    color: *color,
                    thick: *thick,
                }
            }
            Shape::FreehandPressure { points, color } => {
                let scaled_points: Vec<(i32, i32, f32)> = points
                    .iter()
                    .map(|(px, py, pressure)| {
                        let (nx, ny) = Self::scale_point(
                            *px as f64, *py as f64, anchor_x, anchor_y, scale_x, scale_y,
                        );
                        (nx.round() as i32, ny.round() as i32, *pressure)
                    })
                    .collect();
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
                let scaled_points: Vec<(i32, i32)> = points
                    .iter()
                    .map(|(px, py)| {
                        let (nx, ny) = Self::scale_point(
                            *px as f64, *py as f64, anchor_x, anchor_y, scale_x, scale_y,
                        );
                        (nx.round() as i32, ny.round() as i32)
                    })
                    .collect();
                Shape::MarkerStroke {
                    points: scaled_points,
                    color: *color,
                    thick: *thick,
                }
            }
            Shape::StepMarker { x, y, color, label } => {
                let (nx, ny) =
                    Self::scale_point(*x as f64, *y as f64, anchor_x, anchor_y, scale_x, scale_y);
                Shape::StepMarker {
                    x: nx.round() as i32,
                    y: ny.round() as i32,
                    color: *color,
                    label: label.clone(),
                }
            }
            Shape::EraserStroke { points, brush } => {
                let scaled_points: Vec<(i32, i32)> = points
                    .iter()
                    .map(|(px, py)| {
                        let (nx, ny) = Self::scale_point(
                            *px as f64, *py as f64, anchor_x, anchor_y, scale_x, scale_y,
                        );
                        (nx.round() as i32, ny.round() as i32)
                    })
                    .collect();
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
                let (nx, ny) =
                    Self::scale_point(*x as f64, *y as f64, anchor_x, anchor_y, scale_x, scale_y);
                Shape::Text {
                    x: nx.round() as i32,
                    y: ny.round() as i32,
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
                let (nx, ny) =
                    Self::scale_point(*x as f64, *y as f64, anchor_x, anchor_y, scale_x, scale_y);
                Shape::StickyNote {
                    x: nx.round() as i32,
                    y: ny.round() as i32,
                    text: text.clone(),
                    background: *background,
                    size: *size,
                    font_descriptor: font_descriptor.clone(),
                    wrap_width: *wrap_width,
                }
            }
        }
    }

    fn scale_point(
        x: f64,
        y: f64,
        anchor_x: f64,
        anchor_y: f64,
        scale_x: f64,
        scale_y: f64,
    ) -> (f64, f64) {
        let dx = x - anchor_x;
        let dy = y - anchor_y;
        (anchor_x + dx * scale_x, anchor_y + dy * scale_y)
    }

    /// Restore shapes from snapshots (used for cancel).
    pub(crate) fn restore_resize_from_snapshots(&mut self, snapshots: &[(ShapeId, ShapeSnapshot)]) {
        let mut dirty_rects: Vec<Option<Rect>> = Vec::new();
        let mut ids_to_invalidate = Vec::new();

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
