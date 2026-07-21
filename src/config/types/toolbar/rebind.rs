use serde::{Deserialize, Serialize};

/// Modifier chord that turns a toolbar click into shortcut rebinding.
#[cfg_attr(feature = "config-schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ToolbarRebindModifier {
    Disabled,
    #[default]
    CtrlShift,
    CtrlAlt,
    ShiftAlt,
    CtrlShiftAlt,
}

impl ToolbarRebindModifier {
    pub fn matches(self, ctrl: bool, shift: bool, alt: bool) -> bool {
        match self {
            Self::Disabled => false,
            Self::CtrlShift => ctrl && shift && !alt,
            Self::CtrlAlt => ctrl && alt && !shift,
            Self::ShiftAlt => shift && alt && !ctrl,
            Self::CtrlShiftAlt => ctrl && shift && alt,
        }
    }

    /// The `"<chord>+click"` gesture label, e.g. `"Ctrl+Shift+click"`.
    /// `None` when rebind-by-click is disabled. The single source of truth for
    /// shortcut-rebind copy across the onboarding surfaces (tour + first-run
    /// cards), so no key strings are ever hardcoded.
    pub fn click_label(self) -> Option<&'static str> {
        match self {
            Self::Disabled => None,
            Self::CtrlShift => Some("Ctrl+Shift+click"),
            Self::CtrlAlt => Some("Ctrl+Alt+click"),
            Self::ShiftAlt => Some("Shift+Alt+click"),
            Self::CtrlShiftAlt => Some("Ctrl+Shift+Alt+click"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modifier_matching_is_exact_and_can_be_disabled() {
        assert!(ToolbarRebindModifier::CtrlShift.matches(true, true, false));
        assert!(!ToolbarRebindModifier::CtrlShift.matches(true, true, true));
        assert!(!ToolbarRebindModifier::Disabled.matches(true, true, false));
    }
}
