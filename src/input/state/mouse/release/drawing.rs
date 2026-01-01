use log::warn;

use crate::draw::Shape;
use crate::draw::frame::UndoAction;
use crate::draw::shape::EraserBrush;
use crate::input::{EraserMode, InputState, Tool};
use crate::util;

pub(super) fn finish_drawing(
    state: &mut InputState,
    tool: Tool,
    start_x: i32,
    start_y: i32,
    points: Vec<(i32, i32)>,
    end_x: i32,
    end_y: i32,
) {
    let shape = match tool {
        Tool::Pen => Shape::Freehand {
            points,
            color: state.current_color,
            thick: state.current_thickness,
        },
        Tool::Line => Shape::Line {
            x1: start_x,
            y1: start_y,
            x2: end_x,
            y2: end_y,
            color: state.current_color,
            thick: state.current_thickness,
        },
        Tool::Rect => {
            let (left, width) = if end_x >= start_x {
                (start_x, end_x - start_x)
            } else {
                (end_x, start_x - end_x)
            };
            let (top, height) = if end_y >= start_y {
                (start_y, end_y - start_y)
            } else {
                (end_y, start_y - end_y)
            };
            Shape::Rect {
                x: left,
                y: top,
                w: width,
                h: height,
                fill: state.fill_enabled,
                color: state.current_color,
                thick: state.current_thickness,
            }
        }
        Tool::Ellipse => {
            let (cx, cy, rx, ry) = util::ellipse_bounds(start_x, start_y, end_x, end_y);
            Shape::Ellipse {
                cx,
                cy,
                rx,
                ry,
                fill: state.fill_enabled,
                color: state.current_color,
                thick: state.current_thickness,
            }
        }
        Tool::Arrow => Shape::Arrow {
            x1: start_x,
            y1: start_y,
            x2: end_x,
            y2: end_y,
            color: state.current_color,
            thick: state.current_thickness,
            arrow_length: state.arrow_length,
            arrow_angle: state.arrow_angle,
            head_at_end: state.arrow_head_at_end,
        },
        Tool::Marker => Shape::MarkerStroke {
            points,
            color: state.marker_color(),
            thick: state.current_thickness,
        },
        Tool::Eraser => {
            if state.eraser_mode == EraserMode::Stroke {
                state.clear_provisional_dirty();
                let mut path = points;
                if path.last().copied() != Some((end_x, end_y)) {
                    path.push((end_x, end_y));
                }
                state.erase_strokes_by_points(&path);
                return;
            }
            Shape::EraserStroke {
                points,
                brush: EraserBrush {
                    size: state.eraser_size,
                    kind: state.eraser_kind,
                },
            }
        }
        Tool::Highlight => {
            state.clear_provisional_dirty();
            return;
        }
        Tool::Select => {
            state.clear_provisional_dirty();
            return;
        }
    };

    let bounds = shape.bounding_box();
    state.clear_provisional_dirty();

    let mut limit_reached = false;
    let addition = {
        let frame = state.canvas_set.active_frame_mut();
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
    } else if limit_reached {
        warn!(
            "Shape limit ({}) reached; discarding new shape",
            state.max_shapes_per_frame
        );
    }
}
