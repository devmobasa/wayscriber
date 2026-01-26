//! Color picker popup rendering.
//!
//! Renders a modal popup with a large gradient color picker,
//! hex input field, and OK/Cancel buttons.

use crate::draw::Color;
use crate::input::InputState;
use crate::input::state::COLOR_PICKER_PREVIEW_SIZE;
use crate::ui::primitives::{draw_rounded_rect, text_extents_for};

use super::constants::{
    self, BORDER_MODAL, INPUT_BG, INPUT_BORDER_FOCUSED, INPUT_CARET, OVERLAY_DIM_MEDIUM,
    PANEL_BG_MODAL, RADIUS_MD, RADIUS_PANEL, TEXT_PRIMARY,
};

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
    ctx.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Bold);
    ctx.set_font_size(16.0);
    constants::set_color(ctx, TEXT_PRIMARY);
    let title_y = layout.origin_y + 20.0 + 16.0;
    ctx.move_to(layout.origin_x + 20.0, title_y);
    let _ = ctx.show_text("Select Color");

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

    // Determine button hover states
    let hover_pos = input_state.color_picker_popup_hover();
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
    ctx.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Normal);
    ctx.set_font_size(10.0);
    ctx.set_source_rgba(0.6, 0.6, 0.65, 0.7);
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
    ctx.move_to(hint_x, hint_y);
    let _ = ctx.show_text(hint);

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
    ctx.set_source_rgba(1.0, 1.0, 1.0, 0.4);
    ctx.rectangle(x + 0.5, y + 0.5, w - 1.0, h - 1.0);
    ctx.set_line_width(1.0);
    let _ = ctx.stroke();
}

/// Draw the color indicator dot on the gradient.
fn draw_color_indicator(ctx: &cairo::Context, x: f64, y: f64, color: Color) {
    let radius = 6.0;

    // Outer white ring
    ctx.set_source_rgba(1.0, 1.0, 1.0, 0.95);
    ctx.arc(x, y, radius + 2.0, 0.0, std::f64::consts::PI * 2.0);
    let _ = ctx.fill();

    // Inner color circle
    ctx.set_source_rgba(color.r, color.g, color.b, 1.0);
    ctx.arc(x, y, radius, 0.0, std::f64::consts::PI * 2.0);
    let _ = ctx.fill();

    // Dark outline
    ctx.set_source_rgba(0.0, 0.0, 0.0, 0.4);
    ctx.set_line_width(1.0);
    ctx.arc(x, y, radius + 2.0, 0.0, std::f64::consts::PI * 2.0);
    let _ = ctx.stroke();
}

/// Draw the preview swatch.
fn draw_preview_swatch(ctx: &cairo::Context, x: f64, y: f64, size: f64, color: Color) {
    // Draw checkered background for transparency preview
    let check_size = 6.0;
    ctx.set_source_rgba(0.6, 0.6, 0.6, 1.0);
    draw_rounded_rect(ctx, x, y, size, size, 4.0);
    let _ = ctx.fill();

    ctx.set_source_rgba(0.4, 0.4, 0.4, 1.0);
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
    draw_rounded_rect(ctx, x, y, size, size, 4.0);
    let _ = ctx.fill();

    // Border
    let luminance = 0.299 * color.r + 0.587 * color.g + 0.114 * color.b;
    if luminance < 0.3 {
        ctx.set_source_rgba(0.5, 0.5, 0.5, 0.8);
    } else {
        ctx.set_source_rgba(0.2, 0.2, 0.2, 0.6);
    }
    ctx.set_line_width(1.5);
    draw_rounded_rect(ctx, x, y, size, size, 4.0);
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
    // Outer glow when focused - red if invalid, blue if valid
    if focused {
        if valid {
            ctx.set_source_rgba(0.3, 0.5, 0.9, 0.2);
        } else {
            ctx.set_source_rgba(0.9, 0.3, 0.3, 0.25);
        }
        draw_rounded_rect(ctx, x - 2.0, y - 2.0, w + 4.0, h + 4.0, 6.0);
        let _ = ctx.fill();
    }

    // Background
    constants::set_color(ctx, INPUT_BG);
    draw_rounded_rect(ctx, x, y, w, h, 4.0);
    let _ = ctx.fill();

    // Border - red if invalid, blue if focused, gray otherwise
    if !valid && focused {
        ctx.set_source_rgba(0.9, 0.35, 0.3, 0.9);
        ctx.set_line_width(2.0);
    } else if focused {
        constants::set_color(ctx, INPUT_BORDER_FOCUSED);
        ctx.set_line_width(2.0);
    } else {
        ctx.set_source_rgba(0.3, 0.3, 0.35, 0.8);
        ctx.set_line_width(1.0);
    }
    draw_rounded_rect(ctx, x, y, w, h, 4.0);
    let _ = ctx.stroke();

    // Text
    ctx.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Normal);
    ctx.set_font_size(13.0);

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
        ctx.set_source_rgba(0.3, 0.5, 0.9, 0.4);
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
    ctx.move_to(text_x, text_y);
    let _ = ctx.show_text(value);

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
            (0.3, 0.5, 0.9, 0.25)
        } else {
            (1.0, 1.0, 1.0, 0.1)
        };
        ctx.set_source_rgba(glow_color.0, glow_color.1, glow_color.2, glow_color.3);
        draw_rounded_rect(ctx, x - 2.0, y - 2.0, w + 4.0, h + 4.0, RADIUS_MD + 2.0);
        let _ = ctx.fill();
    }

    // Background - brighter on hover
    if primary {
        if hover {
            ctx.set_source_rgba(0.30, 0.50, 0.80, 0.98);
        } else {
            ctx.set_source_rgba(0.25, 0.45, 0.75, 0.95);
        }
    } else if hover {
        ctx.set_source_rgba(0.30, 0.30, 0.38, 0.98);
    } else {
        ctx.set_source_rgba(0.25, 0.25, 0.30, 0.95);
    }
    draw_rounded_rect(ctx, x, y, w, h, RADIUS_MD);
    let _ = ctx.fill();

    // Border - stronger on hover
    if primary {
        if hover {
            ctx.set_source_rgba(0.45, 0.65, 0.95, 0.95);
        } else {
            ctx.set_source_rgba(0.35, 0.55, 0.85, 0.9);
        }
    } else if hover {
        ctx.set_source_rgba(0.5, 0.5, 0.55, 0.9);
    } else {
        ctx.set_source_rgba(0.4, 0.4, 0.45, 0.8);
    }
    ctx.set_line_width(1.0);
    draw_rounded_rect(ctx, x, y, w, h, RADIUS_MD);
    let _ = ctx.stroke();

    // Label
    ctx.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Bold);
    ctx.set_font_size(13.0);
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
    ctx.move_to(text_x, text_y);
    let _ = ctx.show_text(label);
}
