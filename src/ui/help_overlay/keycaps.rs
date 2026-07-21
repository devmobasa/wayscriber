use super::super::primitives::{draw_rounded_rect, keycap_size, text_extents_for};
use crate::ui::primitives::draw_keycap;
use crate::ui::theme::toolbar;
use crate::ui_text::{UiTextStyle, draw_text_baseline};

pub(crate) struct KeyComboStyle<'a> {
    pub(crate) font_family: &'a str,
    pub(crate) font_size: f64,
    pub(crate) text_color: [f64; 4],
    pub(crate) separator_color: [f64; 4],
}

fn for_each_key_token(combo: &str, mut emit: impl FnMut(&str)) {
    if combo.trim().is_empty() {
        return;
    }

    let mut last_was_plus = false;
    for part in combo.split('+') {
        let trimmed = part.trim();
        if trimmed.is_empty() {
            if last_was_plus {
                continue;
            }
            last_was_plus = true;
            emit("+");
        } else {
            last_was_plus = false;
            emit(trimmed);
        }
    }
}

/// Draw a single keycap in the shared keycap language ([`crate::ui::primitives::draw_keycap`]),
/// anchored on a text `baseline` so it lines up with the row's description
/// text. Returns the drawn cap width.
///
/// The cap is centred vertically on the same point the previous bespoke 3D cap
/// used (`baseline - font_size / 2`), so replacing the cap did not shift the
/// rows or the highlight geometry in [`draw_key_combo_highlight`].
fn draw_single_keycap(
    ctx: &cairo::Context,
    x: f64,
    baseline: f64,
    text: &str,
    font_size: f64,
    text_color: [f64; 4],
) -> f64 {
    let (_, cap_height) = keycap_size(ctx, text, font_size);
    let top_y = baseline - font_size / 2.0 - cap_height / 2.0;
    let (cap_width, _) = draw_keycap(
        ctx,
        x,
        top_y,
        text,
        font_size,
        toolbar::COLOR_BADGE_BACKGROUND,
        (text_color[0], text_color[1], text_color[2], text_color[3]),
    );
    cap_width
}

/// Measure the width of a key combination string with keycap styling
pub(crate) fn measure_key_combo(
    ctx: &cairo::Context,
    key_str: &str,
    font_family: &str,
    font_size: f64,
) -> f64 {
    let key_gap = 5.0;
    let separator_gap = 6.0;

    let mut total_width = 0.0;
    let mut any_key = false;

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

        let mut key_idx = 0;
        for_each_key_token(alt, |key| {
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

            // Cap width comes from the shared keycap sizer so measuring and
            // drawing can never disagree about the chip footprint.
            let (cap_width, _) = keycap_size(ctx, key, font_size);
            total_width += cap_width + key_gap;
            key_idx += 1;
            any_key = true;
        });
    }

    if any_key {
        total_width - key_gap
    } else {
        total_width
    }
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
    let mut any_key = false;

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

        let mut key_idx = 0;
        for_each_key_token(alt, |key| {
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

            let cap_width = draw_single_keycap(
                ctx,
                cursor_x,
                baseline,
                key,
                style.font_size,
                style.text_color,
            );
            cursor_x += cap_width + key_gap;
            key_idx += 1;
            any_key = true;
        });
    }

    if any_key { cursor_x - x - key_gap } else { 0.0 }
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
