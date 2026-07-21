use crate::draw::{EraserReplayContext, render_eraser_stroke, render_shape};
use crate::input::BoardBackground;
use crate::input::state::{PAGE_NAME_HEIGHT, PAGE_NAME_PADDING};
use crate::ui::constants::{self, RADIUS_STD, TEXT_HINT, TEXT_TERTIARY};
use crate::ui::primitives::draw_rounded_rect;
use crate::ui::theme::Rgba;
use crate::ui_text::{UiTextStyle, draw_text_baseline};

use super::types::PageContentArgs;

/// Transparent-board thumbnail backdrop: faint white tint plus a cross-hatch
/// stroke (no matching theme token; kept from the pre-theme literals).
const TRANSPARENT_TINT: Rgba = (1.0, 1.0, 1.0, 0.06);
const TRANSPARENT_CROSS: Rgba = (1.0, 1.0, 1.0, 0.08);

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
    let radius = RADIUS_STD;
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
            constants::set_color(ctx, TRANSPARENT_TINT);
            ctx.rectangle(x, y, width, height);
            let _ = ctx.fill();
            constants::set_color(ctx, TRANSPARENT_CROSS);
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
        surface: None,
        backdrop_cache_key: None,
        bg_color: match background {
            BoardBackground::Solid(color) => Some(*color),
            BoardBackground::Transparent => None,
        },
        logical_to_image_scale_x: 1.0,
        logical_to_image_scale_y: 1.0,
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
    let label_style = UiTextStyle {
        family: "Sans",
        slant: cairo::FontSlant::Normal,
        weight: cairo::FontWeight::Normal,
        size: 10.5,
    };
    let label_x = x + 2.0;
    let label_y = y + height + PAGE_NAME_PADDING + PAGE_NAME_HEIGHT * 0.8;
    let color = if name.is_some() {
        TEXT_TERTIARY
    } else {
        TEXT_HINT
    };
    constants::set_color(ctx, constants::with_alpha(color, 0.85));
    let _ = ctx.save();
    ctx.rectangle(
        label_x,
        y + height + PAGE_NAME_PADDING,
        max_w,
        PAGE_NAME_HEIGHT,
    );
    ctx.clip();
    draw_text_baseline(ctx, label_style, label, label_x, label_y, None);
    let _ = ctx.restore();
}
