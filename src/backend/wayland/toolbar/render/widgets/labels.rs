use super::constants::{COLOR_LABEL_SECTION, COLOR_TEXT_PRIMARY, set_color};
use crate::ui_text::{UiTextStyle, draw_text_baseline, text_layout};

pub(in crate::backend::wayland::toolbar::render) fn draw_label_center(
    ctx: &cairo::Context,
    style: UiTextStyle<'_>,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    text: &str,
) {
    let layout = text_layout(ctx, style, text, None);
    let ext = layout.ink_extents();
    let tx = x + (w - ext.width()) / 2.0 - ext.x_bearing();
    let ty = y + (h - ext.height()) / 2.0 - ext.y_bearing();
    set_color(ctx, COLOR_TEXT_PRIMARY);
    layout.show_at_baseline(ctx, tx, ty);
}

#[allow(clippy::too_many_arguments)]
pub(in crate::backend::wayland::toolbar::render) fn draw_label_center_color(
    ctx: &cairo::Context,
    style: UiTextStyle<'_>,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    text: &str,
    color: (f64, f64, f64, f64),
) {
    let layout = text_layout(ctx, style, text, None);
    let ext = layout.ink_extents();
    let tx = x + (w - ext.width()) / 2.0 - ext.x_bearing();
    let ty = y + (h - ext.height()) / 2.0 - ext.y_bearing();
    ctx.set_source_rgba(color.0, color.1, color.2, color.3);
    layout.show_at_baseline(ctx, tx, ty);
}

pub(in crate::backend::wayland::toolbar::render) fn draw_label_left(
    ctx: &cairo::Context,
    style: UiTextStyle<'_>,
    x: f64,
    y: f64,
    _w: f64,
    h: f64,
    text: &str,
) {
    let layout = text_layout(ctx, style, text, None);
    let ext = layout.ink_extents();
    let ty = y + (h - ext.height()) / 2.0 - ext.y_bearing();
    set_color(ctx, COLOR_TEXT_PRIMARY);
    layout.show_at_baseline(ctx, x, ty);
}

pub(in crate::backend::wayland::toolbar::render) fn draw_section_label(
    ctx: &cairo::Context,
    style: UiTextStyle<'_>,
    x: f64,
    y: f64,
    text: &str,
) {
    set_color(ctx, COLOR_LABEL_SECTION);
    draw_text_baseline(ctx, style, text, x, y, None);
}
