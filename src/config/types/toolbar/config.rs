use serde::{Deserialize, Serialize};

use super::{
    ToolbarBackendKind, ToolbarItemsConfig, ToolbarLayoutMode, ToolbarModeOverrides,
    ToolbarRebindModifier, TopDisplayMode,
};

/// Toolbar visibility and pinning configuration.
///
/// Controls which toolbar panels are visible on startup and whether they
/// remain pinned (saved to config) when closed.
#[cfg_attr(feature = "config-schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolbarConfig {
    /// Toolbar frontend ("auto", "gtk", "builtin")
    #[serde(default)]
    pub backend: ToolbarBackendKind,

    /// Toolbar layout preset (simple, regular, advanced)
    #[serde(default = "default_toolbar_layout_mode")]
    pub layout_mode: ToolbarLayoutMode,

    /// Optional per-mode overrides for toolbar sections
    #[serde(default)]
    pub mode_overrides: ToolbarModeOverrides,

    /// Optional item-level toolbar visibility customizations.
    #[serde(default)]
    pub items: ToolbarItemsConfig,

    /// Show the top toolbar (tool selection) on startup
    #[serde(default = "default_toolbar_top_pinned")]
    pub top_pinned: bool,

    /// Show the side toolbar (colors, settings) on startup
    #[serde(default = "default_toolbar_side_pinned")]
    pub side_pinned: bool,

    /// Start the top toolbar minimized to its edge restore tab
    #[serde(default)]
    pub top_minimized: bool,

    /// Display form of the top strip restored at startup ("full", "micro").
    /// "hidden" is accepted but treated as "full"; visibility at startup is
    /// governed by `top_pinned`.
    #[serde(default)]
    pub top_display_mode: TopDisplayMode,

    /// Start the side toolbar minimized to its edge restore tab
    #[serde(default)]
    pub side_minimized: bool,

    /// Side-palette pane restored at startup ("draw", "canvas", "session", "settings")
    #[serde(default = "default_side_active_pane")]
    pub side_active_pane: String,

    /// Side-palette sections collapsed to their header row
    #[serde(default)]
    pub collapsed_sections: Vec<String>,

    /// Use icons instead of text labels in toolbars
    #[serde(default = "default_toolbar_use_icons")]
    pub use_icons: bool,

    /// Scale factor for toolbar UI (icons + layout). 1.0 = default.
    #[serde(default = "default_toolbar_scale")]
    pub scale: f64,

    /// Show extended color palette
    #[serde(default = "default_show_more_colors")]
    pub show_more_colors: bool,

    /// Show the Actions section (undo/redo/clear)
    #[serde(default = "default_show_actions_section")]
    pub show_actions_section: bool,

    /// Show advanced actions (undo all, delay, freeze, etc.)
    #[serde(default = "default_show_actions_advanced")]
    pub show_actions_advanced: bool,

    /// Show zoom actions (zoom in/out/reset/lock)
    #[serde(default = "default_show_zoom_actions")]
    pub show_zoom_actions: bool,

    /// Show the Pages section in the side toolbar
    #[serde(default = "default_show_pages_section")]
    pub show_pages_section: bool,

    /// Show the Boards section in the side toolbar
    #[serde(default = "default_show_boards_section")]
    pub show_boards_section: bool,

    /// Show the presets section in the side toolbar
    #[serde(default = "default_show_presets")]
    pub show_presets: bool,

    /// Show the Step Undo/Redo section
    #[serde(default = "default_show_step_section")]
    pub show_step_section: bool,

    /// Keep text controls visible even when text is not active
    #[serde(default = "default_show_text_controls")]
    pub show_text_controls: bool,

    /// Deprecated compatibility mirror. Settings navigation is always reachable.
    #[serde(default = "default_show_settings_section")]
    pub show_settings_section: bool,

    /// Show delay sliders in Step Undo/Redo section
    #[serde(default = "default_show_delay_sliders")]
    pub show_delay_sliders: bool,

    /// Show the marker opacity slider section in the side toolbar
    #[serde(default = "default_show_marker_opacity_section")]
    pub show_marker_opacity_section: bool,

    /// Enable context-aware UI that shows/hides controls based on the active tool
    #[serde(default = "default_context_aware_ui")]
    pub context_aware_ui: bool,

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

    /// Modifier chord used to edit a clicked control's keyboard shortcut.
    #[serde(default)]
    pub rebind_modifier: ToolbarRebindModifier,
}

impl Default for ToolbarConfig {
    fn default() -> Self {
        Self {
            backend: ToolbarBackendKind::default(),
            layout_mode: default_toolbar_layout_mode(),
            mode_overrides: ToolbarModeOverrides::default(),
            items: ToolbarItemsConfig::default(),
            top_pinned: default_toolbar_top_pinned(),
            side_pinned: default_toolbar_side_pinned(),
            top_minimized: false,
            top_display_mode: TopDisplayMode::default(),
            side_minimized: false,
            side_active_pane: default_side_active_pane(),
            collapsed_sections: Vec::new(),
            use_icons: default_toolbar_use_icons(),
            scale: default_toolbar_scale(),
            show_more_colors: default_show_more_colors(),
            show_actions_section: default_show_actions_section(),
            show_actions_advanced: default_show_actions_advanced(),
            show_zoom_actions: default_show_zoom_actions(),
            show_pages_section: default_show_pages_section(),
            show_boards_section: default_show_boards_section(),
            show_presets: default_show_presets(),
            show_step_section: default_show_step_section(),
            show_text_controls: default_show_text_controls(),
            show_settings_section: default_show_settings_section(),
            show_delay_sliders: default_show_delay_sliders(),
            show_marker_opacity_section: default_show_marker_opacity_section(),
            context_aware_ui: default_context_aware_ui(),
            show_preset_toasts: default_show_preset_toasts(),
            show_tool_preview: default_show_tool_preview(),
            top_offset: 0.0,
            top_offset_y: 0.0,
            side_offset: 0.0,
            side_offset_x: 0.0,
            force_inline: false,
            rebind_modifier: ToolbarRebindModifier::default(),
        }
    }
}

fn default_toolbar_top_pinned() -> bool {
    true
}

fn default_toolbar_side_pinned() -> bool {
    true
}

fn default_side_active_pane() -> String {
    "draw".to_string()
}

fn default_toolbar_use_icons() -> bool {
    true
}

fn default_toolbar_scale() -> f64 {
    1.0
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

fn default_show_zoom_actions() -> bool {
    true
}

fn default_show_pages_section() -> bool {
    true
}

fn default_show_boards_section() -> bool {
    true
}

fn default_show_presets() -> bool {
    true
}

fn default_show_step_section() -> bool {
    false
}

fn default_show_text_controls() -> bool {
    true
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

fn default_context_aware_ui() -> bool {
    true
}

fn default_show_preset_toasts() -> bool {
    true
}

fn default_show_tool_preview() -> bool {
    false
}
