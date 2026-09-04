use super::super::super::base::InputState;
use crate::draw::frame::UndoAction;
use crate::draw::{TextMeasurer, with_legacy_measurer};

const DUPLICATE_OFFSET: i32 = 12;

#[allow(dead_code)]
impl InputState {
    pub(crate) fn duplicate_selection(&mut self) -> bool {
        with_legacy_measurer(|measurer| self.duplicate_selection_with(measurer))
    }

    pub(crate) fn duplicate_selection_with(&mut self, measurer: &TextMeasurer) -> bool {
        let ids_len = self.selected_shape_ids().len();
        if ids_len == 0 {
            return false;
        }

        let mut created = Vec::new();
        let mut new_ids = Vec::new();
        for idx in 0..ids_len {
            let id = self.selected_shape_ids()[idx];
            let original = {
                let frame = self.boards.active_frame();
                frame.shape(id).cloned()
            };
            let Some(shape) = original else {
                continue;
            };
            if shape.locked {
                continue;
            }

            let mut cloned_shape = shape.shape.clone();
            cloned_shape.translate(DUPLICATE_OFFSET, DUPLICATE_OFFSET);
            let new_id = {
                let frame = self.boards.active_frame_mut();
                frame.add_shape(cloned_shape)
            };

            if let Some((index, stored)) = {
                let frame = self.boards.active_frame();
                frame
                    .find_index(new_id)
                    .and_then(|idx| frame.shape(new_id).map(|s| (idx, s.clone())))
            } {
                self.mark_selection_dirty_region(stored.bounding_box_with(measurer));
                self.invalidate_hit_cache_for_with(measurer, new_id);
                created.push((index, stored));
                new_ids.push(new_id);
            }
        }

        if created.is_empty() {
            return false;
        }

        self.boards.active_frame_mut().push_undo_action(
            UndoAction::Create { shapes: created },
            self.history_limits.undo_stack_limit(),
        );
        self.mark_session_dirty();
        self.needs_redraw = true;
        self.set_selection(new_ids);
        true
    }
}
