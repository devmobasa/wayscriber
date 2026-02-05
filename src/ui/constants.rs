//! Centralized UI constants for colors, spacing, and timing.
//!
//! This module provides a single source of truth for UI styling values
//! used throughout the application's overlay and modal rendering code.

#![allow(dead_code)] // Some constants reserved for future use

// ============================================================================
// OVERLAY DIMMING - Background alpha values for modals and overlays
// ============================================================================

/// Popover/quick overlay dimming (e.g., board picker quick mode)
pub const OVERLAY_DIM_LIGHT: f64 = 0.20;

/// Standard modal dimming (e.g., board picker full, command palette)
pub const OVERLAY_DIM_MEDIUM: f64 = 0.50;

/// Heavy dimming for important/tutorial overlays (e.g., tour)
pub const OVERLAY_DIM_HEAVY: f64 = 0.70;

/// Help overlay dimming
pub const OVERLAY_DIM_HELP: f64 = 0.55;

// ============================================================================
// PANEL BACKGROUNDS - RGBA tuples for modal/panel backgrounds
// ============================================================================

/// Context menu background
pub const PANEL_BG_CONTEXT_MENU: (f64, f64, f64, f64) = (0.10, 0.13, 0.17, 0.95);

/// Board picker panel background
pub const PANEL_BG_BOARD_PICKER: (f64, f64, f64, f64) = (0.09, 0.11, 0.15, 0.96);

/// Properties panel background
pub const PANEL_BG_PROPERTIES: (f64, f64, f64, f64) = (0.08, 0.11, 0.17, 0.92);

/// Command palette background
pub const PANEL_BG_COMMAND_PALETTE: (f64, f64, f64, f64) = (0.15, 0.15, 0.18, 0.98);

/// Tour/modal dialog background
pub const PANEL_BG_MODAL: (f64, f64, f64, f64) = (0.15, 0.15, 0.18, 0.98);

// ============================================================================
// PANEL BORDERS - RGBA tuples for modal/panel borders
// ============================================================================

/// Context menu border
pub const BORDER_CONTEXT_MENU: (f64, f64, f64, f64) = (0.18, 0.22, 0.28, 0.9);

/// Board picker border
pub const BORDER_BOARD_PICKER: (f64, f64, f64, f64) = (0.20, 0.24, 0.30, 0.9);

/// Properties panel border
pub const BORDER_PROPERTIES: (f64, f64, f64, f64) = (0.18, 0.22, 0.30, 0.95);

/// Command palette border
pub const BORDER_COMMAND_PALETTE: (f64, f64, f64, f64) = (0.40, 0.40, 0.45, 0.5);

/// Tour/modal border
pub const BORDER_MODAL: (f64, f64, f64, f64) = (0.40, 0.40, 0.50, 0.6);

// ============================================================================
// TEXT COLORS - Primary, secondary, hint, and disabled text
// ============================================================================

/// Primary text (titles, main content) - high contrast
pub const TEXT_PRIMARY: (f64, f64, f64, f64) = (0.93, 0.95, 0.99, 1.0);

/// Secondary text (body content) - slightly dimmer
pub const TEXT_SECONDARY: (f64, f64, f64, f64) = (0.86, 0.89, 0.94, 1.0);

/// Hint/shortcut text - improved contrast (was 0.8 alpha)
pub const TEXT_HINT: (f64, f64, f64, f64) = (0.70, 0.73, 0.78, 0.9);

/// Disabled text - improved readability (was 0.5 alpha)
pub const TEXT_DISABLED: (f64, f64, f64, f64) = (0.60, 0.64, 0.68, 0.65);

/// Placeholder text
pub const TEXT_PLACEHOLDER: (f64, f64, f64, f64) = (0.50, 0.50, 0.55, 0.7);

/// Tertiary/muted text (footers, less important info)
pub const TEXT_TERTIARY: (f64, f64, f64, f64) = (0.64, 0.69, 0.76, 0.9);

/// Active/highlighted text
pub const TEXT_ACTIVE: (f64, f64, f64, f64) = (0.96, 0.98, 1.0, 1.0);

/// White text (for use on colored backgrounds)
pub const TEXT_WHITE: (f64, f64, f64, f64) = (1.0, 1.0, 1.0, 1.0);

/// Description/subtitle text - improved contrast (was 0.6-0.65)
pub const TEXT_DESCRIPTION: (f64, f64, f64, f64) = (0.65, 0.68, 0.73, 0.85);

// ============================================================================
// INTERACTIVE STATES - Hover, focus, active, and selection backgrounds
// ============================================================================

/// Hover state background (mouse hover)
pub const BG_HOVER: (f64, f64, f64, f64) = (0.25, 0.32, 0.45, 0.9);

/// Keyboard focus border color
pub const BORDER_FOCUS: (f64, f64, f64, f64) = (0.40, 0.60, 0.95, 0.9);

/// Selection/highlight background
pub const BG_SELECTION: (f64, f64, f64, f64) = (0.22, 0.28, 0.38, 0.9);

/// Active/selected item indicator
pub const BG_SELECTED_INDICATOR: (f64, f64, f64, f64) = (0.33, 0.42, 0.58, 0.9);

/// Accent color for highlights and active elements
pub const ACCENT_PRIMARY: (f64, f64, f64, f64) = (0.30, 0.50, 0.80, 0.9);

/// Command palette/input selection highlight
pub const BG_INPUT_SELECTION: (f64, f64, f64, f64) = (0.30, 0.50, 0.80, 0.4);

// ============================================================================
// INPUT ELEMENTS - Text inputs, search boxes
// ============================================================================

/// Input field background
pub const INPUT_BG: (f64, f64, f64, f64) = (0.10, 0.10, 0.12, 1.0);

/// Input field border (focused)
pub const INPUT_BORDER_FOCUSED: (f64, f64, f64, f64) = (0.30, 0.50, 0.80, 0.6);

/// Caret/cursor color
pub const INPUT_CARET: (f64, f64, f64, f64) = (0.98, 0.92, 0.55, 1.0);

// ============================================================================
// DIVIDERS AND SEPARATORS
// ============================================================================

/// Standard divider line
pub const DIVIDER: (f64, f64, f64, f64) = (0.35, 0.40, 0.50, 0.9);

/// Lighter divider (for subtle separation)
pub const DIVIDER_LIGHT: (f64, f64, f64, f64) = (0.35, 0.40, 0.48, 0.6);

// ============================================================================
// SHADOWS
// ============================================================================

/// Standard drop shadow
pub const SHADOW: (f64, f64, f64, f64) = (0.0, 0.0, 0.0, 0.30);

/// Deeper shadow for layered elements
pub const SHADOW_DEEP: (f64, f64, f64, f64) = (0.0, 0.0, 0.0, 0.35);

// ============================================================================
// TOAST COLORS - Background colors by toast type
// ============================================================================

/// Info toast background
pub const TOAST_INFO: (f64, f64, f64) = (0.25, 0.70, 0.90);

/// Warning toast background
pub const TOAST_WARNING: (f64, f64, f64) = (0.92, 0.62, 0.18);

/// Error toast background
pub const TOAST_ERROR: (f64, f64, f64) = (0.90, 0.30, 0.30);

/// Success toast background (for preset apply)
pub const TOAST_SUCCESS: (f64, f64, f64) = (0.20, 0.70, 0.40);

/// Blocked action border flash
pub const BLOCKED_FLASH: (f64, f64, f64) = (0.90, 0.20, 0.20);

// ============================================================================
// PROGRESS INDICATORS
// ============================================================================

/// Progress bar track/background
pub const PROGRESS_TRACK: (f64, f64, f64, f64) = (0.30, 0.30, 0.35, 1.0);

/// Progress bar fill
pub const PROGRESS_FILL: (f64, f64, f64, f64) = (0.40, 0.60, 1.0, 1.0);

// ============================================================================
// SPECIAL ELEMENT COLORS
// ============================================================================

/// Active board indicator (gold)
pub const INDICATOR_ACTIVE_BOARD: (f64, f64, f64, f64) = (0.90, 0.83, 0.32, 0.95);

/// Pin icon active (gold)
pub const ICON_PIN_ACTIVE: (f64, f64, f64, f64) = (0.96, 0.82, 0.28, 0.95);

/// Pin icon inactive
pub const ICON_PIN_INACTIVE: (f64, f64, f64, f64) = (0.60, 0.65, 0.72, 0.5);

/// Drag handle dots
pub const ICON_DRAG_HANDLE: (f64, f64, f64, f64) = (0.58, 0.63, 0.70, 0.85);

/// Submenu arrow
pub const ICON_SUBMENU_ARROW: (f64, f64, f64, f64) = (0.75, 0.78, 0.84, 1.0);

// ============================================================================
// SPACING - Consistent spacing scale (in pixels)
// ============================================================================

/// Extra small spacing (2px)
pub const SPACING_XS: f64 = 2.0;

/// Small spacing (4px)
pub const SPACING_SM: f64 = 4.0;

/// Medium spacing (8px)
pub const SPACING_MD: f64 = 8.0;

/// Standard spacing (12px)
pub const SPACING_STD: f64 = 12.0;

/// Large spacing (16px)
pub const SPACING_LG: f64 = 16.0;

/// Extra large spacing (24px)
pub const SPACING_XL: f64 = 24.0;

/// XXL spacing (32px)
pub const SPACING_XXL: f64 = 32.0;

/// Panel padding (48px) - for large modals like tour
pub const SPACING_PANEL: f64 = 48.0;

// ============================================================================
// CORNER RADII
// ============================================================================

/// Small corner radius (4px)
pub const RADIUS_SM: f64 = 4.0;

/// Standard corner radius (6px)
pub const RADIUS_STD: f64 = 6.0;

/// Medium corner radius (8px)
pub const RADIUS_MD: f64 = 8.0;

/// Large corner radius (10px)
pub const RADIUS_LG: f64 = 10.0;

/// Panel corner radius (12px)
pub const RADIUS_PANEL: f64 = 12.0;

/// XL corner radius (16px) - for large overlays
pub const RADIUS_XL: f64 = 16.0;

// ============================================================================
// ANIMATION TIMING (milliseconds)
// ============================================================================

/// Fast animation (100ms) - quick feedback
pub const ANIM_FAST_MS: u64 = 100;

/// Normal animation (200ms) - standard transitions
pub const ANIM_NORMAL_MS: u64 = 200;

/// Slow animation (300ms) - modal open/close
pub const ANIM_SLOW_MS: u64 = 300;

// ============================================================================
// FOCUS RING STYLING
// ============================================================================

/// Focus ring width
pub const FOCUS_RING_WIDTH: f64 = 2.0;

/// Focus ring offset from element
pub const FOCUS_RING_OFFSET: f64 = 2.0;

// ============================================================================
// KEYBOARD NAVIGATION HINT TEXT
// ============================================================================

/// Context menu navigation hint
pub const NAV_HINT_MENU: &str = "↑↓ to navigate • Enter to select • Esc to close";

/// Board picker navigation hint
pub const NAV_HINT_BOARD_PICKER: &str = "↑↓ Navigate • Type to search";

/// Modal close hint
pub const HINT_PRESS_ESC: &str = "Press Escape to close";

// ============================================================================
// EMPTY STATE MESSAGES
// ============================================================================

/// Properties panel empty state
pub const EMPTY_PROPERTIES: &str = "Select a shape to view properties";

/// Command palette no results - main message
pub const EMPTY_COMMAND_PALETTE: &str = "No matching commands";

/// Command palette no results - suggestions
pub const EMPTY_COMMAND_SUGGESTIONS: &str = "Try: pen, color, undo, help";

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

/// Apply an RGBA color tuple to a Cairo context
#[inline]
pub fn set_color(ctx: &cairo::Context, color: (f64, f64, f64, f64)) {
    ctx.set_source_rgba(color.0, color.1, color.2, color.3);
}

/// Apply an RGB color tuple with custom alpha to a Cairo context
#[inline]
pub fn set_color_alpha(ctx: &cairo::Context, color: (f64, f64, f64), alpha: f64) {
    ctx.set_source_rgba(color.0, color.1, color.2, alpha);
}

/// Apply an RGBA color with modified alpha
#[inline]
pub fn with_alpha(color: (f64, f64, f64, f64), alpha: f64) -> (f64, f64, f64, f64) {
    (color.0, color.1, color.2, alpha)
}

/// Linear interpolation between two colors
#[inline]
pub fn lerp_color(
    from: (f64, f64, f64, f64),
    to: (f64, f64, f64, f64),
    t: f64,
) -> (f64, f64, f64, f64) {
    let t = t.clamp(0.0, 1.0);
    (
        from.0 + (to.0 - from.0) * t,
        from.1 + (to.1 - from.1) * t,
        from.2 + (to.2 - from.2) * t,
        from.3 + (to.3 - from.3) * t,
    )
}
