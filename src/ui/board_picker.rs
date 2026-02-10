use crate::input::InputState;
use crate::ui::primitives::{draw_rounded_rect, text_extents_for};

use super::constants::{
    self, BORDER_BOARD_PICKER, NAV_HINT_BOARD_PICKER, OVERLAY_DIM_LIGHT, OVERLAY_DIM_MEDIUM,
    PANEL_BG_BOARD_PICKER, RADIUS_PANEL, TEXT_HINT, TEXT_PRIMARY, TEXT_TERTIARY,
};

mod helpers;
mod page_panel;
mod palette;
mod rows;

use page_panel::render_page_panel;
use palette::render_board_palette;
use rows::render_board_rows;

pub fn render_board_picker(
    ctx: &cairo::Context,
    input_state: &InputState,
    screen_width: u32,
    screen_height: u32,
) {
    if !input_state.is_board_picker_open() {
        return;
    }

    let layout = match input_state.board_picker_layout() {
        Some(layout) => *layout,
        None => return,
    };

    let _ = ctx.save();

    // Dim background (lighter in quick mode for a popover feel)
    let dim_alpha = if input_state.board_picker_is_quick() {
        OVERLAY_DIM_LIGHT
    } else {
        OVERLAY_DIM_MEDIUM
    };
    ctx.set_source_rgba(0.0, 0.0, 0.0, dim_alpha);
    ctx.rectangle(0.0, 0.0, screen_width as f64, screen_height as f64);
    let _ = ctx.fill();

    // Panel
    draw_rounded_rect(
        ctx,
        layout.origin_x,
        layout.origin_y,
        layout.width,
        layout.height,
        RADIUS_PANEL,
    );
    constants::set_color(ctx, PANEL_BG_BOARD_PICKER);
    let _ = ctx.fill_preserve();
    constants::set_color(ctx, BORDER_BOARD_PICKER);
    ctx.set_line_width(1.0);
    let _ = ctx.stroke();

    // Title
    let board_count = input_state.boards.board_count();
    let max_count = input_state.boards.max_count();
    let title = input_state.board_picker_title(board_count, max_count);
    ctx.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Bold);
    ctx.set_font_size(layout.title_font_size);
    constants::set_color(ctx, TEXT_PRIMARY);
    let title_y = layout.origin_y + layout.padding_y + layout.title_font_size;
    ctx.move_to(layout.origin_x + layout.padding_x, title_y);
    let _ = ctx.show_text(&title);

    // Footer with navigation hint
    let footer = input_state.board_picker_footer_text();
    let recent = input_state.board_picker_recent_label();
    ctx.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Normal);
    ctx.set_font_size(layout.footer_font_size);
    constants::set_color(ctx, TEXT_TERTIARY);
    let footer_y = layout.origin_y + layout.height - layout.padding_y;
    let footer_extents = text_extents_for(
        ctx,
        "Sans",
        cairo::FontSlant::Normal,
        cairo::FontWeight::Normal,
        layout.footer_font_size,
        &footer,
    );
    ctx.move_to(layout.origin_x + layout.padding_x, footer_y);
    let _ = ctx.show_text(&footer);
    // Navigation hint on right side
    ctx.set_source_rgba(TEXT_HINT.0, TEXT_HINT.1, TEXT_HINT.2, 0.7);
    let nav_extents = text_extents_for(
        ctx,
        "Sans",
        cairo::FontSlant::Normal,
        cairo::FontWeight::Normal,
        layout.footer_font_size,
        NAV_HINT_BOARD_PICKER,
    );
    let nav_start = layout.origin_x + layout.width - layout.padding_x - nav_extents.width();
    let footer_end = layout.origin_x + layout.padding_x + footer_extents.width();
    if footer_end + layout.footer_font_size * 0.5 <= nav_start {
        ctx.move_to(nav_start, footer_y);
        let _ = ctx.show_text(NAV_HINT_BOARD_PICKER);
    }
    if let Some(recent) = recent {
        let recent_y = footer_y - layout.recent_height;
        ctx.set_source_rgba(TEXT_HINT.0, TEXT_HINT.1, TEXT_HINT.2, 0.8);
        ctx.move_to(layout.origin_x + layout.padding_x, recent_y);
        let _ = ctx.show_text(&recent);
    }

    render_board_rows(ctx, input_state, layout, board_count, max_count);
    render_board_palette(ctx, input_state, layout);

    render_page_panel(ctx, input_state, layout, screen_width, screen_height);

    let _ = ctx.restore();
}
