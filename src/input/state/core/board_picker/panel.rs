use std::time::Instant;

use super::{
    BoardPickerDrag, BoardPickerLayout, BoardPickerPageDrag, BoardPickerPageEdit, BoardPickerState,
};
use crate::input::state::core::base::BoardPickerClickState;

/// Modal, layout, search, edit, and drag state for the board picker.
#[derive(Debug)]
pub struct BoardPickerPanel {
    pub state: BoardPickerState,
    pub drag: Option<BoardPickerDrag>,
    pub page_drag: Option<BoardPickerPageDrag>,
    pub page_edit: Option<BoardPickerPageEdit>,
    pub layout: Option<BoardPickerLayout>,
    pub search: String,
    pub search_last_input: Option<Instant>,
    pub(crate) last_click: Option<BoardPickerClickState>,
}

impl Default for BoardPickerPanel {
    fn default() -> Self {
        Self {
            state: BoardPickerState::Hidden,
            drag: None,
            page_drag: None,
            page_edit: None,
            layout: None,
            search: String::new(),
            search_last_input: None,
            last_click: None,
        }
    }
}
