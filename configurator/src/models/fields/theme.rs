use wayscriber::config::{ReducedMotion, UiTheme};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiThemeOption {
    Auto,
    Dark,
    Light,
}

impl UiThemeOption {
    pub fn list() -> Vec<Self> {
        vec![
            UiThemeOption::Auto,
            UiThemeOption::Dark,
            UiThemeOption::Light,
        ]
    }

    pub fn label(&self) -> &'static str {
        match self {
            UiThemeOption::Auto => "Auto",
            UiThemeOption::Dark => "Dark",
            UiThemeOption::Light => "Light",
        }
    }

    pub fn to_theme(self) -> UiTheme {
        match self {
            UiThemeOption::Auto => UiTheme::Auto,
            UiThemeOption::Dark => UiTheme::Dark,
            UiThemeOption::Light => UiTheme::Light,
        }
    }

    pub fn from_theme(theme: UiTheme) -> Self {
        match theme {
            UiTheme::Auto => UiThemeOption::Auto,
            UiTheme::Dark => UiThemeOption::Dark,
            UiTheme::Light => UiThemeOption::Light,
        }
    }
}

impl std::fmt::Display for UiThemeOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReducedMotionOption {
    Auto,
    On,
    Off,
}

impl ReducedMotionOption {
    pub fn list() -> Vec<Self> {
        vec![
            ReducedMotionOption::Auto,
            ReducedMotionOption::On,
            ReducedMotionOption::Off,
        ]
    }

    pub fn label(&self) -> &'static str {
        match self {
            ReducedMotionOption::Auto => "Auto",
            ReducedMotionOption::On => "On",
            ReducedMotionOption::Off => "Off",
        }
    }

    pub fn to_reduced_motion(self) -> ReducedMotion {
        match self {
            ReducedMotionOption::Auto => ReducedMotion::Auto,
            ReducedMotionOption::On => ReducedMotion::On,
            ReducedMotionOption::Off => ReducedMotion::Off,
        }
    }

    pub fn from_reduced_motion(value: ReducedMotion) -> Self {
        match value {
            ReducedMotion::Auto => ReducedMotionOption::Auto,
            ReducedMotion::On => ReducedMotionOption::On,
            ReducedMotion::Off => ReducedMotionOption::Off,
        }
    }
}

impl std::fmt::Display for ReducedMotionOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}
