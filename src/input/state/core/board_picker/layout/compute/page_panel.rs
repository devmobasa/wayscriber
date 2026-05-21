use super::super::super::super::base::InputState;
use super::super::super::{
    PAGE_NAME_HEIGHT, PAGE_NAME_PADDING, PAGE_PANEL_ADD_BUTTON_GAP, PAGE_PANEL_ADD_BUTTON_HEIGHT,
    PAGE_PANEL_GAP, PAGE_PANEL_HEADER_HEIGHT, PAGE_PANEL_MAX_COLS, PAGE_PANEL_MAX_ROWS,
    PAGE_PANEL_PADDING_X, PAGE_THUMB_GAP, PAGE_THUMB_HEIGHT, PAGE_THUMB_MAX_WIDTH,
    PAGE_THUMB_MIN_WIDTH,
};
use super::BoardPickerPagePanelMetrics;

impl InputState {
    pub(super) fn compute_board_picker_page_panel_metrics(
        &mut self,
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
            viewport_x: 0.0,
            viewport_y: 0.0,
            viewport_width: 0.0,
            viewport_height: 0.0,
            add_button_x: 0.0,
            add_button_y: 0.0,
            add_button_width: 0.0,
            add_button_height: 0.0,
            thumb_width: 0.0,
            cols: 0,
            rows: 0,
            total_rows: 0,
            scroll_row: 0,
            max_scroll_row: 0,
            first_visible_index: 0,
            visible_slots: 0,
            count: 0,
            visible_count: 0,
            board_index: None,
        };

        if !self.board_picker_is_quick()
            && let Some(board_index) = self.board_picker_page_panel_board_index()
            && let Some(board) = self.boards.board_states().get(board_index)
        {
            metrics.count = board.pages.page_count();
            metrics.board_index = Some(board_index);
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

            metrics.total_rows = metrics.count.max(1).div_ceil(metrics.cols);
            metrics.rows = metrics.total_rows.max(1).clamp(1, PAGE_PANEL_MAX_ROWS);
            metrics.max_scroll_row = metrics.total_rows.saturating_sub(metrics.rows);
            self.clamp_board_picker_page_panel_state(&mut metrics);

            metrics.viewport_x = PAGE_PANEL_PADDING_X;
            metrics.viewport_y = PAGE_PANEL_PADDING_X + PAGE_PANEL_HEADER_HEIGHT;
            metrics.viewport_width = metrics.width - PAGE_PANEL_PADDING_X * 2.0;
            metrics.viewport_height = metrics.rows as f64 * page_row_height
                + (metrics.rows.saturating_sub(1) as f64) * PAGE_THUMB_GAP;
            metrics.add_button_x = PAGE_PANEL_PADDING_X;
            metrics.add_button_y =
                metrics.viewport_y + metrics.viewport_height + PAGE_PANEL_ADD_BUTTON_GAP;
            metrics.add_button_width = metrics.viewport_width;
            metrics.add_button_height = PAGE_PANEL_ADD_BUTTON_HEIGHT;
            metrics.height = PAGE_PANEL_PADDING_X * 2.0
                + PAGE_PANEL_HEADER_HEIGHT
                + metrics.viewport_height
                + PAGE_PANEL_ADD_BUTTON_GAP
                + PAGE_PANEL_ADD_BUTTON_HEIGHT
                + footer_height;
            panel_height = panel_height.max(metrics.height);
            metrics.enabled = true;
        }

        (metrics, panel_height)
    }

    fn clamp_board_picker_page_panel_state(&mut self, metrics: &mut BoardPickerPagePanelMetrics) {
        let count = metrics.count;
        let cols = metrics.cols.max(1);
        let rows = metrics.rows.max(1);
        let max_scroll_row = metrics.max_scroll_row;
        let Some((scroll_row, focus_page, target_page)) =
            self.board_picker_page_panel_state_parts()
        else {
            metrics.scroll_row = 0;
            metrics.first_visible_index = 0;
            metrics.visible_slots = rows.saturating_mul(cols);
            metrics.visible_count = count.min(metrics.visible_slots);
            return;
        };

        let clamped_focus = focus_page.map(|index| clamp_page_index(index, count));
        let scroll_target = target_page.map(|index| clamp_page_index(index, count));

        let mut next_scroll_row = scroll_row.min(max_scroll_row);
        if let Some(page_index) = scroll_target {
            next_scroll_row =
                scroll_row_for_page(page_index, cols, rows, next_scroll_row, max_scroll_row);
        } else if let Some(page_index) = clamped_focus {
            next_scroll_row =
                scroll_row_for_page(page_index, cols, rows, next_scroll_row, max_scroll_row);
        }

        self.set_board_picker_page_panel_state_parts(next_scroll_row, clamped_focus, None);

        metrics.scroll_row = next_scroll_row;
        metrics.first_visible_index = next_scroll_row.saturating_mul(cols).min(count);
        metrics.visible_slots = rows.saturating_mul(cols);
        metrics.visible_count = count
            .saturating_sub(metrics.first_visible_index)
            .min(metrics.visible_slots);
    }
}

fn clamp_page_index(index: usize, page_count: usize) -> usize {
    if page_count == 0 {
        0
    } else {
        index.min(page_count.saturating_sub(1))
    }
}

fn scroll_row_for_page(
    page_index: usize,
    cols: usize,
    visible_rows: usize,
    current_scroll_row: usize,
    max_scroll_row: usize,
) -> usize {
    let page_row = page_index / cols.max(1);
    let visible_rows = visible_rows.max(1);
    let next = if page_row < current_scroll_row {
        page_row
    } else if page_row >= current_scroll_row.saturating_add(visible_rows) {
        page_row.saturating_add(1).saturating_sub(visible_rows)
    } else {
        current_scroll_row
    };
    next.min(max_scroll_row)
}
