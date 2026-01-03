use crate::draw::{BLACK, BLUE, Color, GREEN, ORANGE, PINK, RED, WHITE, YELLOW};
use crate::input::{BoardBackground, InputState};
use crate::ui::primitives::{draw_rounded_rect, text_extents_for};
use std::f64::consts::PI;

const PALETTE_SWATCH_SIZE: f64 = 18.0;
const PALETTE_SWATCH_GAP: f64 = 6.0;

const BOARD_PALETTE: [Color; 11] = [
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
        0.15
    } else {
        0.35
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
        12.0,
    );
    ctx.set_source_rgba(0.09, 0.11, 0.15, 0.96);
    let _ = ctx.fill_preserve();
    ctx.set_source_rgba(0.2, 0.24, 0.3, 0.9);
    ctx.set_line_width(1.0);
    let _ = ctx.stroke();

    // Title
    let board_count = input_state.boards.board_count();
    let max_count = input_state.boards.max_count();
    let title = input_state.board_picker_title(board_count, max_count);
    ctx.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Bold);
    ctx.set_font_size(layout.title_font_size);
    ctx.set_source_rgba(0.92, 0.94, 0.98, 1.0);
    let title_y = layout.origin_y + layout.padding_y + layout.title_font_size;
    ctx.move_to(layout.origin_x + layout.padding_x, title_y);
    let _ = ctx.show_text(&title);

    // Footer
    let footer = input_state.board_picker_footer_text();
    let recent = input_state.board_picker_recent_label();
    ctx.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Normal);
    ctx.set_font_size(layout.footer_font_size);
    ctx.set_source_rgba(0.64, 0.69, 0.76, 0.9);
    let footer_y = layout.origin_y + layout.height - layout.padding_y;
    ctx.move_to(layout.origin_x + layout.padding_x, footer_y);
    let _ = ctx.show_text(&footer);
    if let Some(recent) = recent {
        let recent_y = footer_y - layout.recent_height;
        ctx.set_source_rgba(0.52, 0.58, 0.66, 0.9);
        ctx.move_to(layout.origin_x + layout.padding_x, recent_y);
        let _ = ctx.show_text(&recent);
    }

    let rows_top = layout.origin_y + layout.padding_y + layout.header_height;
    let name_x = layout.origin_x + layout.padding_x + layout.swatch_size + layout.swatch_padding;
    let handle_x = if layout.handle_width > 0.0 {
        Some(layout.origin_x + layout.width - layout.padding_x - layout.handle_width)
    } else {
        None
    };
    let hint_right_edge = if let Some(handle_x) = handle_x {
        handle_x - layout.handle_gap
    } else {
        layout.origin_x + layout.width - layout.padding_x
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
            ctx.set_source_rgba(0.22, 0.28, 0.38, 0.9);
            ctx.rectangle(
                layout.origin_x + 6.0,
                row_top,
                layout.width - 12.0,
                layout.row_height,
            );
            let _ = ctx.fill();
        }

        if is_selected {
            ctx.set_source_rgba(0.33, 0.42, 0.58, 0.9);
            ctx.rectangle(layout.origin_x + 6.0, row_top, 3.0, layout.row_height);
            let _ = ctx.fill();
        }

        let swatch_x = layout.origin_x + layout.padding_x;
        let swatch_y = row_center - layout.swatch_size * 0.5;

        let is_new_row = row >= board_count;
        if is_new_row {
            ctx.set_source_rgba(0.45, 0.5, 0.58, 0.9);
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
                ctx.set_source_rgba(0.9, 0.83, 0.32, 0.95);
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
            ctx.set_source_rgba(0.7, 0.74, 0.8, 0.9);
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
                        r: 0.96,
                        g: 0.82,
                        b: 0.28,
                        a: 0.95,
                    },
                    true,
                )
            } else {
                (
                    Color {
                        r: 0.6,
                        g: 0.65,
                        b: 0.72,
                        a: 0.5,
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
            [0.96, 0.98, 1.0, 1.0]
        } else {
            [0.86, 0.89, 0.94, 1.0]
        };
        ctx.set_source_rgba(name_color[0], name_color[1], name_color[2], name_color[3]);
        ctx.move_to(name_x, row_center + layout.body_font_size * 0.35);
        let _ = ctx.show_text(&name);

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
            ctx.set_source_rgba(0.98, 0.92, 0.55, 1.0);
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
                ctx.set_source_rgba(0.6, 0.65, 0.72, 0.9);
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
                    ctx.set_source_rgba(0.98, 0.92, 0.55, 1.0);
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
                    ctx.set_source_rgba(0.98, 0.92, 0.55, 0.95);
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

    let _ = ctx.restore();
}

fn board_slot_hint(state: &InputState, index: usize) -> Option<String> {
    use crate::config::Action;
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

fn draw_pin_icon(ctx: &cairo::Context, x: f64, y: f64, size: f64, color: Color, filled: bool) {
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

fn draw_drag_handle(ctx: &cairo::Context, x: f64, y: f64, width: f64) {
    let dot_radius = (width * 0.18).clamp(1.2, 2.2);
    let gap = dot_radius * 2.2;
    let col_gap = dot_radius * 2.6;
    let start_x = x + width * 0.5 - col_gap * 0.5;
    let start_y = y - gap;
    ctx.set_source_rgba(0.58, 0.63, 0.7, 0.85);
    for row in 0..3 {
        for col in 0..2 {
            let cx = start_x + col as f64 * col_gap;
            let cy = start_y + row as f64 * gap;
            ctx.arc(cx, cy, dot_radius, 0.0, PI * 2.0);
            let _ = ctx.fill();
        }
    }
}
