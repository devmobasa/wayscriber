use super::super::super::base::InputState;
use crate::draw::frame::UndoAction;

const DUPLICATE_OFFSET: i32 = 12;

#[allow(dead_code)]
impl InputState {
    pub(crate) fn duplicate_selection(&mut self) -> bool {
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
            Self::translate_shape(&mut cloned_shape, DUPLICATE_OFFSET, DUPLICATE_OFFSET);
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
                self.mark_selection_dirty_region(stored.bounding_box());
                self.invalidate_hit_cache_for(new_id);
                created.push((index, stored));
                new_ids.push(new_id);
            }
        }

        if created.is_empty() {
            return false;
        }

        self.boards.active_frame_mut().push_undo_action(
            UndoAction::Create { shapes: created },
            self.undo_stack_limit,
        );
        self.mark_session_dirty();
        self.needs_redraw = true;
        self.set_selection(new_ids);
        true
    }
}
