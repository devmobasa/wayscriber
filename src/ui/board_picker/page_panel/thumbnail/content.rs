use crate::draw::{EraserReplayContext, render_eraser_stroke, render_shape};
use crate::input::BoardBackground;
use crate::input::state::{PAGE_NAME_HEIGHT, PAGE_NAME_PADDING};
use crate::ui::constants::{TEXT_HINT, TEXT_TERTIARY};
use crate::ui::primitives::draw_rounded_rect;

use super::types::PageContentArgs;

pub(super) fn render_page_content(args: PageContentArgs<'_>) {
    let PageContentArgs {
        ctx,
        frame,
        background,
        x,
        y,
        width,
        height,
        screen_width,
        screen_height,
    } = args;
    let radius = 6.0;
    let _ = ctx.save();
    draw_rounded_rect(ctx, x, y, width, height, radius);
    ctx.clip();

    match background {
        BoardBackground::Solid(color) => {
            ctx.set_source_rgba(color.r, color.g, color.b, 1.0);
            ctx.rectangle(x, y, width, height);
            let _ = ctx.fill();
        }
        BoardBackground::Transparent => {
            ctx.set_source_rgba(1.0, 1.0, 1.0, 0.06);
            ctx.rectangle(x, y, width, height);
            let _ = ctx.fill();
            ctx.set_source_rgba(1.0, 1.0, 1.0, 0.08);
            ctx.set_line_width(1.0);
            ctx.move_to(x, y);
            ctx.line_to(x + width, y + height);
            ctx.move_to(x + width, y);
            ctx.line_to(x, y + height);
            let _ = ctx.stroke();
        }
    }

    let inset = 2.0;
    let content_w = (width - inset * 2.0).max(1.0);
    let content_h = (height - inset * 2.0).max(1.0);
    let scale = (content_w / screen_width as f64).min(content_h / screen_height as f64);
    let offset_x = (content_w - screen_width as f64 * scale) * 0.5;
    let offset_y = (content_h - screen_height as f64 * scale) * 0.5;

    let _ = ctx.save();
    ctx.translate(x + inset + offset_x, y + inset + offset_y);
    ctx.scale(scale, scale);
    render_frame_shapes(ctx, frame, background);
    let _ = ctx.restore();
    let _ = ctx.restore();
}

fn render_frame_shapes(
    ctx: &cairo::Context,
    frame: &crate::draw::Frame,
    background: &BoardBackground,
) {
    let eraser_ctx = EraserReplayContext {
        pattern: None,
        bg_color: match background {
            BoardBackground::Solid(color) => Some(*color),
            BoardBackground::Transparent => None,
        },
    };

    for drawn in &frame.shapes {
        match &drawn.shape {
            crate::draw::Shape::EraserStroke { points, brush } => {
                render_eraser_stroke(ctx, points, brush, &eraser_ctx);
            }
            _ => {
                render_shape(ctx, &drawn.shape);
            }
        }
    }
}

pub(super) fn render_page_name_label(
    ctx: &cairo::Context,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    name: Option<&str>,
    is_hovered: bool,
) {
    let label = match name {
        Some(value) => value,
        None if is_hovered => "Add name",
        None => return,
    };
    let max_w = width - 4.0;
    ctx.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Normal);
    ctx.set_font_size(10.5);
    let label_x = x + 2.0;
    let label_y = y + height + PAGE_NAME_PADDING + PAGE_NAME_HEIGHT * 0.8;
    let color = if name.is_some() {
        TEXT_TERTIARY
    } else {
        TEXT_HINT
    };
    ctx.set_source_rgba(color.0, color.1, color.2, 0.85);
    let _ = ctx.save();
    ctx.rectangle(
        label_x,
        y + height + PAGE_NAME_PADDING,
        max_w,
        PAGE_NAME_HEIGHT,
    );
    ctx.clip();
    ctx.move_to(label_x, label_y);
    let _ = ctx.show_text(label);
    let _ = ctx.restore();
}
