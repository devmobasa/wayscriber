use super::super::primitives::{draw_rounded_rect, text_extents_for};
use crate::ui_text::{UiTextStyle, draw_text_baseline, text_layout};

pub(crate) struct KeyComboStyle<'a> {
    pub(crate) font_family: &'a str,
    pub(crate) font_size: f64,
    pub(crate) text_color: [f64; 4],
    pub(crate) separator_color: [f64; 4],
}

/// Draw a keyboard key with keycap styling
fn draw_keycap(
    ctx: &cairo::Context,
    x: f64,
    y: f64,
    text: &str,
    font_family: &str,
    font_size: f64,
    text_color: [f64; 4],
) -> f64 {
    let padding_x = 8.0;
    let padding_y = 4.0;
    let radius = 5.0;
    let shadow_offset = 2.0;
    let key_style = UiTextStyle {
        family: font_family,
        slant: cairo::FontSlant::Normal,
        weight: cairo::FontWeight::Bold,
        size: font_size,
    };
    let layout = text_layout(ctx, key_style, text, None);
    let extents = layout.ink_extents();

    let cap_width = extents.width() + padding_x * 2.0;
    let cap_height = font_size + padding_y * 2.0;
    let cap_y = y - font_size - padding_y;

    // Drop shadow for 3D depth effect
    draw_rounded_rect(
        ctx,
        x + 1.0,
        cap_y + shadow_offset,
        cap_width,
        cap_height,
        radius,
    );
    ctx.set_source_rgba(0.0, 0.0, 0.0, 0.35);
    let _ = ctx.fill();

    // Keycap main background
    draw_rounded_rect(ctx, x, cap_y, cap_width, cap_height, radius);
    ctx.set_source_rgba(0.18, 0.22, 0.3, 1.0);
    let _ = ctx.fill();

    // Inner highlight for depth
    draw_rounded_rect(
        ctx,
        x + 1.0,
        cap_y + 1.0,
        cap_width - 2.0,
        cap_height - 2.0,
        radius - 1.0,
    );
    ctx.set_source_rgba(1.0, 1.0, 1.0, 0.12);
    ctx.set_line_width(1.0);
    let _ = ctx.stroke();

    // Outer border
    draw_rounded_rect(ctx, x, cap_y, cap_width, cap_height, radius);
    ctx.set_source_rgba(1.0, 1.0, 1.0, 0.2);
    ctx.set_line_width(1.0);
    let _ = ctx.stroke();

    // Text
    ctx.set_source_rgba(text_color[0], text_color[1], text_color[2], text_color[3]);
    layout.show_at_baseline(ctx, x + padding_x, y);

    cap_width
}

/// Measure the width of a key combination string with keycap styling
pub(crate) fn measure_key_combo(
    ctx: &cairo::Context,
    key_str: &str,
    font_family: &str,
    font_size: f64,
) -> f64 {
    let keycap_padding_x = 8.0;
    let key_gap = 5.0;
    let separator_gap = 6.0;

    let mut total_width = 0.0;

    // Split by " / " for alternate bindings
    let alternatives: Vec<&str> = key_str.split(" / ").collect();

    for (alt_idx, alt) in alternatives.iter().enumerate() {
        if alt_idx > 0 {
            // Add separator "/" width
            let slash_ext = text_extents_for(
                ctx,
                font_family,
                cairo::FontSlant::Normal,
                cairo::FontWeight::Normal,
                font_size,
                "/",
            );
            total_width += separator_gap * 2.0 + slash_ext.width();
        }

        // Split by "+" for key combinations
        let keys: Vec<&str> = alt.split('+').collect();
        for (key_idx, key) in keys.iter().enumerate() {
            if key_idx > 0 {
                // Add "+" separator width (matches draw_key_combo)
                let plus_ext = text_extents_for(
                    ctx,
                    font_family,
                    cairo::FontSlant::Normal,
                    cairo::FontWeight::Bold,
                    font_size * 0.9,
                    "+",
                );
                total_width += 6.0 + plus_ext.width();
            }

            let ext = text_extents_for(
                ctx,
                font_family,
                cairo::FontSlant::Normal,
                cairo::FontWeight::Bold,
                font_size,
                key.trim(),
            );
            total_width += ext.width() + keycap_padding_x * 2.0 + key_gap;
        }
    }

    total_width - key_gap // Remove trailing gap
}

/// Draw a key combination string with keycap styling, returns total width
pub(crate) fn draw_key_combo(
    ctx: &cairo::Context,
    x: f64,
    baseline: f64,
    key_str: &str,
    style: &KeyComboStyle<'_>,
) -> f64 {
    let key_gap = 5.0;
    let separator_gap = 6.0;
    let mut cursor_x = x;

    let alternatives: Vec<&str> = key_str.split(" / ").collect();

    for (alt_idx, alt) in alternatives.iter().enumerate() {
        if alt_idx > 0 {
            // Draw separator "/" between alternatives
            let slash_y = baseline;
            cursor_x += separator_gap;
            let slash_style = UiTextStyle {
                family: style.font_family,
                slant: cairo::FontSlant::Normal,
                weight: cairo::FontWeight::Normal,
                size: style.font_size,
            };
            ctx.set_source_rgba(
                style.separator_color[0],
                style.separator_color[1],
                style.separator_color[2],
                0.85,
            );
            let slash_ext = draw_text_baseline(ctx, slash_style, "/", cursor_x, slash_y, None);
            cursor_x += slash_ext.width() + separator_gap;
        }

        // Split by "+" for key combinations
        let keys: Vec<&str> = alt.split('+').collect();
        for (key_idx, key) in keys.iter().enumerate() {
            if key_idx > 0 {
                // Draw "+" separator between keys
                let plus_style = UiTextStyle {
                    family: style.font_family,
                    slant: cairo::FontSlant::Normal,
                    weight: cairo::FontWeight::Bold,
                    size: style.font_size * 0.9,
                };
                ctx.set_source_rgba(
                    style.separator_color[0],
                    style.separator_color[1],
                    style.separator_color[2],
                    0.85,
                );
                cursor_x += 3.0;
                let plus_ext = draw_text_baseline(ctx, plus_style, "+", cursor_x, baseline, None);
                cursor_x += plus_ext.width() + 3.0;
            }

            let cap_width = draw_keycap(
                ctx,
                cursor_x,
                baseline,
                key.trim(),
                style.font_family,
                style.font_size,
                style.text_color,
            );
            cursor_x += cap_width + key_gap;
        }
    }

    cursor_x - x - key_gap // Return total width minus trailing gap
}

pub(crate) fn draw_key_combo_highlight(
    ctx: &cairo::Context,
    x: f64,
    baseline: f64,
    font_size: f64,
    key_width: f64,
    color: [f64; 4],
) {
    if key_width <= 0.0 {
        return;
    }

    let padding_y = 4.0;
    let pad_x = 3.0;
    let pad_y = 3.0;
    let highlight_x = x - pad_x;
    let highlight_y = baseline - font_size - padding_y - pad_y;
    let highlight_width = key_width + pad_x * 2.0;
    let highlight_height = font_size + padding_y * 2.0 + pad_y * 2.0;

    ctx.set_source_rgba(color[0], color[1], color[2], color[3]);
    draw_rounded_rect(
        ctx,
        highlight_x,
        highlight_y,
        highlight_width,
        highlight_height,
        6.0,
    );
    let _ = ctx.fill();
}
