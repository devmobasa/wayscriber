use super::{draw_label_left, draw_round_rect};

#[allow(clippy::too_many_arguments)]
pub(in crate::backend::wayland::toolbar::render) fn draw_checkbox(
    ctx: &cairo::Context,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    checked: bool,
    hover: bool,
    label: &str,
) {
    let (r, g, b, a) = if hover {
        (0.32, 0.34, 0.4, 0.9)
    } else {
        (0.22, 0.24, 0.28, 0.75)
    };
    ctx.set_source_rgba(r, g, b, a);
    draw_round_rect(ctx, x, y, w, h, 4.0);
    let _ = ctx.fill();

    let box_size = h * 0.55;
    let box_x = x + 8.0;
    let box_y = y + (h - box_size) / 2.0;
    ctx.set_source_rgba(1.0, 1.0, 1.0, 0.9);
    ctx.rectangle(box_x, box_y, box_size, box_size);
    ctx.set_line_width(1.5);
    let _ = ctx.stroke();
    if checked {
        ctx.move_to(box_x + 3.0, box_y + box_size / 2.0);
        ctx.line_to(box_x + box_size / 2.0, box_y + box_size - 3.0);
        ctx.line_to(box_x + box_size - 3.0, box_y + 3.0);
        let _ = ctx.stroke();
    }

    let label_x = box_x + box_size + 8.0;
    draw_label_left(ctx, label_x, y, w - (label_x - x), h, label);
}

#[allow(clippy::too_many_arguments)]
pub(in crate::backend::wayland::toolbar::render) fn draw_mini_checkbox(
    ctx: &cairo::Context,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    checked: bool,
    hover: bool,
    label: &str,
) {
    let (r, g, b, a) = if checked {
        (0.25, 0.5, 0.35, 0.9)
    } else if hover {
        (0.32, 0.34, 0.4, 0.85)
    } else {
        (0.2, 0.22, 0.26, 0.7)
    };
    ctx.set_source_rgba(r, g, b, a);
    draw_round_rect(ctx, x, y, w, h, 3.0);
    let _ = ctx.fill();

    let box_size = h * 0.6;
    let box_x = x + 4.0;
    let box_y = y + (h - box_size) / 2.0;
    ctx.set_source_rgba(1.0, 1.0, 1.0, 0.85);
    ctx.rectangle(box_x, box_y, box_size, box_size);
    ctx.set_line_width(1.0);
    let _ = ctx.stroke();

    if checked {
        ctx.move_to(box_x + 2.0, box_y + box_size / 2.0);
        ctx.line_to(box_x + box_size / 2.0, box_y + box_size - 2.0);
        ctx.line_to(box_x + box_size - 2.0, box_y + 2.0);
        let _ = ctx.stroke();
    }

    ctx.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Normal);
    ctx.set_font_size(10.0);
    if let Ok(ext) = ctx.text_extents(label) {
        let label_x = x + box_size + 8.0 + (w - box_size - 12.0 - ext.width()) / 2.0;
        let label_y = y + (h + ext.height()) / 2.0;
        ctx.set_source_rgba(1.0, 1.0, 1.0, 0.9);
        ctx.move_to(label_x, label_y);
        let _ = ctx.show_text(label);
    }
}
