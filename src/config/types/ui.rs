use crate::config::enums::StatusPosition;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{
    ClickHighlightConfig, ContextMenuUiConfig, HelpOverlayStyle, StatusBarStyle, ToolbarConfig,
};

/// UI display preferences.
///
/// Controls the visibility and positioning of on-screen UI elements.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct UiConfig {
    /// Show the status bar displaying current color, thickness, and tool
    #[serde(default = "default_show_status")]
    pub show_status_bar: bool,

    /// Show the board label in the status bar
    #[serde(default = "default_show_status_board_badge")]
    pub show_status_board_badge: bool,

    /// Show the page counter in the status bar
    #[serde(default = "default_show_status_page_badge")]
    pub show_status_page_badge: bool,

    /// Show the board/page badge even when the status bar is visible
    /// (renamed from show_page_badge_with_status_bar for clarity)
    #[serde(
        default = "default_show_page_badge_with_status_bar",
        alias = "show_page_badge_with_status_bar"
    )]
    pub show_floating_badge_always: bool,

    /// Show the frozen-mode badge when frozen is active
    #[serde(default = "default_show_frozen_badge")]
    pub show_frozen_badge: bool,

    /// Status bar screen position (top-left, top-right, bottom-left, bottom-right)
    #[serde(default = "default_status_position")]
    pub status_bar_position: StatusPosition,

    /// Status bar styling options
    #[serde(default)]
    pub status_bar_style: StatusBarStyle,

    /// Help overlay styling options
    #[serde(default)]
    pub help_overlay_style: HelpOverlayStyle,

    /// Filter help overlay sections based on enabled features
    #[serde(default = "default_help_overlay_context_filter")]
    pub help_overlay_context_filter: bool,

    /// Preferred output name for the xdg-shell fallback overlay (GNOME).
    /// Falls back to last entered output or first available.
    #[serde(default)]
    pub preferred_output: Option<String>,

    /// Enable multi-monitor features on layer-shell compositors.
    ///
    /// When disabled, output-cycling actions are ignored and the overlay remains
    /// on the compositor-selected output.
    #[serde(default = "default_multi_monitor_enabled")]
    pub multi_monitor_enabled: bool,

    /// Show active output identity in the status bar.
    #[serde(default = "default_active_output_badge")]
    pub active_output_badge: bool,

    /// Duration for command palette action toasts (ms)
    #[serde(default = "default_command_palette_toast_duration_ms")]
    pub command_palette_toast_duration_ms: u64,

    /// Use fullscreen for the xdg-shell fallback (GNOME). Disable if fullscreen
    /// produces an opaque background; maximized is used when false.
    #[serde(default = "default_xdg_fullscreen")]
    pub xdg_fullscreen: bool,

    /// Click highlight visual indicator settings
    #[serde(default)]
    pub click_highlight: ClickHighlightConfig,

    /// Context menu preferences
    #[serde(default)]
    pub context_menu: ContextMenuUiConfig,

    /// Toolbar visibility and pinning options
    #[serde(default)]
    pub toolbar: ToolbarConfig,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            show_status_bar: default_show_status(),
            show_status_board_badge: default_show_status_board_badge(),
            show_status_page_badge: default_show_status_page_badge(),
            show_floating_badge_always: default_show_page_badge_with_status_bar(),
            show_frozen_badge: default_show_frozen_badge(),
            status_bar_position: default_status_position(),
            status_bar_style: StatusBarStyle::default(),
            help_overlay_style: HelpOverlayStyle::default(),
            help_overlay_context_filter: default_help_overlay_context_filter(),
            preferred_output: None,
            multi_monitor_enabled: default_multi_monitor_enabled(),
            active_output_badge: default_active_output_badge(),
            command_palette_toast_duration_ms: default_command_palette_toast_duration_ms(),
            xdg_fullscreen: default_xdg_fullscreen(),
            click_highlight: ClickHighlightConfig::default(),
            context_menu: ContextMenuUiConfig::default(),
            toolbar: ToolbarConfig::default(),
        }
    }
}

fn default_show_status() -> bool {
    true
}

fn default_show_status_board_badge() -> bool {
    true
}

fn default_show_status_page_badge() -> bool {
    true
}

fn default_show_page_badge_with_status_bar() -> bool {
    false
}

fn default_show_frozen_badge() -> bool {
    false
}

fn default_xdg_fullscreen() -> bool {
    false
}

fn default_help_overlay_context_filter() -> bool {
    true
}

fn default_command_palette_toast_duration_ms() -> u64 {
    1500
}

fn default_multi_monitor_enabled() -> bool {
    true
}

fn default_active_output_badge() -> bool {
    true
}

fn default_status_position() -> StatusPosition {
    StatusPosition::BottomLeft
}
