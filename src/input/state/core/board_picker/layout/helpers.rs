use super::super::super::base::InputState;
use super::super::{BoardPickerLayout, PAGE_NAME_HEIGHT, PAGE_NAME_PADDING, PAGE_PANEL_MAX_COLS};

#[derive(Debug, Clone, Copy)]
pub(super) struct PagePanelInfo {
    pub page_count: usize,
    pub cols: usize,
    pub rows: usize,
    pub total_rows: usize,
    pub scroll_row: usize,
    pub max_scroll_row: usize,
    pub first_visible_page: usize,
    pub visible_pages: usize,
    pub visible_slots: usize,
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
        let rows = layout.page_rows.max(1);
        let slot_count = rows.saturating_mul(cols);
        let first_visible_page = layout.page_first_visible_index.min(page_count);
        let visible_pages = layout
            .page_visible_count
            .min(page_count.saturating_sub(first_visible_page))
            .min(slot_count);
        Some(PagePanelInfo {
            page_count,
            cols,
            rows,
            total_rows: layout.page_total_rows,
            scroll_row: layout.page_scroll_row,
            max_scroll_row: layout.page_max_scroll_row,
            first_visible_page,
            visible_pages,
            visible_slots: layout.page_visible_slots.min(slot_count),
            slot_count,
        })
    }

    pub(super) const fn board_picker_page_row_stride(layout: BoardPickerLayout) -> f64 {
        layout.page_thumb_height + PAGE_NAME_HEIGHT + PAGE_NAME_PADDING + layout.page_thumb_gap
    }

    pub(super) fn board_picker_slot_to_page_index(
        &self,
        layout: BoardPickerLayout,
        board_index: usize,
        slot: usize,
    ) -> Option<usize> {
        let info = self.board_picker_page_panel_info(layout, board_index)?;
        if slot >= info.visible_slots {
            return None;
        }
        let page_index = info.first_visible_page + slot;
        (page_index < info.page_count).then_some(page_index)
    }

    pub(super) fn board_picker_page_index_to_slot(
        &self,
        layout: BoardPickerLayout,
        board_index: usize,
        page_index: usize,
    ) -> Option<usize> {
        let info = self.board_picker_page_panel_info(layout, board_index)?;
        if page_index < info.first_visible_page {
            return None;
        }
        let slot = page_index - info.first_visible_page;
        (slot < info.visible_slots && page_index < info.page_count).then_some(slot)
    }

    pub(super) fn board_picker_page_thumb_origin(
        &self,
        layout: BoardPickerLayout,
        board_index: usize,
        page_index: usize,
    ) -> Option<(PagePanelInfo, usize, usize, f64, f64)> {
        let info = self.board_picker_page_panel_info(layout, board_index)?;
        let slot = self.board_picker_page_index_to_slot(layout, board_index, page_index)?;
        self.board_picker_page_thumb_origin_for_slot(layout, info, slot)
    }

    pub(super) fn board_picker_page_thumb_origin_for_slot(
        &self,
        layout: BoardPickerLayout,
        info: PagePanelInfo,
        slot: usize,
    ) -> Option<(PagePanelInfo, usize, usize, f64, f64)> {
        if slot >= info.slot_count {
            return None;
        }
        let row = slot / info.cols;
        let col = slot % info.cols;
        let stride = Self::board_picker_page_row_stride(layout);
        let thumb_x =
            layout.page_viewport_x + col as f64 * (layout.page_thumb_width + layout.page_thumb_gap);
        let thumb_y = layout.page_viewport_y + row as f64 * stride;
        Some((info, row, col, thumb_x, thumb_y))
    }
}
