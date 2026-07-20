use super::super::super::base::{ClipboardPasteRequest, InputState};
use crate::draw::frame::UndoAction;
use crate::draw::{EmbeddedImage, Shape};
use crate::input::state::{Toast, ToastPriority};

impl InputState {
    pub(crate) fn paste_external_image_from_request(
        &mut self,
        request: &ClipboardPasteRequest,
        image: EmbeddedImage,
    ) -> bool {
        if self.active_clipboard_paste_request_id != Some(request.id) {
            log::info!(
                "Ignoring external image paste request {} because active request is {:?}",
                request.id,
                self.active_clipboard_paste_request_id
            );
            return false;
        }

        let image_mime_type = image.mime_type.clone();
        let image_width = image.width;
        let image_height = image.height;
        let image_bytes = image.bytes.len();
        let target_active = self.clipboard_request_targets_active_page(request);
        let max_shapes = self.max_shapes_per_frame;
        let undo_limit = self.undo_stack_limit;
        let target = self
            .boards
            .board_state_by_id_mut(&request.target_board_id)
            .filter(|board| board.pages.generation() == request.target_page_generation)
            .and_then(|board| board.pages.frame_mut(request.target_page_index));

        let Some(frame) = target else {
            log::warn!(
                "External image paste request {} cancelled because target board '{}' page {} generation {} is no longer available",
                request.id,
                request.target_board_id,
                request.target_page_index,
                request.target_page_generation
            );
            self.push_toast(
                ToastPriority::Info,
                "selection.clipboard",
                Toast::warning("Paste target changed; image paste was cancelled."),
            );
            self.trigger_blocked_feedback();
            return false;
        };

        let (x, y, w, h) = image_display_bounds(request, image.width, image.height);
        let shape = Shape::Image {
            x,
            y,
            w,
            h,
            data: image,
        };
        let Some(new_id) = frame.try_add_shape_with_id(shape, max_shapes) else {
            log::warn!(
                "External image paste request {} rejected by shape limit on board '{}' page {} (max_shapes={}, image_bytes={})",
                request.id,
                request.target_board_id,
                request.target_page_index,
                max_shapes,
                image_bytes
            );
            self.push_toast(
                ToastPriority::Info,
                "selection.clipboard",
                Toast::warning("Shape limit reached; image not pasted."),
            );
            self.trigger_blocked_feedback();
            return false;
        };

        let Some((index, stored)) = frame
            .find_index(new_id)
            .and_then(|index| frame.shape(new_id).map(|shape| (index, shape.clone())))
        else {
            return false;
        };
        let bounds = stored.bounding_box();
        frame.push_undo_action(
            UndoAction::Create {
                shapes: vec![(index, stored)],
            },
            undo_limit,
        );
        let undo_entries = frame.undo_stack_len();
        self.mark_session_dirty();
        if target_active {
            self.mark_selection_dirty_region(bounds);
            self.invalidate_hit_cache_for(new_id);
            self.set_selection(vec![new_id]);
            self.needs_redraw = true;
        }
        log::info!(
            "Pasted external image shape {} from request {} into board '{}' page {}: target_active={}, mime={}, image={}x{}, bytes={}, display_bounds=({}, {}, {}, {}), undo_entries={}",
            new_id,
            request.id,
            request.target_board_id,
            request.target_page_index,
            target_active,
            image_mime_type,
            image_width,
            image_height,
            image_bytes,
            x,
            y,
            w,
            h,
            undo_entries
        );
        true
    }

    pub(super) fn clipboard_request_targets_active_page(
        &self,
        request: &ClipboardPasteRequest,
    ) -> bool {
        self.boards.active_board_id() == request.target_board_id
            && self.boards.active_page_index() == request.target_page_index
            && self.boards.active_page_generation() == request.target_page_generation
    }
}

fn image_display_bounds(
    request: &ClipboardPasteRequest,
    natural_width: u32,
    natural_height: u32,
) -> (i32, i32, i32, i32) {
    let natural_width = natural_width.max(1) as f64;
    let natural_height = natural_height.max(1) as f64;
    let max_width = (request.visible_canvas_rect.width.max(1) as f64 * 0.7).max(1.0);
    let max_height = (request.visible_canvas_rect.height.max(1) as f64 * 0.7).max(1.0);
    let scale = (max_width / natural_width)
        .min(max_height / natural_height)
        .min(1.0);
    let w = (natural_width * scale).round().max(1.0) as i32;
    let h = (natural_height * scale).round().max(1.0) as i32;
    let (anchor_x, anchor_y) = request.anchor.point();
    let mut x = anchor_x.saturating_sub(w / 2);
    let mut y = anchor_y.saturating_sub(h / 2);
    x = clamp_partly_visible(
        x,
        w,
        request.visible_canvas_rect.x,
        request.visible_canvas_rect.width,
    );
    y = clamp_partly_visible(
        y,
        h,
        request.visible_canvas_rect.y,
        request.visible_canvas_rect.height,
    );
    (x, y, w, h)
}

fn clamp_partly_visible(value: i32, size: i32, visible_start: i32, visible_size: i32) -> i32 {
    let visible_end = visible_start.saturating_add(visible_size);
    let min_value = visible_start.saturating_sub(size.saturating_sub(1));
    let max_value = visible_end.saturating_sub(1);
    value.clamp(min_value, max_value)
}
