use std::f64::consts::PI;

use crate::ui::constants::{self, TEXT_TERTIARY};

pub(super) fn icon_alpha(is_hovered: bool, icon_hovered: bool) -> f64 {
    if icon_hovered {
        1.0
    } else if is_hovered {
        0.7
    } else {
        0.2
    }
}

pub(super) fn draw_plus_icon(ctx: &cairo::Context, x: f64, y: f64, size: f64, alpha: f64) {
    let half = size * 0.5;
    constants::set_color(ctx, constants::with_alpha(TEXT_TERTIARY, alpha));
    ctx.set_line_width(1.6);
    ctx.move_to(x - half, y);
    ctx.line_to(x + half, y);
    let _ = ctx.stroke();
    ctx.move_to(x, y - half);
    ctx.line_to(x, y + half);
    let _ = ctx.stroke();
}

pub(super) fn draw_delete_icon(ctx: &cairo::Context, x: f64, y: f64, size: f64, alpha: f64) {
    let radius = size * 0.5;

    ctx.arc(x, y, radius, 0.0, PI * 2.0);
    ctx.set_source_rgba(0.85, 0.2, 0.2, alpha);
    let _ = ctx.fill();

    let x_size = size * 0.28;
    ctx.set_source_rgba(1.0, 1.0, 1.0, alpha.min(0.95));
    ctx.set_line_width(1.8);
    ctx.set_line_cap(cairo::LineCap::Round);
    ctx.move_to(x - x_size, y - x_size);
    ctx.line_to(x + x_size, y + x_size);
    let _ = ctx.stroke();
    ctx.move_to(x + x_size, y - x_size);
    ctx.line_to(x - x_size, y + x_size);
    let _ = ctx.stroke();
}

pub(super) fn draw_duplicate_icon(ctx: &cairo::Context, x: f64, y: f64, size: f64, alpha: f64) {
    let radius = size * 0.5;

    ctx.arc(x, y, radius, 0.0, PI * 2.0);
    ctx.set_source_rgba(0.2, 0.6, 1.0, alpha);
    let _ = ctx.fill();
    ctx.set_source_rgba(1.0, 1.0, 1.0, alpha * 0.6);
    ctx.set_line_width(1.0);
    let _ = ctx.stroke();

    let page_w = size * 0.42;
    let page_h = size * 0.54;
    let offset = size * 0.12;
    ctx.set_source_rgba(1.0, 1.0, 1.0, alpha.min(0.95));
    ctx.set_line_width(1.4);

    ctx.rectangle(
        x - page_w * 0.5 + offset,
        y - page_h * 0.5 - offset,
        page_w,
        page_h,
    );
    let _ = ctx.stroke();

    ctx.rectangle(x - page_w * 0.5, y - page_h * 0.5, page_w, page_h);
    let _ = ctx.stroke();
}

pub(super) fn draw_rename_icon(ctx: &cairo::Context, x: f64, y: f64, size: f64, alpha: f64) {
    let radius = size * 0.5;

    ctx.arc(x, y, radius, 0.0, PI * 2.0);
    ctx.set_source_rgba(0.45, 0.45, 0.5, alpha);
    let _ = ctx.fill();

    let pencil_len = size * 0.55;
    let pencil_w = size * 0.18;
    let angle = PI / 4.0;
    let cos_a = angle.cos();
    let sin_a = angle.sin();

    ctx.set_source_rgba(1.0, 1.0, 1.0, alpha.min(0.95));
    ctx.set_line_width(pencil_w);
    ctx.set_line_cap(cairo::LineCap::Round);

    let start_x = x - cos_a * pencil_len * 0.35;
    let start_y = y - sin_a * pencil_len * 0.35;
    let end_x = x + cos_a * pencil_len * 0.45;
    let end_y = y + sin_a * pencil_len * 0.45;
    ctx.move_to(start_x, start_y);
    ctx.line_to(end_x, end_y);
    let _ = ctx.stroke();

    ctx.set_line_width(1.0);
    let tip_x = end_x + cos_a * size * 0.12;
    let tip_y = end_y + sin_a * size * 0.12;
    ctx.move_to(end_x, end_y);
    ctx.line_to(tip_x, tip_y);
    let _ = ctx.stroke();
}
