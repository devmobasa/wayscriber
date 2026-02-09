use crate::draw::Color;

use super::super::super::base::InputState;
use super::super::{
    PAGE_DELETE_ICON_MARGIN, PAGE_DELETE_ICON_SIZE, PAGE_NAME_HEIGHT, PAGE_NAME_PADDING,
    PAGE_PANEL_PADDING_X,
};

#[derive(Debug, Clone, Copy)]
struct FloatRect {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

impl FloatRect {
    fn contains(self, x: f64, y: f64) -> bool {
        x >= self.x && x <= self.x + self.w && y >= self.y && y <= self.y + self.h
    }
}

impl InputState {
    pub(crate) fn board_picker_index_at(&self, x: i32, y: i32) -> Option<usize> {
        let layout = self.board_picker_layout?;
        let local_x = x as f64 - layout.origin_x;
        let local_y = y as f64 - layout.origin_y;
        let row_top = layout.padding_y + layout.header_height;
        let row_bottom = row_top + layout.row_height * layout.row_count as f64;
        if local_x < 0.0 || local_y < 0.0 || local_x > layout.list_width {
            return None;
        }
        if local_y < row_top || local_y >= row_bottom {
            return None;
        }
        let row = ((local_y - row_top) / layout.row_height).floor() as usize;
        if row < layout.row_count {
            Some(row)
        } else {
            None
        }
    }

    pub(crate) fn board_picker_contains_point(&self, x: i32, y: i32) -> bool {
        if let Some(layout) = self.board_picker_layout {
            let rect = FloatRect {
                x: layout.origin_x,
                y: layout.origin_y,
                w: layout.width,
                h: layout.height,
            };
            rect.contains(x as f64, y as f64)
        } else {
            false
        }
    }

    pub(crate) fn board_picker_swatch_index_at(&self, x: i32, y: i32) -> Option<usize> {
        let row = self.board_picker_index_at(x, y)?;
        if self.board_picker_is_new_row(row) {
            return None;
        }
        let layout = self.board_picker_layout?;
        let local_x = x as f64 - layout.origin_x;
        let row_top = layout.padding_y + layout.header_height + row as f64 * layout.row_height;
        let swatch_y = row_top + (layout.row_height - layout.swatch_size) * 0.5;
        let swatch_x = layout.padding_x;
        let rect = FloatRect {
            x: swatch_x,
            y: swatch_y,
            w: layout.swatch_size,
            h: layout.swatch_size,
        };
        if rect.contains(local_x, y as f64 - layout.origin_y) {
            Some(row)
        } else {
            None
        }
    }

    pub(crate) fn board_picker_palette_color_at(&self, x: i32, y: i32) -> Option<Color> {
        let layout = self.board_picker_layout?;
        if layout.palette_rows == 0 || layout.palette_cols == 0 {
            return None;
        }
        let origin_x = layout.origin_x + layout.padding_x;
        let origin_y = layout.palette_top;
        let local_x = x as f64 - origin_x;
        let local_y = y as f64 - origin_y;
        if local_x < 0.0 || local_y < 0.0 {
            return None;
        }
        let cell = super::super::PALETTE_SWATCH_SIZE + super::super::PALETTE_SWATCH_GAP;
        let col = (local_x / cell).floor() as usize;
        let row = (local_y / cell).floor() as usize;
        if row >= layout.palette_rows || col >= layout.palette_cols {
            return None;
        }
        let within_x = local_x - col as f64 * cell;
        let within_y = local_y - row as f64 * cell;
        if within_x > super::super::PALETTE_SWATCH_SIZE
            || within_y > super::super::PALETTE_SWATCH_SIZE
        {
            return None;
        }
        let index = row * layout.palette_cols + col;
        super::super::board_palette_colors().get(index).copied()
    }

    pub(crate) fn board_picker_page_index_at(&self, x: i32, y: i32) -> Option<usize> {
        let layout = self.board_picker_layout?;
        let board_index = layout.page_board_index?;
        let info = self.board_picker_page_panel_info(layout, board_index)?;
        if info.visible_pages == 0 {
            return None;
        }

        let local_x = x as f64;
        let local_y = y as f64;
        for index in 0..info.visible_pages {
            let Some((_info, _row, _col, thumb_x, thumb_y)) =
                self.board_picker_page_thumb_origin(layout, board_index, index)
            else {
                continue;
            };
            let thumb_rect = FloatRect {
                x: thumb_x,
                y: thumb_y,
                w: layout.page_thumb_width,
                h: layout.page_thumb_height,
            };
            if thumb_rect.contains(local_x, local_y) {
                return Some(index);
            }
        }

        None
    }

    pub(crate) fn board_picker_page_name_index_at(&self, x: i32, y: i32) -> Option<usize> {
        let layout = self.board_picker_layout?;
        let board_index = layout.page_board_index?;
        let info = self.board_picker_page_panel_info(layout, board_index)?;
        if info.visible_pages == 0 {
            return None;
        }

        let local_x = x as f64;
        let local_y = y as f64;
        for index in 0..info.visible_pages {
            let Some((_info, _row, _col, thumb_x, thumb_y)) =
                self.board_picker_page_thumb_origin(layout, board_index, index)
            else {
                continue;
            };
            let label_rect = FloatRect {
                x: thumb_x,
                y: thumb_y + layout.page_thumb_height + PAGE_NAME_PADDING,
                w: layout.page_thumb_width,
                h: PAGE_NAME_HEIGHT,
            };
            if label_rect.contains(local_x, local_y) {
                return Some(index);
            }
        }

        None
    }

    pub(crate) fn board_picker_page_add_button_at(&self, x: i32, y: i32) -> bool {
        self.board_picker_page_add_card_at(x, y)
    }

    pub(crate) fn board_picker_page_add_card_at(&self, x: i32, y: i32) -> bool {
        let Some(layout) = self.board_picker_layout else {
            return false;
        };
        let Some(board_index) = layout.page_board_index else {
            return false;
        };
        let Some(info) = self.board_picker_page_panel_info(layout, board_index) else {
            return false;
        };

        let index = info.visible_pages;
        let add_col = index % info.cols;
        let add_row = index / info.cols;
        if add_row >= layout.page_max_rows.max(1) {
            return false;
        }

        let row_stride = Self::board_picker_page_row_stride(layout);
        let thumb_x = layout.page_panel_x
            + PAGE_PANEL_PADDING_X
            + add_col as f64 * (layout.page_thumb_width + layout.page_thumb_gap);
        let thumb_y = layout.page_panel_y + add_row as f64 * row_stride;
        let thumb_rect = FloatRect {
            x: thumb_x,
            y: thumb_y,
            w: layout.page_thumb_width,
            h: layout.page_thumb_height,
        };
        thumb_rect.contains(x as f64, y as f64)
    }

    pub(crate) fn board_picker_page_overflow_at(&self, x: i32, y: i32) -> bool {
        let Some(layout) = self.board_picker_layout else {
            return false;
        };
        let Some(board_index) = layout.page_board_index else {
            return false;
        };
        let Some(info) = self.board_picker_page_panel_info(layout, board_index) else {
            return false;
        };
        if info.page_count <= info.visible_pages {
            return false;
        }

        let hint_x = layout.page_panel_x + PAGE_PANEL_PADDING_X;
        let hint_y = layout.page_panel_y + layout.page_panel_height + layout.footer_font_size + 6.0;
        let hint_rect = FloatRect {
            x: hint_x,
            y: hint_y - layout.footer_font_size,
            w: layout.page_panel_width - PAGE_PANEL_PADDING_X * 2.0,
            h: layout.footer_font_size + 8.0,
        };
        hint_rect.contains(x as f64, y as f64)
    }

    pub(crate) fn board_picker_page_delete_index_at(&self, x: i32, y: i32) -> Option<usize> {
        let layout = self.board_picker_layout?;
        let board_index = layout.page_board_index?;
        let info = self.board_picker_page_panel_info(layout, board_index)?;
        if info.visible_pages == 0 {
            return None;
        }

        let local_x = x as f64;
        let local_y = y as f64;
        for index in 0..info.visible_pages {
            let Some((_info, _row, _col, thumb_x, thumb_y)) =
                self.board_picker_page_thumb_origin(layout, board_index, index)
            else {
                continue;
            };
            let rect = FloatRect {
                x: thumb_x + layout.page_thumb_width
                    - PAGE_DELETE_ICON_SIZE
                    - PAGE_DELETE_ICON_MARGIN,
                y: thumb_y + layout.page_thumb_height
                    - PAGE_DELETE_ICON_SIZE
                    - PAGE_DELETE_ICON_MARGIN,
                w: PAGE_DELETE_ICON_SIZE,
                h: PAGE_DELETE_ICON_SIZE,
            };
            if rect.contains(local_x, local_y) {
                return Some(index);
            }
        }

        None
    }

    pub(crate) fn board_picker_page_duplicate_index_at(&self, x: i32, y: i32) -> Option<usize> {
        let layout = self.board_picker_layout?;
        let board_index = layout.page_board_index?;
        let info = self.board_picker_page_panel_info(layout, board_index)?;
        if info.visible_pages == 0 {
            return None;
        }

        let local_x = x as f64;
        let local_y = y as f64;
        for index in 0..info.visible_pages {
            let Some((_info, _row, _col, thumb_x, thumb_y)) =
                self.board_picker_page_thumb_origin(layout, board_index, index)
            else {
                continue;
            };
            let rect = FloatRect {
                x: thumb_x + layout.page_thumb_width * 0.5 - PAGE_DELETE_ICON_SIZE * 0.5,
                y: thumb_y + layout.page_thumb_height
                    - PAGE_DELETE_ICON_SIZE
                    - PAGE_DELETE_ICON_MARGIN,
                w: PAGE_DELETE_ICON_SIZE,
                h: PAGE_DELETE_ICON_SIZE,
            };
            if rect.contains(local_x, local_y) {
                return Some(index);
            }
        }

        None
    }

    pub(crate) fn board_picker_page_rename_index_at(&self, x: i32, y: i32) -> Option<usize> {
        let layout = self.board_picker_layout?;
        let board_index = layout.page_board_index?;
        let info = self.board_picker_page_panel_info(layout, board_index)?;
        if info.visible_pages == 0 {
            return None;
        }

        let local_x = x as f64;
        let local_y = y as f64;
        for index in 0..info.visible_pages {
            let Some((_info, _row, _col, thumb_x, thumb_y)) =
                self.board_picker_page_thumb_origin(layout, board_index, index)
            else {
                continue;
            };
            let rect = FloatRect {
                x: thumb_x + PAGE_DELETE_ICON_MARGIN,
                y: thumb_y + layout.page_thumb_height
                    - PAGE_DELETE_ICON_SIZE
                    - PAGE_DELETE_ICON_MARGIN,
                w: PAGE_DELETE_ICON_SIZE,
                h: PAGE_DELETE_ICON_SIZE,
            };
            if rect.contains(local_x, local_y) {
                return Some(index);
            }
        }

        None
    }

    pub(crate) fn board_picker_page_handle_index_at(&self, x: i32, y: i32) -> Option<usize> {
        let layout = self.board_picker_layout?;
        let board_index = layout.page_board_index?;
        let info = self.board_picker_page_panel_info(layout, board_index)?;
        if info.visible_pages == 0 {
            return None;
        }

        let local_x = x as f64;
        let local_y = y as f64;
        for index in 0..info.visible_pages {
            let Some((_info, _row, _col, thumb_x, thumb_y)) =
                self.board_picker_page_thumb_origin(layout, board_index, index)
            else {
                continue;
            };
            let handle_size = (layout.page_thumb_height * 0.22).clamp(8.0, 12.0);
            let handle_rect = FloatRect {
                x: thumb_x + layout.page_thumb_width - handle_size - 4.0,
                y: thumb_y + 4.0,
                w: handle_size,
                h: handle_size,
            };
            if handle_rect.contains(local_x, local_y) {
                return Some(index);
            }
        }

        None
    }

    pub(crate) fn board_picker_handle_index_at(&self, x: i32, y: i32) -> Option<usize> {
        let row = self.board_picker_index_at(x, y)?;
        if self.board_picker_is_new_row(row) {
            return None;
        }
        let layout = self.board_picker_layout?;
        if layout.handle_width <= 0.0 || self.board_picker_is_quick() {
            return None;
        }
        let local_x = x as f64;
        let local_y = y as f64;
        let row_top = layout.origin_y
            + layout.padding_y
            + layout.header_height
            + row as f64 * layout.row_height;
        let list_right = layout.origin_x + layout.list_width;
        let handle_x = list_right - layout.padding_x - layout.handle_width;
        let rect = FloatRect {
            x: handle_x,
            y: row_top + (layout.row_height - layout.handle_width) * 0.5,
            w: layout.handle_width,
            h: layout.handle_width,
        };
        if rect.contains(local_x, local_y) {
            Some(row)
        } else {
            None
        }
    }

    pub(crate) fn board_picker_open_icon_index_at(&self, x: i32, y: i32) -> Option<usize> {
        let row = self.board_picker_index_at(x, y)?;
        if self.board_picker_is_new_row(row) {
            return None;
        }
        let layout = self.board_picker_layout?;
        if layout.open_icon_size <= 0.0 || self.board_picker_is_quick() {
            return None;
        }
        let local_x = x as f64;
        let local_y = y as f64;
        let row_top = layout.origin_y
            + layout.padding_y
            + layout.header_height
            + row as f64 * layout.row_height;
        let list_right = layout.origin_x + layout.list_width;
        let handle_x = list_right - layout.padding_x - layout.handle_width;
        let open_x = handle_x - layout.open_icon_gap - layout.open_icon_size;
        let rect = FloatRect {
            x: open_x,
            y: row_top + (layout.row_height - layout.open_icon_size) * 0.5,
            w: layout.open_icon_size,
            h: layout.open_icon_size,
        };
        if rect.contains(local_x, local_y) {
            Some(row)
        } else {
            None
        }
    }

    pub(crate) fn board_picker_pin_index_at(&self, x: i32, y: i32) -> Option<usize> {
        let row = self.board_picker_index_at(x, y)?;
        if self.board_picker_is_new_row(row) {
            return None;
        }
        let layout = self.board_picker_layout?;
        let local_x = x as f64 - layout.origin_x;
        let local_y = y as f64 - layout.origin_y;
        let row_top = layout.padding_y + layout.header_height + row as f64 * layout.row_height;
        let pin_size = layout.swatch_size * super::super::PIN_OFFSET_FACTOR;
        let pin_x = layout.padding_x + layout.swatch_size + layout.swatch_padding - pin_size * 0.25;
        let pin_y = row_top + (layout.row_height - pin_size) * 0.5 - pin_size * 0.25;
        let rect = FloatRect {
            x: pin_x,
            y: pin_y,
            w: pin_size,
            h: pin_size,
        };
        if rect.contains(local_x, local_y) {
            Some(row)
        } else {
            None
        }
    }
}
