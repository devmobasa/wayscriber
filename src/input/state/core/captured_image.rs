use super::InputState;
use crate::draw::frame::UndoAction;
use crate::draw::{EmbeddedImage, Shape};
use crate::screen_pixels::EmbeddedImageLimits;
use crate::util::Rect;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct BoardPasteTarget {
    pub board_id: String,
    pub page_index: usize,
    pub page_generation: u64,
    pub world_bounds: Rect,
}

impl InputState {
    pub(crate) fn insert_captured_image(
        &mut self,
        image: EmbeddedImage,
        target: &BoardPasteTarget,
    ) -> bool {
        let limits = EmbeddedImageLimits::default();
        if !limits.allows_bytes(image.bytes.len()) {
            self.push_toast(
                super::ToastPriority::Info,
                "capture.region.board",
                super::Toast::warning("Region is too large to add to the board."),
            );
            return false;
        }

        let target_active = self.boards.active_board_id() == target.board_id
            && self.boards.active_page_index() == target.page_index
            && self.boards.active_page_generation() == target.page_generation;
        if !target_active {
            self.push_toast(
                super::ToastPriority::Info,
                "capture.region.board",
                super::Toast::warning("Paste target changed; region was not added."),
            );
            return false;
        }
        let max_shapes = self.max_shapes_per_frame();
        let undo_limit = self.history_limits.undo_stack_limit();
        let Some(frame) = self
            .boards
            .board_state_by_id_mut(&target.board_id)
            .filter(|board| board.pages.generation() == target.page_generation)
            .and_then(|board| board.pages.frame_mut(target.page_index))
        else {
            self.push_toast(
                super::ToastPriority::Info,
                "capture.region.board",
                super::Toast::warning("Paste target changed; region was not added."),
            );
            return false;
        };

        let shape = Shape::Image {
            x: target.world_bounds.x,
            y: target.world_bounds.y,
            w: target.world_bounds.width,
            h: target.world_bounds.height,
            data: image,
        };
        let Some(id) = frame.try_add_shape_with_id(shape, max_shapes) else {
            self.push_toast(
                super::ToastPriority::Info,
                "capture.region.board",
                super::Toast::warning("Shape limit reached; region was not added."),
            );
            return false;
        };
        let Some((index, stored)) = frame
            .find_index(id)
            .and_then(|index| frame.shape(id).map(|shape| (index, shape.clone())))
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
        self.mark_session_dirty();
        if target_active {
            self.mark_selection_dirty_region(bounds);
            self.invalidate_hit_cache_for(id);
            self.set_selection(vec![id]);
        }
        self.needs_redraw = true;
        self.push_toast(
            super::ToastPriority::Info,
            "capture.region.board",
            super::Toast::info("Region added to board."),
        );
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::state::test_support::make_test_input_state;

    fn image(bytes: usize) -> EmbeddedImage {
        EmbeddedImage {
            mime_type: "image/png".to_string(),
            width: 10,
            height: 8,
            bytes: vec![0; bytes].into(),
        }
    }

    fn target(state: &InputState) -> BoardPasteTarget {
        BoardPasteTarget {
            board_id: state.boards.active_board_id().to_string(),
            page_index: state.boards.active_page_index(),
            page_generation: state.boards.active_page_generation(),
            world_bounds: Rect::new(12, 23, 40, 32).unwrap(),
        }
    }

    #[test]
    fn capture_image_adds_one_selected_shape_and_one_undo_entry() {
        let mut state = make_test_input_state();
        let target = target(&state);

        assert!(state.insert_captured_image(image(16), &target));

        let frame = state.boards.active_frame();
        assert_eq!(frame.shapes.len(), 1);
        assert_eq!(frame.undo_stack_len(), 1);
        assert_eq!(state.selected_shape_ids().len(), 1);
        assert!(matches!(
            &frame.shapes[0].shape,
            Shape::Image {
                x: 12,
                y: 23,
                w: 40,
                h: 32,
                ..
            }
        ));
    }

    #[test]
    fn capture_image_rejects_a_stale_page_generation_without_mutation() {
        let mut state = make_test_input_state();
        let mut target = target(&state);
        target.page_generation = target.page_generation.wrapping_add(1);

        assert!(!state.insert_captured_image(image(16), &target));
        assert!(state.boards.active_frame().shapes.is_empty());
        assert_eq!(
            state.test_active_toast_message(),
            Some("Paste target changed; region was not added.")
        );
    }

    #[test]
    fn capture_image_rejects_a_target_that_is_no_longer_active() {
        let mut state = make_test_input_state();
        let target = target(&state);
        assert!(state.boards.next_board(), "test config has another board");

        assert!(!state.insert_captured_image(image(16), &target));
        assert!(state.boards.board_states().iter().all(|board| {
            board
                .pages
                .pages()
                .iter()
                .all(|frame| frame.shapes.is_empty())
        }));
        assert_eq!(
            state.test_active_toast_message(),
            Some("Paste target changed; region was not added.")
        );
    }

    #[test]
    fn capture_image_rechecks_encoded_byte_limit_before_insertion() {
        let mut state = make_test_input_state();
        let target = target(&state);
        let too_large = EmbeddedImageLimits::default().max_bytes() + 1;

        assert!(!state.insert_captured_image(image(too_large), &target));
        assert!(state.boards.active_frame().shapes.is_empty());
        assert_eq!(
            state.test_active_toast_message(),
            Some("Region is too large to add to the board.")
        );
    }
}
