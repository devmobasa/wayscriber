use crate::backend::wayland::runtime_ui_state::PreparedToolbarMutation;
use crate::input::InputState;
use crate::input::state::{Toast, ToastPriority};
use crate::ui::toolbar::ToolbarEvent;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ToolbarPinChange {
    Top(bool),
    Side(bool),
}

impl ToolbarPinChange {
    pub(super) fn from_event(event: &ToolbarEvent) -> Option<Self> {
        match event {
            ToolbarEvent::PinTopToolbar(pinned) => Some(Self::Top(*pinned)),
            ToolbarEvent::PinSideToolbar(pinned) => Some(Self::Side(*pinned)),
            _ => None,
        }
    }

    pub(super) fn message(self, durability: ToolbarPinDurability) -> &'static str {
        match (self, durability) {
            (Self::Top(true), ToolbarPinDurability::StartupPersistent) => {
                "Top toolbar will open at startup"
            }
            (Self::Top(false), ToolbarPinDurability::StartupPersistent) => {
                "Top toolbar will be hidden at startup"
            }
            (Self::Side(true), ToolbarPinDurability::StartupPersistent) => {
                "Side toolbar will open at startup"
            }
            (Self::Side(false), ToolbarPinDurability::StartupPersistent) => {
                "Side toolbar will be hidden at startup"
            }
            (Self::Top(true), ToolbarPinDurability::LiveOnly) => {
                "Top toolbar pinned for this run only"
            }
            (Self::Top(false), ToolbarPinDurability::LiveOnly) => {
                "Top toolbar unpinned for this run only"
            }
            (Self::Side(true), ToolbarPinDurability::LiveOnly) => {
                "Side toolbar pinned for this run only"
            }
            (Self::Side(false), ToolbarPinDurability::LiveOnly) => {
                "Side toolbar unpinned for this run only"
            }
        }
    }

    pub(super) fn notify(self, input: &mut InputState, durability: ToolbarPinDurability) {
        input.push_toast(
            ToastPriority::Info,
            "toolbar",
            Toast::info(self.message(durability)),
        );
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ToolbarPinDurability {
    StartupPersistent,
    LiveOnly,
}

pub(super) fn pin_durability(prepared: Option<&PreparedToolbarMutation>) -> ToolbarPinDurability {
    if prepared.is_some_and(PreparedToolbarMutation::is_persistent_preview) {
        ToolbarPinDurability::StartupPersistent
    } else {
        ToolbarPinDurability::LiveOnly
    }
}
