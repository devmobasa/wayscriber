use super::super::super::super::base::InputState;
use super::super::super::{
    BoardPickerEditMode, PALETTE_BOTTOM_GAP, PALETTE_SWATCH_GAP, PALETTE_SWATCH_SIZE,
    PALETTE_TOP_GAP, board_palette_colors,
};
use super::BoardPickerPaletteMetrics;

impl InputState {
    pub(super) fn compute_board_picker_palette_metrics(
        &self,
        edit_state: Option<(BoardPickerEditMode, usize, &str)>,
        list_width: f64,
        padding_x: f64,
    ) -> BoardPickerPaletteMetrics {
        let mut rows = 0usize;
        let mut cols = 0usize;
        let mut palette_height = 0.0;
        if let Some((BoardPickerEditMode::Color, edit_index, _)) = edit_state
            && edit_index < self.boards.board_count()
            && self
                .board_picker_board_index_for_row(edit_index)
                .and_then(|board_index| self.boards.board_states().get(board_index))
                .map(|board| !board.spec.background.is_transparent())
                .unwrap_or(false)
        {
            let colors = board_palette_colors();
            if !colors.is_empty() {
                let available_width = list_width - padding_x * 2.0;
                let unit = PALETTE_SWATCH_SIZE + PALETTE_SWATCH_GAP;
                let max_cols = ((available_width + PALETTE_SWATCH_GAP) / unit).floor() as usize;
                cols = max_cols.clamp(1, colors.len());
                rows = colors.len().div_ceil(cols);
                palette_height = rows as f64 * PALETTE_SWATCH_SIZE
                    + (rows.saturating_sub(1) as f64) * PALETTE_SWATCH_GAP;
            }
        }

        BoardPickerPaletteMetrics {
            rows,
            cols,
            extra_height: if rows > 0 {
                PALETTE_TOP_GAP + palette_height + PALETTE_BOTTOM_GAP
            } else {
                0.0
            },
        }
    }
}
