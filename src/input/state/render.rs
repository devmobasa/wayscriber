use crate::draw::render::{render_freehand_pressure_preview_borrowed, render_polygon_preview};
use crate::draw::shape::bounding_box_for_points;
use crate::draw::{
    Color, Shape, render_freehand_borrowed, render_marker_stroke_borrowed, render_shape,
};
use crate::input::Tool;
use crate::input::tool::{
    PolygonProvisionalSnapshot, ProvisionalToolSnapshot, ProvisionalToolStroke,
};
use crate::util::Rect;
use std::ops::Range;

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
                render_freehand_pressure_preview_borrowed(ctx, points, point_thicknesses, color);
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

    pub(crate) fn render_provisional_tool_stroke_for_damage(
        &self,
        ctx: &cairo::Context,
        stroke: ProvisionalToolStroke<'_>,
        damage_regions: &[Rect],
    ) -> bool {
        match stroke {
            ProvisionalToolStroke::BorrowedFreehand {
                points,
                color,
                size,
            } => {
                let ranges = path_damage_ranges(points, damage_regions, size);
                if ranges.is_empty() {
                    return false;
                }
                for range in ranges {
                    render_freehand_borrowed(ctx, &points[range], color, size);
                }
                true
            }
            ProvisionalToolStroke::BorrowedPressureFreehand {
                points,
                point_thicknesses,
                color,
            } => {
                let max_thick = point_thicknesses
                    .iter()
                    .fold(1.0f64, |max, &thickness| max.max(thickness as f64));
                let ranges = path_damage_ranges(points, damage_regions, max_thick);
                if ranges.is_empty() {
                    return false;
                }
                if pressure_preview_needs_full_mask_render(color, &ranges) {
                    render_freehand_pressure_preview_borrowed(
                        ctx,
                        points,
                        point_thicknesses,
                        color,
                    );
                    return true;
                }
                let mut rendered = false;
                for range in ranges {
                    let thickness_start = range.start.min(point_thicknesses.len());
                    let thickness_end = range.end.min(point_thicknesses.len());
                    if thickness_start >= thickness_end {
                        continue;
                    }
                    render_freehand_pressure_preview_borrowed(
                        ctx,
                        &points[range],
                        &point_thicknesses[thickness_start..thickness_end],
                        color,
                    );
                    rendered = true;
                }
                rendered
            }
            ProvisionalToolStroke::BorrowedMarker {
                points,
                color,
                size,
            } => {
                let inflated = (size * 1.35).max(size + 1.0);
                let ranges = path_damage_ranges(points, damage_regions, inflated);
                if ranges.is_empty() {
                    return false;
                }
                for range in ranges {
                    render_marker_stroke_borrowed(ctx, &points[range], color, size);
                }
                true
            }
            ProvisionalToolStroke::EraserPreview { points, size } => {
                let ranges = path_damage_ranges(points, damage_regions, size);
                if ranges.is_empty() {
                    return false;
                }
                let preview_color = Color {
                    r: 1.0,
                    g: 1.0,
                    b: 1.0,
                    a: 0.35,
                };
                for range in ranges {
                    render_freehand_borrowed(ctx, &points[range], preview_color, size);
                }
                true
            }
            other => self.render_provisional_tool_stroke(ctx, other),
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
                render_polygon_preview(ctx, &preview_points, *fill, *color, *thick);
                true
            }
            _ => false,
        }
    }

    pub(crate) fn render_provisional_shape_for_damage(
        &self,
        ctx: &cairo::Context,
        current_x: i32,
        current_y: i32,
        damage_regions: &[Rect],
    ) -> bool {
        if matches!(self.state, DrawingState::Drawing { .. }) {
            let stroke = self.provisional_tool_stroke(current_x, current_y);
            return self.render_provisional_tool_stroke_for_damage(ctx, stroke, damage_regions);
        }

        self.render_provisional_shape(ctx, current_x, current_y)
    }
}

fn path_damage_ranges(
    points: &[(i32, i32)],
    damage_regions: &[Rect],
    stroke_width: f64,
) -> Vec<Range<usize>> {
    if points.is_empty() {
        return Vec::new();
    }

    if damage_regions.is_empty() {
        return single_range(0..points.len());
    }

    if points.len() == 1 {
        return segment_bounds(points[0], points[0], stroke_width)
            .filter(|bounds| {
                damage_regions
                    .iter()
                    .any(|damage| rects_intersect(*bounds, *damage))
            })
            .map(|_| single_range(0..1))
            .unwrap_or_default();
    }

    let mut ranges = Vec::new();
    for index in 1..points.len() {
        let Some(bounds) = segment_bounds(points[index - 1], points[index], stroke_width) else {
            continue;
        };
        if damage_regions
            .iter()
            .any(|damage| rects_intersect(bounds, *damage))
        {
            let start = index.saturating_sub(2);
            let end = (index + 2).min(points.len());
            push_merged_range(&mut ranges, start..end);
        }
    }
    ranges
}

fn single_range(range: Range<usize>) -> Vec<Range<usize>> {
    std::iter::once(range).collect()
}

fn push_merged_range(ranges: &mut Vec<Range<usize>>, next: Range<usize>) {
    if next.start >= next.end {
        return;
    }

    if let Some(last) = ranges.last_mut()
        && last.end >= next.start
    {
        last.end = last.end.max(next.end);
        return;
    }

    ranges.push(next);
}

fn segment_bounds(a: (i32, i32), b: (i32, i32), stroke_width: f64) -> Option<Rect> {
    bounding_box_for_points(&[a, b], stroke_width)
}

fn rects_intersect(a: Rect, b: Rect) -> bool {
    let a_right = a.x.saturating_add(a.width);
    let a_bottom = a.y.saturating_add(a.height);
    let b_right = b.x.saturating_add(b.width);
    let b_bottom = b.y.saturating_add(b.height);

    !(a.x >= b_right || a_right <= b.x || a.y >= b_bottom || a_bottom <= b.y)
}

fn pressure_preview_needs_full_mask_render(color: Color, ranges: &[Range<usize>]) -> bool {
    color.a < 1.0 && ranges.len() > 1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_damage_ranges_limits_long_path_to_intersecting_tail() {
        let points: Vec<_> = (0..100).map(|index| (index * 10, 0)).collect();
        let damage = Rect::new(975, -5, 10, 10).unwrap();

        let ranges = path_damage_ranges(&points, &[damage], 2.0);

        assert_eq!(ranges, single_range(96..100));
    }

    #[test]
    fn path_damage_ranges_returns_full_path_without_damage_context() {
        let points: Vec<_> = (0..8).map(|index| (index * 10, 0)).collect();

        let ranges = path_damage_ranges(&points, &[], 2.0);

        assert_eq!(ranges, single_range(0..points.len()));
    }

    #[test]
    fn translucent_pressure_preview_uses_full_mask_for_multiple_dirty_ranges() {
        let color = Color {
            r: 1.0,
            g: 0.0,
            b: 0.0,
            a: 0.35,
        };
        let opaque = Color { a: 1.0, ..color };
        let points: Vec<_> = (0..10).map(|index| (index * 20, 0)).collect();
        let damage = [
            Rect::new(18, -4, 4, 8).unwrap(),
            Rect::new(158, -4, 4, 8).unwrap(),
        ];

        let ranges = path_damage_ranges(&points, &damage, 2.0);

        assert_eq!(ranges, vec![0..4, 6..10]);
        assert!(pressure_preview_needs_full_mask_render(color, &ranges));
        assert!(!pressure_preview_needs_full_mask_render(opaque, &ranges));
        assert!(!pressure_preview_needs_full_mask_render(
            color,
            &single_range(0..points.len())
        ));
    }
}
