use crate::draw::render::render_freehand_pressure_borrowed;
use crate::draw::{
    Color, PolygonKind, Shape, render_freehand_borrowed, render_marker_stroke_borrowed,
    render_shape,
};
use crate::input::Tool;
use crate::input::tool::{
    PolygonProvisionalSnapshot, ProvisionalToolSnapshot, ProvisionalToolStroke,
};

use super::{DrawingState, InputState};

impl InputState {
    pub(crate) fn provisional_tool_stroke(
        &self,
        current_x: i32,
        current_y: i32,
    ) -> ProvisionalToolStroke<'_> {
        let DrawingState::Drawing {
            tool,
            start_x,
            start_y,
            points,
            point_thicknesses,
        } = &self.state
        else {
            return ProvisionalToolStroke::None;
        };

        if tool.polygon_template().is_some() {
            let snapshot = PolygonProvisionalSnapshot {
                tool: *tool,
                start: (*start_x, *start_y),
                current: (current_x, current_y),
                color: self.active_drag_color_or_current(),
                size: self.thickness_for_tool(*tool),
                fill_enabled: self.fill_enabled,
                regular_sides: self.polygon_sides,
            };
            return tool.provisional_polygon_stroke(snapshot);
        }

        let snapshot = ProvisionalToolSnapshot {
            tool: *tool,
            start: (*start_x, *start_y),
            current: (current_x, current_y),
            points,
            point_thicknesses,
            color: self.active_drag_color_or_current(),
            size: self.thickness_for_tool(*tool),
            eraser_size: self.eraser_size,
            marker_opacity: self.marker_opacity,
            fill_enabled: self.fill_enabled,
            arrow_length: self.arrow_length,
            arrow_angle: self.arrow_angle,
            arrow_head_at_end: self.arrow_head_at_end,
            arrow_label: if *tool == Tool::Arrow {
                self.next_arrow_label()
            } else {
                None
            },
            step_marker_label: (*tool == Tool::StepMarker).then(|| self.next_step_marker_label()),
        };
        tool.provisional_stroke(snapshot)
    }

    pub(crate) fn render_provisional_tool_stroke(
        &self,
        ctx: &cairo::Context,
        stroke: ProvisionalToolStroke<'_>,
    ) -> bool {
        match stroke {
            ProvisionalToolStroke::BorrowedFreehand {
                points,
                color,
                size,
            } => {
                render_freehand_borrowed(ctx, points, color, size);
                true
            }
            ProvisionalToolStroke::BorrowedPressureFreehand {
                points,
                point_thicknesses,
                color,
            } => {
                render_freehand_pressure_borrowed(ctx, points, point_thicknesses, color);
                true
            }
            ProvisionalToolStroke::BorrowedMarker {
                points,
                color,
                size,
            } => {
                render_marker_stroke_borrowed(ctx, points, color, size);
                true
            }
            ProvisionalToolStroke::EraserPreview { points, size } => {
                let preview_color = Color {
                    r: 1.0,
                    g: 1.0,
                    b: 1.0,
                    a: 0.35,
                };
                render_freehand_borrowed(ctx, points, preview_color, size);
                true
            }
            ProvisionalToolStroke::Shape(shape) => {
                render_shape(ctx, &shape);
                true
            }
            ProvisionalToolStroke::BlurReplayPreview(params) => {
                render_shape(
                    ctx,
                    &Shape::BlurRect {
                        x: params.x,
                        y: params.y,
                        w: params.w,
                        h: params.h,
                        strength: params.strength,
                    },
                );
                true
            }
            ProvisionalToolStroke::None => false,
        }
    }

    /// Renders the provisional shape directly to a Cairo context without cloning.
    ///
    /// This is an optimized version for freehand drawing that avoids cloning
    /// the points vector on every render, preventing quadratic performance.
    ///
    /// # Arguments
    /// * `ctx` - Cairo context to render to
    /// * `current_x` - Current mouse X coordinate
    /// * `current_y` - Current mouse Y coordinate
    ///
    /// # Returns
    /// `true` if a provisional shape was rendered, `false` otherwise
    pub fn render_provisional_shape(
        &self,
        ctx: &cairo::Context,
        current_x: i32,
        current_y: i32,
    ) -> bool {
        match &self.state {
            DrawingState::Drawing { .. } => {
                let stroke = self.provisional_tool_stroke(current_x, current_y);
                self.render_provisional_tool_stroke(ctx, stroke)
            }
            DrawingState::Selecting {
                start_x,
                start_y,
                additive,
            } => {
                let Some(rect) =
                    Self::selection_rect_from_points(*start_x, *start_y, current_x, current_y)
                else {
                    return false;
                };
                let _ = ctx.save();
                ctx.rectangle(
                    rect.x as f64,
                    rect.y as f64,
                    rect.width as f64,
                    rect.height as f64,
                );
                ctx.set_source_rgba(0.2, 0.45, 1.0, 0.12);
                let _ = ctx.fill_preserve();
                if *additive {
                    ctx.set_source_rgba(0.2, 0.75, 0.45, 0.9);
                } else {
                    ctx.set_source_rgba(0.2, 0.45, 1.0, 0.9);
                }
                ctx.set_line_width(1.5);
                ctx.set_dash(&[6.0, 4.0], 0.0);
                let _ = ctx.stroke();
                let _ = ctx.restore();
                true
            }
            DrawingState::BuildingPolygon {
                points,
                preview,
                fill,
                color,
                thick,
            } => {
                let mut preview_points = points.clone();
                if let Some(point) = preview.or(Some((current_x, current_y))) {
                    preview_points.push(point);
                }
                if preview_points.len() >= 3 {
                    render_shape(
                        ctx,
                        &Shape::Polygon {
                            kind: PolygonKind::Freeform,
                            points: preview_points,
                            fill: *fill,
                            color: *color,
                            thick: *thick,
                        },
                    );
                } else {
                    render_freehand_borrowed(ctx, &preview_points, *color, *thick);
                }
                true
            }
            _ => false,
        }
    }
}
