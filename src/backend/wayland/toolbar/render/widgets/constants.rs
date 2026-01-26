//! Rendering constants for toolbar UI elements.
//!
//! This module centralizes colors, font sizes, spacing values, and other
//! magic numbers used throughout toolbar rendering code.

#![allow(dead_code)] // Some constants are reserved for future use

// ============================================================================
// COLORS - RGBA tuples for UI elements
// ============================================================================

/// White text/icon color with high opacity
pub const COLOR_TEXT_PRIMARY: (f64, f64, f64, f64) = (1.0, 1.0, 1.0, 0.95);

/// White text with slightly lower opacity
pub const COLOR_TEXT_SECONDARY: (f64, f64, f64, f64) = (1.0, 1.0, 1.0, 0.9);

/// White text with medium opacity (for less prominent elements)
pub const COLOR_TEXT_TERTIARY: (f64, f64, f64, f64) = (1.0, 1.0, 1.0, 0.85);

/// Section header label color
pub const COLOR_LABEL_SECTION: (f64, f64, f64, f64) = (0.8, 0.8, 0.85, 0.9);

/// Hint/secondary label color
pub const COLOR_LABEL_HINT: (f64, f64, f64, f64) = (0.7, 0.7, 0.75, 0.8);

/// Disabled text color (lower opacity for clearer distinction from enabled)
pub const COLOR_TEXT_DISABLED: (f64, f64, f64, f64) = (0.4, 0.4, 0.45, 0.35);

// Icon colors
/// Icon color when hovered (fully opaque white)
pub const COLOR_ICON_HOVER: (f64, f64, f64, f64) = (1.0, 1.0, 1.0, 1.0);

/// Icon color default state
pub const COLOR_ICON_DEFAULT: (f64, f64, f64, f64) = (0.95, 0.95, 0.95, 0.9);

/// Icon hover background glow (subtle highlight behind icons on hover)
pub const COLOR_ICON_HOVER_BG: (f64, f64, f64, f64) = (1.0, 1.0, 1.0, 0.08);

// Button states
/// Active/selected button background (brighter blue accent for better visibility)
pub const COLOR_BUTTON_ACTIVE: (f64, f64, f64, f64) = (0.3, 0.55, 1.0, 1.0);

/// Hovered button background
pub const COLOR_BUTTON_HOVER: (f64, f64, f64, f64) = (0.35, 0.35, 0.45, 0.85);

/// Default button background
pub const COLOR_BUTTON_DEFAULT: (f64, f64, f64, f64) = (0.2, 0.22, 0.26, 0.75);

// Checkbox states
/// Checkbox hover state
pub const COLOR_CHECKBOX_HOVER: (f64, f64, f64, f64) = (0.32, 0.34, 0.4, 0.9);

/// Checkbox default state
pub const COLOR_CHECKBOX_DEFAULT: (f64, f64, f64, f64) = (0.22, 0.24, 0.28, 0.75);

/// Mini checkbox checked state (green tint)
pub const COLOR_CHECKBOX_CHECKED: (f64, f64, f64, f64) = (0.25, 0.5, 0.35, 0.9);

/// Mini checkbox hover state
pub const COLOR_MINI_CHECKBOX_HOVER: (f64, f64, f64, f64) = (0.32, 0.34, 0.4, 0.85);

/// Mini checkbox default state
pub const COLOR_MINI_CHECKBOX_DEFAULT: (f64, f64, f64, f64) = (0.2, 0.22, 0.26, 0.7);

// Pin button states
/// Pinned state (green)
pub const COLOR_PIN_ACTIVE: (f64, f64, f64, f64) = (0.25, 0.6, 0.35, 0.95);

/// Pin button hover
pub const COLOR_PIN_HOVER: (f64, f64, f64, f64) = (0.35, 0.35, 0.45, 0.85);

/// Pin button default
pub const COLOR_PIN_DEFAULT: (f64, f64, f64, f64) = (0.3, 0.3, 0.35, 0.7);

// Close button
/// Close button hover (red tint)
pub const COLOR_CLOSE_HOVER: (f64, f64, f64, f64) = (0.8, 0.3, 0.3, 0.9);

/// Close button default
pub const COLOR_CLOSE_DEFAULT: (f64, f64, f64, f64) = (0.5, 0.5, 0.55, 0.7);

// Slider/track elements
/// Slider track background
pub const COLOR_TRACK_BACKGROUND: (f64, f64, f64, f64) = (0.5, 0.5, 0.6, 0.6);

/// Slider knob (blue accent)
pub const COLOR_TRACK_KNOB: (f64, f64, f64, f64) = (0.25, 0.5, 0.95, 0.9);

// Card/panel backgrounds
/// Main panel background
pub const COLOR_PANEL_BACKGROUND: (f64, f64, f64, f64) = (0.05, 0.05, 0.08, 0.92);

/// Group card background
pub const COLOR_CARD_BACKGROUND: (f64, f64, f64, f64) = (0.12, 0.12, 0.18, 0.35);

// ============================================================================
// PANEL RADIUS
// ============================================================================

/// Panel corner radius (larger than buttons)
pub const RADIUS_PANEL: f64 = 14.0;

/// Card corner radius
pub const RADIUS_CARD: f64 = 8.0;

// Tooltip
/// Tooltip background
pub const COLOR_TOOLTIP_BACKGROUND: (f64, f64, f64, f64) = (0.1, 0.1, 0.15, 0.95);

/// Tooltip border
pub const COLOR_TOOLTIP_BORDER: (f64, f64, f64, f64) = (0.4, 0.4, 0.5, 0.8);

/// Tooltip shadow
pub const COLOR_TOOLTIP_SHADOW: (f64, f64, f64, f64) = (0.0, 0.0, 0.0, 0.3);

// ============================================================================
// FONT SIZES
// ============================================================================

/// Small font size (hints, counters)
pub const FONT_SIZE_SMALL: f64 = 10.0;

/// Secondary font size (mini checkboxes, delay values)
pub const FONT_SIZE_SECONDARY: f64 = 11.0;

/// Tooltip and settings font size
pub const FONT_SIZE_TOOLTIP: f64 = 12.0;

/// Default label font size
pub const FONT_SIZE_LABEL: f64 = 13.0;

// ============================================================================
// FONT FAMILIES
// ============================================================================

/// Default sans-serif font family
pub const FONT_FAMILY_DEFAULT: &str = "Sans";

/// Monospace font family (for code/fixed-width text)
pub const FONT_FAMILY_MONO: &str = "Monospace";

// ============================================================================
// SPACING & PADDING
// ============================================================================

/// Extra small spacing (checkmark offsets, etc.)
pub const SPACING_XS: f64 = 2.0;

/// Small spacing (mini checkbox padding)
pub const SPACING_SM: f64 = 3.0;

/// Medium spacing (standard gaps, corner radius)
pub const SPACING_MD: f64 = 4.0;

/// Standard spacing (padding, gaps between elements)
pub const SPACING_STD: f64 = 6.0;

/// Large spacing (section gaps, checkbox padding)
pub const SPACING_LG: f64 = 8.0;

// ============================================================================
// CORNER RADIUS
// ============================================================================

/// Small corner radius (mini elements)
pub const RADIUS_SM: f64 = 3.0;

/// Standard corner radius (buttons, cards)
pub const RADIUS_STD: f64 = 4.0;

/// Large corner radius (panels, large buttons)
pub const RADIUS_LG: f64 = 6.0;

// ============================================================================
// LINE WIDTHS
// ============================================================================

/// Thin line width (borders, strokes)
pub const LINE_WIDTH_THIN: f64 = 1.0;

/// Standard line width (checkbox borders)
pub const LINE_WIDTH_STD: f64 = 1.5;

/// Thick line width (X marks, pins)
pub const LINE_WIDTH_THICK: f64 = 2.0;

// ============================================================================
// OPACITY VALUES (for hover/active states)
// ============================================================================

/// Hover state alpha modifier
pub const ALPHA_HOVER: f64 = 0.9;

/// Default state alpha modifier
pub const ALPHA_DEFAULT: f64 = 0.6;

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

/// Apply an RGBA color tuple to a Cairo context
#[inline]
pub fn set_color(ctx: &cairo::Context, color: (f64, f64, f64, f64)) {
    ctx.set_source_rgba(color.0, color.1, color.2, color.3);
}
