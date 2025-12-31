use super::super::base::{InputState, MAX_STROKE_THICKNESS, MIN_STROKE_THICKNESS, UiToastKind};
use super::summary::shape_color;
use super::utils::{SELECTION_COLORS, color_palette_index, cycle_index};
use crate::draw::{Color, RED, Shape, ShapeId};

const SELECTION_THICKNESS_STEP: f64 = 1.0;
const SELECTION_FONT_SIZE_STEP: f64 = 2.0;
const SELECTION_ARROW_LENGTH_STEP: f64 = 2.0;
const SELECTION_ARROW_ANGLE_STEP: f64 = 2.0;
const MIN_FONT_SIZE: f64 = 8.0;
const MAX_FONT_SIZE: f64 = 72.0;
const MIN_ARROW_LENGTH: f64 = 5.0;
const MAX_ARROW_LENGTH: f64 = 50.0;
const MIN_ARROW_ANGLE: f64 = 15.0;
const MAX_ARROW_ANGLE: f64 = 60.0;

#[derive(Default)]
struct SelectionApplyResult {
    changed: usize,
    locked: usize,
    applicable: usize,
}

impl InputState {
    pub(super) fn apply_selection_color(&mut self, direction: i32) -> bool {
        let base_color = self.selection_primary_color().unwrap_or(RED);
        let index = color_palette_index(base_color).unwrap_or(0);
        let offset = if direction == 0 { 1 } else { direction };
        let next = cycle_index(index, SELECTION_COLORS.len(), offset);
        let target = SELECTION_COLORS[next].1;

        let result = self.apply_selection_change(
            |shape| {
                matches!(
                    shape,
                    Shape::Freehand { .. }
                        | Shape::Line { .. }
                        | Shape::Rect { .. }
                        | Shape::Ellipse { .. }
                        | Shape::Arrow { .. }
                        | Shape::MarkerStroke { .. }
                        | Shape::Text { .. }
                        | Shape::StickyNote { .. }
                )
            },
            |shape| match shape {
                Shape::Freehand { color, .. }
                | Shape::Line { color, .. }
                | Shape::Rect { color, .. }
                | Shape::Ellipse { color, .. }
                | Shape::Arrow { color, .. }
                | Shape::Text { color, .. } => {
                    if *color != target {
                        *color = target;
                        true
                    } else {
                        false
                    }
                }
                Shape::MarkerStroke { color, .. } => {
                    let new_color = Color {
                        a: color.a,
                        ..target
                    };
                    if *color != new_color {
                        *color = new_color;
                        true
                    } else {
                        false
                    }
                }
                Shape::StickyNote { background, .. } => {
                    if *background != target {
                        *background = target;
                        true
                    } else {
                        false
                    }
                }
                _ => false,
            },
        );

        self.report_selection_apply_result(result, "color")
    }

    pub(super) fn apply_selection_thickness(&mut self, direction: i32) -> bool {
        let delta = SELECTION_THICKNESS_STEP * direction as f64;
        let result = self.apply_selection_change(
            |shape| {
                matches!(
                    shape,
                    Shape::Freehand { .. }
                        | Shape::Line { .. }
                        | Shape::Rect { .. }
                        | Shape::Ellipse { .. }
                        | Shape::Arrow { .. }
                        | Shape::MarkerStroke { .. }
                )
            },
            |shape| match shape {
                Shape::Freehand { thick, .. }
                | Shape::Line { thick, .. }
                | Shape::Rect { thick, .. }
                | Shape::Ellipse { thick, .. }
                | Shape::Arrow { thick, .. }
                | Shape::MarkerStroke { thick, .. } => {
                    let next = (*thick + delta).clamp(MIN_STROKE_THICKNESS, MAX_STROKE_THICKNESS);
                    if (next - *thick).abs() > f64::EPSILON {
                        *thick = next;
                        true
                    } else {
                        false
                    }
                }
                _ => false,
            },
        );

        self.report_selection_apply_result(result, "thickness")
    }

    pub(super) fn apply_selection_font_size(&mut self, direction: i32) -> bool {
        let delta = SELECTION_FONT_SIZE_STEP * direction as f64;
        let result = self.apply_selection_change(
            |shape| matches!(shape, Shape::Text { .. }),
            |shape| match shape {
                Shape::Text { size, .. } => {
                    let next = (*size + delta).clamp(MIN_FONT_SIZE, MAX_FONT_SIZE);
                    if (next - *size).abs() > f64::EPSILON {
                        *size = next;
                        true
                    } else {
                        false
                    }
                }
                _ => false,
            },
        );

        self.report_selection_apply_result(result, "font size")
    }

    pub(super) fn apply_selection_fill(&mut self, direction: i32) -> bool {
        let target = if direction == 0 {
            self.selection_bool_target(|shape| match shape {
                Shape::Rect { fill, .. } | Shape::Ellipse { fill, .. } => Some(*fill),
                _ => None,
            })
        } else {
            Some(direction > 0)
        };

        let Some(target) = target else {
            self.set_ui_toast(UiToastKind::Warning, "No fill shapes selected.");
            return false;
        };

        let result = self.apply_selection_change(
            |shape| matches!(shape, Shape::Rect { .. } | Shape::Ellipse { .. }),
            |shape| match shape {
                Shape::Rect { fill, .. } | Shape::Ellipse { fill, .. } => {
                    if *fill != target {
                        *fill = target;
                        true
                    } else {
                        false
                    }
                }
                _ => false,
            },
        );

        self.report_selection_apply_result(result, "fill")
    }

    pub(super) fn apply_selection_arrow_head(&mut self, direction: i32) -> bool {
        let target = if direction == 0 {
            self.selection_bool_target(|shape| match shape {
                Shape::Arrow { head_at_end, .. } => Some(*head_at_end),
                _ => None,
            })
        } else {
            Some(direction > 0)
        };

        let Some(target) = target else {
            self.set_ui_toast(UiToastKind::Warning, "No arrows selected.");
            return false;
        };

        let result = self.apply_selection_change(
            |shape| matches!(shape, Shape::Arrow { .. }),
            |shape| match shape {
                Shape::Arrow { head_at_end, .. } => {
                    if *head_at_end != target {
                        *head_at_end = target;
                        true
                    } else {
                        false
                    }
                }
                _ => false,
            },
        );

        self.report_selection_apply_result(result, "arrow head")
    }

    pub(super) fn apply_selection_arrow_length(&mut self, direction: i32) -> bool {
        let delta = SELECTION_ARROW_LENGTH_STEP * direction as f64;
        let result = self.apply_selection_change(
            |shape| matches!(shape, Shape::Arrow { .. }),
            |shape| match shape {
                Shape::Arrow { arrow_length, .. } => {
                    let next = (*arrow_length + delta).clamp(MIN_ARROW_LENGTH, MAX_ARROW_LENGTH);
                    if (next - *arrow_length).abs() > f64::EPSILON {
                        *arrow_length = next;
                        true
                    } else {
                        false
                    }
                }
                _ => false,
            },
        );

        self.report_selection_apply_result(result, "arrow length")
    }

    pub(super) fn apply_selection_arrow_angle(&mut self, direction: i32) -> bool {
        let delta = SELECTION_ARROW_ANGLE_STEP * direction as f64;
        let result = self.apply_selection_change(
            |shape| matches!(shape, Shape::Arrow { .. }),
            |shape| match shape {
                Shape::Arrow { arrow_angle, .. } => {
                    let next = (*arrow_angle + delta).clamp(MIN_ARROW_ANGLE, MAX_ARROW_ANGLE);
                    if (next - *arrow_angle).abs() > f64::EPSILON {
                        *arrow_angle = next;
                        true
                    } else {
                        false
                    }
                }
                _ => false,
            },
        );

        self.report_selection_apply_result(result, "arrow angle")
    }

    pub(super) fn apply_selection_text_background(&mut self, direction: i32) -> bool {
        let target = if direction == 0 {
            self.selection_bool_target(|shape| match shape {
                Shape::Text {
                    background_enabled, ..
                } => Some(*background_enabled),
                _ => None,
            })
        } else {
            Some(direction > 0)
        };

        let Some(target) = target else {
            self.set_ui_toast(UiToastKind::Warning, "No text shapes selected.");
            return false;
        };

        let result = self.apply_selection_change(
            |shape| matches!(shape, Shape::Text { .. }),
            |shape| match shape {
                Shape::Text {
                    background_enabled, ..
                } => {
                    if *background_enabled != target {
                        *background_enabled = target;
                        true
                    } else {
                        false
                    }
                }
                _ => false,
            },
        );

        self.report_selection_apply_result(result, "text background")
    }

    fn selection_primary_color(&self) -> Option<Color> {
        let frame = self.canvas_set.active_frame();
        for id in self.selected_shape_ids() {
            let Some(drawn) = frame.shape(*id) else {
                continue;
            };
            if drawn.locked {
                continue;
            }
            if let Some(color) = shape_color(&drawn.shape) {
                return Some(color);
            }
        }
        None
    }

    fn selection_bool_target<F>(&self, mut extract: F) -> Option<bool>
    where
        F: FnMut(&Shape) -> Option<bool>,
    {
        let frame = self.canvas_set.active_frame();
        let mut applicable = 0;
        let mut editable_values = Vec::new();
        for id in self.selected_shape_ids() {
            if let Some(drawn) = frame.shape(*id)
                && let Some(value) = extract(&drawn.shape)
            {
                applicable += 1;
                if !drawn.locked {
                    editable_values.push(value);
                }
            }
        }
        if applicable == 0 {
            return None;
        }
        if editable_values.is_empty() {
            return Some(true);
        }
        let first = editable_values[0];
        let mixed = editable_values.iter().any(|v| *v != first);
        if mixed { Some(true) } else { Some(!first) }
    }

    fn apply_selection_change<A, F>(
        &mut self,
        mut applicable: A,
        mut apply: F,
    ) -> SelectionApplyResult
    where
        A: FnMut(&Shape) -> bool,
        F: FnMut(&mut Shape) -> bool,
    {
        let ids: Vec<ShapeId> = self.selected_shape_ids().to_vec();
        if ids.is_empty() {
            return SelectionApplyResult::default();
        }

        let mut result = SelectionApplyResult::default();
        let mut actions = Vec::new();
        let mut dirty_regions = Vec::new();

        {
            let frame = self.canvas_set.active_frame_mut();
            for id in ids {
                let Some(drawn) = frame.shape_mut(id) else {
                    continue;
                };
                if !applicable(&drawn.shape) {
                    continue;
                }
                result.applicable += 1;
                if drawn.locked {
                    result.locked += 1;
                    continue;
                }

                let before_bounds = drawn.shape.bounding_box();
                let before_snapshot = crate::draw::frame::ShapeSnapshot {
                    shape: drawn.shape.clone(),
                    locked: drawn.locked,
                };

                let changed = apply(&mut drawn.shape);
                if !changed {
                    continue;
                }

                let after_bounds = drawn.shape.bounding_box();
                let after_snapshot = crate::draw::frame::ShapeSnapshot {
                    shape: drawn.shape.clone(),
                    locked: drawn.locked,
                };

                actions.push(crate::draw::frame::UndoAction::Modify {
                    shape_id: drawn.id,
                    before: before_snapshot,
                    after: after_snapshot,
                });
                dirty_regions.push((drawn.id, before_bounds, after_bounds));
                result.changed += 1;
            }
        }

        if actions.is_empty() {
            return result;
        }

        let undo_action = if actions.len() == 1 {
            actions.into_iter().next().unwrap()
        } else {
            crate::draw::frame::UndoAction::Compound(actions)
        };

        self.canvas_set
            .active_frame_mut()
            .push_undo_action(undo_action, self.undo_stack_limit);

        for (shape_id, before, after) in dirty_regions {
            self.mark_selection_dirty_region(before);
            self.mark_selection_dirty_region(after);
            self.invalidate_hit_cache_for(shape_id);
        }
        self.needs_redraw = true;

        result
    }

    fn report_selection_apply_result(&mut self, result: SelectionApplyResult, label: &str) -> bool {
        if result.applicable == 0 {
            self.set_ui_toast(
                UiToastKind::Warning,
                format!("No {label} to edit in selection."),
            );
            return false;
        }

        if result.changed == 0 {
            if result.locked == result.applicable {
                self.set_ui_toast(
                    UiToastKind::Warning,
                    format!("All {label} shapes are locked."),
                );
            } else {
                self.set_ui_toast(UiToastKind::Info, "No changes applied.");
            }
            return false;
        }

        if result.locked > 0 {
            self.set_ui_toast(
                UiToastKind::Warning,
                format!("{} locked shape(s) unchanged.", result.locked),
            );
        }
        true
    }
}
