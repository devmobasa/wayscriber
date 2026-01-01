use wayscriber::config::{SessionCompression, SessionStorageMode};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionStorageModeOption {
    Auto,
    Config,
    Custom,
}

impl SessionStorageModeOption {
    pub fn list() -> Vec<Self> {
        vec![Self::Auto, Self::Config, Self::Custom]
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Auto => "Auto",
            Self::Config => "Config directory",
            Self::Custom => "Custom directory",
        }
    }

    #[allow(clippy::wrong_self_convention)]
    pub fn to_mode(&self) -> SessionStorageMode {
        match self {
            Self::Auto => SessionStorageMode::Auto,
            Self::Config => SessionStorageMode::Config,
            Self::Custom => SessionStorageMode::Custom,
        }
    }

    pub fn from_mode(mode: SessionStorageMode) -> Self {
        match mode {
            SessionStorageMode::Auto => Self::Auto,
            SessionStorageMode::Config => Self::Config,
            SessionStorageMode::Custom => Self::Custom,
        }
    }
}

impl std::fmt::Display for SessionStorageModeOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionCompressionOption {
    Auto,
    On,
    Off,
}

impl SessionCompressionOption {
    pub fn list() -> Vec<Self> {
        vec![Self::Auto, Self::On, Self::Off]
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Auto => "Auto",
            Self::On => "On",
            Self::Off => "Off",
        }
    }

    #[allow(clippy::wrong_self_convention)]
    pub fn to_compression(&self) -> SessionCompression {
        match self {
            Self::Auto => SessionCompression::Auto,
            Self::On => SessionCompression::On,
            Self::Off => SessionCompression::Off,
        }
    }

    pub fn from_compression(mode: SessionCompression) -> Self {
        match mode {
            SessionCompression::Auto => Self::Auto,
            SessionCompression::On => Self::On,
            SessionCompression::Off => Self::Off,
        }
    }
}

impl std::fmt::Display for SessionCompressionOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}
