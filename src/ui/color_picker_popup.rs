//! Color picker popup rendering.
//!
//! Renders a modal popup with a large gradient color picker,
//! hex input field, and OK/Cancel buttons.

use crate::draw::Color;
use crate::input::InputState;
use crate::input::state::COLOR_PICKER_PREVIEW_SIZE;
use crate::ui::primitives::{draw_rounded_rect, text_extents_for};
use crate::ui::theme::Rgba;
use crate::ui_text::{UiTextStyle, draw_text_baseline};

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
    draw_text_baseline(
        ctx,
        title_style,
        "Select Color",
        layout.origin_x + 20.0,
        title_y,
        None,
    );

    // Gradient picker
    draw_color_gradient(
        ctx,
        layout.gradient_x,
        layout.gradient_y,
        layout.gradient_w,
        layout.gradient_h,
    );

    // Draw color indicator on gradient
    if let Some((norm_x, norm_y)) = input_state.color_picker_popup_gradient_position() {
        let indicator_x = layout.gradient_x + norm_x * layout.gradient_w;
        let indicator_y = layout.gradient_y + norm_y * layout.gradient_h;
        draw_color_indicator(ctx, indicator_x, indicator_y, current_color);
    }

    // Preview swatch
    draw_preview_swatch(
        ctx,
        layout.preview_x,
        layout.preview_y,
        COLOR_PICKER_PREVIEW_SIZE,
        current_color,
    );

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

    let _ = ctx.restore();
}

/// Draw the HSV color gradient.
fn draw_color_gradient(ctx: &cairo::Context, x: f64, y: f64, w: f64, h: f64) {
    // Horizontal hue gradient
    let hue_grad = cairo::LinearGradient::new(x, y, x + w, y);
    hue_grad.add_color_stop_rgba(0.0, 1.0, 0.0, 0.0, 1.0); // Red
    hue_grad.add_color_stop_rgba(0.17, 1.0, 1.0, 0.0, 1.0); // Yellow
    hue_grad.add_color_stop_rgba(0.33, 0.0, 1.0, 0.0, 1.0); // Green
    hue_grad.add_color_stop_rgba(0.5, 0.0, 1.0, 1.0, 1.0); // Cyan
    hue_grad.add_color_stop_rgba(0.66, 0.0, 0.0, 1.0, 1.0); // Blue
    hue_grad.add_color_stop_rgba(0.83, 1.0, 0.0, 1.0, 1.0); // Magenta
    hue_grad.add_color_stop_rgba(1.0, 1.0, 0.0, 0.0, 1.0); // Red

    ctx.rectangle(x, y, w, h);
    let _ = ctx.set_source(&hue_grad);
    let _ = ctx.fill();

    // Vertical value gradient (white at top, black at bottom)
    let val_grad = cairo::LinearGradient::new(x, y, x, y + h);
    val_grad.add_color_stop_rgba(0.0, 1.0, 1.0, 1.0, 0.0); // Transparent white
    val_grad.add_color_stop_rgba(1.0, 0.0, 0.0, 0.0, 0.65); // Black with alpha

    ctx.rectangle(x, y, w, h);
    let _ = ctx.set_source(&val_grad);
    let _ = ctx.fill();

    // Border
    constants::set_color(ctx, GRADIENT_BORDER);
    ctx.rectangle(x + 0.5, y + 0.5, w - 1.0, h - 1.0);
    ctx.set_line_width(1.0);
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
