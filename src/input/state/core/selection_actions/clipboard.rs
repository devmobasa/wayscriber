use super::super::base::{
    ClipboardFingerprint, ClipboardPasteRequest, InputState, PasteAnchor,
    PendingSelectionClipboardPublish, SelectionPublishState, WayscriberClipboardSelection,
};
use crate::draw::Shape;
use crate::draw::frame::UndoAction;
use crate::input::state::{Toast, ToastPriority};
use crate::util::Rect;

const PRIVATE_CLIPBOARD_SCHEMA_VERSION: u32 = 1;

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
        self.selection_clipboard_generation = self.selection_clipboard_generation.wrapping_add(1);
        self.selection_publish_state = SelectionPublishState::NotAttempted;
        self.selection_clipboard = Some(copied.clone());
        self.pending_selection_clipboard_publish = self
            .selection_clipboard_payload(copied)
            .and_then(|payload| {
                serde_json::to_string(&payload).ok().map(|payload_json| {
                    PendingSelectionClipboardPublish {
                        generation: payload.copy_generation,
                        payload_json,
                    }
                })
            });
        count
    }

    fn selection_clipboard_payload(
        &self,
        shapes: Vec<Shape>,
    ) -> Option<WayscriberClipboardSelection> {
        if shapes.is_empty() {
            return None;
        }
        Some(WayscriberClipboardSelection {
            schema_version: PRIVATE_CLIPBOARD_SCHEMA_VERSION,
            app_version: env!("CARGO_PKG_VERSION").to_string(),
            app_instance_id: self.clipboard_app_instance_id.clone(),
            copy_generation: self.selection_clipboard_generation,
            shapes,
        })
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
        let (dx, dy) = shape_paste_translation(&shapes, self.paste_anchor());
        let mut created = Vec::new();
        let mut new_ids = Vec::new();
        let mut limit_hit = false;

        for shape in shapes {
            let mut cloned_shape = shape;
            Self::translate_shape(&mut cloned_shape, dx, dy);
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
            self.undo_stack_limit,
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
        self.clipboard_paste_request_counter = self.clipboard_paste_request_counter.wrapping_add(1);
        let id = self.clipboard_paste_request_counter;
        let request = ClipboardPasteRequest {
            id,
            target_board_id: self.boards.active_board_id().to_string(),
            target_page_index: self.boards.active_page_index(),
            target_page_generation: self.boards.active_page_generation(),
            anchor,
            visible_canvas_rect: self.visible_canvas_rect(),
            screen_size: (self.screen_width, self.screen_height),
            selection_clipboard_generation_at_request: self.selection_clipboard_generation,
            local_selection_fallback_generation: self.local_selection_fallback_generation(),
        };
        self.active_clipboard_paste_request_id = Some(id);
        self.pending_clipboard_paste_request = Some(request.clone());
        request
    }

    pub(crate) fn local_selection_fallback_generation(&self) -> Option<u64> {
        self.local_selection_fallback_allowed()
            .then_some(self.selection_clipboard_generation)
    }

    pub(crate) fn local_selection_fallback_allowed(&self) -> bool {
        if self.selection_clipboard_is_empty() {
            return false;
        }
        match self.selection_publish_state {
            SelectionPublishState::NotAttempted => true,
            SelectionPublishState::Failed { generation, .. } => {
                generation == self.selection_clipboard_generation
            }
            SelectionPublishState::Published { generation } => {
                generation == self.selection_clipboard_generation
            }
            SelectionPublishState::Superseded { .. } => false,
        }
    }

    pub(crate) fn local_selection_shapes_for_fallback(
        &self,
        generation: u64,
    ) -> Option<Vec<Shape>> {
        (generation == self.selection_clipboard_generation
            && self.local_selection_fallback_allowed())
        .then(|| self.selection_clipboard.clone())
        .flatten()
        .filter(|shapes| !shapes.is_empty())
    }

    pub(crate) fn local_selection_shapes_for_pending_publish(
        &self,
        generation: Option<u64>,
    ) -> Option<Vec<Shape>> {
        let generation = generation?;
        (generation == self.selection_clipboard_generation
            && matches!(
                self.selection_publish_state,
                SelectionPublishState::NotAttempted
            ))
        .then(|| self.selection_clipboard.clone())
        .flatten()
        .filter(|shapes| !shapes.is_empty())
    }

    pub(crate) fn has_failed_local_selection_for_generation(
        &self,
        generation: Option<u64>,
    ) -> bool {
        matches!(
            (generation, &self.selection_publish_state),
            (
                Some(request_generation),
                SelectionPublishState::Failed {
                    generation: failed_generation,
                    ..
                },
            ) if *failed_generation == request_generation
                    && *failed_generation == self.selection_clipboard_generation
                    && !self.selection_clipboard_is_empty()
        )
    }

    pub(crate) fn failed_local_selection_probe_for_generation(
        &self,
        generation: Option<u64>,
    ) -> Option<(u64, Option<ClipboardFingerprint>)> {
        let request_generation = generation?;
        let SelectionPublishState::Failed {
            generation,
            clipboard_fingerprint_at_failure,
        } = &self.selection_publish_state
        else {
            return None;
        };
        (*generation == request_generation
            && *generation == self.selection_clipboard_generation
            && !self.selection_clipboard_is_empty())
        .then(|| (*generation, clipboard_fingerprint_at_failure.clone()))
    }

    pub(crate) fn failed_local_selection_after_fingerprint_probe(
        &mut self,
        request_generation: Option<u64>,
        current: Option<ClipboardFingerprint>,
    ) -> Option<Vec<Shape>> {
        let request_generation = request_generation?;
        let SelectionPublishState::Failed {
            generation,
            clipboard_fingerprint_at_failure,
        } = &self.selection_publish_state
        else {
            return None;
        };
        if *generation != request_generation || *generation != self.selection_clipboard_generation {
            return None;
        }

        match (clipboard_fingerprint_at_failure.as_ref(), current.as_ref()) {
            (Some(previous), Some(current)) if previous == current => {}
            (None, None) => return None,
            _ => {
                self.mark_selection_clipboard_superseded_for_generation(Some(*generation));
                return None;
            }
        }

        self.selection_clipboard
            .clone()
            .filter(|shapes| !shapes.is_empty())
    }

    pub(crate) fn mark_selection_clipboard_superseded(&mut self) {
        self.mark_selection_clipboard_superseded_for_generation(Some(
            self.selection_clipboard_generation,
        ));
    }

    pub(crate) fn mark_selection_clipboard_superseded_for_generation(
        &mut self,
        generation: Option<u64>,
    ) {
        if generation == Some(self.selection_clipboard_generation)
            && !self.selection_clipboard_is_empty()
        {
            self.selection_publish_state = SelectionPublishState::Superseded {
                generation: self.selection_clipboard_generation,
            };
        }
    }

    pub(crate) fn private_payload_matches_request_selection(
        &self,
        request: &ClipboardPasteRequest,
        payload: &WayscriberClipboardSelection,
    ) -> bool {
        payload.app_instance_id == self.clipboard_app_instance_id
            && request.local_selection_fallback_generation == Some(payload.copy_generation)
    }

    pub(crate) fn private_payload_is_same_instance(
        &self,
        payload: &WayscriberClipboardSelection,
    ) -> bool {
        payload.app_instance_id == self.clipboard_app_instance_id
    }

    pub(crate) fn private_payload_shapes_for_request(
        &self,
        request: &ClipboardPasteRequest,
        payload: WayscriberClipboardSelection,
    ) -> Option<Vec<Shape>> {
        if payload.app_instance_id == self.clipboard_app_instance_id {
            if request.local_selection_fallback_generation == Some(payload.copy_generation) {
                if self.selection_clipboard_generation == payload.copy_generation
                    && let Some(shapes) = &self.selection_clipboard
                    && !shapes.is_empty()
                {
                    return Some(shapes.clone());
                }
                return non_empty_shapes(payload.shapes);
            }

            if request.local_selection_fallback_generation.is_none()
                && payload.copy_generation == request.selection_clipboard_generation_at_request
            {
                return non_empty_shapes(payload.shapes);
            }

            return None;
        }

        non_empty_shapes(payload.shapes)
    }

    pub(crate) fn paste_clipboard_shapes_from_request(
        &mut self,
        request: &ClipboardPasteRequest,
        shapes: Vec<Shape>,
    ) -> usize {
        if shapes.is_empty() {
            return 0;
        }
        if self.active_clipboard_paste_request_id != Some(request.id) {
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
        let max_shapes = self.max_shapes_per_frame;
        let undo_limit = self.undo_stack_limit;

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
            Self::translate_shape(&mut cloned_shape, dx, dy);
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

fn non_empty_shapes(shapes: Vec<Shape>) -> Option<Vec<Shape>> {
    if shapes.is_empty() {
        None
    } else {
        Some(shapes)
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
