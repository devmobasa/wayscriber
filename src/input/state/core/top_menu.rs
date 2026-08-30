#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TopMenuState {
    #[default]
    Closed,
    ShapePicker,
    TopOverflow,
    CanvasPopover,
    SessionPopover,
    SettingsPopover,
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
        matches!(self, Self::ShapePicker | Self::TopOverflow)
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
