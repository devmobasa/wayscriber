//! Popup color controls and their labels/tooltips.

use super::*;

/// Draw the HSV color gradient.
/// Draw the saturation (x) by value (y) square for one hue.
///
/// White-to-hue horizontally, then black over the top vertically — the standard
/// construction, and the one the toolbar's inline picker already uses. Both
/// axes are real: unlike the previous hue-by-value gradient, every point here
/// maps to the colour actually produced by clicking it.
pub(super) fn draw_sat_val_square(ctx: &cairo::Context, x: f64, y: f64, w: f64, h: f64, hue: f64) {
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
pub(super) fn draw_hue_bar(ctx: &cairo::Context, x: f64, y: f64, w: f64, h: f64) {
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
pub(super) fn draw_recent_swatch(
    ctx: &cairo::Context,
    x: f64,
    y: f64,
    color: Color,
    selected: bool,
) {
    if color.a < 1.0 {
        draw_alpha_checkerboard(ctx, x, y, RECENT_SWATCH_SIZE, RECENT_SWATCH_SIZE);
    }
    ctx.set_source_rgba(color.r, color.g, color.b, color.a);
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

/// Draw the alpha bar for one colour: transparent on the left, opaque on the
/// right, over a checkerboard.
pub(super) fn draw_alpha_bar(ctx: &cairo::Context, x: f64, y: f64, w: f64, h: f64, color: Color) {
    draw_alpha_checkerboard(ctx, x, y, w, h);

    let ramp = cairo::LinearGradient::new(x, y, x + w, y);
    ramp.add_color_stop_rgba(0.0, color.r, color.g, color.b, 0.0);
    ramp.add_color_stop_rgba(1.0, color.r, color.g, color.b, 1.0);
    ctx.rectangle(x, y, w, h);
    let _ = ctx.set_source(&ramp);
    let _ = ctx.fill();

    constants::set_color(ctx, GRADIENT_BORDER);
    ctx.rectangle(x + 0.5, y + 0.5, w - 1.0, h - 1.0);
    ctx.set_line_width(1.0);
    let _ = ctx.stroke();
}

/// Draw the position marker for a horizontal bar.
pub(super) fn draw_bar_marker(ctx: &cairo::Context, x: f64, y: f64, h: f64) {
    constants::set_color(ctx, INDICATOR_OUTLINE);
    ctx.rectangle(x - 2.5, y - 1.5, 5.0, h + 3.0);
    ctx.set_line_width(3.0);
    let _ = ctx.stroke_preserve();
    constants::set_color(ctx, INDICATOR_RING);
    ctx.set_line_width(1.5);
    let _ = ctx.stroke();
}

/// Draw the color indicator dot on the gradient.
pub(super) fn draw_color_indicator(ctx: &cairo::Context, x: f64, y: f64, color: Color) {
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
pub(super) fn draw_preview_swatch(ctx: &cairo::Context, x: f64, y: f64, size: f64, color: Color) {
    // Checkered background for transparency preview, clipped to the rounded
    // swatch so tiles cannot spill past its corners.
    checkerboard_behind(ctx, color.a, |ctx| {
        draw_rounded_rect(ctx, x, y, size, size, RADIUS_SM);
    });

    // Draw color
    ctx.set_source_rgba(color.r, color.g, color.b, color.a);
    draw_rounded_rect(ctx, x, y, size, size, RADIUS_SM);
    let _ = ctx.fill();

    // Border
    let luminance = crate::draw::perceived_luminance(color.r, color.g, color.b);
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
pub(super) fn draw_hex_input(
    engine: &UiTextEngine,
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
    let extents = text_extents_for_with_engine(
        engine,
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
    engine.draw_baseline(ctx, value_style, value, text_x, text_y, None);

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
pub(super) fn draw_action_button(
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
    constants::set_color(ctx, crate::ui::theme::popup::border_modal());
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

pub(super) fn draw_action_tooltip(
    engine: &UiTextEngine,
    ctx: &cairo::Context,
    text: &str,
    anchor_x: f64,
    anchor_y: f64,
    screen_width: f64,
    screen_height: f64,
) {
    let Some((x, y, width, height)) = action_tooltip_geometry(
        engine,
        text,
        anchor_x,
        anchor_y,
        screen_width,
        screen_height,
    ) else {
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
    engine.draw_baseline(
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

pub(super) fn action_tooltip_geometry(
    engine: &UiTextEngine,
    text: &str,
    anchor_x: f64,
    anchor_y: f64,
    screen_width: f64,
    screen_height: f64,
) -> Option<(f64, f64, f64, f64)> {
    let style = action_tooltip_text_style();
    let extents = engine.measure(style, text, None)?;
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
pub(super) fn draw_button(
    engine: &UiTextEngine,
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

    let extents = text_extents_for_with_engine(
        engine,
        ctx,
        "Sans",
        cairo::FontSlant::Normal,
        cairo::FontWeight::Bold,
        13.0,
        label,
    );
    let text_x = x + (w - extents.width()) / 2.0;
    let text_y = y + h / 2.0 + extents.height() / 2.0;
    engine.draw_baseline(ctx, label_style, label, text_x, text_y, None);
}
