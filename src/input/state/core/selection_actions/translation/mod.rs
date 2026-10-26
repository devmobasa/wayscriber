use crate::draw::frame::ShapeSnapshot;
use crate::draw::{ShapeId, TextMeasurer};
use crate::input::InputState;

use bounds::SelectionExtent;

mod bounds;
mod restore;
mod undo;

/// Steps an edge action may take. The delta from a selection edge to a
/// visible edge spans at most three saturated `i32` steps, plus one if a
/// limit shortened a step.
const MAX_ALIGNMENT_STEPS: usize = 4;

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
        if self.view.screen_width() == 0 {
            return false;
        }

        // Align with the current view, which panning and zoom move away from
        // the canvas origin.
        let visible = self.visible_canvas_rect();
        let start = i64::from(visible.x);
        let end = start + i64::from(visible.width);
        self.align_selection_with_undo_with(measurer, |extent| {
            let dx = if to_start {
                start - extent.min_x
            } else {
                end - extent.max_x
            };
            (dx, 0)
        })
    }

    pub(crate) fn move_selection_to_vertical_edge_with(
        &mut self,
        measurer: &TextMeasurer,
        to_start: bool,
    ) -> bool {
        if self.view.screen_height() == 0 {
            return false;
        }

        let visible = self.visible_canvas_rect();
        let start = i64::from(visible.y);
        let end = start + i64::from(visible.height);
        self.align_selection_with_undo_with(measurer, |extent| {
            let dy = if to_start {
                start - extent.min_y
            } else {
                end - extent.max_y
            };
            (0, dy)
        })
    }

    /// Moves the selection by the delta that `delta_for` returns for its
    /// current painted bounds, as one undo step.
    ///
    /// A delta can exceed the `i32` range, so it is applied in steps, and
    /// the bounds are measured again after each one. Each step is limited
    /// like any other move, and alignment stops when nothing is left to move
    /// or a step is fully blocked.
    fn align_selection_with_undo_with(
        &mut self,
        measurer: &TextMeasurer,
        delta_for: impl Fn(SelectionExtent) -> (i64, i64),
    ) -> bool {
        let before = self.capture_movable_selection_snapshots();
        if before.is_empty() {
            return false;
        }

        let mut moved = false;
        for _ in 0..MAX_ALIGNMENT_STEPS {
            let Some(extent) = self.movable_selection_extent(measurer) else {
                break;
            };
            let (dx, dy) = delta_for(extent);
            if (dx, dy) == (0, 0)
                || !self.apply_translation_to_selection_with(
                    measurer,
                    bounds::saturating_delta(dx),
                    bounds::saturating_delta(dy),
                )
            {
                break;
            }
            moved = true;
        }

        if moved {
            self.push_translation_undo(measurer, before);
        }
        moved
    }
}
