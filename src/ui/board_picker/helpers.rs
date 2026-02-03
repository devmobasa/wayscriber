use std::f64::consts::PI;

use crate::config::Action;
use crate::draw::{
    BLACK, BLUE, Color, EraserReplayContext, GREEN, ORANGE, PINK, RED, WHITE, YELLOW,
    render_eraser_stroke, render_shape,
};
use crate::input::state::BoardPickerLayout;
use crate::input::state::{PAGE_DELETE_ICON_MARGIN, PAGE_DELETE_ICON_SIZE, PAGE_HEADER_ICON_SIZE};
use crate::input::{BoardBackground, InputState};
use crate::ui::constants::{
    self, BG_SELECTION, BORDER_BOARD_PICKER, ICON_DRAG_HANDLE, ICON_SUBMENU_ARROW,
    INDICATOR_ACTIVE_BOARD, PANEL_BG_BOARD_PICKER, TEXT_HINT, TEXT_TERTIARY,
};
use crate::ui::primitives::{draw_rounded_rect, text_extents_for};

pub(super) const BOARD_PALETTE: [Color; 11] = [
    RED,
    GREEN,
    BLUE,
    YELLOW,
    WHITE,
    BLACK,
    ORANGE,
    PINK,
    Color {
        r: 0.0,
        g: 1.0,
        b: 1.0,
        a: 1.0,
    },
    Color {
        r: 0.6,
        g: 0.4,
        b: 0.8,
        a: 1.0,
    },
    Color {
        r: 0.4,
        g: 0.4,
        b: 0.4,
        a: 1.0,
    },
];

const PAGE_PANEL_PADDING_X: f64 = 12.0;

pub(super) fn board_slot_hint(state: &InputState, index: usize) -> Option<String> {
    let action = match index {
        0 => Action::Board1,
        1 => Action::Board2,
        2 => Action::Board3,
        3 => Action::Board4,
        4 => Action::Board5,
        5 => Action::Board6,
        6 => Action::Board7,
        7 => Action::Board8,
        8 => Action::Board9,
        _ => return None,
    };
    let label = state.action_binding_label(action);
    if label == "Not bound" {
        None
    } else {
        Some(label)
    }
}

pub(super) fn draw_pin_icon(
    ctx: &cairo::Context,
    x: f64,
    y: f64,
    size: f64,
    color: Color,
    filled: bool,
) {
    let head_radius = (size * 0.22).clamp(2.0, 3.2);
    let stem_length = size * 0.6;
    let head_y = y - stem_length * 0.35;
    ctx.set_source_rgba(color.r, color.g, color.b, color.a);
    ctx.arc(x, head_y, head_radius, 0.0, PI * 2.0);
    if filled {
        let _ = ctx.fill();
    } else {
        ctx.set_line_width(1.2);
        let _ = ctx.stroke();
    }
    ctx.set_line_width(1.2);
    ctx.move_to(x, head_y + head_radius);
    ctx.line_to(x, head_y + head_radius + stem_length);
    let _ = ctx.stroke();
}

pub(super) fn draw_drag_handle(ctx: &cairo::Context, x: f64, y: f64, width: f64) {
    let dot_radius = (width * 0.18).clamp(1.2, 2.2);
    let gap = dot_radius * 2.2;
    let col_gap = dot_radius * 2.6;
    let start_x = x + width * 0.5 - col_gap * 0.5;
    let start_y = y - gap;
    constants::set_color(ctx, ICON_DRAG_HANDLE);
    for row in 0..3 {
        for col in 0..2 {
            let cx = start_x + col as f64 * col_gap;
            let cy = start_y + row as f64 * gap;
            ctx.arc(cx, cy, dot_radius, 0.0, PI * 2.0);
            let _ = ctx.fill();
        }
    }
}

pub(super) fn draw_open_icon(ctx: &cairo::Context, x: f64, y: f64, size: f64, alpha: f64) {
    let half = size * 0.5;
    constants::set_color(ctx, constants::with_alpha(ICON_SUBMENU_ARROW, alpha));
    ctx.move_to(x - half * 0.2, y - half);
    ctx.line_to(x + half, y);
    ctx.line_to(x - half * 0.2, y + half);
    let _ = ctx.fill();
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
    let half = size * 0.5;
    constants::set_color(ctx, constants::with_alpha(TEXT_TERTIARY, alpha));
    ctx.set_line_width(1.6);
    ctx.move_to(x - half, y - half);
    ctx.line_to(x + half, y + half);
    let _ = ctx.stroke();
    ctx.move_to(x + half, y - half);
    ctx.line_to(x - half, y + half);
    let _ = ctx.stroke();
}

pub(super) fn render_page_panel(
    ctx: &cairo::Context,
    input_state: &InputState,
    layout: BoardPickerLayout,
    screen_width: u32,
    screen_height: u32,
) {
    if !layout.page_panel_enabled || layout.page_visible_count == 0 {
        return;
    }
    let Some(board_index) = layout.page_board_index else {
        return;
    };
    let Some(board) = input_state.boards.board_states().get(board_index) else {
        return;
    };
    let pages = board.pages.pages();
    let page_count = board.pages.page_count();
    if page_count == 0 {
        return;
    }

    let label = if page_count > 1 {
        format!("Pages — {}  • drag to reorder", board.spec.name)
    } else {
        format!("Pages — {}", board.spec.name)
    };
    ctx.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Normal);
    ctx.set_font_size(layout.footer_font_size);
    constants::set_color(ctx, TEXT_TERTIARY);
    let label_y = layout.origin_y + layout.padding_y + layout.title_font_size;
    ctx.move_to(layout.page_panel_x + 2.0, label_y);
    let _ = ctx.show_text(&label);

    let (pointer_x, pointer_y) = input_state.pointer_position();
    let add_hover = input_state.board_picker_page_add_button_at(pointer_x, pointer_y);
    let add_center_x = layout.page_panel_x + layout.page_panel_width
        - PAGE_PANEL_PADDING_X
        - PAGE_HEADER_ICON_SIZE * 0.5;
    let add_center_y = label_y - layout.footer_font_size * 0.35;
    let add_alpha = if add_hover { 0.95 } else { 0.65 };
    draw_plus_icon(
        ctx,
        add_center_x,
        add_center_y,
        PAGE_HEADER_ICON_SIZE,
        add_alpha,
    );

    let start_x = layout.page_panel_x + PAGE_PANEL_PADDING_X;
    let start_y = layout.page_panel_y;
    let active_page = board.pages.active_index();
    let drag = input_state.board_picker_page_drag;
    let hover_index = input_state.board_picker_page_index_at(pointer_x, pointer_y);
    let hover_delete = input_state.board_picker_page_delete_index_at(pointer_x, pointer_y);
    let cols = layout.page_cols.max(1);
    let max_rows = layout.page_max_rows.max(1);
    let rows = page_count.div_ceil(cols).min(max_rows);
    let visible = page_count.min(rows.saturating_mul(cols));

    for (index, page) in pages.iter().enumerate().take(visible) {
        let col = index % cols;
        let row = index / cols;
        if row >= rows {
            continue;
        }
        let thumb_x = start_x + col as f64 * (layout.page_thumb_width + layout.page_thumb_gap);
        let thumb_y = start_y + row as f64 * (layout.page_thumb_height + layout.page_thumb_gap);
        let is_active = index == active_page;
        let is_drop_target =
            drag.is_some_and(|d| d.current_index == index && d.board_index == board_index);
        render_page_thumbnail(PageThumbnailArgs {
            ctx,
            frame: page,
            background: &board.spec.background,
            x: thumb_x,
            y: thumb_y,
            width: layout.page_thumb_width,
            height: layout.page_thumb_height,
            screen_width,
            screen_height,
            page_number: index + 1,
            is_active,
            is_drop_target,
            show_delete: hover_index == Some(index),
            delete_hovered: hover_delete == Some(index),
        });
    }

    if page_count > visible {
        let overflow = page_count - visible;
        let hint = format!("+{overflow} more");
        ctx.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Normal);
        ctx.set_font_size(layout.footer_font_size);
        constants::set_color(ctx, TEXT_HINT);
        let hint_y = start_y + layout.page_panel_height + layout.footer_font_size + 6.0;
        ctx.move_to(start_x, hint_y);
        let _ = ctx.show_text(&hint);
    }
}

struct PageThumbnailArgs<'a> {
    ctx: &'a cairo::Context,
    frame: &'a crate::draw::Frame,
    background: &'a BoardBackground,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    screen_width: u32,
    screen_height: u32,
    page_number: usize,
    is_active: bool,
    is_drop_target: bool,
    show_delete: bool,
    delete_hovered: bool,
}

fn render_page_thumbnail(args: PageThumbnailArgs<'_>) {
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
        is_active,
        is_drop_target,
        show_delete,
        delete_hovered,
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

    if show_delete {
        let icon_size = PAGE_DELETE_ICON_SIZE;
        let margin = PAGE_DELETE_ICON_MARGIN;
        let icon_x = x + width - icon_size - margin + icon_size * 0.5;
        let icon_y = y + height - icon_size - margin + icon_size * 0.5;
        let alpha = if delete_hovered { 0.95 } else { 0.6 };
        draw_delete_icon(ctx, icon_x, icon_y, icon_size, alpha);
    }

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
}
