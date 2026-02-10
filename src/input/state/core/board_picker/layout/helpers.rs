use super::super::super::base::InputState;
use super::super::{
    BoardPickerLayout, PAGE_NAME_HEIGHT, PAGE_NAME_PADDING, PAGE_PANEL_MAX_COLS,
    PAGE_PANEL_MAX_ROWS, PAGE_PANEL_PADDING_X,
};

#[derive(Debug, Clone, Copy)]
pub(super) struct PagePanelInfo {
    pub page_count: usize,
    pub cols: usize,
    pub rows: usize,
    pub visible_pages: usize,
    pub slot_count: usize,
}

impl InputState {
    pub(super) fn board_picker_page_panel_info(
        &self,
        layout: BoardPickerLayout,
        board_index: usize,
    ) -> Option<PagePanelInfo> {
        if !layout.page_panel_enabled {
            return None;
        }
        let board = self.boards.board_states().get(board_index)?;
        let page_count = board.pages.page_count();
        let cols = if layout.page_cols > 0 {
            layout.page_cols
        } else {
            PAGE_PANEL_MAX_COLS
        }
        .max(1);
        let max_rows = if layout.page_max_rows > 0 {
            layout.page_max_rows
        } else {
            PAGE_PANEL_MAX_ROWS
        }
        .max(1);
        let rows = page_count.max(1).div_ceil(cols).clamp(1, max_rows);
        let slot_count = rows.saturating_mul(cols);
        let visible_pages = page_count.min(slot_count);
        Some(PagePanelInfo {
            page_count,
            cols,
            rows,
            visible_pages,
            slot_count,
        })
    }

    pub(super) const fn board_picker_page_row_stride(layout: BoardPickerLayout) -> f64 {
        layout.page_thumb_height + PAGE_NAME_HEIGHT + PAGE_NAME_PADDING + layout.page_thumb_gap
    }

    pub(super) fn board_picker_page_thumb_origin(
        &self,
        layout: BoardPickerLayout,
        board_index: usize,
        index: usize,
    ) -> Option<(PagePanelInfo, usize, usize, f64, f64)> {
        let info = self.board_picker_page_panel_info(layout, board_index)?;
        if index >= info.slot_count {
            return None;
        }
        let row = index / info.cols;
        let col = index % info.cols;
        let stride = Self::board_picker_page_row_stride(layout);
        let thumb_x = layout.page_panel_x
            + PAGE_PANEL_PADDING_X
            + col as f64 * (layout.page_thumb_width + layout.page_thumb_gap);
        let thumb_y = layout.page_panel_y + row as f64 * stride;
        Some((info, row, col, thumb_x, thumb_y))
    }
}
