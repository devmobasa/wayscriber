use crate::draw::frame::ShapeSnapshot;
use crate::draw::{ShapeId, TextMeasurer};
use crate::input::InputState;

mod bounds;
mod restore;
mod undo;

impl InputState {
    pub(crate) fn capture_movable_selection_snapshots(&self) -> Vec<(ShapeId, ShapeSnapshot)> {
        crate::input::state::core::editing::CanvasEdit::capture(
            self.boards.active_frame(),
            self.selected_shape_ids(),
        )
        .into_snapshots()
    }

    pub(crate) fn apply_translation_to_selection_with(
        &mut self,
        measurer: &TextMeasurer,
        dx: i32,
        dy: i32,
    ) -> bool {
        if dx == 0 && dy == 0 {
            return false;
        }
        let (dx, dy) = match self.clamp_selection_translation(measurer, dx, dy) {
            Some((dx, dy)) => (dx, dy),
            None => return false,
        };
        if dx == 0 && dy == 0 {
            return false;
        }
        let ids_len = self.selected_shape_ids().len();
        if ids_len == 0 {
            return false;
        }

        let ids = self.selected_shape_ids().to_vec();
        let effects = crate::input::state::core::editing::CanvasEdit::preview_current(
            self.boards.active_frame_mut(),
            &ids,
            measurer,
            |shape| {
                shape.translate(dx, dy);
                true
            },
        );
        self.apply_edit_effects(measurer, effects)
    }

    pub(crate) fn translate_selection_with_undo_with(
        &mut self,
        measurer: &TextMeasurer,
        dx: i32,
        dy: i32,
    ) -> bool {
        if dx == 0 && dy == 0 {
            return false;
        }
        let before = self.capture_movable_selection_snapshots();
        if before.is_empty() {
            return false;
        }
        if !self.apply_translation_to_selection_with(measurer, dx, dy) {
            return false;
        }
        self.push_translation_undo(measurer, before);
        true
    }

    pub(crate) fn move_selection_to_horizontal_edge_with(
        &mut self,
        measurer: &TextMeasurer,
        to_start: bool,
    ) -> bool {
        let Some(bounds) = self.movable_selection_bounds(measurer) else {
            return false;
        };
        if self.view.screen_width() == 0 {
            return false;
        }

        let visible = self.visible_canvas_rect();
        let target_x = if to_start {
            visible.x
        } else {
            visible
                .x
                .saturating_add(visible.width)
                .saturating_sub(bounds.width)
        };
        let dx = target_x.saturating_sub(bounds.x);
        if dx == 0 {
            return false;
        }
        self.translate_selection_with_undo_with(measurer, dx, 0)
    }

    pub(crate) fn move_selection_to_vertical_edge_with(
        &mut self,
        measurer: &TextMeasurer,
        to_start: bool,
    ) -> bool {
        let Some(bounds) = self.movable_selection_bounds(measurer) else {
            return false;
        };
        if self.view.screen_height() == 0 {
            return false;
        }

        let visible = self.visible_canvas_rect();
        let target_y = if to_start {
            visible.y
        } else {
            visible
                .y
                .saturating_add(visible.height)
                .saturating_sub(bounds.height)
        };
        let dy = target_y.saturating_sub(bounds.y);
        if dy == 0 {
            return false;
        }
        self.translate_selection_with_undo_with(measurer, 0, dy)
    }
}
