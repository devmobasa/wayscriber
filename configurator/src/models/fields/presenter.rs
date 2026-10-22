use std::fmt;
use std::fmt::{Display, Formatter};

use wayscriber::i18n::tr;

use wayscriber::config::{PresenterToolBehavior, PresenterToolbarMode};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PresenterToolBehaviorOption {
    Keep,
    ForceHighlight,
    ForceHighlightLocked,
}

impl PresenterToolBehaviorOption {
    pub fn list() -> Vec<Self> {
        vec![
            PresenterToolBehaviorOption::Keep,
            PresenterToolBehaviorOption::ForceHighlight,
            PresenterToolBehaviorOption::ForceHighlightLocked,
        ]
    }

    pub fn label(&self) -> &'static str {
        match self {
            PresenterToolBehaviorOption::Keep => tr("Keep active tool"),
            PresenterToolBehaviorOption::ForceHighlight => tr("Force highlight (switchable)"),
            PresenterToolBehaviorOption::ForceHighlightLocked => tr("Force highlight (locked)"),
        }
    }

    pub fn to_behavior(self) -> PresenterToolBehavior {
        match self {
            PresenterToolBehaviorOption::Keep => PresenterToolBehavior::Keep,
            PresenterToolBehaviorOption::ForceHighlight => PresenterToolBehavior::ForceHighlight,
            PresenterToolBehaviorOption::ForceHighlightLocked => {
                PresenterToolBehavior::ForceHighlightLocked
            }
        }
    }

    pub fn from_behavior(behavior: PresenterToolBehavior) -> Self {
        match behavior {
            PresenterToolBehavior::Keep => PresenterToolBehaviorOption::Keep,
            PresenterToolBehavior::ForceHighlight => PresenterToolBehaviorOption::ForceHighlight,
            PresenterToolBehavior::ForceHighlightLocked => {
                PresenterToolBehaviorOption::ForceHighlightLocked
            }
        }
    }
}

impl Display for PresenterToolBehaviorOption {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PresenterToolbarModeOption {
    Hidden,
    Micro,
}

impl PresenterToolbarModeOption {
    pub fn list() -> Vec<Self> {
        vec![
            PresenterToolbarModeOption::Hidden,
            PresenterToolbarModeOption::Micro,
        ]
    }

    pub fn label(&self) -> &'static str {
        match self {
            PresenterToolbarModeOption::Hidden => tr("Hide completely"),
            PresenterToolbarModeOption::Micro => tr("Micro chip (top toolbar)"),
        }
    }

    pub fn to_mode(self) -> PresenterToolbarMode {
        match self {
            PresenterToolbarModeOption::Hidden => PresenterToolbarMode::Hidden,
            PresenterToolbarModeOption::Micro => PresenterToolbarMode::Micro,
        }
    }

    pub fn from_mode(mode: PresenterToolbarMode) -> Self {
        match mode {
            PresenterToolbarMode::Hidden => PresenterToolbarModeOption::Hidden,
            PresenterToolbarMode::Micro => PresenterToolbarModeOption::Micro,
        }
    }
}

impl Display for PresenterToolbarModeOption {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}
