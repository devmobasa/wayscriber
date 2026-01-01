use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Toolbar layout complexity presets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Default)]
#[serde(rename_all = "kebab-case")]
pub enum ToolbarLayoutMode {
    Simple,
    #[default]
    Regular,
    Advanced,
}

impl ToolbarLayoutMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Simple => "Simple",
            Self::Regular => "Regular",
            Self::Advanced => "Advanced",
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::Simple => Self::Regular,
            Self::Regular => Self::Advanced,
            Self::Advanced => Self::Simple,
        }
    }

    pub fn section_defaults(self) -> ToolbarSectionDefaults {
        match self {
            Self::Simple => ToolbarSectionDefaults {
                show_actions_section: true,
                show_actions_advanced: false,
                show_pages_section: true,
                show_presets: false,
                show_step_section: false,
                show_text_controls: false,
                show_settings_section: false,
            },
            Self::Regular => ToolbarSectionDefaults {
                show_actions_section: true,
                show_actions_advanced: false,
                show_pages_section: true,
                show_presets: true,
                show_step_section: false,
                show_text_controls: false,
                show_settings_section: true,
            },
            Self::Advanced => ToolbarSectionDefaults {
                show_actions_section: true,
                show_actions_advanced: true,
                show_pages_section: true,
                show_presets: true,
                show_step_section: true,
                show_text_controls: true,
                show_settings_section: true,
            },
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ToolbarSectionDefaults {
    pub show_actions_section: bool,
    pub show_actions_advanced: bool,
    pub show_pages_section: bool,
    pub show_presets: bool,
    pub show_step_section: bool,
    pub show_text_controls: bool,
    pub show_settings_section: bool,
}

/// Optional per-mode overrides for toolbar sections.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, Default)]
pub struct ToolbarModeOverride {
    /// Show the Actions section (undo/redo/clear)
    #[serde(default)]
    pub show_actions_section: Option<bool>,

    /// Show advanced action buttons (undo all, zoom, freeze, etc.)
    #[serde(default)]
    pub show_actions_advanced: Option<bool>,

    /// Show the Pages section in the side toolbar
    #[serde(default)]
    pub show_pages_section: Option<bool>,

    /// Show the presets section in the side toolbar
    #[serde(default)]
    pub show_presets: Option<bool>,

    /// Show the Step Undo/Redo section
    #[serde(default)]
    pub show_step_section: Option<bool>,

    /// Keep text controls visible even when text is not active
    #[serde(default)]
    pub show_text_controls: Option<bool>,

    /// Show the Settings section (config shortcuts, layout controls)
    #[serde(default)]
    pub show_settings_section: Option<bool>,
}

/// Mode-specific overrides for toolbar layout presets.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
pub struct ToolbarModeOverrides {
    #[serde(default)]
    pub simple: ToolbarModeOverride,
    #[serde(default)]
    pub regular: ToolbarModeOverride,
    #[serde(default)]
    pub advanced: ToolbarModeOverride,
}

impl ToolbarModeOverrides {
    pub fn for_mode(&self, mode: ToolbarLayoutMode) -> &ToolbarModeOverride {
        match mode {
            ToolbarLayoutMode::Simple => &self.simple,
            ToolbarLayoutMode::Regular => &self.regular,
            ToolbarLayoutMode::Advanced => &self.advanced,
        }
    }
}

/// Toolbar visibility and pinning configuration.
///
/// Controls which toolbar panels are visible on startup and whether they
/// remain pinned (saved to config) when closed.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ToolbarConfig {
    /// Toolbar layout preset (simple, regular, advanced)
    #[serde(default = "default_toolbar_layout_mode")]
    pub layout_mode: ToolbarLayoutMode,

    /// Optional per-mode overrides for toolbar sections
    #[serde(default)]
    pub mode_overrides: ToolbarModeOverrides,

    /// Show the top toolbar (tool selection) on startup
    #[serde(default = "default_toolbar_top_pinned")]
    pub top_pinned: bool,

    /// Show the side toolbar (colors, settings) on startup
    #[serde(default = "default_toolbar_side_pinned")]
    pub side_pinned: bool,

    /// Use icons instead of text labels in toolbars
    #[serde(default = "default_toolbar_use_icons")]
    pub use_icons: bool,

    /// Show extended color palette
    #[serde(default = "default_show_more_colors")]
    pub show_more_colors: bool,

    /// Show the Actions section (undo all, redo all, etc.)
    #[serde(default = "default_show_actions_section")]
    pub show_actions_section: bool,

    /// Show advanced actions (undo all, zoom, freeze, etc.)
    #[serde(default = "default_show_actions_advanced")]
    pub show_actions_advanced: bool,

    /// Show the Pages section in the side toolbar
    #[serde(default = "default_show_pages_section")]
    pub show_pages_section: bool,

    /// Show the presets section in the side toolbar
    #[serde(default = "default_show_presets")]
    pub show_presets: bool,

    /// Show the Step Undo/Redo section
    #[serde(default = "default_show_step_section")]
    pub show_step_section: bool,

    /// Keep text controls visible even when text is not active
    #[serde(default = "default_show_text_controls")]
    pub show_text_controls: bool,

    /// Show the Settings section (config shortcuts, layout controls)
    #[serde(default = "default_show_settings_section")]
    pub show_settings_section: bool,

    /// Show delay sliders in Step Undo/Redo section
    #[serde(default = "default_show_delay_sliders")]
    pub show_delay_sliders: bool,

    /// Show the marker opacity slider section in the side toolbar
    #[serde(default = "default_show_marker_opacity_section")]
    pub show_marker_opacity_section: bool,

    /// Show preset action toast notifications
    #[serde(default = "default_show_preset_toasts")]
    pub show_preset_toasts: bool,

    /// Show the cursor tool preview bubble
    #[serde(default = "default_show_tool_preview")]
    pub show_tool_preview: bool,

    /// Saved horizontal offset for the top toolbar (layer-shell/inline)
    #[serde(default)]
    pub top_offset: f64,

    /// Saved vertical offset for the top toolbar (layer-shell/inline)
    #[serde(default)]
    pub top_offset_y: f64,

    /// Saved vertical offset for the side toolbar (layer-shell/inline)
    #[serde(default)]
    pub side_offset: f64,

    /// Saved horizontal offset for the side toolbar (layer-shell/inline)
    #[serde(default)]
    pub side_offset_x: f64,

    /// Force inline toolbars even when layer-shell is available (debug/compatibility).
    #[serde(default)]
    pub force_inline: bool,
}

impl Default for ToolbarConfig {
    fn default() -> Self {
        Self {
            layout_mode: default_toolbar_layout_mode(),
            mode_overrides: ToolbarModeOverrides::default(),
            top_pinned: default_toolbar_top_pinned(),
            side_pinned: default_toolbar_side_pinned(),
            use_icons: default_toolbar_use_icons(),
            show_more_colors: default_show_more_colors(),
            show_actions_section: default_show_actions_section(),
            show_actions_advanced: default_show_actions_advanced(),
            show_pages_section: default_show_pages_section(),
            show_presets: default_show_presets(),
            show_step_section: default_show_step_section(),
            show_text_controls: default_show_text_controls(),
            show_settings_section: default_show_settings_section(),
            show_delay_sliders: default_show_delay_sliders(),
            show_marker_opacity_section: default_show_marker_opacity_section(),
            show_preset_toasts: default_show_preset_toasts(),
            show_tool_preview: default_show_tool_preview(),
            top_offset: 0.0,
            top_offset_y: 0.0,
            side_offset: 0.0,
            side_offset_x: 0.0,
            force_inline: false,
        }
    }
}

fn default_toolbar_top_pinned() -> bool {
    true
}

fn default_toolbar_side_pinned() -> bool {
    true
}

fn default_toolbar_use_icons() -> bool {
    true
}

fn default_toolbar_layout_mode() -> ToolbarLayoutMode {
    ToolbarLayoutMode::Regular
}

fn default_show_more_colors() -> bool {
    false
}

fn default_show_actions_section() -> bool {
    true
}

fn default_show_actions_advanced() -> bool {
    false
}

fn default_show_pages_section() -> bool {
    true
}

fn default_show_presets() -> bool {
    true
}

fn default_show_step_section() -> bool {
    false
}

fn default_show_text_controls() -> bool {
    false
}

fn default_show_settings_section() -> bool {
    true
}

fn default_show_delay_sliders() -> bool {
    false
}

fn default_show_marker_opacity_section() -> bool {
    false
}

fn default_show_preset_toasts() -> bool {
    true
}

fn default_show_tool_preview() -> bool {
    false
}
