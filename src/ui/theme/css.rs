//! GTK stylesheet value emission from theme tokens.
//!
//! This module owns the mapping from [`crate::ui::theme`] tokens to the
//! `rgba()`/px strings interpolated into the GTK toolbar stylesheet
//! (`crate::toolbar_gtk::css`), so the two frontends cannot drift: the
//! stylesheet template contains no literal style values, only format slots
//! filled from [`GtkStylesheetValues`]. The equality tests next to the
//! stylesheet assert that every color the sheet emits round-trips through a
//! named token and one of the formatters below.

use super::{Rgba, toolbar};

/// Convert a 0.0–1.0 channel to its 8-bit CSS value.
fn channel(value: f64) -> u8 {
    (value * 255.0).round() as u8
}

/// Format an RGBA token as a CSS `rgba()` string with three-decimal alpha —
/// the precision the generated stylesheet has always used for values routed
/// through its original `rgba()` helper.
pub fn rgba_css(color: Rgba) -> String {
    format!(
        "rgba({}, {}, {}, {:.3})",
        channel(color.0),
        channel(color.1),
        channel(color.2),
        color.3
    )
}

/// Format an RGBA token as a CSS `rgba()` string with two-decimal alpha —
/// the precision of values that were hand-written literals in the sheet
/// before M1. Kept distinct from [`rgba_css`] so the token migration stays
/// byte-identical.
// TODO(theme-consolidation): fold into `rgba_css` (visual no-op, byte diff
// only) once byte-stability against the pre-M1 sheet no longer matters.
pub fn rgba_css_compact(color: Rgba) -> String {
    format!(
        "rgba({}, {}, {}, {:.2})",
        channel(color.0),
        channel(color.1),
        channel(color.2),
        color.3
    )
}

/// Format a fully opaque token as a `#rrggbb` hex color (the sheet's
/// shorthand for foregrounds on filled controls).
pub fn hex_css(color: Rgba) -> String {
    debug_assert!(
        color.3 >= 1.0,
        "hex_css drops alpha; token must be fully opaque"
    );
    format!(
        "#{:02x}{:02x}{:02x}",
        channel(color.0),
        channel(color.1),
        channel(color.2)
    )
}

/// A token's channels with alpha forced to integer `0`, as the sheet's
/// capture-suppression rules spell it: keeps box-shadow geometry identical
/// while painting nothing.
pub fn transparent_css(color: Rgba) -> String {
    format!(
        "rgba({}, {}, {}, 0)",
        channel(color.0),
        channel(color.1),
        channel(color.2)
    )
}

/// Scale-dependent pixel size: multiply by the toolbar scale, round, and
/// floor at 1px so no scaled dimension ever vanishes.
pub fn scaled_px(value: f64, scale: f64) -> i32 {
    (value * scale).round().max(1.0) as i32
}

/// Scale-independent pixel size (hairlines and fixed chrome details).
pub fn fixed_px(value: f64) -> i32 {
    value.round() as i32
}

/// Every value the GTK toolbar stylesheet interpolates, derived from
/// [`toolbar`] tokens at a given `toolbar_scale`. Font sizes and paddings
/// scale; hairlines and fixed chrome details do not (widget geometry scales
/// through the size-request path instead).
#[derive(Clone, Debug, PartialEq)]
pub struct GtkStylesheetValues {
    // ---- Colors routed through the sheet's original rgba() helper ----
    pub panel: String,
    pub accent: String,
    pub accent_glow: String,
    pub accent_bright: String,
    pub text_primary: String,
    pub text_disabled: String,
    pub icon_default: String,
    pub button_default: String,
    pub button_hover: String,
    pub button_disabled: String,
    pub button_destructive_hover: String,
    pub button_destructive_active: String,
    pub segment_active: String,
    pub checkbox_default: String,
    pub checkbox_hover: String,
    pub checkbox_checked: String,
    pub pin_default: String,
    pub pin_hover: String,
    pub close_default: String,
    pub close_hover: String,
    pub tooltip_bg: String,
    pub tooltip_border: String,
    pub divider: String,
    // ---- Colors that were hand-written literals before M1 ----
    pub panel_border: String,
    pub drag_handle: String,
    pub drag_handle_hover: String,
    pub checkbox_border: String,
    pub label_hint: String,
    pub badge_text: String,
    pub badge_bg: String,
    pub badge_border: String,
    pub popover_shadow: String,
    pub popover_shadow_clear: String,
    pub header_band: String,
    pub card: String,
    pub section_title: String,
    pub section_header_hover: String,
    pub field_bg: String,
    pub field_bg_hover: String,
    pub field_border: String,
    pub field_border_hover: String,
    pub scrollbar_slider: String,
    pub text_on_fill: String,
    // ---- Scaled metrics (px) ----
    pub radius_panel: i32,
    pub radius_card: i32,
    pub radius_button: i32,
    pub radius_sm: i32,
    pub pad_std: i32,
    pub pad_panel_h: i32,
    pub pad_popover: i32,
    pub check_size: i32,
    pub font_label: i32,
    pub font_small: i32,
    pub font_tooltip: i32,
    pub font_badge: i32,
    pub font_swatch_key: i32,
    // ---- Fixed metrics (px) ----
    pub hairline: i32,
    pub focus_ring_width: i32,
    pub focus_ring_offset: i32,
    pub glow_ring_width: i32,
    pub indicator_height: i32,
    pub radius_full: i32,
    pub radius_lg_fixed: i32,
    pub radius_std_fixed: i32,
    pub scrollbar_radius: i32,
    pub scrollbar_width: i32,
    pub spacing_xxs: i32,
    pub spacing_xs: i32,
    pub spacing_sm: i32,
    pub spacing_md: i32,
    pub spacing_std: i32,
    pub shadow_offset_y: i32,
    pub shadow_blur: i32,
    // ---- Animation / typography ----
    pub transition_ms: u64,
    pub weight_semibold: u32,
    pub weight_bold: u32,
}

impl GtkStylesheetValues {
    /// Build the full value set at the given toolbar scale. Non-finite
    /// scales fall back to 1.0; finite scales clamp to the 0.5–3.0 range
    /// the sheet has always supported.
    pub fn new(scale: f64) -> Self {
        let scale = if scale.is_finite() {
            scale.clamp(0.5, 3.0)
        } else {
            1.0
        };
        let px = |value: f64| scaled_px(value, scale);
        use toolbar as t;
        Self {
            panel: rgba_css(t::COLOR_PANEL_BACKGROUND),
            accent: rgba_css(t::COLOR_ACCENT),
            accent_glow: rgba_css(t::COLOR_ACCENT_GLOW),
            accent_bright: rgba_css(t::COLOR_ACCENT_BRIGHT),
            text_primary: rgba_css(t::COLOR_TEXT_PRIMARY),
            text_disabled: rgba_css(t::COLOR_TEXT_DISABLED),
            icon_default: rgba_css(t::COLOR_ICON_DEFAULT),
            button_default: rgba_css(t::COLOR_BUTTON_DEFAULT),
            button_hover: rgba_css(t::COLOR_BUTTON_HOVER),
            button_disabled: rgba_css(t::COLOR_BUTTON_DISABLED),
            button_destructive_hover: rgba_css(t::COLOR_BUTTON_DESTRUCTIVE_HOVER),
            button_destructive_active: rgba_css(t::COLOR_BUTTON_DESTRUCTIVE_ACTIVE),
            segment_active: rgba_css(t::COLOR_SEGMENT_ACTIVE),
            checkbox_default: rgba_css(t::COLOR_CHECKBOX_DEFAULT),
            checkbox_hover: rgba_css(t::COLOR_CHECKBOX_HOVER),
            checkbox_checked: rgba_css(t::COLOR_CHECKBOX_CHECKED),
            pin_default: rgba_css(t::COLOR_PIN_DEFAULT),
            pin_hover: rgba_css(t::COLOR_PIN_HOVER),
            close_default: rgba_css(t::COLOR_CLOSE_DEFAULT),
            close_hover: rgba_css(t::COLOR_CLOSE_HOVER),
            tooltip_bg: rgba_css(t::COLOR_TOOLTIP_BACKGROUND),
            tooltip_border: rgba_css(t::COLOR_TOOLTIP_BORDER),
            divider: rgba_css(t::COLOR_DIVIDER),
            panel_border: rgba_css_compact(t::COLOR_PANEL_BORDER),
            drag_handle: rgba_css_compact(t::COLOR_DRAG_HANDLE),
            drag_handle_hover: rgba_css_compact(t::COLOR_DRAG_HANDLE_HOVER),
            checkbox_border: rgba_css_compact(t::COLOR_CHECKBOX_BORDER),
            label_hint: rgba_css_compact(t::COLOR_LABEL_HINT),
            badge_text: rgba_css_compact(t::COLOR_BADGE_TEXT),
            badge_bg: rgba_css_compact(t::COLOR_BADGE_BACKGROUND),
            badge_border: rgba_css_compact(t::COLOR_BADGE_BORDER),
            popover_shadow: rgba_css_compact(t::COLOR_POPOVER_SHADOW),
            popover_shadow_clear: transparent_css(t::COLOR_POPOVER_SHADOW),
            header_band: rgba_css_compact(t::COLOR_HEADER_BAND),
            card: rgba_css_compact(t::COLOR_CARD_BACKGROUND),
            section_title: rgba_css_compact(t::COLOR_LABEL_SECTION),
            section_header_hover: rgba_css_compact(t::COLOR_SECTION_HEADER_HOVER),
            field_bg: rgba_css_compact(t::COLOR_FIELD_BACKGROUND),
            field_bg_hover: rgba_css_compact(t::COLOR_FIELD_BACKGROUND_HOVER),
            field_border: rgba_css_compact(t::COLOR_FIELD_BORDER),
            field_border_hover: rgba_css_compact(t::COLOR_FIELD_BORDER_HOVER),
            scrollbar_slider: rgba_css_compact(t::COLOR_SCROLLBAR_SLIDER),
            text_on_fill: hex_css(t::COLOR_TEXT_ON_FILL),
            radius_panel: px(t::RADIUS_PANEL),
            radius_card: px(t::RADIUS_CARD),
            radius_button: px(t::RADIUS_LG),
            radius_sm: px(t::RADIUS_SM),
            pad_std: px(t::SPACING_STD),
            pad_panel_h: px(t::PANEL_PADDING_H),
            pad_popover: px(t::SPACING_LG),
            check_size: px(t::CHECKBOX_SIZE),
            font_label: px(t::FONT_SIZE_LABEL),
            font_small: px(t::FONT_SIZE_SMALL),
            font_tooltip: px(t::FONT_SIZE_TOOLTIP),
            font_badge: px(t::FONT_SIZE_BADGE),
            font_swatch_key: px(t::FONT_SIZE_SWATCH_KEY),
            hairline: fixed_px(t::LINE_WIDTH_THIN),
            focus_ring_width: fixed_px(t::FOCUS_RING_WIDTH),
            focus_ring_offset: fixed_px(t::FOCUS_RING_OFFSET),
            glow_ring_width: fixed_px(t::ACTIVE_GLOW_WIDTH),
            indicator_height: fixed_px(t::ACTIVE_INDICATOR_HEIGHT),
            radius_full: fixed_px(t::RADIUS_FULL),
            radius_lg_fixed: fixed_px(t::RADIUS_LG),
            radius_std_fixed: fixed_px(t::RADIUS_STD),
            scrollbar_radius: fixed_px(t::SCROLLBAR_RADIUS),
            scrollbar_width: fixed_px(t::SCROLLBAR_WIDTH),
            spacing_xxs: fixed_px(t::SPACING_XXS),
            spacing_xs: fixed_px(t::SPACING_XS),
            spacing_sm: fixed_px(t::SPACING_SM),
            spacing_md: fixed_px(t::SPACING_MD),
            spacing_std: fixed_px(t::SPACING_STD),
            shadow_offset_y: fixed_px(t::POPOVER_SHADOW_OFFSET_Y),
            shadow_blur: fixed_px(t::POPOVER_SHADOW_BLUR),
            transition_ms: t::TRANSITION_HOVER_MS,
            weight_semibold: t::FONT_WEIGHT_SEMIBOLD,
            weight_bold: t::FONT_WEIGHT_BOLD,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rgba_css_rounds_channels_to_8_bit_and_pads_alpha_to_three_decimals() {
        assert_eq!(rgba_css(toolbar::COLOR_ACCENT), "rgba(53, 132, 228, 1.000)");
        assert_eq!(
            rgba_css((0.95, 0.95, 0.95, 0.9)),
            "rgba(242, 242, 242, 0.900)"
        );
    }

    #[test]
    fn rgba_css_compact_matches_the_hand_written_literal_precision() {
        // 0.7 * 255 lands exactly on 178.5 in f64 and rounds away from zero
        // to 179 — the value the sheet has always emitted for hints.
        assert_eq!(
            rgba_css_compact(toolbar::COLOR_LABEL_HINT),
            "rgba(179, 179, 191, 0.80)"
        );
        assert_eq!(
            rgba_css_compact(toolbar::COLOR_HEADER_BAND),
            "rgba(31, 33, 46, 0.45)"
        );
    }

    #[test]
    fn hex_css_emits_lowercase_rrggbb() {
        assert_eq!(hex_css(toolbar::COLOR_TEXT_ON_FILL), "#ffffff");
    }

    #[test]
    fn transparent_css_zeroes_alpha_as_integer() {
        assert_eq!(
            transparent_css(toolbar::COLOR_POPOVER_SHADOW),
            "rgba(0, 0, 0, 0)"
        );
    }

    #[test]
    fn scaled_px_floors_at_one_pixel() {
        assert_eq!(scaled_px(0.5, 0.5), 1);
        assert_eq!(scaled_px(13.0, 1.5), 20);
    }

    #[test]
    fn new_clamps_scale_and_maps_non_finite_to_unity() {
        assert_eq!(
            GtkStylesheetValues::new(f64::NAN),
            GtkStylesheetValues::new(1.0)
        );
        assert_eq!(
            GtkStylesheetValues::new(100.0),
            GtkStylesheetValues::new(3.0)
        );
        assert_eq!(GtkStylesheetValues::new(0.1), GtkStylesheetValues::new(0.5));
    }
}
