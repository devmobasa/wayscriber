//! Color picker popup rendering.
//!
//! Renders a modal popup with a large gradient color picker,
//! hex input field, and OK/Cancel buttons.

use crate::draw::Color;
use crate::input::InputState;
use crate::input::state::{
    COLOR_PICKER_PREVIEW_SIZE, COLOR_PICKER_RECENT_SWATCH_COUNT as RECENT_SWATCH_COUNT,
    COLOR_PICKER_RECENT_SWATCH_SIZE as RECENT_SWATCH_SIZE, ColorPickerPopupLayout,
};
use crate::ui::primitives::{draw_rounded_rect, ellipsize_to_fit, text_extents_for};
use crate::ui::theme::{Rgba, toolbar as toolbar_theme};
use crate::ui_text::{UiTextStyle, draw_text_baseline, measure_text};

use super::constants::{
    self, ACCENT_BRIGHT, ACCENT_PRIMARY, BG_INPUT_SELECTION, BORDER_MODAL, INPUT_BG,
    INPUT_BORDER_FOCUSED, INPUT_CARET, OVERLAY_DIM_MEDIUM, PANEL_BG_MODAL, RADIUS_MD, RADIUS_PANEL,
    RADIUS_SM, RADIUS_STD, TEXT_HINT_DIM, TEXT_PRIMARY,
};

// File-local colors with no matching theme token (M1 keep-if-not-matching
// rule: values kept verbatim from the pre-theme literals).
/// Hairline around the HSV gradient so it separates from the panel.
const GRADIENT_BORDER: Rgba = (1.0, 1.0, 1.0, 0.4);
/// White ring around the gradient position indicator.
const INDICATOR_RING: Rgba = (1.0, 1.0, 1.0, 0.95);
/// Dark outline outside the indicator ring (contrast on light hues).
const INDICATOR_OUTLINE: Rgba = (0.0, 0.0, 0.0, 0.4);
/// Alpha-checkerboard tiles behind the preview swatch.
const CHECKER_LIGHT: Rgba = (0.6, 0.6, 0.6, 1.0);
const CHECKER_DARK: Rgba = (0.4, 0.4, 0.4, 1.0);
/// Preview swatch border on dark colors (light gray) / light colors (dark).
/// TODO(theme-consolidation): dark variant duplicates
/// `theme::toolbar::COLOR_SWATCH_HAIRLINE_DARK`.
const SWATCH_BORDER_ON_DARK: Rgba = (0.5, 0.5, 0.5, 0.8);
const SWATCH_BORDER_ON_LIGHT: Rgba = (0.2, 0.2, 0.2, 0.6);
/// Validation-error red for the hex field (softer than DESTRUCTIVE_RGB).
const HEX_INVALID_GLOW: Rgba = (0.9, 0.3, 0.3, 0.25);
const HEX_INVALID_BORDER: Rgba = (0.9, 0.35, 0.3, 0.9);
/// Hex input border when unfocused.
const INPUT_BORDER_IDLE: Rgba = (0.3, 0.3, 0.35, 0.8);
/// Neutral fill for the eyedropper button at rest.
const EYEDROPPER_BG: Rgba = (0.18, 0.2, 0.24, 0.95);
/// Secondary (Cancel) button fill/border ladder.
const BUTTON_SECONDARY_BG: Rgba = (0.25, 0.25, 0.30, 0.95);
const BUTTON_SECONDARY_BG_HOVER: Rgba = (0.30, 0.30, 0.38, 0.98);
const BUTTON_SECONDARY_BORDER: Rgba = (0.4, 0.4, 0.45, 0.8);
const BUTTON_SECONDARY_BORDER_HOVER: Rgba = (0.5, 0.5, 0.55, 0.9);
/// White glow behind hovered secondary buttons.
const BUTTON_HOVER_GLOW: Rgba = (1.0, 1.0, 1.0, 0.1);
const TOOLTIP_PADDING_X: f64 = 8.0;
const TOOLTIP_PADDING_Y: f64 = 5.0;
const TOOLTIP_POINTER_OFFSET: f64 = 12.0;
const TOOLTIP_SCREEN_MARGIN: f64 = 6.0;
const TOOLTIP_SHADOW_OFFSET: f64 = 2.0;
/// Left inset of the title, mirrored on the right as its trim margin.
const TITLE_INSET: f64 = 20.0;

/// Bounds of every pixel the color picker may change while it stays open.
/// The full-screen dimmer is stable between open and close; those transitions
/// already request full damage, so typing and hover updates only need this
/// panel-plus-tooltip footprint.
pub fn color_picker_popup_visual_geometry(
    input_state: &InputState,
    screen_width: u32,
    screen_height: u32,
) -> Option<(f64, f64, f64, f64)> {
    if !input_state.is_color_picker_popup_open() {
        return None;
    }

    let layout = ColorPickerPopupLayout::compute(
        screen_width,
        screen_height,
        input_state.color_picker_popup_shows_default_button(),
    );
    let mut bounds = (
        layout.origin_x,
        layout.origin_y,
        layout.width,
        layout.height,
    );
    if let Some((hover_x, hover_y)) = input_state.color_picker_popup_hover()
        && let Some((tooltip, anchor_x, anchor_y)) =
            layout.action_tooltip_anchor_at(hover_x, hover_y)
        && let Some((x, y, width, height)) = action_tooltip_geometry(
            tooltip,
            anchor_x,
            anchor_y,
            screen_width as f64,
            screen_height as f64,
        )
    {
        bounds = union_bounds(
            bounds,
            (
                x,
                y,
                width + TOOLTIP_SHADOW_OFFSET,
                height + TOOLTIP_SHADOW_OFFSET,
            ),
        );
    }
    Some(bounds)
}

fn union_bounds(a: (f64, f64, f64, f64), b: (f64, f64, f64, f64)) -> (f64, f64, f64, f64) {
    let min_x = a.0.min(b.0);
    let min_y = a.1.min(b.1);
    let max_x = (a.0 + a.2).max(b.0 + b.2);
    let max_y = (a.1 + a.3).max(b.1 + b.3);
    (min_x, min_y, max_x - min_x, max_y - min_y)
}

/// Render the color picker popup.
pub fn render_color_picker_popup(
    ctx: &cairo::Context,
    input_state: &InputState,
    screen_width: u32,
    screen_height: u32,
) {
    if !input_state.is_color_picker_popup_open() {
        return;
    }

    let layout = match input_state.color_picker_popup_layout() {
        Some(layout) => layout,
        None => return,
    };

    let current_color = match input_state.color_picker_popup_current_color() {
        Some(color) => color,
        None => return,
    };

    let hex_buffer = input_state
        .color_picker_popup_hex_buffer()
        .unwrap_or("#000000");
    let is_hex_editing = input_state.color_picker_popup_is_hex_editing();
    let is_hex_selected = input_state.color_picker_popup_hex_selected();

    let _ = ctx.save();

    // Dim background
    ctx.set_source_rgba(0.0, 0.0, 0.0, OVERLAY_DIM_MEDIUM);
    ctx.rectangle(0.0, 0.0, screen_width as f64, screen_height as f64);
    let _ = ctx.fill();

    // Panel background
    draw_rounded_rect(
        ctx,
        layout.origin_x,
        layout.origin_y,
        layout.width,
        layout.height,
        RADIUS_PANEL,
    );
    constants::set_color(ctx, PANEL_BG_MODAL);
    let _ = ctx.fill_preserve();
    constants::set_color(ctx, BORDER_MODAL);
    ctx.set_line_width(1.0);
    let _ = ctx.stroke();

    // Title
    let title_style = UiTextStyle {
        family: "Sans",
        slant: cairo::FontSlant::Normal,
        weight: cairo::FontWeight::Bold,
        size: 16.0,
    };
    constants::set_color(ctx, TEXT_PRIMARY);
    let title_y = layout.origin_y + 20.0 + 16.0;
    // Names the swatch when the popup was opened to recolor one, so the target
    // of OK is never ambiguous. A recolor title carries a user-authored label,
    // so it is trimmed to the panel: unwrapped text wider than the panel would
    // draw outside the bounds this popup reports as damaged.
    let title = fit_title_to_panel(
        ctx,
        title_style,
        &input_state.color_picker_popup_title(),
        &layout,
    );
    draw_text_baseline(
        ctx,
        title_style,
        &title,
        layout.origin_x + TITLE_INSET,
        title_y,
        None,
    );

    // Saturation/value square, tinted by the hue the picker remembers rather
    // than by the current color: grey and black carry no hue of their own.
    let hue = input_state.color_picker_popup_hue_position().unwrap_or(0.0);
    draw_sat_val_square(ctx, layout.sv_x, layout.sv_y, layout.sv_w, layout.sv_h, hue);

    if let Some((norm_x, norm_y)) = input_state.color_picker_popup_gradient_position() {
        let indicator_x = layout.sv_x + norm_x * layout.sv_w;
        let indicator_y = layout.sv_y + norm_y * layout.sv_h;
        draw_color_indicator(ctx, indicator_x, indicator_y, current_color);
    }

    // Hue bar
    draw_hue_bar(ctx, layout.hue_x, layout.hue_y, layout.hue_w, layout.hue_h);
    let hue_marker_x = layout.hue_x + hue * layout.hue_w;
    draw_bar_marker(ctx, hue_marker_x, layout.hue_y, layout.hue_h);

    // Preview swatch
    draw_preview_swatch(
        ctx,
        layout.preview_x,
        layout.preview_y,
        COLOR_PICKER_PREVIEW_SIZE,
        current_color,
    );

    // Recent colors, most-recent-first. Empty until something has been
    // applied, so a fresh session shows no strip rather than dead slots.
    let recents = input_state.recent_colors();
    for (index, color) in recents.iter().take(RECENT_SWATCH_COUNT).enumerate() {
        let (sx, sy) = layout.recent_swatch_origin(index);
        draw_recent_swatch(ctx, sx, sy, *color, *color == current_color);
    }

    // Check if hex value is valid (for validation feedback)
    let hex_valid = input_state.color_picker_popup_hex_valid();

    // Hex input field
    draw_hex_input(
        ctx,
        layout.hex_input_x,
        layout.hex_input_y,
        layout.hex_input_w,
        layout.hex_input_h,
        hex_buffer,
        is_hex_editing,
        is_hex_selected,
        hex_valid,
    );

    // Trailing action cluster on the preview row: copy the live hex, paste a
    // hex from the clipboard, or sample a color from the screen. Copy/paste
    // restore the inline hex actions the pre-overhaul color section carried.
    let hover_pos = input_state.color_picker_popup_hover();
    let size = layout.action_btn_size;
    let copy_hover = hover_pos
        .map(|(hx, hy)| layout.point_in_copy_button(hx, hy))
        .unwrap_or(false);
    let paste_hover = hover_pos
        .map(|(hx, hy)| layout.point_in_paste_button(hx, hy))
        .unwrap_or(false);
    let eyedropper_hover = hover_pos
        .map(|(hx, hy)| layout.point_in_eyedropper_button(hx, hy))
        .unwrap_or(false);
    draw_action_button(
        ctx,
        layout.copy_btn_x,
        layout.copy_btn_y,
        size,
        copy_hover,
        crate::toolbar_icons::draw_icon_copy,
        16.0,
    );
    draw_action_button(
        ctx,
        layout.paste_btn_x,
        layout.paste_btn_y,
        size,
        paste_hover,
        crate::toolbar_icons::draw_icon_paste,
        16.0,
    );
    draw_action_button(
        ctx,
        layout.eyedropper_btn_x,
        layout.eyedropper_btn_y,
        size,
        eyedropper_hover,
        crate::toolbar_icons::draw_icon_eyedropper,
        18.0,
    );

    // Determine button hover states
    let ok_hover = hover_pos
        .map(|(hx, hy)| layout.point_in_ok_button(hx, hy))
        .unwrap_or(false);
    let cancel_hover = hover_pos
        .map(|(hx, hy)| layout.point_in_cancel_button(hx, hy))
        .unwrap_or(false);

    // "Default" button: only while recoloring a slot the shipped palette
    // defines. It stages the built-in color instead of committing, so it is a
    // secondary action next to OK/Cancel.
    if let Some((default_x, default_y)) = layout.default_btn {
        let default_hover = hover_pos
            .map(|(hx, hy)| layout.point_in_default_button(hx, hy))
            .unwrap_or(false);
        draw_button(
            ctx,
            default_x,
            default_y,
            layout.btn_width,
            layout.btn_height,
            "Default",
            false, // secondary
            default_hover,
        );
    }

    // OK button
    draw_button(
        ctx,
        layout.ok_btn_x,
        layout.ok_btn_y,
        layout.btn_width,
        layout.btn_height,
        "OK",
        true, // primary
        ok_hover,
    );

    // Cancel button
    draw_button(
        ctx,
        layout.cancel_btn_x,
        layout.cancel_btn_y,
        layout.btn_width,
        layout.btn_height,
        "Cancel",
        false, // secondary
        cancel_hover,
    );

    // Keyboard shortcut hint
    let hint_style = UiTextStyle {
        family: "Sans",
        slant: cairo::FontSlant::Normal,
        weight: cairo::FontWeight::Normal,
        size: 10.0,
    };
    constants::set_color(ctx, TEXT_HINT_DIM);
    let hint = "Enter = OK  •  Esc = Cancel";
    let hint_extents = text_extents_for(
        ctx,
        "Sans",
        cairo::FontSlant::Normal,
        cairo::FontWeight::Normal,
        10.0,
        hint,
    );
    let hint_x = layout.origin_x + (layout.width - hint_extents.width()) / 2.0;
    let hint_y = layout.ok_btn_y + layout.btn_height + 12.0;
    draw_text_baseline(ctx, hint_style, hint, hint_x, hint_y, None);

    if let Some((hover_x, hover_y)) = hover_pos
        && let Some((tooltip, anchor_x, anchor_y)) =
            layout.action_tooltip_anchor_at(hover_x, hover_y)
    {
        draw_action_tooltip(
            ctx,
            tooltip,
            anchor_x,
            anchor_y,
            screen_width as f64,
            screen_height as f64,
        );
    }

    let _ = ctx.restore();
}

/// Trim the popup title to the panel's content width. Slot labels come from
/// `config.toml`, so no character budget can bound the shaped width; the real
/// text is measured instead.
fn fit_title_to_panel(
    ctx: &cairo::Context,
    style: UiTextStyle<'_>,
    title: &str,
    layout: &ColorPickerPopupLayout,
) -> String {
    let max_width = (layout.width - TITLE_INSET * 2.0).max(0.0);
    ellipsize_to_fit(
        ctx,
        title,
        style.family,
        style.size,
        style.weight,
        max_width,
    )
}

/// Draw the HSV color gradient.
/// Draw the saturation (x) by value (y) square for one hue.
///
/// White-to-hue horizontally, then black over the top vertically — the standard
/// construction, and the one the toolbar's inline picker already uses. Both
/// axes are real: unlike the previous hue-by-value gradient, every point here
/// maps to the colour actually produced by clicking it.
fn draw_sat_val_square(ctx: &cairo::Context, x: f64, y: f64, w: f64, h: f64, hue: f64) {
    let full = crate::draw::color::hsv_to_rgb(hue, 1.0, 1.0);

    let sat_grad = cairo::LinearGradient::new(x, y, x + w, y);
    sat_grad.add_color_stop_rgba(0.0, 1.0, 1.0, 1.0, 1.0);
    sat_grad.add_color_stop_rgba(1.0, full.r, full.g, full.b, 1.0);
    ctx.rectangle(x, y, w, h);
    let _ = ctx.set_source(&sat_grad);
    let _ = ctx.fill();

    let val_grad = cairo::LinearGradient::new(x, y, x, y + h);
    val_grad.add_color_stop_rgba(0.0, 0.0, 0.0, 0.0, 0.0);
    val_grad.add_color_stop_rgba(1.0, 0.0, 0.0, 0.0, 1.0);
    ctx.rectangle(x, y, w, h);
    let _ = ctx.set_source(&val_grad);
    let _ = ctx.fill();

    constants::set_color(ctx, GRADIENT_BORDER);
    ctx.rectangle(x + 0.5, y + 0.5, w - 1.0, h - 1.0);
    ctx.set_line_width(1.0);
    let _ = ctx.stroke();
}

/// Draw the horizontal hue bar.
fn draw_hue_bar(ctx: &cairo::Context, x: f64, y: f64, w: f64, h: f64) {
    let hue_grad = cairo::LinearGradient::new(x, y, x + w, y);
    for step in 0..=6 {
        let t = f64::from(step) / 6.0;
        let color = crate::draw::color::hsv_to_rgb(t, 1.0, 1.0);
        hue_grad.add_color_stop_rgba(t, color.r, color.g, color.b, 1.0);
    }
    ctx.rectangle(x, y, w, h);
    let _ = ctx.set_source(&hue_grad);
    let _ = ctx.fill();

    constants::set_color(ctx, GRADIENT_BORDER);
    ctx.rectangle(x + 0.5, y + 0.5, w - 1.0, h - 1.0);
    ctx.set_line_width(1.0);
    let _ = ctx.stroke();
}

/// Draw one recent-color swatch, ringed when it matches the live color.
fn draw_recent_swatch(ctx: &cairo::Context, x: f64, y: f64, color: Color, selected: bool) {
    ctx.set_source_rgba(color.r, color.g, color.b, 1.0);
    ctx.rectangle(x, y, RECENT_SWATCH_SIZE, RECENT_SWATCH_SIZE);
    let _ = ctx.fill();

    if selected {
        constants::set_color(ctx, INDICATOR_RING);
        ctx.set_line_width(2.0);
        ctx.rectangle(
            x - 1.0,
            y - 1.0,
            RECENT_SWATCH_SIZE + 2.0,
            RECENT_SWATCH_SIZE + 2.0,
        );
    } else {
        constants::set_color(ctx, GRADIENT_BORDER);
        ctx.set_line_width(1.0);
        ctx.rectangle(
            x + 0.5,
            y + 0.5,
            RECENT_SWATCH_SIZE - 1.0,
            RECENT_SWATCH_SIZE - 1.0,
        );
    }
    let _ = ctx.stroke();
}

/// Draw the position marker for a horizontal bar.
fn draw_bar_marker(ctx: &cairo::Context, x: f64, y: f64, h: f64) {
    constants::set_color(ctx, INDICATOR_OUTLINE);
    ctx.rectangle(x - 2.5, y - 1.5, 5.0, h + 3.0);
    ctx.set_line_width(3.0);
    let _ = ctx.stroke_preserve();
    constants::set_color(ctx, INDICATOR_RING);
    ctx.set_line_width(1.5);
    let _ = ctx.stroke();
}

/// Draw the color indicator dot on the gradient.
fn draw_color_indicator(ctx: &cairo::Context, x: f64, y: f64, color: Color) {
    let radius = 6.0;

    // Outer white ring
    constants::set_color(ctx, INDICATOR_RING);
    ctx.arc(x, y, radius + 2.0, 0.0, std::f64::consts::PI * 2.0);
    let _ = ctx.fill();

    // Inner color circle
    ctx.set_source_rgba(color.r, color.g, color.b, 1.0);
    ctx.arc(x, y, radius, 0.0, std::f64::consts::PI * 2.0);
    let _ = ctx.fill();

    // Dark outline
    constants::set_color(ctx, INDICATOR_OUTLINE);
    ctx.set_line_width(1.0);
    ctx.arc(x, y, radius + 2.0, 0.0, std::f64::consts::PI * 2.0);
    let _ = ctx.stroke();
}

/// Draw the preview swatch.
fn draw_preview_swatch(ctx: &cairo::Context, x: f64, y: f64, size: f64, color: Color) {
    // Draw checkered background for transparency preview
    let check_size = 6.0;
    constants::set_color(ctx, CHECKER_LIGHT);
    draw_rounded_rect(ctx, x, y, size, size, RADIUS_SM);
    let _ = ctx.fill();

    constants::set_color(ctx, CHECKER_DARK);
    let mut cy = y;
    let mut row = 0;
    while cy < y + size {
        let mut cx = x + if row % 2 == 0 { 0.0 } else { check_size };
        while cx < x + size {
            let w = (x + size - cx).min(check_size);
            let h = (y + size - cy).min(check_size);
            ctx.rectangle(cx, cy, w, h);
            let _ = ctx.fill();
            cx += check_size * 2.0;
        }
        cy += check_size;
        row += 1;
    }

    // Draw color
    ctx.set_source_rgba(color.r, color.g, color.b, color.a);
    draw_rounded_rect(ctx, x, y, size, size, RADIUS_SM);
    let _ = ctx.fill();

    // Border
    let luminance = 0.299 * color.r + 0.587 * color.g + 0.114 * color.b;
    if luminance < 0.3 {
        constants::set_color(ctx, SWATCH_BORDER_ON_DARK);
    } else {
        constants::set_color(ctx, SWATCH_BORDER_ON_LIGHT);
    }
    ctx.set_line_width(1.5);
    draw_rounded_rect(ctx, x, y, size, size, RADIUS_SM);
    let _ = ctx.stroke();
}

/// Draw the hex input field with validation feedback.
#[allow(clippy::too_many_arguments)]
fn draw_hex_input(
    ctx: &cairo::Context,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    value: &str,
    focused: bool,
    selected: bool,
    valid: bool,
) {
    // Outer glow when focused - red if invalid, accent if valid
    if focused {
        if valid {
            constants::set_color(ctx, constants::with_alpha(ACCENT_PRIMARY, 0.2));
        } else {
            constants::set_color(ctx, HEX_INVALID_GLOW);
        }
        draw_rounded_rect(ctx, x - 2.0, y - 2.0, w + 4.0, h + 4.0, RADIUS_STD);
        let _ = ctx.fill();
    }

    // Background
    constants::set_color(ctx, INPUT_BG);
    draw_rounded_rect(ctx, x, y, w, h, RADIUS_SM);
    let _ = ctx.fill();

    // Border - red if invalid, blue if focused, gray otherwise
    if !valid && focused {
        constants::set_color(ctx, HEX_INVALID_BORDER);
        ctx.set_line_width(2.0);
    } else if focused {
        constants::set_color(ctx, INPUT_BORDER_FOCUSED);
        ctx.set_line_width(2.0);
    } else {
        constants::set_color(ctx, INPUT_BORDER_IDLE);
        ctx.set_line_width(1.0);
    }
    draw_rounded_rect(ctx, x, y, w, h, RADIUS_SM);
    let _ = ctx.stroke();

    // Text
    let value_style = UiTextStyle {
        family: "Sans",
        slant: cairo::FontSlant::Normal,
        weight: cairo::FontWeight::Normal,
        size: 13.0,
    };
    let extents = text_extents_for(
        ctx,
        "Sans",
        cairo::FontSlant::Normal,
        cairo::FontWeight::Normal,
        13.0,
        value,
    );
    let text_x = x + 8.0;
    let text_y = y + h / 2.0 + extents.height() / 2.0;

    // Draw selection highlight when selected (full text selected)
    if selected {
        constants::set_color(ctx, BG_INPUT_SELECTION);
        draw_rounded_rect(
            ctx,
            text_x - 2.0,
            y + 3.0,
            extents.width() + 4.0,
            h - 6.0,
            2.0,
        );
        let _ = ctx.fill();
    }

    constants::set_color(ctx, TEXT_PRIMARY);
    draw_text_baseline(ctx, value_style, value, text_x, text_y, None);

    // Cursor when focused (at end of text)
    if focused {
        constants::set_color(ctx, INPUT_CARET);
        let cursor_x = text_x + extents.width() + 2.0;
        ctx.set_line_width(1.5);
        ctx.move_to(cursor_x, y + 4.0);
        ctx.line_to(cursor_x, y + h - 4.0);
        let _ = ctx.stroke();
    }
}

/// Draw one square action button (copy / paste / eyedropper) on the popup's
/// preview row: a neutral rounded fill washed with the accent on hover, and a
/// centered icon.
fn draw_action_button(
    ctx: &cairo::Context,
    x: f64,
    y: f64,
    size: f64,
    hovered: bool,
    icon: fn(&cairo::Context, f64, f64, f64),
    icon_size: f64,
) {
    draw_rounded_rect(ctx, x, y, size, size, RADIUS_MD);
    if hovered {
        constants::set_color(ctx, constants::with_alpha(ACCENT_PRIMARY, 0.8));
    } else {
        constants::set_color(ctx, EYEDROPPER_BG);
    }
    let _ = ctx.fill_preserve();
    constants::set_color(ctx, BORDER_MODAL);
    ctx.set_line_width(1.0);
    let _ = ctx.stroke();
    constants::set_color(ctx, TEXT_PRIMARY);
    icon(
        ctx,
        x + (size - icon_size) / 2.0,
        y + (size - icon_size) / 2.0,
        icon_size,
    );
}

fn draw_action_tooltip(
    ctx: &cairo::Context,
    text: &str,
    anchor_x: f64,
    anchor_y: f64,
    screen_width: f64,
    screen_height: f64,
) {
    let Some((x, y, width, height)) =
        action_tooltip_geometry(text, anchor_x, anchor_y, screen_width, screen_height)
    else {
        return;
    };
    let style = action_tooltip_text_style();

    constants::set_color(ctx, toolbar_theme::COLOR_TOOLTIP_SHADOW);
    draw_rounded_rect(
        ctx,
        x + TOOLTIP_SHADOW_OFFSET,
        y + TOOLTIP_SHADOW_OFFSET,
        width,
        height,
        RADIUS_SM,
    );
    let _ = ctx.fill();

    constants::set_color(ctx, toolbar_theme::COLOR_TOOLTIP_BACKGROUND);
    draw_rounded_rect(ctx, x, y, width, height, RADIUS_SM);
    let _ = ctx.fill_preserve();
    constants::set_color(ctx, toolbar_theme::COLOR_TOOLTIP_BORDER);
    ctx.set_line_width(1.0);
    let _ = ctx.stroke();

    constants::set_color(ctx, TEXT_PRIMARY);
    draw_text_baseline(
        ctx,
        style,
        text,
        x + TOOLTIP_PADDING_X,
        y + TOOLTIP_PADDING_Y + style.size,
        None,
    );
}

fn action_tooltip_text_style() -> UiTextStyle<'static> {
    UiTextStyle {
        family: toolbar_theme::FONT_FAMILY_DEFAULT,
        slant: cairo::FontSlant::Normal,
        weight: cairo::FontWeight::Normal,
        size: toolbar_theme::FONT_SIZE_TOOLTIP,
    }
}

fn action_tooltip_geometry(
    text: &str,
    anchor_x: f64,
    anchor_y: f64,
    screen_width: f64,
    screen_height: f64,
) -> Option<(f64, f64, f64, f64)> {
    let style = action_tooltip_text_style();
    let extents = measure_text(style, text, None)?;
    let width = extents.width() + TOOLTIP_PADDING_X * 2.0;
    let height = style.size + TOOLTIP_PADDING_Y * 2.0;
    let max_x = (screen_width - width - TOOLTIP_SCREEN_MARGIN).max(TOOLTIP_SCREEN_MARGIN);
    let x = (anchor_x + TOOLTIP_POINTER_OFFSET).clamp(TOOLTIP_SCREEN_MARGIN, max_x);
    let above_y = anchor_y - height - TOOLTIP_POINTER_OFFSET;
    let preferred_y = if above_y >= TOOLTIP_SCREEN_MARGIN {
        above_y
    } else {
        anchor_y + TOOLTIP_POINTER_OFFSET
    };
    let max_y = (screen_height - height - TOOLTIP_SCREEN_MARGIN).max(TOOLTIP_SCREEN_MARGIN);
    let y = preferred_y.clamp(TOOLTIP_SCREEN_MARGIN, max_y);
    Some((x, y, width, height))
}

/// Draw a button with hover state.
#[allow(clippy::too_many_arguments)]
fn draw_button(
    ctx: &cairo::Context,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    label: &str,
    primary: bool,
    hover: bool,
) {
    // Hover glow effect
    if hover {
        let glow_color = if primary {
            constants::with_alpha(ACCENT_PRIMARY, 0.25)
        } else {
            BUTTON_HOVER_GLOW
        };
        constants::set_color(ctx, glow_color);
        draw_rounded_rect(ctx, x - 2.0, y - 2.0, w + 4.0, h + 4.0, RADIUS_MD + 2.0);
        let _ = ctx.fill();
    }

    // Background - brighter on hover
    if primary {
        if hover {
            // Accent nudged towards accent-bright so hover reads brighter
            let fill = constants::lerp_color(ACCENT_PRIMARY, ACCENT_BRIGHT, 0.25);
            constants::set_color(ctx, constants::with_alpha(fill, 0.98));
        } else {
            constants::set_color(ctx, constants::with_alpha(ACCENT_PRIMARY, 0.95));
        }
    } else if hover {
        constants::set_color(ctx, BUTTON_SECONDARY_BG_HOVER);
    } else {
        constants::set_color(ctx, BUTTON_SECONDARY_BG);
    }
    draw_rounded_rect(ctx, x, y, w, h, RADIUS_MD);
    let _ = ctx.fill();

    // Border - stronger on hover
    if primary {
        if hover {
            constants::set_color(ctx, ACCENT_BRIGHT);
        } else {
            constants::set_color(ctx, constants::with_alpha(ACCENT_BRIGHT, 0.9));
        }
    } else if hover {
        constants::set_color(ctx, BUTTON_SECONDARY_BORDER_HOVER);
    } else {
        constants::set_color(ctx, BUTTON_SECONDARY_BORDER);
    }
    ctx.set_line_width(1.0);
    draw_rounded_rect(ctx, x, y, w, h, RADIUS_MD);
    let _ = ctx.stroke();

    // Label
    let label_style = UiTextStyle {
        family: "Sans",
        slant: cairo::FontSlant::Normal,
        weight: cairo::FontWeight::Bold,
        size: 13.0,
    };
    constants::set_color(ctx, TEXT_PRIMARY);

    let extents = text_extents_for(
        ctx,
        "Sans",
        cairo::FontSlant::Normal,
        cairo::FontWeight::Bold,
        13.0,
        label,
    );
    let text_x = x + (w - extents.width()) / 2.0;
    let text_y = y + h / 2.0 + extents.height() / 2.0;
    draw_text_baseline(ctx, label_style, label, text_x, text_y, None);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_context() -> cairo::Context {
        let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, 1920, 1080)
            .expect("test image surface");
        cairo::Context::new(&surface).expect("test cairo context")
    }

    fn title_style() -> UiTextStyle<'static> {
        UiTextStyle {
            family: "Sans",
            slant: cairo::FontSlant::Normal,
            weight: cairo::FontWeight::Bold,
            size: 16.0,
        }
    }

    fn measured_width(ctx: &cairo::Context, text: &str) -> f64 {
        let style = title_style();
        text_extents_for(
            ctx,
            style.family,
            style.slant,
            style.weight,
            style.size,
            text,
        )
        .width()
    }

    #[test]
    fn wide_titles_are_trimmed_to_the_panel_not_a_character_budget() {
        let ctx = test_context();
        let layout = ColorPickerPopupLayout::compute(1920, 1080, true);
        let content_width = layout.width - TITLE_INSET * 2.0;

        // Wide glyphs: few characters, far more pixels than a Latin label of
        // the same length, which is why the budget has to be measured.
        let wide = format!("Recolor {}", "W".repeat(40));
        let fitted = fit_title_to_panel(&ctx, title_style(), &wide, &layout);
        assert!(
            measured_width(&ctx, &wide) > content_width,
            "fixture is wide"
        );
        assert!(measured_width(&ctx, &fitted) <= content_width);
        assert!(fitted.len() < wide.len());

        // Non-Latin scripts take the same path.
        let cjk = format!("Recolor {}", "測".repeat(40));
        let fitted_cjk = fit_title_to_panel(&ctx, title_style(), &cjk, &layout);
        assert!(measured_width(&ctx, &fitted_cjk) <= content_width);

        // A title that already fits is left exactly as composed.
        let short = "Recolor Pink";
        assert_eq!(
            fit_title_to_panel(&ctx, title_style(), short, &layout),
            short
        );
    }
}
