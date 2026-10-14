use wayscriber::i18n::tr;

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
            Self::Normal => tr("Normal"),
            Self::Italic => tr("Italic"),
            Self::Oblique => tr("Oblique"),
            Self::Custom => tr("Custom"),
        }
    }

    pub fn canonical_value(&self) -> &'static str {
        match self {
            Self::Normal => tr("normal"),
            Self::Italic => tr("italic"),
            Self::Oblique => tr("oblique"),
            Self::Custom => "",
        }
    }

    pub fn from_value(value: &str) -> (Self, String) {
        let lower = value.trim().to_lowercase();
        match lower.as_str() {
            "normal" => (Self::Normal, tr("normal").to_string()),
            "italic" => (Self::Italic, tr("italic").to_string()),
            "oblique" => (Self::Oblique, tr("oblique").to_string()),
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
            Self::Normal => tr("Normal"),
            Self::Bold => tr("Bold"),
            Self::Light => tr("Light"),
            Self::Ultralight => tr("Ultralight"),
            Self::Heavy => tr("Heavy"),
            Self::Ultrabold => tr("Ultrabold"),
            Self::Custom => tr("Custom"),
        }
    }

    pub fn canonical_value(&self) -> &'static str {
        match self {
            Self::Normal => tr("normal"),
            Self::Bold => tr("bold"),
            Self::Light => tr("light"),
            Self::Ultralight => tr("ultralight"),
            Self::Heavy => tr("heavy"),
            Self::Ultrabold => tr("ultrabold"),
            Self::Custom => "",
        }
    }

    pub fn from_value(value: &str) -> (Self, String) {
        let lower = value.trim().to_lowercase();
        match lower.as_str() {
            "normal" => (Self::Normal, tr("normal").to_string()),
            "bold" => (Self::Bold, tr("bold").to_string()),
            "light" => (Self::Light, tr("light").to_string()),
            "ultralight" => (Self::Ultralight, tr("ultralight").to_string()),
            "heavy" => (Self::Heavy, tr("heavy").to_string()),
            "ultrabold" => (Self::Ultrabold, tr("ultrabold").to_string()),
            _ => (Self::Custom, value.to_string()),
        }
    }
}

impl std::fmt::Display for FontWeightOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}
