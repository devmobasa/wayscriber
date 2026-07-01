use log::warn;

use crate::draw::Shape;
use crate::draw::frame::UndoAction;
use crate::draw::shape::bounding_box_for_points;
use crate::input::tool::{FinishedToolStroke, PolygonStrokeSnapshot, ToolStrokeSnapshot};
use crate::input::{InputState, Tool};
use crate::util::Rect;

const FINISHED_PATH_DAMAGE_MAX_SPAN: f64 = 128.0;

pub(super) struct DrawingRelease {
    pub(super) start: (i32, i32),
    pub(super) end: (i32, i32),
    pub(super) points: Vec<(i32, i32)>,
    pub(super) point_thicknesses: Vec<f32>,
}

pub(super) fn finish_drawing(state: &mut InputState, tool: Tool, release: DrawingRelease) {
    let drawing_color = state.active_drag_color_or_tool(tool);
    let drawing_thickness = state.thickness_for_tool(tool);
    let pressure_preview_exceeds_final_width = pressure_preview_exceeds_final_freehand_width(
        release.points.len(),
        &release.point_thicknesses,
        drawing_thickness,
    );
    let finished = if tool.polygon_template().is_some() {
        let snapshot = PolygonStrokeSnapshot {
            tool,
            start: release.start,
            end: release.end,
            color: drawing_color,
            size: drawing_thickness,
            fill_enabled: state.fill_enabled,
            regular_sides: state.polygon_sides,
        };
        tool.finish_polygon_stroke(snapshot)
    } else {
        let snapshot = ToolStrokeSnapshot {
            tool,
            start: release.start,
            end: release.end,
            points: release.points,
            point_thicknesses: release.point_thicknesses,
            color: drawing_color,
            size: drawing_thickness,
            marker_opacity: state.marker_opacity,
            fill_enabled: state.fill_enabled,
            arrow_length: state.arrow_length,
            arrow_angle: state.arrow_angle,
            arrow_head_at_end: state.arrow_head_at_end,
            arrow_label: state.next_arrow_label(),
            step_marker_label: state.next_step_marker_label(),
            eraser_mode: state.eraser_mode,
            eraser_size: state.eraser_size,
            eraser_kind: state.eraser_kind,
            pressure_variation_threshold: state.pressure_variation_threshold,
        };
        tool.finish_stroke(snapshot)
    };

    let (shape, usage) = match finished {
        FinishedToolStroke::Shape { shape, usage } => (shape, usage),
        FinishedToolStroke::EraseStroke { path } => {
            state.clear_provisional_dirty();
            if state.erase_strokes_by_points(&path) {
                state.mark_session_dirty();
            }
            return;
        }
        FinishedToolStroke::Noop => {
            state.clear_provisional_dirty();
            return;
        }
    };

    let bounds = shape.bounding_box();
    let path_damage = finished_path_damage_regions(&shape, bounds);
    let preserve_provisional_cleanup =
        matches!(shape, Shape::Freehand { .. }) && pressure_preview_exceeds_final_width;

    let mut limit_reached = false;
    let addition = {
        let frame = state.boards.active_frame_mut();
        match frame.try_add_shape_with_id(shape.clone(), state.max_shapes_per_frame) {
            Some(new_id) => {
                if let Some(index) = frame.find_index(new_id) {
                    if let Some(new_shape) = frame.shape(new_id) {
                        let snapshot = new_shape.clone();
                        frame.push_undo_action(
                            UndoAction::Create {
                                shapes: vec![(index, snapshot.clone())],
                            },
                            state.undo_stack_limit,
                        );
                        Some((new_id, snapshot))
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
            None => {
                limit_reached = true;
                None
            }
        }
    };

    if let Some((new_id, _snapshot)) = addition {
        state.invalidate_hit_cache_for(new_id);
        if let Some(path_damage) = path_damage {
            let provisional_bounds = state.take_provisional_dirty_bounds();
            for region in path_damage {
                state.dirty_tracker.mark_rect(region);
            }
            if preserve_provisional_cleanup {
                state.dirty_tracker.mark_optional_rect(provisional_bounds);
            }
        } else {
            state.clear_provisional_dirty();
            state.dirty_tracker.mark_optional_rect(bounds);
        }
        state.clear_selection();
        state.needs_redraw = true;
        state.mark_session_dirty();
        state.record_first_stroke_done_for_onboarding();
        if usage.bump_arrow_label {
            state.bump_arrow_label();
        }
        if usage.bump_step_marker {
            state.bump_step_marker();
        }
    } else {
        state.clear_provisional_dirty();
        if limit_reached {
            warn!(
                "Shape limit ({}) reached; discarding new shape",
                state.max_shapes_per_frame
            );
        }
    }
}

fn pressure_preview_exceeds_final_freehand_width(
    point_count: usize,
    point_thicknesses: &[f32],
    final_width: f64,
) -> bool {
    point_thicknesses.len() == point_count
        && point_thicknesses
            .iter()
            .any(|&thickness| f64::from(thickness) > final_width)
}

fn finished_path_damage_regions(shape: &Shape, fallback: Option<Rect>) -> Option<Vec<Rect>> {
    match shape {
        Shape::Freehand { points, thick, .. } => {
            Some(split_path_damage_regions(points, *thick, fallback?))
        }
        Shape::FreehandPressure { points, .. } => {
            let max_thick = points
                .iter()
                .fold(1.0f64, |max, &(_, _, thickness)| max.max(thickness as f64));
            let points = points.iter().map(|&(x, y, _)| (x, y)).collect::<Vec<_>>();
            Some(split_path_damage_regions(&points, max_thick, fallback?))
        }
        Shape::MarkerStroke { points, thick, .. } => {
            let inflated = (*thick * 1.35).max(*thick + 1.0);
            Some(split_path_damage_regions(points, inflated, fallback?))
        }
        Shape::EraserStroke { points, brush } => Some(split_path_damage_regions(
            points,
            brush.size.max(1.0),
            fallback?,
        )),
        _ => None,
    }
}

fn split_path_damage_regions(
    points: &[(i32, i32)],
    stroke_width: f64,
    fallback: Rect,
) -> Vec<Rect> {
    if points.len() < 2 {
        return vec![fallback];
    }

    let mut regions = Vec::new();
    for segment in points.windows(2) {
        append_segment_damage_regions(segment[0], segment[1], stroke_width, &mut regions);
    }

    if regions.is_empty() {
        vec![fallback]
    } else {
        regions
    }
}

fn append_segment_damage_regions(
    start: (i32, i32),
    end: (i32, i32),
    stroke_width: f64,
    regions: &mut Vec<Rect>,
) {
    if start == end {
        if let Some(region) = bounding_box_for_points(&[start], stroke_width) {
            regions.push(region);
        }
        return;
    }

    let dx = f64::from(end.0) - f64::from(start.0);
    let dy = f64::from(end.1) - f64::from(start.1);
    let steps = (dx.abs().max(dy.abs()) / FINISHED_PATH_DAMAGE_MAX_SPAN).ceil() as usize;
    let steps = steps.max(1);

    for step in 0..steps {
        let t0 = step as f64 / steps as f64;
        let t1 = (step + 1) as f64 / steps as f64;
        let p0 = (
            (f64::from(start.0) + dx * t0).round() as i32,
            (f64::from(start.1) + dy * t0).round() as i32,
        );
        let p1 = (
            (f64::from(start.0) + dx * t1).round() as i32,
            (f64::from(start.1) + dy * t1).round() as i32,
        );
        if let Some(region) = bounding_box_for_points(&[p0, p1], stroke_width) {
            regions.push(region);
        }
    }
}
