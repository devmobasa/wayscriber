use std::fmt;
use std::fmt::{Display, Formatter};

use wayscriber::i18n::tr;

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
            Self::Auto => tr("Auto"),
            Self::Config => tr("Config directory"),
            Self::Custom => tr("Custom directory"),
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

impl Display for SessionStorageModeOption {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
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
            Self::Auto => tr("Auto"),
            Self::On => tr("On"),
            Self::Off => tr("Off"),
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

impl Display for SessionCompressionOption {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}
