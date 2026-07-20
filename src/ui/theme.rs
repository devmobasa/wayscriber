//! Canonical design tokens for all wayscriber chrome (M1 theme foundation).
//!
//! This module is the single source of truth for UI styling values. The
//! legacy constant modules ([`crate::ui::constants`] and
//! `crate::backend::wayland::toolbar::render::widgets::constants`) are thin
//! re-export shims kept so existing call sites keep compiling; new code
//! should reference `theme::` directly.
//!
//! Layout:
//! - core palette: RGB roots shared by every surface (accent, destructive,
//!   shadow) so derived tints cannot drift apart again
//! - [`overlay`]: tokens for overlay-drawn surfaces (popups, panels, HUD,
//!   help, palette) — moved verbatim from the old `ui::constants`
//! - [`toolbar`]: tokens for the toolbar renderers (builtin Cairo bars and
//!   the generated GTK stylesheet) — moved verbatim from the old
//!   `toolbar::render::widgets::constants`
//! - runtime [`Theme`] with dark/light chrome variants, consumed by surfaces
//!   as they migrate in M2+ (`[ui] theme` config selects the mode)
//!
//! Where `overlay` and `toolbar` hold near-but-not-equal values for the same
//! semantic role, both tokens are kept and marked `TODO(theme-consolidation)`
//! — consolidation is deliberate follow-up work, not a side effect.

pub mod css;

use std::sync::OnceLock;

/// RGB color tuple (0.0–1.0 channels).
pub type Rgb = (f64, f64, f64);

/// RGBA color tuple (0.0–1.0 channels).
pub type Rgba = (f64, f64, f64, f64);

/// Attach an alpha channel to an RGB root in const context.
pub const fn rgba(rgb: Rgb, alpha: f64) -> Rgba {
    (rgb.0, rgb.1, rgb.2, alpha)
}

// ============================================================================
// CORE PALETTE — shared RGB roots
// ============================================================================

/// The one saturated accent: #3584E4 (GNOME blue). Every accent-family token
/// in `overlay`/`toolbar` derives from this root.
pub const ACCENT_RGB: Rgb = (0.2078, 0.5176, 0.8941);

/// Lighter accent tint root (focus borders, carets, bright indicators).
pub const ACCENT_BRIGHT_RGB: Rgb = (0.41, 0.72, 1.0);

/// Destructive red root: #F5333F-family (Clear hover, delete confirm).
pub const DESTRUCTIVE_RGB: Rgb = (0.9608, 0.2, 0.2471);

/// Standard drop shadow color.
pub const SHADOW_RGBA: Rgba = (0.0, 0.0, 0.0, 0.30);

// ============================================================================
// OVERLAY TOKENS — overlay-drawn surfaces (moved from `ui::constants`)
// ============================================================================

pub mod overlay {
    use super::{ACCENT_BRIGHT_RGB, ACCENT_RGB, Rgba, rgba};

    // ---- Overlay dimming ----
    /// Popover/quick overlay dimming (e.g., board picker quick mode)
    pub const OVERLAY_DIM_LIGHT: f64 = 0.20;
    /// Standard modal dimming (e.g., board picker full, command palette)
    pub const OVERLAY_DIM_MEDIUM: f64 = 0.50;
    /// Heavy dimming for important/tutorial overlays (e.g., tour)
    pub const OVERLAY_DIM_HEAVY: f64 = 0.70;
    /// Help overlay dimming
    pub const OVERLAY_DIM_HELP: f64 = 0.55;

    // ---- Panel backgrounds ----
    // TODO(theme-consolidation): five near-duplicate panel backgrounds (plus
    // `toolbar::COLOR_PANEL_BACKGROUND`) should converge on the runtime
    // Theme's surface tokens as surfaces migrate.
    /// Context menu background
    pub const PANEL_BG_CONTEXT_MENU: Rgba = (0.10, 0.13, 0.17, 0.95);
    /// Board picker panel background
    pub const PANEL_BG_BOARD_PICKER: Rgba = (0.09, 0.11, 0.15, 0.96);
    /// Properties panel background
    pub const PANEL_BG_PROPERTIES: Rgba = (0.08, 0.11, 0.17, 0.92);
    /// Command palette background
    pub const PANEL_BG_COMMAND_PALETTE: Rgba = (0.15, 0.15, 0.18, 0.98);
    /// Tour/modal dialog background
    pub const PANEL_BG_MODAL: Rgba = (0.15, 0.15, 0.18, 0.98);

    // ---- Panel borders ----
    /// Context menu border
    pub const BORDER_CONTEXT_MENU: Rgba = (0.18, 0.22, 0.28, 0.9);
    /// Board picker border
    pub const BORDER_BOARD_PICKER: Rgba = (0.20, 0.24, 0.30, 0.9);
    /// Properties panel border
    pub const BORDER_PROPERTIES: Rgba = (0.18, 0.22, 0.30, 0.95);
    /// Command palette border
    pub const BORDER_COMMAND_PALETTE: Rgba = (0.40, 0.40, 0.45, 0.5);
    /// Tour/modal border
    pub const BORDER_MODAL: Rgba = (0.40, 0.40, 0.50, 0.6);

    // ---- Text colors ----
    // TODO(theme-consolidation): overlay text is cool-tinted while toolbar
    // text is pure-white alpha ladder; unify when surfaces read Theme.
    /// Primary text (titles, main content) - high contrast
    pub const TEXT_PRIMARY: Rgba = (0.93, 0.95, 0.99, 1.0);
    /// Secondary text (body content) - slightly dimmer
    pub const TEXT_SECONDARY: Rgba = (0.86, 0.89, 0.94, 1.0);
    /// Hint/shortcut text
    pub const TEXT_HINT: Rgba = (0.70, 0.73, 0.78, 0.9);
    /// Dim keyboard-shortcut hint inside overlay popups (dimmer than
    /// `TEXT_HINT`): the color popup's hint row, the precise-entry hint.
    pub const TEXT_HINT_DIM: Rgba = (0.6, 0.6, 0.65, 0.7);
    /// Disabled text
    pub const TEXT_DISABLED: Rgba = (0.60, 0.64, 0.68, 0.65);
    /// Placeholder text
    pub const TEXT_PLACEHOLDER: Rgba = (0.50, 0.50, 0.55, 0.7);
    /// Tertiary/muted text (footers, less important info)
    pub const TEXT_TERTIARY: Rgba = (0.64, 0.69, 0.76, 0.9);
    /// Active/highlighted text
    pub const TEXT_ACTIVE: Rgba = (0.96, 0.98, 1.0, 1.0);
    /// White text (for use on colored backgrounds)
    pub const TEXT_WHITE: Rgba = (1.0, 1.0, 1.0, 1.0);
    /// Description/subtitle text
    pub const TEXT_DESCRIPTION: Rgba = (0.65, 0.68, 0.73, 0.85);

    // ---- Interactive states ----
    /// Hover state background (mouse hover)
    pub const BG_HOVER: Rgba = (0.25, 0.32, 0.45, 0.9);
    /// State-ladder hover wash: white at 8% painted over the resting
    /// surface. Sits below the accent-filled selected state.
    pub const BG_HOVER_WASH: Rgba = (1.0, 1.0, 1.0, 0.08);
    /// Keyboard focus border color (lighter tint of the accent)
    pub const BORDER_FOCUS: Rgba = rgba(ACCENT_BRIGHT_RGB, 0.9);
    /// Selection/highlight background
    pub const BG_SELECTION: Rgba = (0.22, 0.28, 0.38, 0.9);
    /// Active/selected item indicator
    pub const BG_SELECTED_INDICATOR: Rgba = (0.33, 0.42, 0.58, 0.9);
    /// Accent color for highlights and active elements: #3584E4
    pub const ACCENT_PRIMARY: Rgba = rgba(ACCENT_RGB, 0.9);
    /// Lighter tint of the accent (hover feedback, bright borders)
    pub const ACCENT_BRIGHT: Rgba = rgba(ACCENT_BRIGHT_RGB, 0.95);
    /// Command palette/input selection highlight (accent at reduced alpha)
    pub const BG_INPUT_SELECTION: Rgba = rgba(ACCENT_RGB, 0.4);

    // ---- Input elements ----
    /// Input field background
    pub const INPUT_BG: Rgba = (0.10, 0.10, 0.12, 1.0);
    /// Input field border (focused): accent at reduced alpha
    pub const INPUT_BORDER_FOCUSED: Rgba = rgba(ACCENT_RGB, 0.6);
    /// Caret/cursor color (accent-bright, fully opaque for visibility)
    pub const INPUT_CARET: Rgba = rgba(ACCENT_BRIGHT_RGB, 1.0);

    // ---- Dividers ----
    /// Standard divider line
    pub const DIVIDER: Rgba = (0.35, 0.40, 0.50, 0.9);
    /// Lighter divider (for subtle separation)
    pub const DIVIDER_LIGHT: Rgba = (0.35, 0.40, 0.48, 0.6);

    // ---- Shadows ----
    /// Standard drop shadow
    pub const SHADOW: Rgba = super::SHADOW_RGBA;
    /// Deeper shadow for layered elements
    pub const SHADOW_DEEP: Rgba = (0.0, 0.0, 0.0, 0.35);

    // ---- Toast colors ----
    /// Info toast background
    pub const TOAST_INFO: super::Rgb = (0.25, 0.70, 0.90);
    /// Warning toast background
    pub const TOAST_WARNING: super::Rgb = (0.96, 0.62, 0.04);
    /// Error toast background
    pub const TOAST_ERROR: super::Rgb = (0.90, 0.30, 0.30);
    /// Success toast background (for preset apply)
    pub const TOAST_SUCCESS: super::Rgb = (0.20, 0.70, 0.40);
    /// Blocked action border flash
    pub const BLOCKED_FLASH: super::Rgb = (0.90, 0.20, 0.20);

    // ---- Progress indicators ----
    /// Progress bar track/background
    pub const PROGRESS_TRACK: Rgba = (0.30, 0.30, 0.35, 1.0);
    /// Progress bar fill (the accent)
    pub const PROGRESS_FILL: Rgba = rgba(ACCENT_RGB, 1.0);

    // ---- Special element colors ----
    /// Pin icon active (the accent - unified with other active states)
    pub const ICON_PIN_ACTIVE: Rgba = rgba(ACCENT_RGB, 0.95);
    /// Pin icon inactive
    pub const ICON_PIN_INACTIVE: Rgba = (0.60, 0.65, 0.72, 0.5);
    /// Drag handle dots
    pub const ICON_DRAG_HANDLE: Rgba = (0.58, 0.63, 0.70, 0.85);
    /// Submenu arrow
    pub const ICON_SUBMENU_ARROW: Rgba = (0.75, 0.78, 0.84, 1.0);

    // ---- Spacing scale (px) ----
    // TODO(theme-consolidation): overlay and toolbar spacing scales use the
    // same names with different values; keep separate until surfaces migrate.
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

    // ---- Corner radii ----
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

    // ---- Animation timing (ms) ----
    /// Fast animation (100ms) - quick feedback
    pub const ANIM_FAST_MS: u64 = 100;
    /// Normal animation (200ms) - standard transitions
    pub const ANIM_NORMAL_MS: u64 = 200;
    /// Slow animation (300ms) - modal open/close
    pub const ANIM_SLOW_MS: u64 = 300;

    // ---- Focus ring ----
    /// Focus ring width
    pub const FOCUS_RING_WIDTH: f64 = 2.0;
    /// Focus ring offset from element
    pub const FOCUS_RING_OFFSET: f64 = 2.0;

    // ---- Radial menu (Cairo overlay only; no GTK CSS rows) ----
    /// Wedge glyph size on the radial compass ring.
    pub const RADIAL_WEDGE_ICON_SIZE: f64 = 17.0;
    /// Lift of the wedge glyph center above the wedge midpoint.
    pub const RADIAL_WEDGE_ICON_LIFT: f64 = 15.0;
    /// Drop of the wedge label center below the wedge midpoint when a glyph
    /// sits above it.
    pub const RADIAL_WEDGE_LABEL_DROP: f64 = 5.0;
    /// Drop of the keycap hint's top edge below the wedge midpoint when a
    /// glyph/label stack sits above it.
    pub const RADIAL_WEDGE_HINT_DROP: f64 = 13.0;
    /// Sub-ring wedges whose mid-radius arc is narrower than this render
    /// their glyph only (no label/keycap).
    pub const RADIAL_SUB_LABEL_MIN_ARC: f64 = 34.0;
    /// Drop of the thickness numeral keycap center below the center-well
    /// midpoint.
    pub const RADIAL_CENTER_NUMERAL_DROP: f64 = 16.0;
    /// Radial inset of the size-ring track stroke inside its band (both
    /// edges), keeping the gauge visually thinner than its hit band.
    pub const RADIAL_SIZE_TRACK_INSET: f64 = 3.0;
    /// Outer-radius inset of the recent-color swatches on the radial color
    /// ring, visually separating the recents arc from the quick palette.
    pub const RADIAL_RECENT_OUTER_INSET: f64 = 5.0;

    // ---- Tool-preview cursor (Cairo overlay only; no GTK CSS rows) ----
    // The bubble that trails the pointer showing the active tool, its draw
    // color, and its width. Builtin overlay with no GTK equivalent, so these
    // carry no CSS rows / equality tests.
    /// Background fill of the tool-preview bubble.
    pub const CURSOR_PREVIEW_BG: Rgba = (0.08, 0.08, 0.10, 0.6);
    /// Hairline border of the tool-preview bubble.
    pub const CURSOR_PREVIEW_BORDER: Rgba = (1.0, 1.0, 1.0, 0.10);
    /// Neutral foreground for the tool glyph and for tools without a meaningful
    /// draw color (eraser, select).
    pub const CURSOR_PREVIEW_NEUTRAL: Rgba = (0.95, 0.95, 0.98, 0.95);
    /// Auto-contrast outline around the width dot when the dot color is dark
    /// (see [`super::cursor_preview_outline`]).
    pub const CURSOR_PREVIEW_OUTLINE_LIGHT: Rgba = (1.0, 1.0, 1.0, 0.92);
    /// Auto-contrast outline around the width dot when the dot color is light.
    pub const CURSOR_PREVIEW_OUTLINE_DARK: Rgba = (0.0, 0.0, 0.0, 0.72);
    /// Tool glyph size inside the preview bubble (px).
    pub const CURSOR_PREVIEW_GLYPH_SIZE: f64 = 12.0;
    /// Smallest rendered diameter of the width dot (px).
    pub const CURSOR_PREVIEW_DOT_MIN: f64 = 3.0;
    /// Largest rendered diameter of the width dot (px), keeping the bubble
    /// within the cursor damage radius.
    pub const CURSOR_PREVIEW_DOT_MAX: f64 = 14.0;
    /// Interior padding of the preview bubble (px).
    pub const CURSOR_PREVIEW_PAD: f64 = 6.0;
    /// Gap between the tool glyph and the width dot (px).
    pub const CURSOR_PREVIEW_GAP: f64 = 4.0;
    /// Offset of the bubble from the pointer hotspot (px).
    pub const CURSOR_PREVIEW_OFFSET: f64 = 16.0;

    // ---- Keyboard navigation hint text ----
    /// Context menu navigation hint
    pub const NAV_HINT_MENU: &str = "↑↓ to navigate • Enter to select • Esc to close";
    /// Board picker navigation hint
    pub const NAV_HINT_BOARD_PICKER: &str = "↑↓ Navigate • Type to search";
    /// Modal close hint
    pub const HINT_PRESS_ESC: &str = "Press Escape to close";

    // ---- Empty state messages ----
    /// Properties panel empty state
    pub const EMPTY_PROPERTIES: &str = "Select a shape to view properties";
    /// Command palette no results - main message
    pub const EMPTY_COMMAND_PALETTE: &str = "No matching commands";
    /// Command palette no results - suggestions
    pub const EMPTY_COMMAND_SUGGESTIONS: &str = "Try: pen, color, undo, help";
}

// ============================================================================
// TOOLBAR TOKENS — builtin Cairo bars + generated GTK CSS (moved from
// `toolbar::render::widgets::constants`)
// ============================================================================

pub mod toolbar {
    use super::{ACCENT_BRIGHT_RGB, ACCENT_RGB, DESTRUCTIVE_RGB, Rgb, Rgba, rgba};

    // ---- Accent family ----
    /// The one saturated accent color (active tool, selected value): #3584E4
    pub const COLOR_ACCENT: Rgba = rgba(ACCENT_RGB, 1.0);
    /// Soft accent glow halo behind active elements
    pub const COLOR_ACCENT_GLOW: Rgba = rgba(ACCENT_RGB, 0.25);
    /// Lighter accent tint (active-button bottom indicator)
    pub const COLOR_ACCENT_BRIGHT: Rgba = rgba(ACCENT_BRIGHT_RGB, 0.95);

    // ---- Text ----
    /// White text/icon color with high opacity
    pub const COLOR_TEXT_PRIMARY: Rgba = (1.0, 1.0, 1.0, 0.95);
    /// White text with slightly lower opacity
    pub const COLOR_TEXT_SECONDARY: Rgba = (1.0, 1.0, 1.0, 0.9);
    /// White text with medium opacity (for less prominent elements)
    pub const COLOR_TEXT_TERTIARY: Rgba = (1.0, 1.0, 1.0, 0.85);
    /// Section header label color
    pub const COLOR_LABEL_SECTION: Rgba = (0.8, 0.8, 0.85, 0.9);
    /// Hint/secondary label color
    pub const COLOR_LABEL_HINT: Rgba = (0.7, 0.7, 0.75, 0.8);
    /// Disabled text/icon color: dimmed but still legible against the dimmed
    /// disabled button body (the body carries most of the "inert" signal)
    pub const COLOR_TEXT_DISABLED: Rgba = (0.62, 0.62, 0.68, 0.45);

    // ---- Icons ----
    /// Icon color when hovered (fully opaque white)
    pub const COLOR_ICON_HOVER: Rgba = (1.0, 1.0, 1.0, 1.0);
    /// Icon color default state
    pub const COLOR_ICON_DEFAULT: Rgba = (0.95, 0.95, 0.95, 0.9);
    /// Icon hover background glow (subtle highlight behind icons on hover)
    pub const COLOR_ICON_HOVER_BG: Rgba = (1.0, 1.0, 1.0, 0.15);
    /// Keyboard focus ring color (accent at reduced alpha)
    pub const COLOR_FOCUS_RING: Rgba = rgba(ACCENT_RGB, 0.9);

    // ---- Button states ----
    /// Active/selected button background (the accent)
    pub const COLOR_BUTTON_ACTIVE: Rgba = COLOR_ACCENT;
    /// Hovered button background
    pub const COLOR_BUTTON_HOVER: Rgba = (0.35, 0.35, 0.45, 0.85);
    /// Default button background
    pub const COLOR_BUTTON_DEFAULT: Rgba = (0.2, 0.22, 0.26, 0.75);
    /// Disabled button background: visibly dimmer than default so the tile
    /// itself reads inert (icon/label dimming alone is too subtle)
    pub const COLOR_BUTTON_DISABLED: Rgba = (0.2, 0.22, 0.26, 0.35);
    /// Destructive button hover fill (#F5333F-family red at a tint alpha).
    /// Destructive buttons render as normal flat buttons at rest; the red
    /// appears only on hover/press so the bar carries no persistent red.
    pub const COLOR_BUTTON_DESTRUCTIVE_HOVER: Rgba = rgba(DESTRUCTIVE_RGB, 0.55);
    /// Destructive button pressed fill (GTK `:active`): a stronger tint of
    /// the same red so the press reads as commitment
    pub const COLOR_BUTTON_DESTRUCTIVE_ACTIVE: Rgba = rgba(DESTRUCTIVE_RGB, 0.78);

    // ---- Checkbox states ----
    /// Checkbox hover state
    pub const COLOR_CHECKBOX_HOVER: Rgba = (0.32, 0.34, 0.4, 0.9);
    /// Checkbox default state
    pub const COLOR_CHECKBOX_DEFAULT: Rgba = (0.22, 0.24, 0.28, 0.75);
    /// Mini checkbox checked state (green tint)
    pub const COLOR_CHECKBOX_CHECKED: Rgba = (0.25, 0.5, 0.35, 0.9);
    /// Mini checkbox hover state
    pub const COLOR_MINI_CHECKBOX_HOVER: Rgba = (0.32, 0.34, 0.4, 0.85);
    /// Mini checkbox default state
    pub const COLOR_MINI_CHECKBOX_DEFAULT: Rgba = (0.2, 0.22, 0.26, 0.7);

    // ---- Pin button ----
    /// Pinned state (the accent - unified with other active states)
    pub const COLOR_PIN_ACTIVE: Rgba = COLOR_ACCENT;
    /// Pin button hover
    pub const COLOR_PIN_HOVER: Rgba = (0.35, 0.35, 0.45, 0.85);
    /// Pin button default
    pub const COLOR_PIN_DEFAULT: Rgba = (0.3, 0.3, 0.35, 0.7);

    // ---- Close button ----
    /// Close button hover (red tint)
    // TODO(theme-consolidation): near-duplicate of the destructive family.
    pub const COLOR_CLOSE_HOVER: Rgba = (0.8, 0.3, 0.3, 0.9);
    /// Close button default
    pub const COLOR_CLOSE_DEFAULT: Rgba = (0.5, 0.5, 0.55, 0.7);

    // ---- Segmented control ----
    /// Segmented control outer background
    pub const COLOR_SEGMENT_BG: Rgba = (0.15, 0.17, 0.22, 0.85);
    /// Active segment background: the accent at reduced alpha, deliberately
    /// quieter than COLOR_ACCENT so segmented-control selection (Ico/Txt,
    /// Simple/Full) never competes with the active-tool highlight
    pub const COLOR_SEGMENT_ACTIVE: Rgba = rgba(ACCENT_RGB, 0.55);
    /// Active segment text color
    pub const COLOR_SEGMENT_TEXT_ACTIVE: Rgba = (1.0, 1.0, 1.0, 1.0);
    /// Inactive segment text color
    pub const COLOR_SEGMENT_TEXT_INACTIVE: Rgba = (0.65, 0.68, 0.75, 0.9);
    /// Hovered (inactive) segment background
    pub const COLOR_SEGMENT_HOVER: Rgba = (0.22, 0.25, 0.32, 0.9);
    /// Center divider between segments
    pub const COLOR_SEGMENT_DIVIDER: Rgba = (0.35, 0.38, 0.45, 0.5);

    // ---- Segmented control metrics (M7-C3) ----
    /// Outer container corner radius of a segmented control.
    pub const SEGMENT_RADIUS: f64 = RADIUS_LG;
    /// Selected-segment pill corner radius (the rounded highlight inset
    /// inside the container).
    pub const SEGMENT_SELECTED_RADIUS: f64 = RADIUS_STD;
    /// Inset of the selected/hovered segment pill from the container edge, so
    /// the highlight reads as a rounded pill with breathing room rather than
    /// filling the whole half.
    pub const SEGMENT_PADDING: f64 = SPACING_SM;
    /// Horizontal breathing room between a segment label and its edge (the
    /// GTK `.tab` horizontal padding), so "Sans│Mono" never crowd the seam.
    pub const SEGMENT_LABEL_PAD_H: f64 = SPACING_STD;
    /// Extra clear gap before a segmented control in the style pill, on top of
    /// the standard control gap, so the segment does not crowd the numeral
    /// ("72pt") to its left.
    pub const SEGMENT_LEADING_GAP: f64 = SPACING_MD;

    // ---- Preset slot (M7-C2) ----
    /// Tool-glyph size inside a presets-island slot, as a fraction of the
    /// slot's shorter side (mirrors the side-palette preset icon ratio).
    pub const PRESET_SLOT_ICON_RATIO: f64 = 0.5;
    /// Corner color-swatch size inside a filled presets-island slot, as a
    /// fraction of the slot's shorter side. The saved preset color rides here
    /// as a separate accent so the tool glyph can stay a neutral, always-legible
    /// foreground rather than being tinted invisible by a dark preset color
    /// (the side-palette convention, M7-C2 legibility fix).
    pub const PRESET_SLOT_SWATCH_RATIO: f64 = 0.4;
    /// Inset of the corner color swatch from the slot's bottom-right edge.
    pub const PRESET_SLOT_SWATCH_INSET: f64 = SPACING_XS;
    /// Corner radius of the preset slot's color swatch.
    pub const PRESET_SLOT_SWATCH_RADIUS: f64 = RADIUS_SM;

    // ---- Slider/track ----
    /// Slider track background
    pub const COLOR_TRACK_BACKGROUND: Rgba = (0.5, 0.5, 0.6, 0.6);
    /// Slider knob (accent at reduced alpha)
    pub const COLOR_TRACK_KNOB: Rgba = rgba(ACCENT_RGB, 0.9);

    // ---- Card/panel backgrounds ----
    /// Main panel background
    pub const COLOR_PANEL_BACKGROUND: Rgba = (0.05, 0.05, 0.08, 0.92);
    /// Group card background
    pub const COLOR_CARD_BACKGROUND: Rgba = (0.12, 0.12, 0.18, 0.35);

    // ---- Panel radius ----
    /// Panel corner radius (larger than buttons)
    pub const RADIUS_PANEL: f64 = 14.0;
    /// Card corner radius
    pub const RADIUS_CARD: f64 = 8.0;

    // ---- Tooltip ----
    /// Tooltip background
    pub const COLOR_TOOLTIP_BACKGROUND: Rgba = (0.1, 0.1, 0.15, 0.95);
    /// Tooltip border
    pub const COLOR_TOOLTIP_BORDER: Rgba = (0.4, 0.4, 0.5, 0.8);
    /// Tooltip shadow
    pub const COLOR_TOOLTIP_SHADOW: Rgba = super::SHADOW_RGBA;

    // ---- Dividers / header accents ----
    /// Subtle divider color for grouping
    pub const COLOR_DIVIDER: Rgba = (1.0, 1.0, 1.0, 0.08);
    /// Header band background to add hierarchy
    pub const COLOR_HEADER_BAND: Rgba = (0.12, 0.13, 0.18, 0.45);

    // ---- Font sizes ----
    /// Small font size (hints, counters)
    pub const FONT_SIZE_SMALL: f64 = 10.0;
    /// Secondary font size (mini checkboxes, delay values)
    pub const FONT_SIZE_SECONDARY: f64 = 11.0;
    /// Tooltip and settings font size
    pub const FONT_SIZE_TOOLTIP: f64 = 12.0;
    /// Default label font size
    pub const FONT_SIZE_LABEL: f64 = 13.0;

    // ---- Font families ----
    /// Default sans-serif font family
    pub const FONT_FAMILY_DEFAULT: &str = "Sans";
    /// Monospace font family (for code/fixed-width text)
    pub const FONT_FAMILY_MONO: &str = "Monospace";

    // ---- Spacing (px) ----
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

    // ---- Corner radius ----
    /// Small corner radius (mini elements)
    pub const RADIUS_SM: f64 = 3.0;
    /// Standard corner radius (buttons, cards)
    pub const RADIUS_STD: f64 = 4.0;
    /// Large corner radius (panels, large buttons)
    pub const RADIUS_LG: f64 = 6.0;

    // ---- Line widths ----
    /// Thin line width (borders, strokes)
    pub const LINE_WIDTH_THIN: f64 = 1.0;
    /// Standard line width (checkbox borders)
    pub const LINE_WIDTH_STD: f64 = 1.5;
    /// Thick line width (X marks, pins)
    pub const LINE_WIDTH_THICK: f64 = 2.0;

    // ---- Opacity values ----
    /// Hover state alpha modifier
    pub const ALPHA_HOVER: f64 = 0.9;
    /// Default state alpha modifier
    pub const ALPHA_DEFAULT: f64 = 0.6;

    // ========================================================================
    // GTK stylesheet tokens (toolbar-gtk frontend)
    // ========================================================================
    // Values that existed only as hand-written literals in the generated GTK
    // stylesheet (`crate::toolbar_gtk::css`) before M1. Named here — at their
    // exact pre-M1 values, zero visual change — so the sheet interpolates
    // tokens instead of literals (`crate::ui::theme::css` owns the mapping).

    // ---- GTK colors ----
    /// Hairline border on GTK panels and popovers (matches the runtime
    /// `Theme::dark` `border_hairline`)
    pub const COLOR_PANEL_BORDER: Rgba = (1.0, 1.0, 1.0, 0.10);
    /// Foreground on filled controls (active/pinned/destructive fills and the
    /// checked checkbox glyph): pure white
    pub const COLOR_TEXT_ON_FILL: Rgba = (1.0, 1.0, 1.0, 1.0);
    /// Drag handle at rest (quiet until hovered)
    pub const COLOR_DRAG_HANDLE: Rgba = (1.0, 1.0, 1.0, 0.45);
    /// Drag handle on hover
    pub const COLOR_DRAG_HANDLE_HOVER: Rgba = (1.0, 1.0, 1.0, 0.75);
    /// Checkbox outline
    pub const COLOR_CHECKBOX_BORDER: Rgba = (1.0, 1.0, 1.0, 0.25);
    /// Popover drop shadow.
    // TODO(theme-consolidation): same value as `overlay::SHADOW_DEEP`.
    pub const COLOR_POPOVER_SHADOW: Rgba = (0.0, 0.0, 0.0, 0.35);
    /// Shortcut badge text
    pub const COLOR_BADGE_TEXT: Rgba = (1.0, 1.0, 1.0, 0.96);
    /// Shortcut badge background (near-black chip behind the key letter);
    /// channels are the exact 8-bit values the sheet always emitted
    pub const COLOR_BADGE_BACKGROUND: Rgba = (9.0 / 255.0, 10.0 / 255.0, 15.0 / 255.0, 0.82);
    /// Shortcut badge border
    pub const COLOR_BADGE_BORDER: Rgba = (1.0, 1.0, 1.0, 0.24);
    /// Collapsible section header hover wash
    pub const COLOR_SECTION_HEADER_HOVER: Rgba = (1.0, 1.0, 1.0, 0.04);
    /// Shared border root for board chips and text entries in the side
    /// palette (8-bit 166/179/204)
    const FIELD_BORDER_RGB: Rgb = (166.0 / 255.0, 179.0 / 255.0, 204.0 / 255.0);
    /// Board chip / text entry background (8-bit 56/61/71)
    pub const COLOR_FIELD_BACKGROUND: Rgba = (56.0 / 255.0, 61.0 / 255.0, 71.0 / 255.0, 0.95);
    /// Board chip hover background (8-bit 71/77/87)
    pub const COLOR_FIELD_BACKGROUND_HOVER: Rgba = (71.0 / 255.0, 77.0 / 255.0, 87.0 / 255.0, 0.95);
    /// Board chip / text entry border
    pub const COLOR_FIELD_BORDER: Rgba = rgba(FIELD_BORDER_RGB, 0.45);
    /// Board chip hover border
    pub const COLOR_FIELD_BORDER_HOVER: Rgba = rgba(FIELD_BORDER_RGB, 0.70);
    /// Scrollbar slider fill
    pub const COLOR_SCROLLBAR_SLIDER: Rgba = (1.0, 1.0, 1.0, 0.35);
    /// Inner hairline that keeps a swatch fill defined against the panel
    /// (shared by the builtin renderer and the GTK swatch widget).
    pub const COLOR_SWATCH_HAIRLINE: Rgba = (1.0, 1.0, 1.0, 0.16);
    /// Contrast-boost variant of the swatch hairline for near-black fills.
    pub const COLOR_SWATCH_HAIRLINE_DARK: Rgba = (0.5, 0.5, 0.5, 0.8);

    // ---- GTK metrics: scale with `toolbar_scale` ----
    /// Panel horizontal padding
    pub const PANEL_PADDING_H: f64 = 10.0;
    /// Top-strip island (pill) inner horizontal padding. One source for
    /// both frontends: the builtin planner consumes it as
    /// `ToolbarLayoutSpec::TOP_ISLAND_PAD` (width budgeting and pill
    /// geometry) and the GTK stylesheet interpolates it as the `.pill`
    /// horizontal padding, so the two cannot drift.
    pub const ISLAND_PAD: f64 = 8.0;
    /// Compact-mode counterpart of [`ISLAND_PAD`] (the last-resort
    /// tightened presentation), shared the same way.
    pub const COMPACT_ISLAND_PAD: f64 = 4.0;
    /// Session/Settings popover content-column width, in spec units
    /// (mirrors the retired side palette's content column). One source for
    /// both frontends: the builtin popover tree builds its rows at this
    /// width and the GTK popover sizes its viewport from it.
    pub const MENU_CONTENT_W: f64 = 232.0;
    /// Cap on the Session/Settings popover's visible content height, in
    /// spec units; taller content scrolls internally. Shared by the builtin
    /// scroll viewport and the GTK `ScrolledWindow` max content height.
    pub const MENU_MAX_CONTENT_H: f64 = 420.0;
    /// Checkbox indicator square size
    pub const CHECKBOX_SIZE: f64 = 14.0;
    /// Boxed shortcut badge font size
    pub const FONT_SIZE_BADGE: f64 = 8.0;
    /// Swatch key-letter caption font size (one step above boxed badges)
    pub const FONT_SIZE_SWATCH_KEY: f64 = 9.0;

    // ---- GTK metrics: fixed chrome details (never scaled) ----
    /// Hairline padding (chip/entry vertical padding)
    pub const SPACING_XXS: f64 = 1.0;
    /// Keyboard focus outline width
    pub const FOCUS_RING_WIDTH: f64 = 2.0;
    /// Keyboard focus outline offset
    pub const FOCUS_RING_OFFSET: f64 = 1.0;
    /// Glow ring (box-shadow spread) around active/pinned controls
    pub const ACTIVE_GLOW_WIDTH: f64 = 2.0;
    /// Inset bottom accent indicator on active buttons
    pub const ACTIVE_INDICATOR_HEIGHT: f64 = 2.0;
    /// Fully-round radius for circular chrome buttons
    pub const RADIUS_FULL: f64 = 9999.0;
    /// Scrollbar slider corner radius
    pub const SCROLLBAR_RADIUS: f64 = 2.0;
    /// Scrollbar slider thickness
    pub const SCROLLBAR_WIDTH: f64 = 4.0;
    /// Popover drop shadow vertical offset
    pub const POPOVER_SHADOW_OFFSET_Y: f64 = 2.0;
    /// Popover drop shadow blur radius
    pub const POPOVER_SHADOW_BLUR: f64 = 12.0;

    // ---- GTK animation / typography ----
    /// Hover/state transition duration (ms)
    pub const TRANSITION_HOVER_MS: u64 = 120;
    /// Semibold font weight (buttons)
    pub const FONT_WEIGHT_SEMIBOLD: u32 = 600;
    /// Bold font weight (section titles, tabs, chips, badges)
    pub const FONT_WEIGHT_BOLD: u32 = 700;
}

// ============================================================================
// RUNTIME THEME — dark/light chrome variants
// ============================================================================

/// Chrome color set for one theme variant. Consumed by surfaces as they
/// migrate to runtime theming (M2+); the const tokens above remain the
/// canonical dark values in the meantime.
#[derive(Clone, Debug, PartialEq)]
pub struct Theme {
    /// Floating island/pill surfaces
    pub surface_pill: Rgba,
    /// Popovers (pickers, board picker, palette)
    pub surface_popover: Rgba,
    /// Panels (toolbar panel background)
    pub surface_panel: Rgba,
    /// Nested cards/sections inside panels
    pub surface_card: Rgba,
    /// 1px hairline border on every chrome surface
    pub border_hairline: Rgba,
    /// Primary text/icon color
    pub text_primary: Rgba,
    /// Secondary text
    pub text_secondary: Rgba,
    /// Tertiary/hint text
    pub text_tertiary: Rgba,
    /// Drop shadow color
    pub shadow: Rgba,
    /// Accent (selection, focus, active)
    pub accent: Rgba,
    /// Bright accent tint (focus borders, carets)
    pub accent_bright: Rgba,
    /// Destructive red (hover/confirm only)
    pub destructive: Rgba,
}

impl Theme {
    /// The default OSD-dark chrome (canonical values, matching the const
    /// tokens above).
    pub fn dark() -> Self {
        Self {
            surface_pill: toolbar::COLOR_PANEL_BACKGROUND,
            surface_popover: overlay::PANEL_BG_BOARD_PICKER,
            surface_panel: toolbar::COLOR_PANEL_BACKGROUND,
            surface_card: toolbar::COLOR_CARD_BACKGROUND,
            border_hairline: toolbar::COLOR_PANEL_BORDER,
            text_primary: toolbar::COLOR_TEXT_PRIMARY,
            text_secondary: toolbar::COLOR_TEXT_SECONDARY,
            text_tertiary: toolbar::COLOR_LABEL_HINT,
            shadow: SHADOW_RGBA,
            accent: rgba(ACCENT_RGB, 1.0),
            accent_bright: rgba(ACCENT_BRIGHT_RGB, 0.95),
            destructive: rgba(DESTRUCTIVE_RGB, 1.0),
        }
    }

    /// Status chrome palette `(bg, text)` that contrasts with a solid board
    /// background: light boards get dark chrome, dark boards get light
    /// chrome. Board adaptivity is orthogonal to the installed variant
    /// (`ThemeMode::Auto` still resolves the overall theme to dark), so this
    /// is an associated helper rather than a method on the active theme.
    pub fn status_palette_for_background(r: f64, g: f64, b: f64) -> ([f64; 4], [f64; 4]) {
        if relative_luminance(r, g, b) > STATUS_PALETTE_LUMINANCE_THRESHOLD {
            ([0.15, 0.15, 0.15, 0.85], [1.0, 1.0, 1.0, 1.0])
        } else {
            ([0.85, 0.85, 0.85, 0.85], [0.0, 0.0, 0.0, 1.0])
        }
    }

    /// Light chrome variant (for light solid boards once surfaces consume
    /// the runtime theme). Accent/radii/spacing match dark.
    pub fn light() -> Self {
        Self {
            surface_pill: (0.980, 0.980, 0.988, 0.88),
            surface_popover: (1.0, 1.0, 1.0, 0.97),
            surface_panel: (0.980, 0.980, 0.988, 0.92),
            surface_card: (0.0, 0.0, 0.024, 0.06),
            border_hairline: (0.0, 0.0, 0.024, 0.15),
            // HIG: light-mode fg is near-black, never pure black
            text_primary: (0.0, 0.0, 0.024, 0.8),
            text_secondary: (0.0, 0.0, 0.024, 0.6),
            text_tertiary: (0.0, 0.0, 0.024, 0.4),
            shadow: (0.0, 0.0, 0.0, 0.18),
            accent: rgba(ACCENT_RGB, 1.0),
            accent_bright: rgba(ACCENT_BRIGHT_RGB, 0.95),
            destructive: rgba(DESTRUCTIVE_RGB, 1.0),
        }
    }
}

/// Theme selection mode (`[ui] theme` config key).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThemeMode {
    /// Follow context. Currently resolves to dark; board-luminance-adaptive
    /// selection lands when surfaces consume the runtime theme (M2+).
    Auto,
    /// Always dark OSD chrome.
    Dark,
    /// Always light chrome.
    Light,
}

static CURRENT: OnceLock<Theme> = OnceLock::new();

/// Install the theme for this process. Call once at startup after config is
/// loaded; later calls are no-ops (first writer wins).
pub fn init(mode: ThemeMode) {
    let theme = match mode {
        ThemeMode::Light => Theme::light(),
        ThemeMode::Auto | ThemeMode::Dark => Theme::dark(),
    };
    let _ = CURRENT.set(theme);
}

/// The active theme. Falls back to dark if `init` was never called (tests,
/// early rendering).
pub fn current() -> &'static Theme {
    CURRENT.get_or_init(Theme::dark)
}

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

/// Backgrounds brighter than this luminance get dark status chrome.
const STATUS_PALETTE_LUMINANCE_THRESHOLD: f64 = 0.5;

/// Width-dot colors brighter than this luminance take the dark outline; darker
/// dots take the light one, keeping the dot edge legible on the dark bubble.
const CURSOR_PREVIEW_OUTLINE_LUMINANCE_THRESHOLD: f64 = 0.6;

/// Rec. 709 relative luminance of an RGB color (0.0–1.0 channels).
#[inline]
pub fn relative_luminance(r: f64, g: f64, b: f64) -> f64 {
    0.2126 * r + 0.7152 * g + 0.0722 * b
}

/// Auto-contrast outline for the tool-preview width dot: a dark ring on light
/// dot colors, a light ring on dark ones (by Rec. 709 luminance).
#[inline]
pub fn cursor_preview_outline(r: f64, g: f64, b: f64) -> Rgba {
    if relative_luminance(r, g, b) > CURSOR_PREVIEW_OUTLINE_LUMINANCE_THRESHOLD {
        overlay::CURSOR_PREVIEW_OUTLINE_DARK
    } else {
        overlay::CURSOR_PREVIEW_OUTLINE_LIGHT
    }
}

/// Apply an RGBA color tuple to a Cairo context
#[inline]
pub fn set_color(ctx: &cairo::Context, color: Rgba) {
    ctx.set_source_rgba(color.0, color.1, color.2, color.3);
}

/// Apply an RGB color tuple with custom alpha to a Cairo context
#[inline]
pub fn set_color_alpha(ctx: &cairo::Context, color: Rgb, alpha: f64) {
    ctx.set_source_rgba(color.0, color.1, color.2, alpha);
}

/// Apply an RGBA color with modified alpha
#[inline]
pub fn with_alpha(color: Rgba, alpha: f64) -> Rgba {
    (color.0, color.1, color.2, alpha)
}

/// Linear interpolation between two colors
#[inline]
pub fn lerp_color(from: Rgba, to: Rgba, t: f64) -> Rgba {
    let t = t.clamp(0.0, 1.0);
    (
        from.0 + (to.0 - from.0) * t,
        from.1 + (to.1 - from.1) * t,
        from.2 + (to.2 - from.2) * t,
        from.3 + (to.3 - from.3) * t,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn with_alpha_replaces_only_alpha_component() {
        assert_eq!(with_alpha((0.1, 0.2, 0.3, 0.4), 0.9), (0.1, 0.2, 0.3, 0.9));
    }

    #[test]
    fn lerp_color_clamps_progress_before_zero() {
        assert_eq!(
            lerp_color((0.0, 0.2, 0.4, 0.6), (1.0, 0.8, 0.6, 0.4), -1.0),
            (0.0, 0.2, 0.4, 0.6)
        );
    }

    #[test]
    fn lerp_color_interpolates_midpoint() {
        let color = lerp_color((0.0, 0.0, 0.0, 0.0), (1.0, 0.5, 0.25, 1.0), 0.5);
        assert!((color.0 - 0.5).abs() < 1e-9);
        assert!((color.1 - 0.25).abs() < 1e-9);
        assert!((color.2 - 0.125).abs() < 1e-9);
        assert!((color.3 - 0.5).abs() < 1e-9);
    }

    #[test]
    fn lerp_color_clamps_progress_after_one() {
        assert_eq!(
            lerp_color((0.0, 0.2, 0.4, 0.6), (1.0, 0.8, 0.6, 0.4), 2.0),
            (1.0, 0.8, 0.6, 0.4)
        );
    }

    #[test]
    fn accent_family_tokens_share_the_accent_root() {
        assert_eq!(
            (
                overlay::ACCENT_PRIMARY.0,
                overlay::ACCENT_PRIMARY.1,
                overlay::ACCENT_PRIMARY.2
            ),
            ACCENT_RGB
        );
        assert_eq!(
            (
                toolbar::COLOR_ACCENT.0,
                toolbar::COLOR_ACCENT.1,
                toolbar::COLOR_ACCENT.2
            ),
            ACCENT_RGB
        );
        assert_eq!(
            (
                toolbar::COLOR_SEGMENT_ACTIVE.0,
                toolbar::COLOR_SEGMENT_ACTIVE.1,
                toolbar::COLOR_SEGMENT_ACTIVE.2
            ),
            ACCENT_RGB
        );
    }

    #[test]
    fn current_falls_back_to_dark_without_init() {
        assert_eq!(*current(), Theme::dark());
    }

    #[test]
    fn relative_luminance_uses_rec709_weights() {
        assert!((relative_luminance(1.0, 0.0, 0.0) - 0.2126).abs() < 1e-9);
        assert!((relative_luminance(0.0, 1.0, 0.0) - 0.7152).abs() < 1e-9);
        assert!((relative_luminance(0.0, 0.0, 1.0) - 0.0722).abs() < 1e-9);
        assert!((relative_luminance(1.0, 1.0, 1.0) - 1.0).abs() < 1e-9);
        assert_eq!(relative_luminance(0.0, 0.0, 0.0), 0.0);
    }

    #[test]
    fn cursor_preview_outline_is_dark_on_bright_dot_colors() {
        // White / yellow / green dots are bright: a dark ring separates them.
        assert_eq!(
            cursor_preview_outline(1.0, 1.0, 1.0),
            overlay::CURSOR_PREVIEW_OUTLINE_DARK
        );
        assert_eq!(
            cursor_preview_outline(1.0, 1.0, 0.0),
            overlay::CURSOR_PREVIEW_OUTLINE_DARK
        );
        assert_eq!(
            cursor_preview_outline(0.0, 1.0, 0.0),
            overlay::CURSOR_PREVIEW_OUTLINE_DARK
        );
    }

    #[test]
    fn cursor_preview_outline_is_light_on_dark_dot_colors() {
        // Black / red / blue dots are dark: a light ring keeps them legible on
        // the dark bubble.
        assert_eq!(
            cursor_preview_outline(0.0, 0.0, 0.0),
            overlay::CURSOR_PREVIEW_OUTLINE_LIGHT
        );
        assert_eq!(
            cursor_preview_outline(1.0, 0.0, 0.0),
            overlay::CURSOR_PREVIEW_OUTLINE_LIGHT
        );
        assert_eq!(
            cursor_preview_outline(0.0, 0.0, 1.0),
            overlay::CURSOR_PREVIEW_OUTLINE_LIGHT
        );
    }

    #[test]
    fn cursor_preview_outline_tracks_the_luminance_threshold() {
        // The decision follows Rec. 709 luminance against the documented
        // threshold, not any single channel.
        let below = CURSOR_PREVIEW_OUTLINE_LUMINANCE_THRESHOLD - 0.05;
        let above = CURSOR_PREVIEW_OUTLINE_LUMINANCE_THRESHOLD + 0.05;
        assert_eq!(
            cursor_preview_outline(below, below, below),
            overlay::CURSOR_PREVIEW_OUTLINE_LIGHT
        );
        assert_eq!(
            cursor_preview_outline(above, above, above),
            overlay::CURSOR_PREVIEW_OUTLINE_DARK
        );
    }

    #[test]
    fn status_palette_darkens_chrome_on_light_backgrounds() {
        let (bg, text) = Theme::status_palette_for_background(1.0, 1.0, 1.0);
        assert_eq!(bg, [0.15, 0.15, 0.15, 0.85]);
        assert_eq!(text, [1.0, 1.0, 1.0, 1.0]);

        // Saturated green is bright enough (luminance 0.7152) for dark chrome.
        let (green_bg, _) = Theme::status_palette_for_background(0.0, 1.0, 0.0);
        assert_eq!(green_bg, [0.15, 0.15, 0.15, 0.85]);
    }

    #[test]
    fn status_palette_lightens_chrome_on_dark_backgrounds() {
        let (bg, text) = Theme::status_palette_for_background(0.0, 0.0, 0.0);
        assert_eq!(bg, [0.85, 0.85, 0.85, 0.85]);
        assert_eq!(text, [0.0, 0.0, 0.0, 1.0]);

        // Exactly at the threshold stays on the dark-background side.
        let (mid_bg, mid_text) = Theme::status_palette_for_background(0.5, 0.5, 0.5);
        assert_eq!(mid_bg, [0.85, 0.85, 0.85, 0.85]);
        assert_eq!(mid_text, [0.0, 0.0, 0.0, 1.0]);
    }
}
