use log::warn;

use crate::draw::frame::UndoAction;
use crate::input::tool::{FinishedToolStroke, PolygonStrokeSnapshot, ToolStrokeSnapshot};
use crate::input::{InputState, Tool};

pub(super) struct DrawingRelease {
    pub(super) start: (i32, i32),
    pub(super) end: (i32, i32),
    pub(super) points: Vec<(i32, i32)>,
    pub(super) point_thicknesses: Vec<f32>,
}

pub(super) fn finish_drawing(state: &mut InputState, tool: Tool, release: DrawingRelease) {
    let drawing_color = state.active_drag_color_or_tool(tool);
    let drawing_thickness = state.thickness_for_tool(tool);
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
    state.clear_provisional_dirty();

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
        state.dirty_tracker.mark_optional_rect(bounds);
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
    } else if limit_reached {
        warn!(
            "Shape limit ({}) reached; discarding new shape",
            state.max_shapes_per_frame
        );
    }
}
