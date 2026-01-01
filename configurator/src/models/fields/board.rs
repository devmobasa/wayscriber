#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoardModeOption {
    Transparent,
    Whiteboard,
    Blackboard,
}

impl BoardModeOption {
    pub fn list() -> Vec<Self> {
        vec![
            BoardModeOption::Transparent,
            BoardModeOption::Whiteboard,
            BoardModeOption::Blackboard,
        ]
    }

    pub fn label(&self) -> &'static str {
        match self {
            BoardModeOption::Transparent => "Transparent",
            BoardModeOption::Whiteboard => "Whiteboard",
            BoardModeOption::Blackboard => "Blackboard",
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            BoardModeOption::Transparent => "transparent",
            BoardModeOption::Whiteboard => "whiteboard",
            BoardModeOption::Blackboard => "blackboard",
        }
    }

    pub fn from_str(value: &str) -> Option<Self> {
        match value {
            "transparent" => Some(BoardModeOption::Transparent),
            "whiteboard" => Some(BoardModeOption::Whiteboard),
            "blackboard" => Some(BoardModeOption::Blackboard),
            _ => None,
        }
    }
}

impl std::fmt::Display for BoardModeOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}
