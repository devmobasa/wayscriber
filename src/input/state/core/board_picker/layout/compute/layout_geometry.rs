use super::super::super::super::base::InputState;
use super::super::super::PAGE_PANEL_GAP;
use super::{BoardPickerLayoutGeometry, BoardPickerPagePanelMetrics};

impl InputState {
    pub(super) fn compute_board_picker_layout_geometry(
        &self,
        screen_width: u32,
        screen_height: u32,
        list_width: f64,
        panel_height: f64,
        page_panel: &BoardPickerPagePanelMetrics,
    ) -> BoardPickerLayoutGeometry {
        let total_width = if page_panel.enabled {
            list_width + PAGE_PANEL_GAP + page_panel.width
        } else {
            list_width
        };
        let max_width = (screen_width as f64 - 40.0).max(220.0);
        let mut final_total_width = total_width.min(max_width);

        let mut final_list_width = list_width;
        if page_panel.enabled {
            let available_for_list =
                (final_total_width - PAGE_PANEL_GAP - page_panel.width).max(180.0);
            final_list_width = final_list_width.min(available_for_list);
        } else {
            final_list_width = final_total_width;
        }

        final_total_width = if page_panel.enabled {
            final_list_width + PAGE_PANEL_GAP + page_panel.width
        } else {
            final_list_width
        };

        let origin_x = (screen_width as f64 - final_total_width) * 0.5;
        let origin_y = (screen_height as f64 - panel_height) * 0.5;
        let page_panel_x = if page_panel.enabled {
            origin_x + final_list_width + PAGE_PANEL_GAP
        } else {
            0.0
        };

        BoardPickerLayoutGeometry {
            origin_x,
            origin_y,
            width: final_total_width,
            list_width: final_list_width,
            page_panel_x,
            page_panel_y: origin_y,
        }
    }
}
