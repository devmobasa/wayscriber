use std::fmt;
use std::fmt::{Display, Formatter};

use wayscriber::i18n::tr;

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
            Self::Native => tr("Native"),
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

impl Display for RegionPickerOption {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}
