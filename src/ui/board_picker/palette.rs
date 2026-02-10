use crate::input::{BoardBackground, InputState};
use crate::ui::primitives::draw_rounded_rect;

use super::constants::{self, INPUT_CARET};
use super::helpers::BOARD_PALETTE;

const PALETTE_SWATCH_SIZE: f64 = 18.0;
const PALETTE_SWATCH_GAP: f64 = 6.0;

pub(super) fn render_board_palette(
    ctx: &cairo::Context,
    input_state: &InputState,
    layout: crate::input::state::BoardPickerLayout,
) {
    if layout.palette_rows == 0 || layout.palette_cols == 0 {
        return;
    }

    let palette_x = layout.origin_x + layout.padding_x;
    let palette_y = layout.palette_top;
    let edit_state = input_state.board_picker_edit_state();
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
