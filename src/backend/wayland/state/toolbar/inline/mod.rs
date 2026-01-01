use super::*;
use crate::backend::wayland::toolbar::ToolbarFocusTarget;

mod drag;
mod focus;
mod input;
mod render;

impl WaylandState {
    pub(super) fn clear_inline_toolbar_hits(&mut self) {
        self.data.inline_top_hits.clear();
        self.data.inline_side_hits.clear();
        self.data.inline_top_rect = None;
        self.data.inline_side_rect = None;
    }

    pub(super) fn clear_inline_toolbar_hover(&mut self) {
        self.data.inline_top_hover = None;
        self.data.inline_side_hover = None;
    }

    pub(super) fn clear_inline_toolbar_focus(&mut self) {
        self.data.inline_top_focus_index = None;
        self.data.inline_side_focus_index = None;
    }

    fn inline_focus_index(&self, target: ToolbarFocusTarget) -> Option<usize> {
        match target {
            ToolbarFocusTarget::Top => self.data.inline_top_focus_index,
            ToolbarFocusTarget::Side => self.data.inline_side_focus_index,
        }
    }

    fn inline_focus_index_mut(&mut self, target: ToolbarFocusTarget) -> &mut Option<usize> {
        match target {
            ToolbarFocusTarget::Top => &mut self.data.inline_top_focus_index,
            ToolbarFocusTarget::Side => &mut self.data.inline_side_focus_index,
        }
    }
}
