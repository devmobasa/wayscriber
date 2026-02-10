use crate::input::state::{PAGE_DELETE_ICON_MARGIN, PAGE_DELETE_ICON_SIZE};
use crate::ui::constants::{
    self, BG_SELECTION, BORDER_BOARD_PICKER, BORDER_FOCUS, INDICATOR_ACTIVE_BOARD,
    PANEL_BG_BOARD_PICKER,
};
use crate::ui::primitives::{draw_rounded_rect, text_extents_for};

use super::super::super::helpers::draw_drag_handle;
use super::content::{render_page_content, render_page_name_label};
use super::icons::{
    draw_delete_icon, draw_duplicate_icon, draw_plus_icon, draw_rename_icon, icon_alpha,
};
use super::types::{PREVIEW_SCALE, PageContentArgs, PagePreviewArgs, PageThumbnailArgs};

pub(in crate::ui::board_picker::page_panel) fn render_page_thumbnail(args: PageThumbnailArgs<'_>) {
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
        is_keyboard_focused,
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

    render_page_content(PageContentArgs {
        ctx,
        frame,
        background,
        x,
        y,
        width,
        height,
        screen_width,
        screen_height,
    });

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

    if is_keyboard_focused {
        constants::set_color(ctx, BORDER_FOCUS);
        ctx.set_line_width(1.5);
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

    draw_thumbnail_actions(
        ctx,
        x,
        y,
        width,
        height,
        is_hovered,
        rename_hovered,
        duplicate_hovered,
        delete_hovered,
    );
    draw_page_badge(ctx, x, y, page_number, is_hovered);
    render_page_name_label(ctx, x, y, width, height, page_name, is_hovered);
}

#[allow(clippy::too_many_arguments)]
fn draw_thumbnail_actions(
    ctx: &cairo::Context,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    is_hovered: bool,
    rename_hovered: bool,
    duplicate_hovered: bool,
    delete_hovered: bool,
) {
    let handle_size = (height * 0.22).clamp(8.0, 12.0);
    let handle_x = x + width - handle_size - 4.0;
    let handle_y = y + 4.0 + handle_size * 0.5;
    draw_drag_handle(ctx, handle_x, handle_y, handle_size);

    let icon_size = PAGE_DELETE_ICON_SIZE;
    let margin = PAGE_DELETE_ICON_MARGIN;
    let icon_y = y + height - icon_size * 0.5 - margin;

    let rename_x = x + icon_size * 0.5 + margin;
    let delete_x = x + width - icon_size * 0.5 - margin;
    let duplicate_x = x + width * 0.5;

    draw_rename_icon(
        ctx,
        rename_x,
        icon_y,
        icon_size,
        icon_alpha(is_hovered, rename_hovered),
    );
    draw_duplicate_icon(
        ctx,
        duplicate_x,
        icon_y,
        icon_size,
        icon_alpha(is_hovered, duplicate_hovered),
    );
    draw_delete_icon(
        ctx,
        delete_x,
        icon_y,
        icon_size,
        icon_alpha(is_hovered, delete_hovered),
    );
}

fn draw_page_badge(ctx: &cairo::Context, x: f64, y: f64, page_number: usize, is_hovered: bool) {
    let badge = page_number.to_string();
    let badge_font_size = if is_hovered { 10.0 } else { 9.0 };
    let badge_bg_alpha = if is_hovered { 0.6 } else { 0.35 };
    ctx.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Bold);
    ctx.set_font_size(badge_font_size);
    let extents = text_extents_for(
        ctx,
        "Sans",
        cairo::FontSlant::Normal,
        cairo::FontWeight::Bold,
        badge_font_size,
        &badge,
    );
    let badge_w = extents.width() + 8.0;
    let badge_h = 14.0;
    let badge_x = x + 6.0;
    let badge_y = y + 6.0;
    ctx.set_source_rgba(0.0, 0.0, 0.0, badge_bg_alpha);
    draw_rounded_rect(ctx, badge_x, badge_y, badge_w, badge_h, 4.0);
    let _ = ctx.fill();
    ctx.set_source_rgba(1.0, 1.0, 1.0, 0.9);
    ctx.move_to(badge_x + 4.0, badge_y + badge_h - 4.0);
    let _ = ctx.show_text(&badge);
}

pub(in crate::ui::board_picker::page_panel) fn render_add_page_card(
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
        ctx.set_source_rgba(1.0, 1.0, 1.0, 0.10);
    } else {
        ctx.set_source_rgba(1.0, 1.0, 1.0, 0.03);
    }
    let _ = ctx.fill_preserve();

    constants::set_color(ctx, BORDER_BOARD_PICKER);
    ctx.set_line_width(1.0);
    if !is_hovered {
        ctx.set_dash(&[4.0, 3.0], 0.0);
    }
    let _ = ctx.stroke();
    if !is_hovered {
        ctx.set_dash(&[], 0.0);
    }

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

pub(in crate::ui::board_picker::page_panel) fn render_page_preview(args: PagePreviewArgs<'_>) {
    let PagePreviewArgs {
        ctx,
        frame,
        background,
        thumb_x,
        thumb_y,
        thumb_w,
        thumb_h,
        screen_width,
        screen_height,
        page_number,
    } = args;
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

    render_page_content(PageContentArgs {
        ctx,
        frame,
        background,
        x: preview_x,
        y: preview_y,
        width: preview_w,
        height: preview_h,
        screen_width,
        screen_height,
    });

    let label = frame
        .page_name()
        .map(|name| format!("Page {} — {}", page_number, name))
        .unwrap_or_else(|| format!("Page {page_number}"));
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
