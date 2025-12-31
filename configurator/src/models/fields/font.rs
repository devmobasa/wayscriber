#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontStyleOption {
    Normal,
    Italic,
    Oblique,
    Custom,
}

impl FontStyleOption {
    pub fn list() -> Vec<Self> {
        vec![Self::Normal, Self::Italic, Self::Oblique, Self::Custom]
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Normal => "Normal",
            Self::Italic => "Italic",
            Self::Oblique => "Oblique",
            Self::Custom => "Custom",
        }
    }

    pub fn canonical_value(&self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::Italic => "italic",
            Self::Oblique => "oblique",
            Self::Custom => "",
        }
    }

    pub fn from_value(value: &str) -> (Self, String) {
        let lower = value.trim().to_lowercase();
        match lower.as_str() {
            "normal" => (Self::Normal, "normal".to_string()),
            "italic" => (Self::Italic, "italic".to_string()),
            "oblique" => (Self::Oblique, "oblique".to_string()),
            _ => (Self::Custom, value.to_string()),
        }
    }
}

impl std::fmt::Display for FontStyleOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontWeightOption {
    Normal,
    Bold,
    Light,
    Ultralight,
    Heavy,
    Ultrabold,
    Custom,
}

impl FontWeightOption {
    pub fn list() -> Vec<Self> {
        vec![
            Self::Normal,
            Self::Bold,
            Self::Light,
            Self::Ultralight,
            Self::Heavy,
            Self::Ultrabold,
            Self::Custom,
        ]
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Normal => "Normal",
            Self::Bold => "Bold",
            Self::Light => "Light",
            Self::Ultralight => "Ultralight",
            Self::Heavy => "Heavy",
            Self::Ultrabold => "Ultrabold",
            Self::Custom => "Custom",
        }
    }

    pub fn canonical_value(&self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::Bold => "bold",
            Self::Light => "light",
            Self::Ultralight => "ultralight",
            Self::Heavy => "heavy",
            Self::Ultrabold => "ultrabold",
            Self::Custom => "",
        }
    }

    pub fn from_value(value: &str) -> (Self, String) {
        let lower = value.trim().to_lowercase();
        match lower.as_str() {
            "normal" => (Self::Normal, "normal".to_string()),
            "bold" => (Self::Bold, "bold".to_string()),
            "light" => (Self::Light, "light".to_string()),
            "ultralight" => (Self::Ultralight, "ultralight".to_string()),
            "heavy" => (Self::Heavy, "heavy".to_string()),
            "ultrabold" => (Self::Ultrabold, "ultrabold".to_string()),
            _ => (Self::Custom, value.to_string()),
        }
    }
}

impl std::fmt::Display for FontWeightOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}
