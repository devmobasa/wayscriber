use super::super::super::super::base::InputState;
use super::super::super::{
    PAGE_NAME_HEIGHT, PAGE_NAME_PADDING, PAGE_PANEL_GAP, PAGE_PANEL_MAX_COLS, PAGE_PANEL_MAX_ROWS,
    PAGE_PANEL_PADDING_X, PAGE_THUMB_GAP, PAGE_THUMB_HEIGHT, PAGE_THUMB_MAX_WIDTH,
    PAGE_THUMB_MIN_WIDTH,
};
use super::BoardPickerPagePanelMetrics;

impl InputState {
    pub(super) fn compute_board_picker_page_panel_metrics(
        &self,
        screen_width: u32,
        screen_height: u32,
        footer_height: f64,
        mut panel_height: f64,
    ) -> (BoardPickerPagePanelMetrics, f64) {
        let page_row_height = PAGE_THUMB_HEIGHT + PAGE_NAME_HEIGHT + PAGE_NAME_PADDING;
        let mut metrics = BoardPickerPagePanelMetrics {
            enabled: false,
            width: 0.0,
            height: 0.0,
            thumb_width: 0.0,
            cols: 0,
            rows: 0,
            count: 0,
            visible_count: 0,
            board_index: None,
        };

        if !self.board_picker_is_quick()
            && let Some(board_index) = self.board_picker_page_panel_board_index()
            && let Some(board) = self.boards.board_states().get(board_index)
        {
            metrics.count = board.pages.page_count();
            let aspect = if screen_height == 0 {
                1.0
            } else {
                screen_width as f64 / screen_height as f64
            };
            let base_thumb_width =
                (PAGE_THUMB_HEIGHT * aspect).clamp(PAGE_THUMB_MIN_WIDTH, PAGE_THUMB_MAX_WIDTH);

            let available_right =
                (screen_width as f64 - (PAGE_PANEL_GAP + 32.0)).max(base_thumb_width + 32.0);
            let mut candidate_cols = PAGE_PANEL_MAX_COLS.max(1);
            loop {
                let candidate_width = PAGE_PANEL_PADDING_X * 2.0
                    + candidate_cols as f64 * base_thumb_width
                    + (candidate_cols.saturating_sub(1) as f64) * PAGE_THUMB_GAP;
                if candidate_width <= available_right || candidate_cols == 1 {
                    metrics.width = candidate_width.min(available_right);
                    metrics.cols = candidate_cols;
                    break;
                }
                candidate_cols -= 1;
            }

            if metrics.cols == 0 {
                metrics.cols = 1;
            }

            metrics.thumb_width = ((metrics.width
                - PAGE_PANEL_PADDING_X * 2.0
                - (metrics.cols.saturating_sub(1) as f64) * PAGE_THUMB_GAP)
                / metrics.cols as f64)
                .clamp(PAGE_THUMB_MIN_WIDTH, PAGE_THUMB_MAX_WIDTH);

            let total_rows = metrics.count.max(1).div_ceil(metrics.cols);
            metrics.rows = total_rows.max(1).clamp(1, PAGE_PANEL_MAX_ROWS);
            metrics.visible_count = metrics.count.min(metrics.rows.saturating_mul(metrics.cols));
            metrics.height = PAGE_PANEL_PADDING_X * 2.0
                + metrics.rows as f64 * page_row_height
                + (metrics.rows.saturating_sub(1) as f64) * PAGE_THUMB_GAP
                + footer_height;
            panel_height = panel_height.max(metrics.height);
            metrics.enabled = true;
            metrics.board_index = Some(board_index);
        }

        (metrics, panel_height)
    }
}
