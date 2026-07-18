//! Stylesheet for the GTK toolbars, generated from the same design tokens
//! the built-in Cairo bars use (`render/widgets/constants.rs` values are
//! mirrored here as rgba strings) so both frontends share one visual
//! language. Regenerated when `toolbar_scale` changes: the scale multiplies
//! font sizes and paddings while widget geometry scales through the
//! size-request path.

fn rgba(color: (f64, f64, f64, f64)) -> String {
    format!(
        "rgba({}, {}, {}, {:.3})",
        (color.0 * 255.0).round() as u8,
        (color.1 * 255.0).round() as u8,
        (color.2 * 255.0).round() as u8,
        color.3
    )
}

// Mirrors of render/widgets/constants.rs tokens.
const ACCENT: (f64, f64, f64, f64) = (0.3, 0.55, 1.0, 1.0);
const ACCENT_GLOW: (f64, f64, f64, f64) = (0.3, 0.55, 1.0, 0.25);
const ACCENT_BRIGHT: (f64, f64, f64, f64) = (0.5, 0.75, 1.0, 0.95);
const TEXT_PRIMARY: (f64, f64, f64, f64) = (1.0, 1.0, 1.0, 0.95);
const TEXT_DISABLED: (f64, f64, f64, f64) = (0.62, 0.62, 0.68, 0.45);
const ICON_DEFAULT: (f64, f64, f64, f64) = (0.95, 0.95, 0.95, 0.9);
const BUTTON_HOVER: (f64, f64, f64, f64) = (0.35, 0.35, 0.45, 0.85);
const BUTTON_DEFAULT: (f64, f64, f64, f64) = (0.2, 0.22, 0.26, 0.75);
const BUTTON_DISABLED: (f64, f64, f64, f64) = (0.2, 0.22, 0.26, 0.35);
const BUTTON_DESTRUCTIVE: (f64, f64, f64, f64) = (0.34, 0.2, 0.2, 0.8);
const BUTTON_DESTRUCTIVE_HOVER: (f64, f64, f64, f64) = (0.52, 0.24, 0.22, 0.9);
const CHECKBOX_DEFAULT: (f64, f64, f64, f64) = (0.22, 0.24, 0.28, 0.75);
const CHECKBOX_HOVER: (f64, f64, f64, f64) = (0.32, 0.34, 0.4, 0.9);
const CHECKBOX_CHECKED: (f64, f64, f64, f64) = (0.25, 0.5, 0.35, 0.9);
const PIN_DEFAULT: (f64, f64, f64, f64) = (0.3, 0.3, 0.35, 0.7);
const PIN_HOVER: (f64, f64, f64, f64) = (0.35, 0.35, 0.45, 0.85);
const CLOSE_DEFAULT: (f64, f64, f64, f64) = (0.5, 0.5, 0.55, 0.7);
const CLOSE_HOVER: (f64, f64, f64, f64) = (0.8, 0.3, 0.3, 0.9);
const PANEL_BACKGROUND: (f64, f64, f64, f64) = (0.05, 0.05, 0.08, 0.92);
const TOOLTIP_BACKGROUND: (f64, f64, f64, f64) = (0.1, 0.1, 0.15, 0.95);
const TOOLTIP_BORDER: (f64, f64, f64, f64) = (0.4, 0.4, 0.5, 0.8);
const DIVIDER: (f64, f64, f64, f64) = (1.0, 1.0, 1.0, 0.08);

pub(super) const CAPTURE_TRANSPARENT_CLASS: &str = "wayscriber-capture-transparent";

/// Full stylesheet at the given toolbar scale.
pub(super) fn stylesheet(scale: f64) -> String {
    let scale = if scale.is_finite() {
        scale.clamp(0.5, 3.0)
    } else {
        1.0
    };
    let px = |value: f64| -> i32 { (value * scale).round().max(1.0) as i32 };
    format!(
        r#"
window.wayscriber-toolbar {{
    background: transparent;
}}

/* ===== Panels ========================================================== */
.wayscriber-toolbar .panel {{
    background-color: {panel};
    border: 1px solid rgba(255, 255, 255, 0.10);
    border-radius: {radius_panel}px;
    padding: {pad_panel_v}px {pad_panel_h}px;
}}

/* Minimized restore tabs hug the button, like the builtin 64x24 tab. */
.wayscriber-toolbar .panel.minimized {{
    padding: 0;
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
    font-weight: 600;
    transition: background-color 120ms ease, color 120ms ease;
}}
.wayscriber-toolbar button:hover {{
    background-color: {button_hover};
    color: {text_primary};
}}
.wayscriber-toolbar button:focus-visible {{
    outline: 2px solid {accent};
    outline-offset: 1px;
}}
.wayscriber-toolbar button.active {{
    background-color: {accent};
    color: #ffffff;
    box-shadow: 0 0 0 2px {accent_glow},
        inset 0 -2px 0 0 {accent_bright};
}}
.wayscriber-toolbar button.destructive {{
    background-color: {button_destructive};
    color: {text_primary};
}}
.wayscriber-toolbar button.destructive:hover {{
    background-color: {button_destructive_hover};
    color: #ffffff;
}}
.wayscriber-toolbar button:disabled {{
    background-color: {button_disabled};
    color: {text_disabled};
}}

/* Drag handle: quiet until hovered. */
.wayscriber-toolbar .drag-handle {{
    background: none;
    color: rgba(255, 255, 255, 0.45);
    padding: 0;
}}
.wayscriber-toolbar .drag-handle:hover {{
    color: rgba(255, 255, 255, 0.75);
}}

/* Round chrome buttons (pin, minimize, overflow, restore tabs). */
.wayscriber-toolbar button.chrome {{
    border-radius: 9999px;
    background-color: {pin_default};
}}
.wayscriber-toolbar button.chrome:hover {{
    background-color: {pin_hover};
}}
.wayscriber-toolbar button.chrome.pinned {{
    background-color: {accent};
    color: #ffffff;
    box-shadow: 0 0 0 2px {accent_glow};
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
    border: 1px solid rgba(255, 255, 255, 0.25);
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
    color: #ffffff;
}}

/* ===== Dividers ======================================================== */
.wayscriber-toolbar separator {{
    background-color: {divider};
    min-width: 1px;
    min-height: 1px;
}}

/* ===== Labels ========================================================== */
.wayscriber-toolbar label {{
    color: {text_primary};
    font-size: {font_label}px;
}}
.wayscriber-toolbar label.hint {{
    color: rgba(179, 179, 191, 0.80);
    font-size: {font_small}px;
}}
.wayscriber-toolbar label.shortcut-badge {{
    color: rgba(255, 255, 255, 0.96);
    background-color: rgba(9, 10, 15, 0.82);
    border: 1px solid rgba(255, 255, 255, 0.24);
    border-radius: {radius_sm}px;
    padding: 0 2px;
    font-size: {font_badge}px;
    font-weight: 700;
}}
.wayscriber-toolbar label.shortcut-badge.above-swatch {{
    background-color: transparent;
    border-color: transparent;
    padding: 0;
}}

/* ===== Popovers ======================================================== */
.wayscriber-toolbar popover > arrow {{
    background: none;
}}
.wayscriber-toolbar popover > contents {{
    background-color: {panel};
    border: 1px solid rgba(255, 255, 255, 0.10);
    border-radius: {radius_panel}px;
    padding: {pad_popover}px;
    box-shadow: 0 10px 30px rgba(0, 0, 0, 0.50);
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
    box-shadow: 0 10px 30px rgba(0, 0, 0, 0);
    outline-color: transparent;
}}

/* ===== Side palette ==================================================== */
.wayscriber-toolbar .header-band {{
    background-color: rgba(31, 33, 46, 0.45);
    border-radius: {radius_card}px;
    padding: 3px 4px;
}}
.wayscriber-toolbar .card {{
    background-color: rgba(31, 31, 46, 0.35);
    border-radius: {radius_card}px;
    padding: {pad_card}px;
}}
.wayscriber-toolbar .section-title {{
    color: rgba(204, 204, 217, 0.90);
    font-size: {font_label}px;
    font-weight: 700;
}}
.wayscriber-toolbar button.section-header {{
    background: none;
    background-color: transparent;
    box-shadow: none;
    padding: 2px 0;
}}
.wayscriber-toolbar button.section-header:hover {{
    background-color: rgba(255, 255, 255, 0.04);
}}
.wayscriber-toolbar button.tab {{
    font-size: {font_tooltip}px;
    font-weight: 700;
    padding: 2px 0;
}}
.wayscriber-toolbar button.tab.active {{
    background-color: rgba(61, 92, 148, 1.0);
    box-shadow: none;
}}
.wayscriber-toolbar button.board-chip {{
    background-color: rgba(56, 61, 71, 0.95);
    border: 1px solid rgba(166, 179, 204, 0.45);
    border-radius: 6px;
    padding: 1px 6px;
    font-size: {font_label}px;
    font-weight: 700;
}}
.wayscriber-toolbar button.board-chip:hover {{
    background-color: rgba(71, 77, 87, 0.95);
    border-color: rgba(166, 179, 204, 0.70);
}}
.wayscriber-toolbar entry {{
    background-color: rgba(56, 61, 71, 0.95);
    border: 1px solid rgba(166, 179, 204, 0.45);
    border-radius: 4px;
    color: {text_primary};
    font-size: {font_tooltip}px;
    min-height: 0;
    padding: 1px 4px;
    caret-color: {accent};
}}
.wayscriber-toolbar scrollbar {{
    background: transparent;
}}
.wayscriber-toolbar scrollbar slider {{
    background-color: rgba(255, 255, 255, 0.35);
    border-radius: 2px;
    min-width: 4px;
}}

/* ===== Tooltips ======================================================== */
tooltip {{
    background-color: {tooltip_bg};
    border: 1px solid {tooltip_border};
    border-radius: 6px;
    color: {text_primary};
    padding: 2px 6px;
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
        panel = rgba(PANEL_BACKGROUND),
        accent = rgba(ACCENT),
        accent_glow = rgba(ACCENT_GLOW),
        accent_bright = rgba(ACCENT_BRIGHT),
        text_primary = rgba(TEXT_PRIMARY),
        text_disabled = rgba(TEXT_DISABLED),
        icon_default = rgba(ICON_DEFAULT),
        button_default = rgba(BUTTON_DEFAULT),
        button_hover = rgba(BUTTON_HOVER),
        button_disabled = rgba(BUTTON_DISABLED),
        button_destructive = rgba(BUTTON_DESTRUCTIVE),
        button_destructive_hover = rgba(BUTTON_DESTRUCTIVE_HOVER),
        checkbox_default = rgba(CHECKBOX_DEFAULT),
        checkbox_hover = rgba(CHECKBOX_HOVER),
        checkbox_checked = rgba(CHECKBOX_CHECKED),
        pin_default = rgba(PIN_DEFAULT),
        pin_hover = rgba(PIN_HOVER),
        close_default = rgba(CLOSE_DEFAULT),
        close_hover = rgba(CLOSE_HOVER),
        tooltip_bg = rgba(TOOLTIP_BACKGROUND),
        tooltip_border = rgba(TOOLTIP_BORDER),
        divider = rgba(DIVIDER),
        capture_transparent_class = CAPTURE_TRANSPARENT_CLASS,
        radius_panel = px(14.0),
        radius_card = px(8.0),
        pad_card = px(6.0),
        radius_button = px(6.0),
        radius_sm = px(3.0),
        pad_panel_v = px(6.0),
        pad_panel_h = px(10.0),
        pad_popover = px(8.0),
        check_size = px(14.0),
        font_label = px(13.0),
        font_badge = px(8.0),
        font_small = px(10.0),
        font_tooltip = px(12.0),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capture_suppression_clears_private_tooltip_window_chrome() {
        let css = stylesheet(1.0);
        assert!(css.contains(&format!("tooltip.{CAPTURE_TRANSPARENT_CLASS} {{")));
        assert!(css.contains("background: transparent;"));
        assert!(css.contains("border-color: transparent;"));
        assert!(css.contains("box-shadow: none;"));
    }
}
