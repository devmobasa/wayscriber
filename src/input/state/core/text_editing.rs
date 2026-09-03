use super::base::{TextBlockDrag, TextClickState, TextEditEntryFeedback, TextInputMode};
use super::ime::ImeCompositionState;
use crate::draw::ShapeId;
use crate::draw::frame::ShapeSnapshot;
use crate::util::Rect;

/// Text-editor mode, asynchronous identity, composition, and pointer state.
#[derive(Debug, Clone)]
pub(crate) struct TextEditing {
    pub(crate) text_input_mode: TextInputMode,
    pub(crate) text_input_cursor_rect_dirty: bool,
    pub(crate) text_input_external_change_dirty: bool,
    pub(crate) text_input_generation: u64,
    pub(crate) text_input_revision: u64,
    pub(crate) text_edit_target: Option<(ShapeId, ShapeSnapshot)>,
    pub(crate) text_block_drag: Option<TextBlockDrag>,
    pub(crate) text_edit_entry_feedback: Option<TextEditEntryFeedback>,
    pub(crate) ime: ImeCompositionState,
    pub(crate) last_text_click: Option<TextClickState>,
    pub(crate) last_text_preview_bounds: Option<Rect>,
}

impl Default for TextEditing {
    fn default() -> Self {
        Self {
            text_input_mode: TextInputMode::Plain,
            text_input_cursor_rect_dirty: false,
            text_input_external_change_dirty: false,
            text_input_generation: 0,
            text_input_revision: 0,
            text_edit_target: None,
            text_block_drag: None,
            text_edit_entry_feedback: None,
            ime: ImeCompositionState::default(),
            last_text_click: None,
            last_text_preview_bounds: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_starts_a_plain_editor_without_an_active_session() {
        let editing = TextEditing::default();

        assert_eq!(editing.text_input_mode, TextInputMode::Plain);
        assert_eq!(
            (editing.text_input_generation, editing.text_input_revision),
            (0, 0)
        );
        assert!(editing.text_edit_target.is_none());
        assert!(editing.text_block_drag.is_none());
        assert!(editing.text_edit_entry_feedback.is_none());
        assert!(editing.ime.preedit().is_none());
        assert!(editing.last_text_click.is_none());
        assert!(editing.last_text_preview_bounds.is_none());
        assert!(!editing.text_input_cursor_rect_dirty);
        assert!(!editing.text_input_external_change_dirty);
    }
}
