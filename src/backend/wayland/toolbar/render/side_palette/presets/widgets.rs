use super::super::super::widgets::constants::{
    COLOR_CARD_BACKGROUND, COLOR_TEXT_PRIMARY, FONT_FAMILY_DEFAULT, FONT_SIZE_SECONDARY,
    FONT_SIZE_SMALL, set_color,
};
use super::super::super::widgets::{draw_label_center_color, draw_round_rect};
use crate::ui::theme::{Rgb, rgba, set_color_alpha, with_alpha};
use crate::ui_text::{UiTextStyle, text_layout};

/// White root for the keycap's border/text alpha ladder.
const WHITE_RGB: Rgb = (1.0, 1.0, 1.0);

pub(super) fn draw_keycap(
    ctx: &cairo::Context,
    key_x: f64,
    key_y: f64,
    size: f64,
    radius: f64,
    label: &str,
    active: bool,
) {
    // Consistent alpha progressions: active is +0.35 from inactive
    let (bg_alpha, border_alpha, text_alpha) = if active {
        (0.65, 0.50, 0.95)
    } else {
        (0.30, 0.25, 0.55)
    };
    set_color(ctx, with_alpha(COLOR_CARD_BACKGROUND, bg_alpha));
    draw_round_rect(ctx, key_x, key_y, size, size, radius);
    let _ = ctx.fill();
    set_color_alpha(ctx, WHITE_RGB, border_alpha);
    ctx.set_line_width(1.0);
    draw_round_rect(ctx, key_x, key_y, size, size, radius);
    let _ = ctx.stroke();
    let keycap_style = UiTextStyle {
        family: FONT_FAMILY_DEFAULT,
        slant: cairo::FontSlant::Normal,
        weight: cairo::FontWeight::Bold,
        size: FONT_SIZE_SECONDARY,
    };
    draw_label_center_color(
        ctx,
        keycap_style,
        key_x,
        key_y,
        size,
        size,
        label,
        rgba(WHITE_RGB, text_alpha),
    );
}

#[allow(clippy::too_many_arguments)]
pub(super) fn draw_preset_name_tag(
    ctx: &cairo::Context,
    name: &str,
    slot_x: f64,
    slot_row_y: f64,
    slot_size: f64,
    card_x: f64,
    card_w: f64,
    section_y: f64,
) {
    let tag_style = UiTextStyle {
        family: FONT_FAMILY_DEFAULT,
        slant: cairo::FontSlant::Normal,
        weight: cairo::FontWeight::Normal,
        size: FONT_SIZE_SMALL,
    };
    let layout = text_layout(ctx, tag_style, name, None);
    let extents = layout.ink_extents();
    let pad_x = 5.0;
    let pad_y = 2.0;
    let label_w = extents.width() + pad_x * 2.0;
    let label_h = extents.height() + pad_y * 2.0;
    let mut label_x = slot_x + (slot_size - label_w) / 2.0;
    let label_y = (slot_row_y - label_h - 2.0).max(section_y + 2.0);
    let min_x = card_x + 2.0;
    let max_x = card_x + card_w - label_w - 2.0;
    if label_x < min_x {
        label_x = min_x;
    }
    if label_x > max_x {
        label_x = max_x;
    }
    // Near-opaque card tint so the tag reads over any slot content.
    set_color(ctx, with_alpha(COLOR_CARD_BACKGROUND, 0.92));
    draw_round_rect(ctx, label_x, label_y, label_w, label_h, 4.0);
    let _ = ctx.fill();
    set_color(ctx, COLOR_TEXT_PRIMARY);
    layout.show_at_baseline(
        ctx,
        label_x + pad_x - extents.x_bearing(),
        label_y + pad_y - extents.y_bearing(),
    );
}
