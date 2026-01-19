use super::super::base::InputState;
use crate::draw::frame::{ShapeSnapshot, UndoAction};
use crate::util::Rect;

const SELECTION_HALO_PADDING: i32 = 6;

impl InputState {
    pub(crate) fn set_selection_locked(&mut self, locked: bool) -> bool {
        let ids_len = self.selected_shape_ids().len();
        if ids_len == 0 {
            return false;
        }

        let mut actions = Vec::new();
        for idx in 0..ids_len {
            let id = self.selected_shape_ids()[idx];
            let result = {
                let frame = self.boards.active_frame_mut();
                if let Some(shape) = frame.shape_mut(id) {
                    if shape.locked == locked {
                        None
                    } else {
                        let before = ShapeSnapshot {
                            shape: shape.shape.clone(),
                            locked: !locked,
                        };
                        shape.locked = locked;
                        let after = ShapeSnapshot {
                            shape: shape.shape.clone(),
                            locked,
                        };
                        Some((before, after, shape.shape.clone()))
                    }
                } else {
                    None
                }
            };

            if let Some((before, after, shape_for_dirty)) = result {
                actions.push(UndoAction::Modify {
                    shape_id: id,
                    before,
                    after,
                });
                self.dirty_tracker.mark_shape(&shape_for_dirty);
                self.invalidate_hit_cache_for(id);
            }
        }

        if actions.is_empty() {
            return false;
        }

        self.boards
            .active_frame_mut()
            .push_undo_action(UndoAction::Compound(actions), self.undo_stack_limit);
        true
    }

    pub(crate) fn clear_all(&mut self) -> bool {
        let removed = {
            let frame = self.boards.active_frame();
            if frame.shapes.is_empty() {
                return false;
            }
            frame
                .shapes
                .iter()
                .cloned()
                .enumerate()
                .filter(|(_, shape)| !shape.locked)
                .collect::<Vec<_>>()
        };
        if removed.is_empty() {
            return false;
        }

        {
            let frame = self.boards.active_frame_mut();
            for (index, _) in removed.iter().rev() {
                frame.shapes.remove(*index);
            }
            frame.push_undo_action(
                UndoAction::Delete { shapes: removed },
                self.undo_stack_limit,
            );
        }
        self.invalidate_hit_cache();
        self.clear_selection();
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
        true
    }

    pub(crate) fn mark_selection_dirty_region(&mut self, rect: Option<Rect>) {
        if self.is_properties_panel_open() {
            self.properties_panel_needs_refresh = true;
        }
        if let Some(rect) = rect {
            if let Some(inflated) = rect.inflated(SELECTION_HALO_PADDING) {
                self.dirty_tracker.mark_rect(inflated);
            } else {
                self.dirty_tracker.mark_rect(rect);
            }
        }
    }
}
