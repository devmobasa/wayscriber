use wayscriber::config::PresenterToolBehavior;

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
            PresenterToolBehaviorOption::Keep => "Keep active tool",
            PresenterToolBehaviorOption::ForceHighlight => "Force highlight (switchable)",
            PresenterToolBehaviorOption::ForceHighlightLocked => "Force highlight (locked)",
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

impl std::fmt::Display for PresenterToolBehaviorOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}
