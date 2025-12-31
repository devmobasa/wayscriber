use super::super::super::widgets::{draw_label_center_color, draw_round_rect};

pub(super) fn draw_keycap(
    ctx: &cairo::Context,
    key_x: f64,
    key_y: f64,
    size: f64,
    radius: f64,
    label: &str,
    active: bool,
) {
    let (bg_alpha, border_alpha, text_alpha) = if active {
        (0.75, 0.55, 0.95)
    } else {
        (0.4, 0.35, 0.6)
    };
    ctx.set_source_rgba(0.12, 0.12, 0.18, bg_alpha);
    draw_round_rect(ctx, key_x, key_y, size, size, radius);
    let _ = ctx.fill();
    ctx.set_source_rgba(1.0, 1.0, 1.0, border_alpha);
    ctx.set_line_width(1.0);
    draw_round_rect(ctx, key_x, key_y, size, size, radius);
    let _ = ctx.stroke();
    ctx.set_font_size(11.0);
    draw_label_center_color(
        ctx,
        key_x,
        key_y,
        size,
        size,
        label,
        (1.0, 1.0, 1.0, text_alpha),
    );
    ctx.set_font_size(13.0);
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
    ctx.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Normal);
    ctx.set_font_size(10.0);
    if let Ok(extents) = ctx.text_extents(name) {
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
        ctx.set_source_rgba(0.12, 0.12, 0.18, 0.92);
        draw_round_rect(ctx, label_x, label_y, label_w, label_h, 4.0);
        let _ = ctx.fill();
        ctx.set_source_rgba(1.0, 1.0, 1.0, 0.95);
        ctx.move_to(
            label_x + pad_x - extents.x_bearing(),
            label_y + pad_y - extents.y_bearing(),
        );
        let _ = ctx.show_text(name);
    }
}
