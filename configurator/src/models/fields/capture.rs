use wayscriber::config::RegionPicker;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegionPickerOption {
    Native,
    Slurp,
}

impl RegionPickerOption {
    pub fn list() -> Vec<Self> {
        vec![Self::Native, Self::Slurp]
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Native => "Native",
            Self::Slurp => "Slurp",
        }
    }

    pub fn to_picker(self) -> RegionPicker {
        match self {
            Self::Native => RegionPicker::Native,
            Self::Slurp => RegionPicker::Slurp,
        }
    }

    pub fn from_picker(picker: RegionPicker) -> Self {
        match picker {
            RegionPicker::Native => Self::Native,
            RegionPicker::Slurp => Self::Slurp,
        }
    }
}

impl std::fmt::Display for RegionPickerOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}
