//! Stylesheet for the GTK toolbars, generated from the same design tokens
//! the built-in Cairo bars use: every value below is interpolated from
//! [`crate::ui::theme::toolbar`] tokens via
//! [`crate::ui::theme::css::GtkStylesheetValues`], so both frontends share
//! one visual language and the template holds no literal style values.
//! Regenerated when `toolbar_scale` changes: the scale multiplies font sizes
//! and paddings while widget geometry scales through the size-request path.

use crate::ui::theme::css::GtkStylesheetValues;

// Tokens the GTK widgets also paint directly (swatch fills and the slider
// DrawingArea); re-exported under their pre-M1 local names.
pub(super) use crate::ui::theme::toolbar::{
    COLOR_ACCENT as ACCENT, COLOR_TRACK_BACKGROUND as TRACK_BACKGROUND,
    COLOR_TRACK_KNOB as TRACK_KNOB,
};

pub(super) const CAPTURE_TRANSPARENT_CLASS: &str = "wayscriber-capture-transparent";

/// Full stylesheet at the given toolbar scale.
pub(super) fn stylesheet(scale: f64) -> String {
    let v = GtkStylesheetValues::new(scale);
    format!(
        r#"
window.wayscriber-toolbar {{
    background: transparent;
}}

/* ===== Panels ========================================================== */
.wayscriber-toolbar .panel {{
    background-color: {panel};
    border: {hairline}px solid {panel_border};
    border-radius: {radius_panel}px;
    padding: {pad_std}px {pad_panel_h}px;
}}

/* Minimized restore tabs hug the button, like the builtin 64x24 tab. */
.wayscriber-toolbar .panel.minimized {{
    padding: 0;
}}

/* Top-strip islands: detached pills sharing the panel treatment. The
   window root stays transparent behind them. The horizontal padding is the
   island pad the builtin planner budgets with (TOP_ISLAND_PAD), not the
   wider panel padding. */
.wayscriber-toolbar .pill {{
    background-color: {panel};
    border: {hairline}px solid {panel_border};
    border-radius: {radius_panel}px;
    padding: {pad_std}px {pad_island}px;
}}

/* Compact plans tighten the island inner padding, mirroring the builtin
   TOP_COMPACT_ISLAND_PAD. */
.wayscriber-toolbar .pill.compact {{
    padding: {pad_std}px {pad_island_compact}px;
}}

/* ===== Buttons ========================================================= */
.wayscriber-toolbar button {{
    min-width: 0;
    min-height: 0;
    padding: 0;
    margin: 0;
    border: none;
    outline: none;
    background: none;
    background-color: {button_default};
    border-radius: {radius_button}px;
    color: {icon_default};
    font-size: {font_label}px;
    font-weight: {weight_semibold};
    transition: background-color {transition_ms}ms ease, color {transition_ms}ms ease;
}}
.wayscriber-toolbar button:hover {{
    background-color: {button_hover};
    color: {text_primary};
}}
.wayscriber-toolbar button:focus-visible {{
    outline: {focus_ring_width}px solid {accent};
    outline-offset: {focus_ring_offset}px;
}}
.wayscriber-toolbar button.active {{
    background-color: {accent};
    color: {text_on_fill};
    box-shadow: 0 0 0 {glow_ring_width}px {accent_glow},
        inset 0 -{indicator_height}px 0 0 {accent_bright};
}}
/* Destructive buttons are normal flat buttons at rest; the red fill
   appears only on hover/press. */
.wayscriber-toolbar button.destructive:hover {{
    background-color: {button_destructive_hover};
    color: {text_on_fill};
}}
.wayscriber-toolbar button.destructive:active {{
    background-color: {button_destructive_active};
    color: {text_on_fill};
}}
.wayscriber-toolbar button:disabled {{
    background-color: {button_disabled};
    color: {text_disabled};
}}

/* Drag handle: quiet until hovered. */
.wayscriber-toolbar .drag-handle {{
    background: none;
    color: {drag_handle};
    padding: 0;
}}
.wayscriber-toolbar .drag-handle:hover {{
    color: {drag_handle_hover};
}}

/* Round chrome buttons (pin, minimize, restore tabs). */
.wayscriber-toolbar button.chrome {{
    border-radius: {radius_full}px;
    background-color: {pin_default};
}}
.wayscriber-toolbar button.chrome:hover {{
    background-color: {pin_hover};
}}
.wayscriber-toolbar button.chrome.pinned {{
    background-color: {accent};
    color: {text_on_fill};
    box-shadow: 0 0 0 {glow_ring_width}px {accent_glow};
}}
.wayscriber-toolbar button.chrome.minimize {{
    background-color: {close_default};
}}
.wayscriber-toolbar button.chrome.minimize:hover {{
    background-color: {close_hover};
}}

/* Swatch buttons paint their fill in a DrawingArea; the button itself
   stays transparent so only the drawn swatch shows. */
.wayscriber-toolbar button.swatch {{
    background: none;
    background-color: transparent;
    box-shadow: none;
}}

/* ===== Checkboxes ====================================================== */
.wayscriber-toolbar checkbutton {{
    color: {text_primary};
    font-size: {font_label}px;
}}
.wayscriber-toolbar checkbutton.mini {{
    font-size: {font_small}px;
}}
.wayscriber-toolbar checkbutton check {{
    background-color: {checkbox_default};
    border: {hairline}px solid {checkbox_border};
    border-radius: {radius_sm}px;
    min-width: {check_size}px;
    min-height: {check_size}px;
    -gtk-icon-source: none;
}}
.wayscriber-toolbar checkbutton:hover check {{
    background-color: {checkbox_hover};
}}
.wayscriber-toolbar checkbutton check:checked {{
    background-color: {checkbox_checked};
    -gtk-icon-source: -gtk-icontheme("object-select-symbolic");
    color: {text_on_fill};
}}

/* ===== Dividers ======================================================== */
.wayscriber-toolbar separator {{
    background-color: {divider};
    min-width: {hairline}px;
    min-height: {hairline}px;
}}

/* ===== Labels ========================================================== */
.wayscriber-toolbar label {{
    color: {text_primary};
    font-size: {font_label}px;
}}
.wayscriber-toolbar label.hint {{
    color: {label_hint};
    font-size: {font_small}px;
}}
.wayscriber-toolbar label.shortcut-badge {{
    color: {badge_text};
    background-color: {badge_bg};
    border: {hairline}px solid {badge_border};
    border-radius: {radius_sm}px;
    padding: 0 {spacing_xs}px;
    font-size: {font_badge}px;
    font-weight: {weight_bold};
}}
/* Swatch key letters (above) and icon-button captions (below) read as
   small unboxed captions in the secondary text color (mirrors
   COLOR_LABEL_HINT), one step larger than boxed corner badges. */
.wayscriber-toolbar label.shortcut-badge.above-swatch,
.wayscriber-toolbar label.shortcut-badge.below-icon {{
    background-color: transparent;
    border-color: transparent;
    padding: 0;
    color: {label_hint};
    font-size: {font_swatch_key}px;
}}

/* ===== Popovers ======================================================== */
.wayscriber-toolbar popover > arrow {{
    background: none;
}}
.wayscriber-toolbar popover > contents {{
    background-color: {panel};
    border: {hairline}px solid {panel_border};
    border-radius: {radius_panel}px;
    padding: {pad_popover}px;
    box-shadow: 0 {shadow_offset_y}px {shadow_blur}px {popover_shadow};
}}
/* A popover is its own native surface. Its contents and arrow CSS nodes paint
   outside the application-provided child, so capture suppression must clear
   their pixels too. Keep border and shadow dimensions unchanged to avoid a
   resize while the transparent replacement buffer is being acknowledged. */
.wayscriber-toolbar popover.{capture_transparent_class},
.wayscriber-toolbar popover.{capture_transparent_class} > arrow {{
    background: transparent;
    border-color: transparent;
    box-shadow: none;
    outline-color: transparent;
}}
.wayscriber-toolbar popover.{capture_transparent_class} > contents {{
    background: transparent;
    border-color: transparent;
    box-shadow: 0 {shadow_offset_y}px {shadow_blur}px {popover_shadow_clear};
    outline-color: transparent;
}}

/* ===== Side palette ==================================================== */
.wayscriber-toolbar .header-band {{
    background-color: {header_band};
    border-radius: {radius_card}px;
    padding: {spacing_sm}px {spacing_md}px;
}}
.wayscriber-toolbar .card {{
    background-color: {card};
    border-radius: {radius_card}px;
    padding: {pad_std}px;
}}
.wayscriber-toolbar .section-title {{
    color: {section_title};
    font-size: {font_label}px;
    font-weight: {weight_bold};
}}
.wayscriber-toolbar button.section-header {{
    background: none;
    background-color: transparent;
    box-shadow: none;
    padding: {spacing_xs}px 0;
}}
.wayscriber-toolbar button.section-header:hover {{
    background-color: {section_header_hover};
}}
.wayscriber-toolbar button.tab {{
    font-size: {font_tooltip}px;
    font-weight: {weight_bold};
    padding: {spacing_xs}px 0;
}}
.wayscriber-toolbar button.tab.active {{
    background-color: {segment_active};
    box-shadow: none;
}}
.wayscriber-toolbar button.board-chip {{
    background-color: {field_bg};
    border: {hairline}px solid {field_border};
    border-radius: {radius_lg_fixed}px;
    padding: {spacing_xxs}px {spacing_std}px;
    font-size: {font_label}px;
    font-weight: {weight_bold};
}}
.wayscriber-toolbar button.board-chip:hover {{
    background-color: {field_bg_hover};
    border-color: {field_border_hover};
}}
.wayscriber-toolbar entry {{
    background-color: {field_bg};
    border: {hairline}px solid {field_border};
    border-radius: {radius_std_fixed}px;
    color: {text_primary};
    font-size: {font_tooltip}px;
    min-height: 0;
    padding: {spacing_xxs}px {spacing_md}px;
    caret-color: {accent};
}}
.wayscriber-toolbar scrollbar {{
    background: transparent;
}}
.wayscriber-toolbar scrollbar slider {{
    background-color: {scrollbar_slider};
    border-radius: {scrollbar_radius}px;
    min-width: {scrollbar_width}px;
}}

/* ===== Tooltips ======================================================== */
tooltip {{
    background-color: {tooltip_bg};
    border: {hairline}px solid {tooltip_border};
    border-radius: {radius_lg_fixed}px;
    color: {text_primary};
    padding: {spacing_xs}px {spacing_std}px;
    font-size: {font_tooltip}px;
}}
/* GtkTooltipWindow is a private GtkNative with its own popup surface. Its
   native CSS node paints outside the application-provided custom widget, so
   suppress the chrome without unmapping the popup. */
tooltip.{capture_transparent_class} {{
    background: transparent;
    border-color: transparent;
    box-shadow: none;
    outline-color: transparent;
    color: transparent;
}}
"#,
        panel = v.panel,
        panel_border = v.panel_border,
        accent = v.accent,
        accent_glow = v.accent_glow,
        accent_bright = v.accent_bright,
        text_primary = v.text_primary,
        text_disabled = v.text_disabled,
        text_on_fill = v.text_on_fill,
        icon_default = v.icon_default,
        button_default = v.button_default,
        button_hover = v.button_hover,
        button_disabled = v.button_disabled,
        button_destructive_hover = v.button_destructive_hover,
        button_destructive_active = v.button_destructive_active,
        segment_active = v.segment_active,
        checkbox_default = v.checkbox_default,
        checkbox_hover = v.checkbox_hover,
        checkbox_checked = v.checkbox_checked,
        checkbox_border = v.checkbox_border,
        pin_default = v.pin_default,
        pin_hover = v.pin_hover,
        close_default = v.close_default,
        close_hover = v.close_hover,
        tooltip_bg = v.tooltip_bg,
        tooltip_border = v.tooltip_border,
        divider = v.divider,
        drag_handle = v.drag_handle,
        drag_handle_hover = v.drag_handle_hover,
        label_hint = v.label_hint,
        badge_text = v.badge_text,
        badge_bg = v.badge_bg,
        badge_border = v.badge_border,
        popover_shadow = v.popover_shadow,
        popover_shadow_clear = v.popover_shadow_clear,
        header_band = v.header_band,
        card = v.card,
        section_title = v.section_title,
        section_header_hover = v.section_header_hover,
        field_bg = v.field_bg,
        field_bg_hover = v.field_bg_hover,
        field_border = v.field_border,
        field_border_hover = v.field_border_hover,
        scrollbar_slider = v.scrollbar_slider,
        capture_transparent_class = CAPTURE_TRANSPARENT_CLASS,
        radius_panel = v.radius_panel,
        radius_card = v.radius_card,
        radius_button = v.radius_button,
        radius_sm = v.radius_sm,
        radius_full = v.radius_full,
        radius_lg_fixed = v.radius_lg_fixed,
        radius_std_fixed = v.radius_std_fixed,
        pad_std = v.pad_std,
        pad_panel_h = v.pad_panel_h,
        pad_island = v.pad_island,
        pad_island_compact = v.pad_island_compact,
        pad_popover = v.pad_popover,
        check_size = v.check_size,
        font_label = v.font_label,
        font_badge = v.font_badge,
        font_swatch_key = v.font_swatch_key,
        font_small = v.font_small,
        font_tooltip = v.font_tooltip,
        hairline = v.hairline,
        focus_ring_width = v.focus_ring_width,
        focus_ring_offset = v.focus_ring_offset,
        glow_ring_width = v.glow_ring_width,
        indicator_height = v.indicator_height,
        spacing_xxs = v.spacing_xxs,
        spacing_xs = v.spacing_xs,
        spacing_sm = v.spacing_sm,
        spacing_md = v.spacing_md,
        spacing_std = v.spacing_std,
        scrollbar_radius = v.scrollbar_radius,
        scrollbar_width = v.scrollbar_width,
        shadow_offset_y = v.shadow_offset_y,
        shadow_blur = v.shadow_blur,
        transition_ms = v.transition_ms,
        weight_semibold = v.weight_semibold,
        weight_bold = v.weight_bold,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::theme::css::{hex_css, rgba_css, rgba_css_compact, transparent_css};
    use crate::ui::theme::toolbar as t;

    /// Every color the stylesheet emits, as a (token name, emitted CSS
    /// string) pair — one row per (theme token, formatter) combination the
    /// sheet uses. The two tests below assert both directions: each row
    /// appears in the output, and every color in the output matches a row.
    /// Adding a color to the stylesheet therefore means adding its theme
    /// token here; a hand-edited literal fails the no-rogue-color test.
    fn emitted_colors() -> Vec<(&'static str, String)> {
        vec![
            // Routed through rgba_css (three-decimal alpha).
            (
                "COLOR_PANEL_BACKGROUND",
                rgba_css(t::COLOR_PANEL_BACKGROUND),
            ),
            ("COLOR_ACCENT", rgba_css(t::COLOR_ACCENT)),
            ("COLOR_ACCENT_GLOW", rgba_css(t::COLOR_ACCENT_GLOW)),
            ("COLOR_ACCENT_BRIGHT", rgba_css(t::COLOR_ACCENT_BRIGHT)),
            ("COLOR_TEXT_PRIMARY", rgba_css(t::COLOR_TEXT_PRIMARY)),
            ("COLOR_TEXT_DISABLED", rgba_css(t::COLOR_TEXT_DISABLED)),
            ("COLOR_ICON_DEFAULT", rgba_css(t::COLOR_ICON_DEFAULT)),
            ("COLOR_BUTTON_DEFAULT", rgba_css(t::COLOR_BUTTON_DEFAULT)),
            ("COLOR_BUTTON_HOVER", rgba_css(t::COLOR_BUTTON_HOVER)),
            ("COLOR_BUTTON_DISABLED", rgba_css(t::COLOR_BUTTON_DISABLED)),
            (
                "COLOR_BUTTON_DESTRUCTIVE_HOVER",
                rgba_css(t::COLOR_BUTTON_DESTRUCTIVE_HOVER),
            ),
            (
                "COLOR_BUTTON_DESTRUCTIVE_ACTIVE",
                rgba_css(t::COLOR_BUTTON_DESTRUCTIVE_ACTIVE),
            ),
            ("COLOR_SEGMENT_ACTIVE", rgba_css(t::COLOR_SEGMENT_ACTIVE)),
            (
                "COLOR_CHECKBOX_DEFAULT",
                rgba_css(t::COLOR_CHECKBOX_DEFAULT),
            ),
            ("COLOR_CHECKBOX_HOVER", rgba_css(t::COLOR_CHECKBOX_HOVER)),
            (
                "COLOR_CHECKBOX_CHECKED",
                rgba_css(t::COLOR_CHECKBOX_CHECKED),
            ),
            ("COLOR_PIN_DEFAULT", rgba_css(t::COLOR_PIN_DEFAULT)),
            ("COLOR_PIN_HOVER", rgba_css(t::COLOR_PIN_HOVER)),
            ("COLOR_CLOSE_DEFAULT", rgba_css(t::COLOR_CLOSE_DEFAULT)),
            ("COLOR_CLOSE_HOVER", rgba_css(t::COLOR_CLOSE_HOVER)),
            (
                "COLOR_TOOLTIP_BACKGROUND",
                rgba_css(t::COLOR_TOOLTIP_BACKGROUND),
            ),
            ("COLOR_TOOLTIP_BORDER", rgba_css(t::COLOR_TOOLTIP_BORDER)),
            ("COLOR_DIVIDER", rgba_css(t::COLOR_DIVIDER)),
            // Routed through rgba_css_compact (two-decimal alpha; these were
            // hand-written literals before M1).
            (
                "COLOR_PANEL_BORDER",
                rgba_css_compact(t::COLOR_PANEL_BORDER),
            ),
            ("COLOR_DRAG_HANDLE", rgba_css_compact(t::COLOR_DRAG_HANDLE)),
            (
                "COLOR_DRAG_HANDLE_HOVER",
                rgba_css_compact(t::COLOR_DRAG_HANDLE_HOVER),
            ),
            (
                "COLOR_CHECKBOX_BORDER",
                rgba_css_compact(t::COLOR_CHECKBOX_BORDER),
            ),
            ("COLOR_LABEL_HINT", rgba_css_compact(t::COLOR_LABEL_HINT)),
            ("COLOR_BADGE_TEXT", rgba_css_compact(t::COLOR_BADGE_TEXT)),
            (
                "COLOR_BADGE_BACKGROUND",
                rgba_css_compact(t::COLOR_BADGE_BACKGROUND),
            ),
            (
                "COLOR_BADGE_BORDER",
                rgba_css_compact(t::COLOR_BADGE_BORDER),
            ),
            (
                "COLOR_POPOVER_SHADOW",
                rgba_css_compact(t::COLOR_POPOVER_SHADOW),
            ),
            ("COLOR_HEADER_BAND", rgba_css_compact(t::COLOR_HEADER_BAND)),
            (
                "COLOR_CARD_BACKGROUND",
                rgba_css_compact(t::COLOR_CARD_BACKGROUND),
            ),
            (
                "COLOR_LABEL_SECTION",
                rgba_css_compact(t::COLOR_LABEL_SECTION),
            ),
            (
                "COLOR_SECTION_HEADER_HOVER",
                rgba_css_compact(t::COLOR_SECTION_HEADER_HOVER),
            ),
            (
                "COLOR_FIELD_BACKGROUND",
                rgba_css_compact(t::COLOR_FIELD_BACKGROUND),
            ),
            (
                "COLOR_FIELD_BACKGROUND_HOVER",
                rgba_css_compact(t::COLOR_FIELD_BACKGROUND_HOVER),
            ),
            (
                "COLOR_FIELD_BORDER",
                rgba_css_compact(t::COLOR_FIELD_BORDER),
            ),
            (
                "COLOR_FIELD_BORDER_HOVER",
                rgba_css_compact(t::COLOR_FIELD_BORDER_HOVER),
            ),
            (
                "COLOR_SCROLLBAR_SLIDER",
                rgba_css_compact(t::COLOR_SCROLLBAR_SLIDER),
            ),
            // Special forms.
            (
                "COLOR_POPOVER_SHADOW (alpha zeroed)",
                transparent_css(t::COLOR_POPOVER_SHADOW),
            ),
            ("COLOR_TEXT_ON_FILL", hex_css(t::COLOR_TEXT_ON_FILL)),
        ]
    }

    /// All `rgba(...)` substrings in the stylesheet, in order.
    fn rgba_occurrences(css: &str) -> Vec<String> {
        let mut found = Vec::new();
        let mut rest = css;
        while let Some(start) = rest.find("rgba(") {
            let tail = &rest[start..];
            let len = tail.find(')').expect("unterminated rgba( in stylesheet") + 1;
            found.push(tail[..len].to_string());
            rest = &tail[len..];
        }
        found
    }

    /// All `#hex` color substrings in the stylesheet.
    fn hex_occurrences(css: &str) -> Vec<String> {
        let mut found = Vec::new();
        for (i, _) in css.match_indices('#') {
            let digits: String = css[i + 1..]
                .chars()
                .take_while(|c| c.is_ascii_hexdigit())
                .collect();
            if !digits.is_empty() {
                found.push(format!("#{digits}"));
            }
        }
        found
    }

    #[test]
    fn every_theme_token_color_appears_in_the_stylesheet() {
        let css = stylesheet(1.0);
        for (token, value) in emitted_colors() {
            assert!(
                css.contains(&value),
                "stylesheet no longer emits {token} as {value}"
            );
        }
    }

    #[test]
    fn stylesheet_has_no_color_literal_outside_the_theme_tokens() {
        let css = stylesheet(1.0);
        let known: std::collections::HashSet<String> = emitted_colors()
            .into_iter()
            .map(|(_, value)| value)
            .collect();
        let rgba_found = rgba_occurrences(&css);
        let hex_found = hex_occurrences(&css);
        // Sanity: the scans actually see the sheet's colors.
        assert!(rgba_found.len() >= known.len() - 1);
        assert!(!hex_found.is_empty());
        for occurrence in rgba_found.iter().chain(hex_found.iter()) {
            assert!(
                known.contains(occurrence),
                "hand-written color literal in stylesheet: {occurrence} \
                 (route it through a theme token and emitted_colors())"
            );
        }
    }

    #[test]
    fn capture_suppression_clears_private_native_popup_chrome() {
        let css = stylesheet(1.0);
        assert!(css.contains(&format!("tooltip.{CAPTURE_TRANSPARENT_CLASS} {{")));
        assert!(css.contains(&format!(
            "popover.{CAPTURE_TRANSPARENT_CLASS} > contents {{"
        )));
        assert!(css.contains("background: transparent;"));
        assert!(css.contains("border-color: transparent;"));
        assert!(css.contains("box-shadow: none;"));
    }
}
