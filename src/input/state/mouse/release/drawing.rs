use log::warn;

use crate::draw::Shape;
use crate::draw::frame::UndoAction;
use crate::draw::shape::EraserBrush;
use crate::input::{EraserMode, InputState, Tool};
use crate::util;

pub(super) struct DrawingRelease {
    pub(super) start: (i32, i32),
    pub(super) end: (i32, i32),
    pub(super) points: Vec<(i32, i32)>,
    pub(super) point_thicknesses: Vec<f32>,
}

pub(super) fn finish_drawing(state: &mut InputState, tool: Tool, release: DrawingRelease) {
    let (start_x, start_y) = release.start;
    let (end_x, end_y) = release.end;
    let DrawingRelease {
        points,
        point_thicknesses,
        ..
    } = release;
    let label = if matches!(tool, Tool::Arrow) {
        state.next_arrow_label()
    } else {
        None
    };
    let used_arrow_label = label.is_some();
    let shape = match tool {
        Tool::Pen => {
            // Check if we have pressure data and if it varies enough to matter
            let use_pressure = if point_thicknesses.len() == points.len() {
                let min_t = point_thicknesses
                    .iter()
                    .fold(f32::INFINITY, |a, &b| a.min(b));
                let max_t = point_thicknesses
                    .iter()
                    .fold(f32::NEG_INFINITY, |a, &b| a.max(b));
                (max_t - min_t).abs() > state.pressure_variation_threshold as f32
            } else {
                false
            };

            if use_pressure {
                let points_with_pressure: Vec<(i32, i32, f32)> = points
                    .iter()
                    .zip(point_thicknesses.iter())
                    .map(|(&(x, y), &t)| (x, y, t))
                    .collect();

                Shape::FreehandPressure {
                    points: points_with_pressure,
                    color: state.current_color,
                }
            } else {
                Shape::Freehand {
                    points,
                    color: state.current_color,
                    thick: state.current_thickness,
                }
            }
        }
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
            label,
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
                if state.erase_strokes_by_points(&path) {
                    state.mark_session_dirty();
                }
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
        if used_arrow_label {
            state.bump_arrow_label();
        }
    } else if limit_reached {
        warn!(
            "Shape limit ({}) reached; discarding new shape",
            state.max_shapes_per_frame
        );
    }
}
