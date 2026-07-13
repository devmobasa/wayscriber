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
