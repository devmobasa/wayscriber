use crate::draw::Color;
use crate::input::{BoardBackground, InputState};
use crate::ui::primitives::{draw_rounded_rect, text_extents_for};
use crate::ui::theme::Rgba;
use crate::ui_text::{UiTextStyle, draw_text_baseline};

use super::constants::{
    self, ACCENT_PRIMARY, BG_SELECTED_INDICATOR, BG_SELECTION, DIVIDER_LIGHT, ICON_PIN_ACTIVE,
    ICON_PIN_INACTIVE, INPUT_CARET, TEXT_ACTIVE, TEXT_HINT, TEXT_SECONDARY,
};
use super::helpers::{
    SWATCH_EDGE, board_slot_hint, draw_drag_handle, draw_open_icon, draw_pin_icon,
};

/// Outline + cross for the transparent-board swatch (no matching theme token;
/// kept from the pre-theme literal).
const SWATCH_TRANSPARENT_OUTLINE: Rgba = (0.62, 0.68, 0.76, 0.85);

pub(super) fn render_board_rows(
    ctx: &cairo::Context,
    input_state: &InputState,
    layout: crate::input::state::BoardPickerLayout,
    board_count: usize,
    max_count: usize,
) {
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

    let body_style = UiTextStyle {
        family: "Sans",
        slant: cairo::FontSlant::Normal,
        weight: cairo::FontWeight::Normal,
        size: layout.body_font_size,
    };

    // Draw "Pinned" section label.
    if pinned_count > 0 && !input_state.board_picker_is_quick() {
        let pinned_style = UiTextStyle {
            size: layout.footer_font_size * 0.9,
            ..body_style
        };
        constants::set_color(ctx, constants::with_alpha(TEXT_HINT, 0.6));
        let pinned_label_y = rows_top - layout.footer_font_size * 0.4;
        draw_text_baseline(
            ctx,
            pinned_style,
            "Pinned",
            layout.origin_x + layout.padding_x,
            pinned_label_y,
            None,
        );
    }

    // Draw pinned/unpinned section divider.
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
        if is_new_row && row > 0 {
            constants::set_color(ctx, DIVIDER_LIGHT);
            ctx.set_line_width(0.5);
            ctx.move_to(layout.origin_x + layout.padding_x, row_top);
            ctx.line_to(list_right - layout.padding_x, row_top);
            let _ = ctx.stroke();
        }
        if is_new_row {
            constants::set_color(ctx, TEXT_HINT);
            draw_rounded_rect(
                ctx,
                swatch_x,
                swatch_y,
                layout.swatch_size,
                layout.swatch_size,
                3.5,
            );
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
                    constants::set_color(ctx, SWATCH_TRANSPARENT_OUTLINE);
                    draw_rounded_rect(
                        ctx,
                        swatch_x,
                        swatch_y,
                        layout.swatch_size,
                        layout.swatch_size,
                        3.5,
                    );
                    let _ = ctx.stroke();
                    ctx.move_to(swatch_x, swatch_y);
                    ctx.line_to(swatch_x + layout.swatch_size, swatch_y + layout.swatch_size);
                    ctx.move_to(swatch_x + layout.swatch_size, swatch_y);
                    ctx.line_to(swatch_x, swatch_y + layout.swatch_size);
                    let _ = ctx.stroke();
                }
                BoardBackground::Solid(color) => {
                    ctx.set_source_rgba(color.r, color.g, color.b, 1.0);
                    draw_rounded_rect(
                        ctx,
                        swatch_x,
                        swatch_y,
                        layout.swatch_size,
                        layout.swatch_size,
                        3.5,
                    );
                    let _ = ctx.fill();
                    constants::set_color(ctx, SWATCH_EDGE);
                    draw_rounded_rect(
                        ctx,
                        swatch_x,
                        swatch_y,
                        layout.swatch_size,
                        layout.swatch_size,
                        3.5,
                    );
                    let _ = ctx.stroke();
                }
            }
            if is_active_board {
                constants::set_color(ctx, ACCENT_PRIMARY);
                draw_rounded_rect(
                    ctx,
                    swatch_x - 2.0,
                    swatch_y - 2.0,
                    layout.swatch_size + 4.0,
                    layout.swatch_size + 4.0,
                    4.0,
                );
                let _ = ctx.stroke();
            }
        }

        if is_new_row {
            let label = if board_count >= max_count {
                "New board (max reached)"
            } else {
                "New board"
            };
            constants::set_color(ctx, TEXT_HINT);
            draw_text_baseline(
                ctx,
                body_style,
                label,
                name_x,
                row_center + layout.body_font_size * 0.35,
                None,
            );
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
        draw_text_baseline(
            ctx,
            body_style,
            &name,
            name_x,
            row_center + layout.body_font_size * 0.35,
            None,
        );

        // Show page count badge after board name (skip when page panel is visible).
        let page_count = board.pages.page_count();
        if page_count > 1 && !layout.page_panel_enabled {
            let name_extents = text_extents_for(
                ctx,
                "Sans",
                cairo::FontSlant::Normal,
                cairo::FontWeight::Normal,
                layout.body_font_size,
                &name,
            );
            let page_label = format!(" ({} pages)", page_count);
            constants::set_color(ctx, constants::with_alpha(TEXT_HINT, 0.85));
            draw_text_baseline(
                ctx,
                body_style,
                &page_label,
                name_x + name_extents.width(),
                row_center + layout.body_font_size * 0.35,
                None,
            );
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
            let advance = extents.x_advance();
            let caret_x = name_x + advance + 2.0;
            constants::set_color(ctx, INPUT_CARET);
            ctx.set_line_width(1.0);
            ctx.move_to(caret_x, row_center - layout.body_font_size * 0.5);
            ctx.line_to(caret_x, row_center + layout.body_font_size * 0.5);
            let _ = ctx.stroke();
            ctx.move_to(name_x, row_center + layout.body_font_size * 0.55);
            ctx.line_to(
                name_x + advance + 6.0,
                row_center + layout.body_font_size * 0.55,
            );
            let _ = ctx.stroke();
        }

        if let Some(hint_x) = hint_x {
            let hint = hint_override.or_else(|| board_slot_hint(input_state, board_index));
            if let Some(hint) = hint {
                constants::set_color(ctx, TEXT_HINT);
                draw_text_baseline(
                    ctx,
                    body_style,
                    &hint,
                    hint_x,
                    row_center + layout.body_font_size * 0.35,
                    None,
                );

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
                    let advance = extents.x_advance();
                    let caret_x = hint_x + advance + 2.0;
                    constants::set_color(ctx, INPUT_CARET);
                    ctx.set_line_width(1.0);
                    ctx.move_to(caret_x, row_center - layout.body_font_size * 0.5);
                    ctx.line_to(caret_x, row_center + layout.body_font_size * 0.5);
                    let _ = ctx.stroke();
                    ctx.move_to(hint_x, row_center + layout.body_font_size * 0.55);
                    ctx.line_to(
                        hint_x + advance + 6.0,
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
}
