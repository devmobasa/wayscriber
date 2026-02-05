use std::f64::consts::PI;

use crate::draw::{EraserReplayContext, render_eraser_stroke, render_shape};
use crate::input::BoardBackground;
use crate::input::state::{
    PAGE_DELETE_ICON_MARGIN, PAGE_DELETE_ICON_SIZE, PAGE_NAME_HEIGHT, PAGE_NAME_PADDING,
};
use crate::ui::constants::{
    self, BG_SELECTION, BORDER_BOARD_PICKER, INDICATOR_ACTIVE_BOARD, PANEL_BG_BOARD_PICKER,
    TEXT_HINT, TEXT_TERTIARY,
};
use crate::ui::primitives::{draw_rounded_rect, text_extents_for};

use super::super::helpers::draw_drag_handle;

const PREVIEW_SCALE: f64 = 1.6;

pub(super) struct PageThumbnailArgs<'a> {
    pub(super) ctx: &'a cairo::Context,
    pub(super) frame: &'a crate::draw::Frame,
    pub(super) background: &'a BoardBackground,
    pub(super) x: f64,
    pub(super) y: f64,
    pub(super) width: f64,
    pub(super) height: f64,
    pub(super) screen_width: u32,
    pub(super) screen_height: u32,
    pub(super) page_number: usize,
    pub(super) page_name: Option<&'a str>,
    pub(super) is_active: bool,
    pub(super) is_drop_target: bool,
    pub(super) is_hovered: bool,
    pub(super) delete_hovered: bool,
    pub(super) duplicate_hovered: bool,
    pub(super) rename_hovered: bool,
}

pub(super) fn render_page_thumbnail(args: PageThumbnailArgs<'_>) {
    let PageThumbnailArgs {
        ctx,
        frame,
        background,
        x,
        y,
        width,
        height,
        screen_width,
        screen_height,
        page_number,
        page_name,
        is_active,
        is_drop_target,
        is_hovered,
        delete_hovered,
        duplicate_hovered,
        rename_hovered,
    } = args;
    let radius = 6.0;
    draw_rounded_rect(ctx, x, y, width, height, radius);
    if is_drop_target {
        constants::set_color(ctx, BG_SELECTION);
    } else {
        constants::set_color(ctx, PANEL_BG_BOARD_PICKER);
    }
    let _ = ctx.fill_preserve();
    constants::set_color(ctx, BORDER_BOARD_PICKER);
    ctx.set_line_width(1.0);
    let _ = ctx.stroke();

    render_page_content(
        ctx,
        frame,
        background,
        x,
        y,
        width,
        height,
        screen_width,
        screen_height,
    );

    if is_active {
        constants::set_color(ctx, INDICATOR_ACTIVE_BOARD);
        ctx.set_line_width(2.0);
        draw_rounded_rect(
            ctx,
            x - 1.0,
            y - 1.0,
            width + 2.0,
            height + 2.0,
            radius + 1.0,
        );
        let _ = ctx.stroke();
    }

    let handle_size = (height * 0.22).clamp(8.0, 12.0);
    let handle_x = x + width - handle_size - 4.0;
    let handle_y = y + 4.0 + handle_size * 0.5;
    draw_drag_handle(ctx, handle_x, handle_y, handle_size);

    let icon_size = PAGE_DELETE_ICON_SIZE;
    let margin = PAGE_DELETE_ICON_MARGIN;
    let icon_y = y + height - icon_size * 0.5 - margin;

    // Position icons: rename (left), duplicate (center), delete (right)
    let rename_x = x + icon_size * 0.5 + margin;
    let delete_x = x + width - icon_size * 0.5 - margin;
    let duplicate_x = x + width * 0.5;

    let rename_alpha = if rename_hovered {
        1.0
    } else if is_hovered {
        0.85
    } else {
        0.6
    };
    draw_rename_icon(ctx, rename_x, icon_y, icon_size, rename_alpha);

    let dup_alpha = if duplicate_hovered {
        1.0
    } else if is_hovered {
        0.85
    } else {
        0.6
    };
    draw_duplicate_icon(ctx, duplicate_x, icon_y, icon_size, dup_alpha);

    let del_alpha = if delete_hovered {
        1.0
    } else if is_hovered {
        0.85
    } else {
        0.6
    };
    draw_delete_icon(ctx, delete_x, icon_y, icon_size, del_alpha);

    let badge = page_number.to_string();
    ctx.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Bold);
    ctx.set_font_size(10.0);
    let extents = text_extents_for(
        ctx,
        "Sans",
        cairo::FontSlant::Normal,
        cairo::FontWeight::Bold,
        10.0,
        &badge,
    );
    let badge_w = extents.width() + 8.0;
    let badge_h = 14.0;
    let badge_x = x + 6.0;
    let badge_y = y + 6.0;
    ctx.set_source_rgba(0.0, 0.0, 0.0, 0.6);
    draw_rounded_rect(ctx, badge_x, badge_y, badge_w, badge_h, 4.0);
    let _ = ctx.fill();
    ctx.set_source_rgba(1.0, 1.0, 1.0, 0.9);
    ctx.move_to(badge_x + 4.0, badge_y + badge_h - 4.0);
    let _ = ctx.show_text(&badge);

    render_page_name_label(ctx, x, y, width, height, page_name, is_hovered);
}

pub(super) fn render_add_page_card(
    ctx: &cairo::Context,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    is_hovered: bool,
    is_empty_state: bool,
) {
    let radius = 6.0;

    draw_rounded_rect(ctx, x, y, width, height, radius);
    if is_hovered {
        ctx.set_source_rgba(1.0, 1.0, 1.0, 0.08);
    } else {
        ctx.set_source_rgba(1.0, 1.0, 1.0, 0.03);
    }
    let _ = ctx.fill_preserve();

    constants::set_color(ctx, BORDER_BOARD_PICKER);
    ctx.set_line_width(1.0);
    ctx.set_dash(&[4.0, 3.0], 0.0);
    let _ = ctx.stroke();
    ctx.set_dash(&[], 0.0);

    let icon_size = 16.0;
    let icon_alpha = if is_hovered { 0.8 } else { 0.45 };
    draw_plus_icon(
        ctx,
        x + width * 0.5,
        y + height * 0.4,
        icon_size,
        icon_alpha,
    );

    let label = if is_empty_state {
        "Add first page"
    } else {
        "Add page"
    };
    ctx.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Normal);
    ctx.set_font_size(10.0);
    let text_alpha = if is_hovered { 0.7 } else { 0.4 };
    ctx.set_source_rgba(1.0, 1.0, 1.0, text_alpha);
    if let Ok(extents) = ctx.text_extents(label) {
        ctx.move_to(
            x + (width - extents.width()) * 0.5,
            y + height * 0.65 + extents.height() * 0.5,
        );
        let _ = ctx.show_text(label);
    }
}

pub(super) fn render_page_preview(
    ctx: &cairo::Context,
    frame: &crate::draw::Frame,
    background: &BoardBackground,
    thumb_x: f64,
    thumb_y: f64,
    thumb_w: f64,
    thumb_h: f64,
    screen_width: u32,
    screen_height: u32,
    page_number: usize,
) {
    let base_w = thumb_w * PREVIEW_SCALE;
    let base_h = thumb_h * PREVIEW_SCALE;
    let margin = 8.0;
    let max_w = (screen_width as f64 - margin * 2.0).max(1.0);
    let max_h = (screen_height as f64 - margin * 2.0).max(1.0);
    let scale = (max_w / base_w).min(max_h / base_h).min(1.0);
    let preview_w = base_w * scale;
    let preview_h = base_h * scale;
    let mut preview_x = thumb_x + thumb_w + 12.0;
    let mut preview_y = thumb_y;
    let max_x = screen_width as f64 - margin - preview_w;
    let max_y = screen_height as f64 - margin - preview_h;
    preview_x = preview_x.clamp(margin, max_x.max(margin));
    preview_y = preview_y.clamp(margin, max_y.max(margin));

    ctx.set_source_rgba(0.0, 0.0, 0.0, 0.35);
    draw_rounded_rect(
        ctx,
        preview_x + 4.0,
        preview_y + 6.0,
        preview_w,
        preview_h,
        8.0,
    );
    let _ = ctx.fill();

    draw_rounded_rect(ctx, preview_x, preview_y, preview_w, preview_h, 8.0);
    constants::set_color(ctx, PANEL_BG_BOARD_PICKER);
    let _ = ctx.fill_preserve();
    constants::set_color(ctx, BORDER_BOARD_PICKER);
    ctx.set_line_width(1.2);
    let _ = ctx.stroke();

    render_page_content(
        ctx,
        frame,
        background,
        preview_x,
        preview_y,
        preview_w,
        preview_h,
        screen_width,
        screen_height,
    );

    let label = frame
        .page_name()
        .map(|name| format!("Page {} — {}", page_number, name))
        .unwrap_or_else(|| format!("Page {}", page_number));
    ctx.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Normal);
    ctx.set_font_size(11.0);
    if let Ok(extents) = ctx.text_extents(&label) {
        let label_w = (extents.width() + 10.0).min(preview_w - 8.0);
        let label_x = preview_x + 6.0;
        let label_y = preview_y + 6.0;
        ctx.set_source_rgba(0.0, 0.0, 0.0, 0.55);
        draw_rounded_rect(ctx, label_x, label_y, label_w, 16.0, 4.0);
        let _ = ctx.fill();
        ctx.set_source_rgba(1.0, 1.0, 1.0, 0.9);
        let _ = ctx.save();
        ctx.rectangle(label_x + 4.0, label_y, label_w - 8.0, 16.0);
        ctx.clip();
        ctx.move_to(label_x + 4.0, label_y + 12.0);
        let _ = ctx.show_text(&label);
        let _ = ctx.restore();
    }
}

fn draw_plus_icon(ctx: &cairo::Context, x: f64, y: f64, size: f64, alpha: f64) {
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

fn draw_delete_icon(ctx: &cairo::Context, x: f64, y: f64, size: f64, alpha: f64) {
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

fn draw_duplicate_icon(ctx: &cairo::Context, x: f64, y: f64, size: f64, alpha: f64) {
    let radius = size * 0.5;

    // Background circle
    ctx.arc(x, y, radius, 0.0, PI * 2.0);
    ctx.set_source_rgba(0.2, 0.6, 1.0, alpha);
    let _ = ctx.fill();
    ctx.set_source_rgba(1.0, 1.0, 1.0, alpha * 0.6);
    ctx.set_line_width(1.0);
    let _ = ctx.stroke();

    // Two stacked pages
    let page_w = size * 0.42;
    let page_h = size * 0.54;
    let offset = size * 0.12;
    ctx.set_source_rgba(1.0, 1.0, 1.0, alpha.min(0.95));
    ctx.set_line_width(1.4);

    // Back page (offset up-right)
    ctx.rectangle(
        x - page_w * 0.5 + offset,
        y - page_h * 0.5 - offset,
        page_w,
        page_h,
    );
    let _ = ctx.stroke();

    // Front page
    ctx.rectangle(x - page_w * 0.5, y - page_h * 0.5, page_w, page_h);
    let _ = ctx.stroke();
}

fn draw_rename_icon(ctx: &cairo::Context, x: f64, y: f64, size: f64, alpha: f64) {
    let radius = size * 0.5;

    // Background circle
    ctx.arc(x, y, radius, 0.0, PI * 2.0);
    ctx.set_source_rgba(0.45, 0.45, 0.5, alpha);
    let _ = ctx.fill();

    // Pencil body (rotated 45 degrees)
    let pencil_len = size * 0.55;
    let pencil_w = size * 0.18;
    let angle = PI / 4.0;
    let cos_a = angle.cos();
    let sin_a = angle.sin();

    ctx.set_source_rgba(1.0, 1.0, 1.0, alpha.min(0.95));
    ctx.set_line_width(pencil_w);
    ctx.set_line_cap(cairo::LineCap::Round);

    // Main shaft
    let start_x = x - cos_a * pencil_len * 0.35;
    let start_y = y - sin_a * pencil_len * 0.35;
    let end_x = x + cos_a * pencil_len * 0.45;
    let end_y = y + sin_a * pencil_len * 0.45;
    ctx.move_to(start_x, start_y);
    ctx.line_to(end_x, end_y);
    let _ = ctx.stroke();

    // Pencil tip (triangle)
    ctx.set_line_width(1.0);
    let tip_x = end_x + cos_a * size * 0.12;
    let tip_y = end_y + sin_a * size * 0.12;
    ctx.move_to(end_x, end_y);
    ctx.line_to(tip_x, tip_y);
    let _ = ctx.stroke();
}

fn render_page_content(
    ctx: &cairo::Context,
    frame: &crate::draw::Frame,
    background: &BoardBackground,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    screen_width: u32,
    screen_height: u32,
) {
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
    let _ = ctx.restore();
    let _ = ctx.restore();
}

fn render_page_name_label(
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
    ctx.set_font_size(9.5);
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
