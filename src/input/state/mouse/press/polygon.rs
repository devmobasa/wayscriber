use crate::draw::Shape;
use crate::draw::frame::UndoAction;
use crate::draw::shape::{PolygonKind, has_minimum_distinct_points};
use crate::input::Tool;
use crate::input::state::{Toast, ToastPriority};
use std::time::Instant;

use super::super::super::core::PolygonClickState;
use super::super::super::{DrawingState, InputState};
use super::super::{TEXT_DOUBLE_CLICK_DISTANCE, TEXT_DOUBLE_CLICK_MS};

impl InputState {
    pub(crate) fn start_building_polygon(&mut self, x: i32, y: i32) {
        self.sync_current_settings_for_tool(Tool::FreeformPolygon);
        let color = self.color_for_tool(Tool::FreeformPolygon);
        let thick = self.thickness_for_tool(Tool::FreeformPolygon);
        self.clear_selection();
        self.last_polygon_click = Some(PolygonClickState {
            x,
            y,
            at: Instant::now(),
        });
        self.state = DrawingState::BuildingPolygon {
            points: vec![(x, y)],
            preview: None,
            fill: self.fill_enabled,
            color,
            thick,
        };
        self.last_provisional_bounds = None;
        self.update_provisional_dirty(x, y);
        self.push_toast(
            ToastPriority::Info,
            "draw.polygon",
            Toast::info("Click points. Enter/double-click to finish. Backspace undo. Esc cancel."),
        );
        self.needs_redraw = true;
    }

    fn should_finish_building_polygon_on_click(&self, x: i32, y: i32) -> bool {
        let Some(last) = self.last_polygon_click else {
            return false;
        };
        if Instant::now().duration_since(last.at).as_millis() > TEXT_DOUBLE_CLICK_MS as u128 {
            return false;
        }
        if (x - last.x).abs() > TEXT_DOUBLE_CLICK_DISTANCE
            || (y - last.y).abs() > TEXT_DOUBLE_CLICK_DISTANCE
        {
            return false;
        }
        let DrawingState::BuildingPolygon { points, .. } = &self.state else {
            return false;
        };
        has_minimum_distinct_points(points)
    }

    pub(crate) fn handle_building_polygon_left_click(&mut self, x: i32, y: i32) {
        if self.should_finish_building_polygon_on_click(x, y) {
            self.finish_building_polygon();
        } else {
            self.append_building_polygon_point(x, y);
        }
    }

    pub(crate) fn append_building_polygon_point(&mut self, x: i32, y: i32) {
        let DrawingState::BuildingPolygon {
            points, preview, ..
        } = &mut self.state
        else {
            return;
        };
        points.push((x, y));
        *preview = None;
        self.last_polygon_click = Some(PolygonClickState {
            x,
            y,
            at: Instant::now(),
        });
        self.update_provisional_dirty(x, y);
        self.needs_redraw = true;
    }

    pub(crate) fn pop_building_polygon_point(&mut self) {
        let DrawingState::BuildingPolygon { points, .. } = &mut self.state else {
            return;
        };
        let _ = points.pop();
        if points.is_empty() {
            self.clear_provisional_dirty();
            self.last_polygon_click = None;
            self.state = DrawingState::Idle;
        } else {
            let (x, y) = self.canvas_pointer_position();
            self.last_polygon_click = None;
            self.update_provisional_dirty(x, y);
        }
        self.needs_redraw = true;
    }

    pub(crate) fn finish_building_polygon(&mut self) {
        let state = std::mem::replace(&mut self.state, DrawingState::Idle);
        let DrawingState::BuildingPolygon {
            points,
            fill,
            color,
            thick,
            ..
        } = state
        else {
            self.state = state;
            return;
        };

        self.clear_provisional_dirty();
        self.last_polygon_click = None;
        if !has_minimum_distinct_points(&points) {
            self.needs_redraw = true;
            return;
        }

        let shape = Shape::Polygon {
            kind: PolygonKind::Freeform,
            points,
            fill,
            color,
            thick,
        };
        let bounds = shape.bounding_box();
        let addition = {
            let frame = self.boards.active_frame_mut();
            frame
                .try_add_shape_with_id(shape, self.max_shapes_per_frame)
                .and_then(|new_id| {
                    let index = frame.find_index(new_id)?;
                    let snapshot = frame.shape(new_id)?.clone();
                    frame.push_undo_action(
                        UndoAction::Create {
                            shapes: vec![(index, snapshot.clone())],
                        },
                        self.undo_stack_limit,
                    );
                    Some((new_id, snapshot))
                })
        };
        if let Some((new_id, _snapshot)) = addition {
            self.invalidate_hit_cache_for(new_id);
            self.dirty_tracker.mark_optional_rect(bounds);
            self.mark_session_dirty();
            self.record_first_stroke_done_for_onboarding();
        }
        self.needs_redraw = true;
    }
}
