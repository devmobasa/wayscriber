use super::constants::{
    COLOR_CHECKBOX_CHECKED, COLOR_CHECKBOX_DEFAULT, COLOR_CHECKBOX_HOVER,
    COLOR_MINI_CHECKBOX_DEFAULT, COLOR_MINI_CHECKBOX_HOVER, COLOR_TEXT_SECONDARY,
    COLOR_TEXT_TERTIARY, FONT_FAMILY_DEFAULT, FONT_SIZE_SMALL, LINE_WIDTH_STD, LINE_WIDTH_THIN,
    RADIUS_SM, RADIUS_STD, SPACING_LG, SPACING_SM, SPACING_XS, set_color,
};
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
    set_color(
        ctx,
        if hover {
            COLOR_CHECKBOX_HOVER
        } else {
            COLOR_CHECKBOX_DEFAULT
        },
    );
    draw_round_rect(ctx, x, y, w, h, RADIUS_STD);
    let _ = ctx.fill();

    let box_size = h * 0.55;
    let box_x = x + SPACING_LG;
    let box_y = y + (h - box_size) / 2.0;
    set_color(ctx, COLOR_TEXT_SECONDARY);
    ctx.rectangle(box_x, box_y, box_size, box_size);
    ctx.set_line_width(LINE_WIDTH_STD);
    let _ = ctx.stroke();
    if checked {
        ctx.move_to(box_x + SPACING_SM, box_y + box_size / 2.0);
        ctx.line_to(box_x + box_size / 2.0, box_y + box_size - SPACING_SM);
        ctx.line_to(box_x + box_size - SPACING_SM, box_y + SPACING_SM);
        let _ = ctx.stroke();
    }

    let label_x = box_x + box_size + SPACING_LG;
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
    let color = if checked {
        COLOR_CHECKBOX_CHECKED
    } else if hover {
        COLOR_MINI_CHECKBOX_HOVER
    } else {
        COLOR_MINI_CHECKBOX_DEFAULT
    };
    set_color(ctx, color);
    draw_round_rect(ctx, x, y, w, h, RADIUS_SM);
    let _ = ctx.fill();

    let box_size = h * 0.6;
    let box_x = x + SPACING_SM + 1.0;
    let box_y = y + (h - box_size) / 2.0;
    set_color(ctx, COLOR_TEXT_TERTIARY);
    ctx.rectangle(box_x, box_y, box_size, box_size);
    ctx.set_line_width(LINE_WIDTH_THIN);
    let _ = ctx.stroke();

    if checked {
        ctx.move_to(box_x + SPACING_XS, box_y + box_size / 2.0);
        ctx.line_to(box_x + box_size / 2.0, box_y + box_size - SPACING_XS);
        ctx.line_to(box_x + box_size - SPACING_XS, box_y + SPACING_XS);
        let _ = ctx.stroke();
    }

    ctx.select_font_face(
        FONT_FAMILY_DEFAULT,
        cairo::FontSlant::Normal,
        cairo::FontWeight::Normal,
    );
    ctx.set_font_size(FONT_SIZE_SMALL);
    if let Ok(ext) = ctx.text_extents(label) {
        let label_x = x + box_size + SPACING_LG + (w - box_size - 12.0 - ext.width()) / 2.0;
        let label_y = y + (h + ext.height()) / 2.0;
        set_color(ctx, COLOR_TEXT_SECONDARY);
        ctx.move_to(label_x, label_y);
        let _ = ctx.show_text(label);
    }
}
