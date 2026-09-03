use super::super::board_picker::BoardPickerPageTarget;
use super::{ContextMenuLayout, ContextMenuState};

/// Lifecycle, target, and cached layout for the context menu.
#[derive(Debug)]
pub struct ContextMenuPanel {
    pub state: ContextMenuState,
    pub(crate) page_target: Option<BoardPickerPageTarget>,
    pub(crate) enabled: bool,
    pub layout: Option<ContextMenuLayout>,
}

impl Default for ContextMenuPanel {
    fn default() -> Self {
        Self {
            state: ContextMenuState::Hidden,
            page_target: None,
            enabled: true,
            layout: None,
        }
    }
}
