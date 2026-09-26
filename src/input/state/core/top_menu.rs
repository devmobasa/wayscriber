#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TopMenuState {
    #[default]
    Closed,
    ShapePicker,
    TopOverflow,
    CanvasPopover,
    SessionPopover,
    SettingsPopover,
    /// The chrome island's layout-preset menu (Simple / Regular / Advanced).
    LayoutMenu,
    /// The style pill's Pen feel panel (smoothing and Shape Pen detection).
    PenFeelPanel,
}

impl TopMenuState {
    pub(crate) const fn is_open(self) -> bool {
        !matches!(self, Self::Closed)
    }

    pub(crate) const fn is_popover(self) -> bool {
        matches!(
            self,
            Self::CanvasPopover | Self::SessionPopover | Self::SettingsPopover
        )
    }

    pub(crate) const fn is_flyout(self) -> bool {
        matches!(
            self,
            Self::ShapePicker | Self::TopOverflow | Self::LayoutMenu | Self::PenFeelPanel
        )
    }

    pub(crate) fn set_open(&mut self, target: Self, open: bool) -> bool {
        debug_assert!(target.is_open(), "Closed is not an open menu target");
        let next = if open {
            target
        } else if *self == target {
            Self::Closed
        } else {
            *self
        };
        let changed = *self != next;
        *self = next;
        changed
    }

    pub(crate) fn close(&mut self) -> bool {
        let changed = self.is_open();
        *self = Self::Closed;
        changed
    }
}

#[cfg(test)]
mod tests {
    use super::TopMenuState;

    /// The built-in toolbar's own key handler closes flyouts on Escape before
    /// anything else sees the key; the Pen feel panel is one, like the layout
    /// menu, and every other key reaches the overlay's routing.
    #[test]
    fn the_pen_feel_panel_is_a_flyout_menu() {
        assert!(TopMenuState::PenFeelPanel.is_open());
        assert!(TopMenuState::PenFeelPanel.is_flyout());
        assert!(!TopMenuState::PenFeelPanel.is_popover());
    }
}
