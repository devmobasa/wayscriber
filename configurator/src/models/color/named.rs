#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorMode {
    Named,
    Rgb,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NamedColorOption {
    Red,
    Green,
    Blue,
    Yellow,
    Orange,
    Pink,
    White,
    Black,
    Custom,
}

impl NamedColorOption {
    pub fn list() -> Vec<Self> {
        vec![
            Self::Red,
            Self::Green,
            Self::Blue,
            Self::Yellow,
            Self::Orange,
            Self::Pink,
            Self::White,
            Self::Black,
            Self::Custom,
        ]
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Red => "Red",
            Self::Green => "Green",
            Self::Blue => "Blue",
            Self::Yellow => "Yellow",
            Self::Orange => "Orange",
            Self::Pink => "Pink",
            Self::White => "White",
            Self::Black => "Black",
            Self::Custom => "Custom",
        }
    }

    pub fn as_value(&self) -> &'static str {
        match self {
            Self::Red => "red",
            Self::Green => "green",
            Self::Blue => "blue",
            Self::Yellow => "yellow",
            Self::Orange => "orange",
            Self::Pink => "pink",
            Self::White => "white",
            Self::Black => "black",
            Self::Custom => "",
        }
    }

    pub fn from_str(value: &str) -> Self {
        match value.trim().to_lowercase().as_str() {
            "red" => Self::Red,
            "green" => Self::Green,
            "blue" => Self::Blue,
            "yellow" => Self::Yellow,
            "orange" => Self::Orange,
            "pink" => Self::Pink,
            "white" => Self::White,
            "black" => Self::Black,
            _ => Self::Custom,
        }
    }
}

impl std::fmt::Display for NamedColorOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}
