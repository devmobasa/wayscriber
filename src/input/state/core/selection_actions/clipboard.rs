use super::super::base::{InputState, UiToastKind};
use crate::draw::frame::UndoAction;

const COPY_PASTE_OFFSET: i32 = 12;

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
            Self::translate_shape(&mut cloned_shape, COPY_PASTE_OFFSET, COPY_PASTE_OFFSET);
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
                self.mark_selection_dirty_region(stored.shape.bounding_box());
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

    pub(crate) fn copy_selection(&mut self) -> usize {
        let copied = {
            let ids = self.selected_shape_ids();
            if ids.is_empty() {
                return 0;
            }

            let frame = self.boards.active_frame();
            let mut copied = Vec::new();
            for id in ids {
                if let Some(shape) = frame.shape(*id) {
                    if shape.locked {
                        continue;
                    }
                    copied.push(shape.shape.clone());
                }
            }
            copied
        };

        if copied.is_empty() {
            return 0;
        }

        let count = copied.len();
        self.selection_clipboard = Some(copied);
        self.clipboard_paste_offset = 0;
        count
    }

    pub(crate) fn selection_clipboard_is_empty(&self) -> bool {
        self.selection_clipboard
            .as_ref()
            .is_none_or(|clipboard| clipboard.is_empty())
    }

    pub(crate) fn paste_selection(&mut self) -> usize {
        let Some(shapes) = self.selection_clipboard.clone() else {
            return 0;
        };
        if shapes.is_empty() {
            return 0;
        }

        let total = shapes.len();
        let offset = self
            .clipboard_paste_offset
            .saturating_add(COPY_PASTE_OFFSET);
        let mut created = Vec::new();
        let mut new_ids = Vec::new();
        let mut limit_hit = false;

        for shape in shapes {
            let mut cloned_shape = shape;
            Self::translate_shape(&mut cloned_shape, offset, offset);
            let new_id = {
                let frame = self.boards.active_frame_mut();
                frame.try_add_shape_with_id(cloned_shape, self.max_shapes_per_frame)
            };

            let Some(new_id) = new_id else {
                limit_hit = true;
                break;
            };

            if let Some((index, stored)) = {
                let frame = self.boards.active_frame();
                frame
                    .find_index(new_id)
                    .and_then(|idx| frame.shape(new_id).map(|s| (idx, s.clone())))
            } {
                self.mark_selection_dirty_region(stored.shape.bounding_box());
                self.invalidate_hit_cache_for(new_id);
                created.push((index, stored));
                new_ids.push(new_id);
            }
        }

        if created.is_empty() {
            if limit_hit {
                self.set_ui_toast(UiToastKind::Warning, "Shape limit reached; nothing pasted.");
            }
            return 0;
        }

        let created_len = created.len();
        self.boards.active_frame_mut().push_undo_action(
            UndoAction::Create { shapes: created },
            self.undo_stack_limit,
        );
        self.mark_session_dirty();
        self.needs_redraw = true;
        self.set_selection(new_ids);
        self.clipboard_paste_offset = offset;
        if limit_hit {
            self.set_ui_toast(
                UiToastKind::Warning,
                format!("Shape limit reached; pasted {created_len} of {total}."),
            );
        }
        created_len
    }
}
