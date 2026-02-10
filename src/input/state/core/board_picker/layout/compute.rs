use cairo::Context as CairoContext;

use crate::config::Action;

use super::super::super::base::InputState;
use super::super::{
    BOARD_PICKER_RECENT_LINE_HEIGHT, BOARD_PICKER_RECENT_LINE_HEIGHT_COMPACT, BODY_FONT_SIZE,
    BoardPickerEditMode, BoardPickerLayout, COMPACT_BODY_FONT_SIZE, COMPACT_FOOTER_FONT_SIZE,
    COMPACT_FOOTER_HEIGHT, COMPACT_HEADER_HEIGHT, COMPACT_PADDING_X, COMPACT_PADDING_Y,
    COMPACT_ROW_HEIGHT, COMPACT_SWATCH_PADDING, COMPACT_SWATCH_SIZE, COMPACT_TITLE_FONT_SIZE,
    FOOTER_FONT_SIZE, FOOTER_HEIGHT, HANDLE_GAP, HANDLE_WIDTH, HEADER_HEIGHT, OPEN_ICON_GAP,
    OPEN_ICON_SIZE, PADDING_X, PADDING_Y, PAGE_PANEL_GAP, PAGE_PANEL_MAX_COLS, PAGE_PANEL_MAX_ROWS,
    PAGE_PANEL_PADDING_X, PAGE_THUMB_GAP, PAGE_THUMB_HEIGHT, PAGE_THUMB_MAX_WIDTH,
    PAGE_THUMB_MIN_WIDTH, PALETTE_BOTTOM_GAP, PALETTE_SWATCH_GAP, PALETTE_SWATCH_SIZE,
    PALETTE_TOP_GAP, ROW_HEIGHT, SWATCH_PADDING, SWATCH_SIZE, TITLE_FONT_SIZE,
    board_palette_colors,
};

impl InputState {
    pub(crate) fn board_picker_layout(&self) -> Option<&BoardPickerLayout> {
        self.board_picker_layout.as_ref()
    }

    pub(crate) fn clear_board_picker_layout(&mut self) {
        self.board_picker_layout = None;
    }

    pub(crate) fn update_board_picker_layout(
        &mut self,
        ctx: &CairoContext,
        screen_width: u32,
        screen_height: u32,
    ) {
        if !self.is_board_picker_open() {
            self.board_picker_layout = None;
            return;
        }

        let row_count = self.board_picker_row_count();
        if row_count == 0 {
            self.board_picker_layout = None;
            return;
        }

        let board_count = self.boards.board_count();
        let max_count = self.boards.max_count();

        let (
            title_font_size,
            body_font_size,
            footer_font_size,
            row_height,
            header_height,
            base_footer_height,
            padding_x,
            padding_y,
            swatch_size,
            swatch_padding,
            recent_line_height,
            handle_width,
            handle_gap,
            open_icon_size,
            open_icon_gap,
        ) = if self.board_picker_is_quick() {
            (
                COMPACT_TITLE_FONT_SIZE,
                COMPACT_BODY_FONT_SIZE,
                COMPACT_FOOTER_FONT_SIZE,
                COMPACT_ROW_HEIGHT,
                COMPACT_HEADER_HEIGHT,
                COMPACT_FOOTER_HEIGHT,
                COMPACT_PADDING_X,
                COMPACT_PADDING_Y,
                COMPACT_SWATCH_SIZE,
                COMPACT_SWATCH_PADDING,
                BOARD_PICKER_RECENT_LINE_HEIGHT_COMPACT,
                0.0,
                0.0,
                0.0,
                0.0,
            )
        } else {
            (
                TITLE_FONT_SIZE,
                BODY_FONT_SIZE,
                FOOTER_FONT_SIZE,
                ROW_HEIGHT,
                HEADER_HEIGHT,
                FOOTER_HEIGHT,
                PADDING_X,
                PADDING_Y,
                SWATCH_SIZE,
                SWATCH_PADDING,
                BOARD_PICKER_RECENT_LINE_HEIGHT,
                HANDLE_WIDTH,
                HANDLE_GAP,
                OPEN_ICON_SIZE,
                OPEN_ICON_GAP,
            )
        };

        let title = self.board_picker_title(board_count, max_count);
        let footer = self.board_picker_footer_text();
        let recent_label = self.board_picker_recent_label();

        let _ = ctx.save();
        ctx.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Bold);
        ctx.set_font_size(title_font_size);
        let title_width = text_width(ctx, &title, title_font_size);
        ctx.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Normal);
        ctx.set_font_size(footer_font_size);
        let footer_width = text_width(ctx, &footer, footer_font_size);
        let recent_width = recent_label
            .as_deref()
            .map(|label| text_width(ctx, label, footer_font_size))
            .unwrap_or(0.0);

        let mut max_name_width: f64 = 0.0;
        let mut max_hint_width: f64 = 0.0;

        let edit_state = self.board_picker_edit_state();
        let show_hints = !self.board_picker_is_quick();

        for index in 0..row_count {
            let (label, hint) = if index < board_count {
                let board_index = self
                    .board_picker_board_index_for_row(index)
                    .unwrap_or(index);
                let board = &self.boards.board_states()[board_index];
                let label = match edit_state {
                    Some((BoardPickerEditMode::Name, edit_index, buffer))
                        if edit_index == index =>
                    {
                        buffer.to_string()
                    }
                    _ => board.spec.name.clone(),
                };
                let hint = if show_hints {
                    match edit_state {
                        Some((BoardPickerEditMode::Color, edit_index, buffer))
                            if edit_index == index =>
                        {
                            Some(buffer.to_string())
                        }
                        _ => board_slot_hint(self, board_index),
                    }
                } else {
                    None
                };
                (label, hint)
            } else {
                let label = if board_count >= max_count {
                    "New board (max reached)".to_string()
                } else {
                    "New board".to_string()
                };
                (label, None)
            };

            max_name_width = max_name_width.max(text_width(ctx, &label, body_font_size));
            if let Some(hint) = hint {
                max_hint_width = max_hint_width.max(text_width(ctx, &hint, body_font_size));
            }
        }

        let _ = ctx.restore();

        let mut content_width = swatch_size + swatch_padding + max_name_width;
        if max_hint_width > 0.0 {
            content_width += super::super::COLUMN_GAP + max_hint_width;
        }
        if handle_width > 0.0 {
            content_width += handle_gap + handle_width;
            if open_icon_size > 0.0 {
                content_width += open_icon_gap + open_icon_size;
            }
        }

        let mut list_width = padding_x * 2.0 + content_width;
        list_width = list_width.max(title_width + padding_x * 2.0);
        list_width = list_width.max(footer_width + padding_x * 2.0);
        list_width = list_width.max(recent_width + padding_x * 2.0);

        let mut palette_rows = 0usize;
        let mut palette_cols = 0usize;
        let mut palette_height = 0.0;
        if let Some((BoardPickerEditMode::Color, edit_index, _)) = edit_state
            && edit_index < board_count
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
                palette_cols = max_cols.clamp(1, colors.len());
                palette_rows = colors.len().div_ceil(palette_cols);
                palette_height = palette_rows as f64 * PALETTE_SWATCH_SIZE
                    + (palette_rows.saturating_sub(1) as f64) * PALETTE_SWATCH_GAP;
            }
        }

        let palette_extra = if palette_rows > 0 {
            PALETTE_TOP_GAP + palette_height + PALETTE_BOTTOM_GAP
        } else {
            0.0
        };

        let recent_height = if recent_label.is_some() {
            recent_line_height
        } else {
            0.0
        };
        let footer_height = base_footer_height + recent_height;

        let panel_height = padding_y * 2.0
            + header_height
            + row_height * row_count as f64
            + palette_extra
            + footer_height;
        let mut panel_height = panel_height;

        let mut page_panel_enabled = false;
        let mut page_panel_width = 0.0;
        let mut page_panel_height = 0.0;
        let mut page_thumb_width = 0.0;
        let page_thumb_height = PAGE_THUMB_HEIGHT;
        let page_thumb_gap = PAGE_THUMB_GAP;
        let page_row_height =
            page_thumb_height + super::super::PAGE_NAME_HEIGHT + super::super::PAGE_NAME_PADDING;
        let mut page_cols = 0usize;
        let mut page_rows = 0usize;
        let mut page_count = 0usize;
        let mut page_visible_count = 0usize;
        let mut page_board_index = None;

        if !self.board_picker_is_quick()
            && let Some(board_index) = self.board_picker_page_panel_board_index()
            && let Some(board) = self.boards.board_states().get(board_index)
        {
            page_count = board.pages.page_count();
            // Always show page panel (even empty state with 0 pages for "Add first page" CTA)
            let aspect = screen_width as f64 / screen_height as f64;
            let base_thumb_width =
                (page_thumb_height * aspect).clamp(PAGE_THUMB_MIN_WIDTH, PAGE_THUMB_MAX_WIDTH);

            let available_right =
                (screen_width as f64 - (PAGE_PANEL_GAP + 32.0)).max(base_thumb_width + 32.0);
            let mut candidate_cols = PAGE_PANEL_MAX_COLS.max(1);
            loop {
                let candidate_width = PAGE_PANEL_PADDING_X * 2.0
                    + candidate_cols as f64 * base_thumb_width
                    + (candidate_cols.saturating_sub(1) as f64) * page_thumb_gap;
                if candidate_width <= available_right || candidate_cols == 1 {
                    page_cols = candidate_cols;
                    page_panel_width = candidate_width.min(available_right);
                    break;
                }
                candidate_cols -= 1;
            }

            if page_cols == 0 {
                page_cols = 1;
            }

            page_thumb_width = ((page_panel_width
                - PAGE_PANEL_PADDING_X * 2.0
                - (page_cols.saturating_sub(1) as f64) * page_thumb_gap)
                / page_cols as f64)
                .clamp(PAGE_THUMB_MIN_WIDTH, PAGE_THUMB_MAX_WIDTH);

            let total_rows = page_count.max(1).div_ceil(page_cols);
            page_rows = total_rows.max(1).clamp(1, PAGE_PANEL_MAX_ROWS);
            page_visible_count = page_count.min(page_rows.saturating_mul(page_cols));
            page_panel_height = PAGE_PANEL_PADDING_X * 2.0
                + page_rows as f64 * page_row_height
                + (page_rows.saturating_sub(1) as f64) * page_thumb_gap
                + footer_height;
            panel_height = panel_height.max(page_panel_height);
            page_panel_enabled = true;
            page_board_index = Some(board_index);
        }

        let total_width = if page_panel_enabled {
            list_width + PAGE_PANEL_GAP + page_panel_width
        } else {
            list_width
        };

        let max_width = (screen_width as f64 - 40.0).max(220.0);
        let final_total_width = total_width.min(max_width);

        let mut final_list_width = list_width;
        if page_panel_enabled {
            let available_for_list =
                (final_total_width - PAGE_PANEL_GAP - page_panel_width).max(180.0);
            final_list_width = final_list_width.min(available_for_list);
        } else {
            final_list_width = final_total_width;
        }

        let final_total_width = if page_panel_enabled {
            final_list_width + PAGE_PANEL_GAP + page_panel_width
        } else {
            final_list_width
        };

        let origin_x = (screen_width as f64 - final_total_width) * 0.5;
        let origin_y = (screen_height as f64 - panel_height) * 0.5;

        let page_panel_x = if page_panel_enabled {
            origin_x + final_list_width + PAGE_PANEL_GAP
        } else {
            0.0
        };
        let page_panel_y = origin_y;

        self.board_picker_layout = Some(BoardPickerLayout {
            origin_x,
            origin_y,
            width: final_total_width,
            height: panel_height,
            list_width: final_list_width,
            title_font_size,
            body_font_size,
            footer_font_size,
            row_height,
            header_height,
            footer_height,
            padding_x,
            padding_y,
            swatch_size,
            swatch_padding,
            hint_width: max_hint_width,
            row_count,
            palette_top: origin_y
                + padding_y
                + header_height
                + row_height * row_count as f64
                + PALETTE_TOP_GAP,
            palette_rows,
            palette_cols,
            recent_height,
            handle_width,
            handle_gap,
            open_icon_size,
            open_icon_gap,
            page_panel_enabled,
            page_panel_x,
            page_panel_y,
            page_panel_width,
            page_panel_height,
            page_thumb_width,
            page_thumb_height,
            page_thumb_gap,
            page_cols,
            page_rows,
            page_max_rows: PAGE_PANEL_MAX_ROWS,
            page_count,
            page_visible_count,
            page_board_index,
        });
    }
}

fn text_width(ctx: &CairoContext, text: &str, font_size: f64) -> f64 {
    match ctx.text_extents(text) {
        Ok(extents) => extents.width(),
        Err(_) => text.len() as f64 * font_size * 0.5,
    }
}

fn board_slot_hint(state: &InputState, index: usize) -> Option<String> {
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
