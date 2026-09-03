use crate::config::ZoomChipDisplay;
use crate::ui::{ZoomChipButtonKind, ZoomChipLayout, ZoomChipPress};

/// Display policy, cached geometry, and pointer interaction state for the zoom chip.
#[derive(Debug)]
pub struct ZoomChipState {
    pub(crate) display: ZoomChipDisplay,
    pub(crate) hover: Option<ZoomChipButtonKind>,
    pub(crate) layout: Option<ZoomChipLayout>,
    pub(in crate::input::state) press_pending: ZoomChipPress,
}

impl ZoomChipState {
    pub fn layout(&self) -> Option<&ZoomChipLayout> {
        self.layout.as_ref()
    }

    pub(crate) fn is_enabled(
        &self,
        show_zoom_actions: bool,
        show_zoom_chip: bool,
        zoom_active: bool,
    ) -> bool {
        show_zoom_actions
            && show_zoom_chip
            && (self.display == ZoomChipDisplay::Always || zoom_active)
    }

    pub(crate) fn replace_layout(&mut self, layout: Option<ZoomChipLayout>) {
        self.layout = layout;
    }

    pub(crate) fn clear_layout(&mut self) {
        self.layout = None;
        self.hover = None;
    }

    pub(crate) fn clear_hover(&mut self) -> bool {
        self.hover.take().is_some()
    }

    pub(crate) fn update_hover(&mut self, hover: Option<ZoomChipButtonKind>) -> bool {
        if self.hover == hover {
            return false;
        }
        self.hover = hover;
        true
    }

    pub(crate) fn contains(&self, enabled: bool, x: i32, y: i32) -> bool {
        enabled
            && self
                .layout
                .as_ref()
                .is_some_and(|layout| layout.chip_contains(x as f64, y as f64))
    }

    pub(crate) fn button_at(&self, x: i32, y: i32) -> Option<ZoomChipButtonKind> {
        self.layout
            .as_ref()
            .and_then(|layout| layout.button_at(x as f64, y as f64))
    }

    pub(in crate::input::state) fn set_press_pending(&mut self, pressed: ZoomChipPress) {
        self.press_pending = pressed;
    }

    pub(in crate::input::state) fn clear_press_pending(&mut self) {
        self.press_pending = ZoomChipPress::None;
    }

    pub(in crate::input::state) fn take_press_pending(&mut self) -> ZoomChipPress {
        std::mem::replace(&mut self.press_pending, ZoomChipPress::None)
    }
}

impl Default for ZoomChipState {
    fn default() -> Self {
        Self {
            display: ZoomChipDisplay::Always,
            hover: None,
            layout: None,
            press_pending: ZoomChipPress::None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn while_zoomed_policy_requires_active_zoom() {
        let state = ZoomChipState {
            display: ZoomChipDisplay::WhileZoomed,
            ..Default::default()
        };

        assert!(!state.is_enabled(true, true, false));
        assert!(state.is_enabled(true, true, true));
        assert!(!state.is_enabled(false, true, true));
        assert!(!state.is_enabled(true, false, true));
    }

    #[test]
    fn taking_a_press_resets_the_contract() {
        let mut state = ZoomChipState::default();
        state.set_press_pending(ZoomChipPress::Passive);

        assert_eq!(state.take_press_pending(), ZoomChipPress::Passive);
        assert_eq!(state.take_press_pending(), ZoomChipPress::None);
    }
}
