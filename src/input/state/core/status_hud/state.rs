use crate::config::{StatusBarStyle, StatusPosition};
use crate::ui::{StatusHudLayout, StatusHudSegmentKind};

#[derive(Debug, Clone)]
pub(super) struct StatusHudRebuildInputs {
    pub(super) position: StatusPosition,
    pub(super) style: StatusBarStyle,
    pub(super) screen_width: u32,
    pub(super) screen_height: u32,
}

/// Cached geometry and pointer interaction state for the status HUD.
#[derive(Debug, Default)]
pub struct StatusHudState {
    pub(crate) hover: Option<StatusHudSegmentKind>,
    pub(crate) layout: Option<StatusHudLayout>,
    pub(super) rebuild_inputs: Option<StatusHudRebuildInputs>,
    pub(in crate::input::state) press_pending: bool,
}

impl StatusHudState {
    pub fn is_effectively_visible(&self) -> bool {
        self.rebuild_inputs.is_some() && self.layout.is_some()
    }

    pub fn layout(&self) -> Option<&StatusHudLayout> {
        self.layout.as_ref()
    }

    pub(super) fn rebuild_inputs(&self) -> Option<StatusHudRebuildInputs> {
        self.rebuild_inputs.clone()
    }

    pub(super) fn replace_layout(
        &mut self,
        inputs: StatusHudRebuildInputs,
        layout: Option<StatusHudLayout>,
    ) {
        self.rebuild_inputs = Some(inputs);
        self.layout = layout;
    }

    pub(crate) fn clear_layout(&mut self) {
        self.layout = None;
        self.rebuild_inputs = None;
        self.hover = None;
    }

    pub(crate) fn clear_hover(&mut self) -> bool {
        self.hover.take().is_some()
    }

    pub(crate) fn update_hover(&mut self, hover: Option<StatusHudSegmentKind>) -> bool {
        if self.hover == hover {
            return false;
        }
        self.hover = hover;
        true
    }

    pub(in crate::input::state) fn set_press_pending(&mut self) {
        self.press_pending = true;
    }

    pub(in crate::input::state) fn clear_press_pending(&mut self) {
        self.press_pending = false;
    }

    pub(in crate::input::state) fn take_press_pending(&mut self) -> bool {
        std::mem::take(&mut self.press_pending)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hover_updates_only_on_identity_changes() {
        let mut state = StatusHudState::default();
        assert!(state.update_hover(Some(StatusHudSegmentKind::Tool)));
        assert!(!state.update_hover(Some(StatusHudSegmentKind::Tool)));
        assert!(state.clear_hover());
        assert!(!state.clear_hover());
    }

    #[test]
    fn taking_a_press_clears_it() {
        let mut state = StatusHudState::default();
        state.set_press_pending();
        assert!(state.take_press_pending());
        assert!(!state.take_press_pending());
    }
}
