use crate::draw::Color;
use crate::input::{BoardBackground, InputState};
use crate::ui::primitives::{draw_rounded_rect, text_extents_for};

use super::constants::{
    self, BG_SELECTED_INDICATOR, BG_SELECTION, BORDER_BOARD_PICKER, DIVIDER_LIGHT, ICON_PIN_ACTIVE,
    ICON_PIN_INACTIVE, INDICATOR_ACTIVE_BOARD, INPUT_CARET, NAV_HINT_BOARD_PICKER,
    OVERLAY_DIM_LIGHT, OVERLAY_DIM_MEDIUM, PANEL_BG_BOARD_PICKER, RADIUS_PANEL, TEXT_ACTIVE,
    TEXT_HINT, TEXT_PRIMARY, TEXT_SECONDARY, TEXT_TERTIARY,
};

mod helpers;
mod page_panel;
use helpers::{BOARD_PALETTE, board_slot_hint, draw_drag_handle, draw_open_icon, draw_pin_icon};
use page_panel::render_page_panel;

const PALETTE_SWATCH_SIZE: f64 = 18.0;
const PALETTE_SWATCH_GAP: f64 = 6.0;

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

    let rows_top = layout.origin_y + layout.padding_y + layout.header_height;
    let name_x = layout.origin_x + layout.padding_x + layout.swatch_size + layout.swatch_padding;
    let list_right = layout.origin_x + layout.list_width;
    let handle_x = if layout.handle_width > 0.0 {
        Some(list_right - layout.padding_x - layout.handle_width)
    } else {
        None
    };
    let open_icon_x = if layout.open_icon_size > 0.0 {
        handle_x.map(|x| x - layout.open_icon_gap - layout.open_icon_size)
    } else {
        None
    };
    let hint_right_edge = if let Some(open_icon_x) = open_icon_x {
        open_icon_x - layout.handle_gap
    } else if let Some(handle_x) = handle_x {
        handle_x - layout.handle_gap
    } else {
        list_right - layout.padding_x
    };
    let hint_x = if layout.hint_width > 0.0 {
        Some(hint_right_edge - layout.hint_width)
    } else {
        None
    };

    let highlight_index = input_state.board_picker_active_index();
    let selected_index = input_state.board_picker_selected_index();
    let active_board_index = input_state.boards.active_index();
    let edit_state = input_state.board_picker_edit_state();
    let pinned_count = input_state.board_picker_pinned_count();

    // Draw pinned/unpinned section divider
    if pinned_count > 0 && pinned_count < board_count {
        let divider_y = rows_top + layout.row_height * pinned_count as f64;
        constants::set_color(ctx, DIVIDER_LIGHT);
        ctx.set_line_width(1.0);
        ctx.move_to(layout.origin_x + layout.padding_x, divider_y);
        ctx.line_to(list_right - layout.padding_x, divider_y);
        let _ = ctx.stroke();
    }

    for row in 0..layout.row_count {
        let row_top = rows_top + layout.row_height * row as f64;
        let row_center = row_top + layout.row_height * 0.5;
        let is_highlighted = highlight_index == Some(row);
        let is_selected = selected_index == Some(row);
        let board_index = if row < board_count {
            input_state
                .board_picker_board_index_for_row(row)
                .unwrap_or(row)
        } else {
            0
        };
        let is_active_board = row < board_count && board_index == active_board_index;

        if is_highlighted {
            constants::set_color(ctx, BG_SELECTION);
            ctx.rectangle(
                layout.origin_x + 6.0,
                row_top,
                layout.list_width - 12.0,
                layout.row_height,
            );
            let _ = ctx.fill();
        }

        if is_selected {
            constants::set_color(ctx, BG_SELECTED_INDICATOR);
            ctx.rectangle(layout.origin_x + 6.0, row_top, 3.0, layout.row_height);
            let _ = ctx.fill();
        }

        let swatch_x = layout.origin_x + layout.padding_x;
        let swatch_y = row_center - layout.swatch_size * 0.5;

        let is_new_row = row >= board_count;
        if is_new_row {
            constants::set_color(ctx, TEXT_HINT);
            ctx.rectangle(swatch_x, swatch_y, layout.swatch_size, layout.swatch_size);
            let _ = ctx.stroke();
            ctx.set_line_width(1.5);
            let mid_x = swatch_x + layout.swatch_size * 0.5;
            let mid_y = swatch_y + layout.swatch_size * 0.5;
            ctx.move_to(mid_x - 4.0, mid_y);
            ctx.line_to(mid_x + 4.0, mid_y);
            ctx.move_to(mid_x, mid_y - 4.0);
            ctx.line_to(mid_x, mid_y + 4.0);
            let _ = ctx.stroke();
        } else {
            let board = &input_state.boards.board_states()[board_index];
            match board.spec.background {
                BoardBackground::Transparent => {
                    ctx.set_source_rgba(0.62, 0.68, 0.76, 0.85);
                    ctx.rectangle(swatch_x, swatch_y, layout.swatch_size, layout.swatch_size);
                    let _ = ctx.stroke();
                    ctx.move_to(swatch_x, swatch_y);
                    ctx.line_to(swatch_x + layout.swatch_size, swatch_y + layout.swatch_size);
                    ctx.move_to(swatch_x + layout.swatch_size, swatch_y);
                    ctx.line_to(swatch_x, swatch_y + layout.swatch_size);
                    let _ = ctx.stroke();
                }
                BoardBackground::Solid(color) => {
                    ctx.set_source_rgba(color.r, color.g, color.b, 1.0);
                    ctx.rectangle(swatch_x, swatch_y, layout.swatch_size, layout.swatch_size);
                    let _ = ctx.fill();
                    ctx.set_source_rgba(0.0, 0.0, 0.0, 0.2);
                    ctx.rectangle(swatch_x, swatch_y, layout.swatch_size, layout.swatch_size);
                    let _ = ctx.stroke();
                }
            }
            if is_active_board {
                constants::set_color(ctx, INDICATOR_ACTIVE_BOARD);
                ctx.rectangle(
                    swatch_x - 2.0,
                    swatch_y - 2.0,
                    layout.swatch_size + 4.0,
                    layout.swatch_size + 4.0,
                );
                let _ = ctx.stroke();
            }
        }

        ctx.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Normal);
        ctx.set_font_size(layout.body_font_size);

        if is_new_row {
            let label = if board_count >= max_count {
                "New board (max reached)"
            } else {
                "New board"
            };
            constants::set_color(ctx, TEXT_HINT);
            ctx.move_to(name_x, row_center + layout.body_font_size * 0.35);
            let _ = ctx.show_text(label);
            continue;
        }

        let board = &input_state.boards.board_states()[board_index];
        let show_pin = board.spec.pinned || is_highlighted || is_selected;
        if show_pin {
            let pin_x = swatch_x - (layout.swatch_padding * 0.6);
            let (color, filled) = if board.spec.pinned {
                (
                    Color {
                        r: ICON_PIN_ACTIVE.0,
                        g: ICON_PIN_ACTIVE.1,
                        b: ICON_PIN_ACTIVE.2,
                        a: ICON_PIN_ACTIVE.3,
                    },
                    true,
                )
            } else {
                (
                    Color {
                        r: ICON_PIN_INACTIVE.0,
                        g: ICON_PIN_INACTIVE.1,
                        b: ICON_PIN_INACTIVE.2,
                        a: ICON_PIN_INACTIVE.3,
                    },
                    false,
                )
            };
            draw_pin_icon(ctx, pin_x, row_center, layout.body_font_size, color, filled);
        }
        let (mut name, mut hint_override) = (board.spec.name.clone(), None);
        if let Some((mode, edit_index, buffer)) = edit_state
            && edit_index == row
        {
            match mode {
                crate::input::state::BoardPickerEditMode::Name => {
                    name = buffer.to_string();
                }
                crate::input::state::BoardPickerEditMode::Color => {
                    hint_override = Some(buffer.to_string());
                }
            }
        }

        let name_color = if is_active_board {
            TEXT_ACTIVE
        } else {
            TEXT_SECONDARY
        };
        constants::set_color(ctx, name_color);
        ctx.move_to(name_x, row_center + layout.body_font_size * 0.35);
        let _ = ctx.show_text(&name);

        // Show page count badge after board name
        let page_count = board.pages.page_count();
        if page_count > 1 {
            let name_extents = text_extents_for(
                ctx,
                "Sans",
                cairo::FontSlant::Normal,
                cairo::FontWeight::Normal,
                layout.body_font_size,
                &name,
            );
            let page_label = format!(" ({} pages)", page_count);
            ctx.set_source_rgba(TEXT_HINT.0, TEXT_HINT.1, TEXT_HINT.2, 0.85);
            ctx.move_to(
                name_x + name_extents.width(),
                row_center + layout.body_font_size * 0.35,
            );
            let _ = ctx.show_text(&page_label);
        }

        if let Some((mode, edit_index, _buffer)) = edit_state
            && edit_index == row
            && mode == crate::input::state::BoardPickerEditMode::Name
        {
            let extents = text_extents_for(
                ctx,
                "Sans",
                cairo::FontSlant::Normal,
                cairo::FontWeight::Normal,
                layout.body_font_size,
                &name,
            );
            let caret_x = name_x + extents.width() + 2.0;
            constants::set_color(ctx, INPUT_CARET);
            ctx.set_line_width(1.0);
            ctx.move_to(caret_x, row_center - layout.body_font_size * 0.5);
            ctx.line_to(caret_x, row_center + layout.body_font_size * 0.5);
            let _ = ctx.stroke();
            ctx.move_to(name_x, row_center + layout.body_font_size * 0.55);
            ctx.line_to(
                name_x + extents.width() + 6.0,
                row_center + layout.body_font_size * 0.55,
            );
            let _ = ctx.stroke();
        }

        if let Some(hint_x) = hint_x {
            let hint = hint_override.or_else(|| board_slot_hint(input_state, board_index));
            if let Some(hint) = hint {
                constants::set_color(ctx, TEXT_HINT);
                ctx.move_to(hint_x, row_center + layout.body_font_size * 0.35);
                let _ = ctx.show_text(&hint);

                if let Some((mode, edit_index, _)) = edit_state
                    && edit_index == row
                    && mode == crate::input::state::BoardPickerEditMode::Color
                {
                    let extents = text_extents_for(
                        ctx,
                        "Sans",
                        cairo::FontSlant::Normal,
                        cairo::FontWeight::Normal,
                        layout.body_font_size,
                        &hint,
                    );
                    let caret_x = hint_x + extents.width() + 2.0;
                    constants::set_color(ctx, INPUT_CARET);
                    ctx.set_line_width(1.0);
                    ctx.move_to(caret_x, row_center - layout.body_font_size * 0.5);
                    ctx.line_to(caret_x, row_center + layout.body_font_size * 0.5);
                    let _ = ctx.stroke();
                    ctx.move_to(hint_x, row_center + layout.body_font_size * 0.55);
                    ctx.line_to(
                        hint_x + extents.width() + 6.0,
                        row_center + layout.body_font_size * 0.55,
                    );
                    let _ = ctx.stroke();
                }
            }
        }

        if let Some(open_icon_x) = open_icon_x
            && !is_new_row
            && !input_state.board_picker_is_quick()
        {
            let alpha = if is_highlighted || is_selected {
                0.95
            } else {
                0.6
            };
            let center_x = open_icon_x + layout.open_icon_size * 0.5;
            draw_open_icon(ctx, center_x, row_center, layout.open_icon_size, alpha);
        }

        if let Some(handle_x) = handle_x
            && !is_new_row
            && !input_state.board_picker_is_quick()
        {
            draw_drag_handle(ctx, handle_x, row_center, layout.handle_width);
        }
    }

    if layout.palette_rows > 0 && layout.palette_cols > 0 {
        let palette_x = layout.origin_x + layout.padding_x;
        let palette_y = layout.palette_top;
        let active_color = edit_state
            .and_then(|(_, edit_index, _)| input_state.board_picker_board_index_for_row(edit_index))
            .and_then(|board_index| input_state.boards.board_states().get(board_index))
            .and_then(|board| match board.spec.background {
                BoardBackground::Solid(color) => Some(color),
                BoardBackground::Transparent => None,
            });

        let mut idx = 0usize;
        for row in 0..layout.palette_rows {
            for col in 0..layout.palette_cols {
                if idx >= BOARD_PALETTE.len() {
                    break;
                }
                let color = BOARD_PALETTE[idx];
                let swatch_x = palette_x + col as f64 * (PALETTE_SWATCH_SIZE + PALETTE_SWATCH_GAP);
                let swatch_y = palette_y + row as f64 * (PALETTE_SWATCH_SIZE + PALETTE_SWATCH_GAP);
                ctx.set_source_rgba(color.r, color.g, color.b, 1.0);
                draw_rounded_rect(
                    ctx,
                    swatch_x,
                    swatch_y,
                    PALETTE_SWATCH_SIZE,
                    PALETTE_SWATCH_SIZE,
                    4.0,
                );
                let _ = ctx.fill();
                ctx.set_source_rgba(0.0, 0.0, 0.0, 0.2);
                draw_rounded_rect(
                    ctx,
                    swatch_x,
                    swatch_y,
                    PALETTE_SWATCH_SIZE,
                    PALETTE_SWATCH_SIZE,
                    4.0,
                );
                let _ = ctx.stroke();

                if active_color.map(|active| active == color).unwrap_or(false) {
                    constants::set_color(ctx, INPUT_CARET);
                    ctx.set_line_width(1.5);
                    draw_rounded_rect(
                        ctx,
                        swatch_x - 2.0,
                        swatch_y - 2.0,
                        PALETTE_SWATCH_SIZE + 4.0,
                        PALETTE_SWATCH_SIZE + 4.0,
                        5.0,
                    );
                    let _ = ctx.stroke();
                }
                idx += 1;
            }
        }
    }

    render_page_panel(ctx, input_state, layout, screen_width, screen_height);

    let _ = ctx.restore();
}
