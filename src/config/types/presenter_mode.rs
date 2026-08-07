use serde::{Deserialize, Serialize};

/// Presenter mode customization options.
#[cfg_attr(feature = "config-schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PresenterModeConfig {
    /// Hide the status bar while presenter mode is active.
    #[serde(default = "default_hide_status_bar")]
    pub hide_status_bar: bool,

    /// Hide the floating toolbars while presenter mode is active.
    #[serde(default = "default_hide_toolbars")]
    pub hide_toolbars: bool,

    /// What `hide_toolbars` does to the top strip: "hidden" removes it
    /// entirely (default, today's behavior); "micro" collapses it to the
    /// 44px micro chip instead.
    #[serde(default)]
    pub toolbar_mode: PresenterToolbarMode,

    /// Hide the tool preview while presenter mode is active.
    #[serde(default = "default_hide_tool_preview")]
    pub hide_tool_preview: bool,

    /// Close the help overlay when entering presenter mode.
    #[serde(default = "default_close_help_overlay")]
    pub close_help_overlay: bool,

    /// Force click highlights on while presenter mode is active.
    #[serde(default = "default_enable_click_highlight")]
    pub enable_click_highlight: bool,

    /// Force the input HUD on while presenter mode is active.
    #[serde(default = "default_enable_input_hud")]
    pub enable_input_hud: bool,

    /// Tool behavior while presenter mode is active.
    #[serde(default)]
    pub tool_behavior: PresenterToolBehavior,

    /// Show the enter/exit toast for presenter mode.
    #[serde(default = "default_show_toast")]
    pub show_toast: bool,
}

impl Default for PresenterModeConfig {
    fn default() -> Self {
        Self {
            hide_status_bar: default_hide_status_bar(),
            hide_toolbars: default_hide_toolbars(),
            toolbar_mode: PresenterToolbarMode::default(),
            hide_tool_preview: default_hide_tool_preview(),
            close_help_overlay: default_close_help_overlay(),
            enable_click_highlight: default_enable_click_highlight(),
            enable_input_hud: default_enable_input_hud(),
            tool_behavior: PresenterToolBehavior::default(),
            show_toast: default_show_toast(),
        }
    }
}

/// How `hide_toolbars` treats the top strip while presenting.
#[cfg_attr(feature = "config-schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum PresenterToolbarMode {
    /// Hide the top strip.
    #[default]
    Hidden,
    /// Collapse the top strip to the micro chip.
    Micro,
}

#[cfg_attr(feature = "config-schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum PresenterToolBehavior {
    /// Leave the active tool unchanged.
    Keep,
    /// Switch to highlight on entry, but allow tool changes afterward.
    #[default]
    ForceHighlight,
    /// Switch to highlight and prevent tool changes while presenting.
    ForceHighlightLocked,
}

fn default_hide_status_bar() -> bool {
    true
}

fn default_hide_toolbars() -> bool {
    true
}

fn default_hide_tool_preview() -> bool {
    true
}

fn default_close_help_overlay() -> bool {
    true
}

fn default_enable_click_highlight() -> bool {
    true
}

fn default_enable_input_hud() -> bool {
    false
}

fn default_show_toast() -> bool {
    true
}
