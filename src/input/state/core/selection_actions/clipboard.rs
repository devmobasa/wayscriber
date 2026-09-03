use super::super::base::{ClipboardFingerprint, ClipboardPasteRequest, InputState, PasteAnchor};
use super::super::selection::LocalSelectionContext;
use crate::draw::Shape;
use crate::draw::frame::UndoAction;
use crate::input::state::{Toast, ToastPriority};
use crate::util::Rect;

mod duplicate;
mod image_paste;

#[allow(dead_code)]
impl InputState {
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
        let pending_publish = self.selection_clipboard.copy_shapes(copied);
        self.replace_selection_clipboard_publish(pending_publish);
        count
    }

    pub(crate) fn paste_selection(&mut self) -> usize {
        let Some(shapes) = self.selection_clipboard.shapes() else {
            return 0;
        };
        if shapes.is_empty() {
            return 0;
        }

        let total = shapes.len();
        let (dx, dy) = shape_paste_translation(&shapes, self.paste_anchor());
        let mut created = Vec::new();
        let mut new_ids = Vec::new();
        let mut limit_hit = false;

        let max_shapes = self.max_shapes_per_frame();
        for shape in shapes {
            let mut cloned_shape = shape;
            cloned_shape.translate(dx, dy);
            let new_id = {
                let frame = self.boards.active_frame_mut();
                frame.try_add_shape_with_id(cloned_shape, max_shapes)
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
                self.mark_selection_dirty_region(stored.bounding_box());
                self.invalidate_hit_cache_for(new_id);
                created.push((index, stored));
                new_ids.push(new_id);
            }
        }

        if created.is_empty() {
            if limit_hit {
                self.push_toast(
                    ToastPriority::Info,
                    "selection.clipboard",
                    Toast::warning("Shape limit reached; nothing pasted."),
                );
            }
            return 0;
        }

        let created_len = created.len();
        self.boards.active_frame_mut().push_undo_action(
            UndoAction::Create { shapes: created },
            self.history_limits.undo_stack_limit(),
        );
        self.mark_session_dirty();
        self.needs_redraw = true;
        self.set_selection(new_ids);
        if limit_hit {
            self.push_toast(
                ToastPriority::Info,
                "selection.clipboard",
                Toast::warning(format!(
                    "Shape limit reached; pasted {created_len} of {total}."
                )),
            );
        }
        created_len
    }

    pub(crate) fn request_clipboard_paste(&mut self) -> ClipboardPasteRequest {
        self.request_clipboard_paste_at_anchor(self.paste_anchor())
    }

    pub(crate) fn request_clipboard_paste_at_anchor(
        &mut self,
        anchor: PasteAnchor,
    ) -> ClipboardPasteRequest {
        let request = self.selection_clipboard.begin_paste_request(
            self.boards.active_board_id().to_string(),
            self.boards.active_page_index(),
            self.boards.active_page_generation(),
            anchor,
            self.visible_canvas_rect(),
            self.view.screen_size(),
        );
        self.emit_input_effect(super::super::base::InputEffect::ClipboardPaste(
            request.clone(),
        ));
        request
    }

    pub(crate) fn selection_clipboard_snapshot(&self) -> LocalSelectionContext {
        self.selection_clipboard.snapshot()
    }

    pub(crate) fn failed_local_selection_after_fingerprint_probe(
        &mut self,
        request_generation: Option<u64>,
        current: Option<ClipboardFingerprint>,
    ) -> Option<Vec<Shape>> {
        self.selection_clipboard
            .failed_after_fingerprint_probe(request_generation, current)
    }

    pub(crate) fn mark_selection_clipboard_superseded(&mut self) {
        let generation = self.selection_clipboard.generation();
        self.selection_clipboard.mark_superseded(Some(generation));
    }

    pub(crate) fn mark_selection_clipboard_superseded_for_generation(
        &mut self,
        generation: Option<u64>,
    ) {
        self.selection_clipboard.mark_superseded(generation);
    }

    pub(crate) fn paste_clipboard_shapes_from_request(
        &mut self,
        request: &ClipboardPasteRequest,
        shapes: Vec<Shape>,
    ) -> usize {
        if shapes.is_empty() {
            return 0;
        }
        if self.selection_clipboard.active_paste_request_id() != Some(request.id) {
            return 0;
        }

        let (dx, dy) = shape_paste_translation(&shapes, request.anchor);
        let target_active = self.clipboard_request_targets_active_page(request);
        let mut created = Vec::new();
        let mut new_ids = Vec::new();
        let mut dirty_bounds = Vec::new();
        let mut hit_ids = Vec::new();
        let mut limit_hit = false;
        let total = shapes.len();
        let max_shapes = self.max_shapes_per_frame();
        let undo_limit = self.history_limits.undo_stack_limit();

        let target = self
            .boards
            .board_state_by_id_mut(&request.target_board_id)
            .filter(|board| board.pages.generation() == request.target_page_generation)
            .and_then(|board| board.pages.frame_mut(request.target_page_index));

        let Some(frame) = target else {
            self.push_toast(
                ToastPriority::Info,
                "selection.clipboard",
                Toast::warning("Paste target changed; clipboard paste was cancelled."),
            );
            self.trigger_blocked_feedback();
            return 0;
        };

        for shape in shapes {
            let mut cloned_shape = shape;
            cloned_shape.translate(dx, dy);
            let Some(new_id) = frame.try_add_shape_with_id(cloned_shape, max_shapes) else {
                limit_hit = true;
                break;
            };

            if let Some(index) = frame.find_index(new_id)
                && let Some(stored) = frame.shape(new_id).cloned()
            {
                dirty_bounds.push(stored.bounding_box());
                hit_ids.push(new_id);
                created.push((index, stored));
                new_ids.push(new_id);
            }
        }

        if created.is_empty() {
            if limit_hit {
                self.push_toast(
                    ToastPriority::Info,
                    "selection.clipboard",
                    Toast::warning("Shape limit reached; nothing pasted."),
                );
            }
            return 0;
        }

        let created_len = created.len();
        frame.push_undo_action(UndoAction::Create { shapes: created }, undo_limit);
        self.mark_session_dirty();
        if target_active {
            for bounds in dirty_bounds {
                self.mark_selection_dirty_region(bounds);
            }
            for shape_id in hit_ids {
                self.invalidate_hit_cache_for(shape_id);
            }
            self.set_selection(new_ids);
            self.needs_redraw = true;
        }
        if limit_hit {
            self.push_toast(
                ToastPriority::Info,
                "selection.clipboard",
                Toast::warning(format!(
                    "Shape limit reached; pasted {created_len} of {total}."
                )),
            );
        }
        created_len
    }
}

fn shape_paste_translation(shapes: &[Shape], anchor: PasteAnchor) -> (i32, i32) {
    let Some(bounds) = shapes_bounding_box(shapes) else {
        return (0, 0);
    };
    let (anchor_x, anchor_y) = anchor.point();
    let center_x = bounds.x.saturating_add(bounds.width / 2);
    let center_y = bounds.y.saturating_add(bounds.height / 2);
    (
        anchor_x.saturating_sub(center_x),
        anchor_y.saturating_sub(center_y),
    )
}

fn shapes_bounding_box(shapes: &[Shape]) -> Option<Rect> {
    let mut min_x = i32::MAX;
    let mut min_y = i32::MAX;
    let mut max_x = i32::MIN;
    let mut max_y = i32::MIN;
    let mut found = false;

    for shape in shapes {
        if let Some(bounds) = shape.bounding_box() {
            min_x = min_x.min(bounds.x);
            min_y = min_y.min(bounds.y);
            max_x = max_x.max(bounds.x + bounds.width);
            max_y = max_y.max(bounds.y + bounds.height);
            found = true;
        }
    }

    found
        .then(|| Rect::from_min_max(min_x, min_y, max_x, max_y))
        .flatten()
}
