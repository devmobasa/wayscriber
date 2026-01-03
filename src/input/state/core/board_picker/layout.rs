use cairo::Context as CairoContext;

use crate::config::Action;
use crate::draw::Color;
use crate::util::Rect;

use super::super::base::InputState;
use super::{
    BOARD_PICKER_RECENT_LINE_HEIGHT, BODY_FONT_SIZE, BoardPickerEditMode, BoardPickerLayout,
    BoardPickerState, COLUMN_GAP, FOOTER_HEIGHT, HEADER_HEIGHT, PADDING_X, PADDING_Y,
    PALETTE_BOTTOM_GAP, PALETTE_SWATCH_GAP, PALETTE_SWATCH_SIZE, PALETTE_TOP_GAP, ROW_HEIGHT,
    SWATCH_PADDING, SWATCH_SIZE, TITLE_FONT_SIZE, board_palette_colors,
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

        let title = self.board_picker_title(board_count, max_count);
        let footer = self.board_picker_footer_text();
        let recent_label = self.board_picker_recent_label();

        let _ = ctx.save();
        ctx.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Bold);
        ctx.set_font_size(TITLE_FONT_SIZE);
        let title_width = text_width(ctx, &title);
        ctx.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Normal);
        ctx.set_font_size(BODY_FONT_SIZE);
        let footer_width = text_width(ctx, &footer);
        let recent_width = recent_label
            .as_deref()
            .map(|label| text_width(ctx, label))
            .unwrap_or(0.0);

        let mut max_name_width: f64 = 0.0;
        let mut max_hint_width: f64 = 0.0;

        let edit_state = self.board_picker_edit_state();
        let show_hints = !self.board_picker_is_quick();

        for index in 0..row_count {
            let (label, hint) = if index < board_count {
                let board = &self.boards.board_states()[index];
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
                        _ => board_slot_hint(self, index),
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

            max_name_width = max_name_width.max(text_width(ctx, &label));
            if let Some(hint) = hint {
                max_hint_width = max_hint_width.max(text_width(ctx, &hint));
            }
        }

        let _ = ctx.restore();

        let mut content_width = SWATCH_SIZE + SWATCH_PADDING + max_name_width;
        if max_hint_width > 0.0 {
            content_width += COLUMN_GAP + max_hint_width;
        }

        let mut panel_width = PADDING_X * 2.0 + content_width;
        panel_width = panel_width.max(title_width + PADDING_X * 2.0);
        panel_width = panel_width.max(footer_width + PADDING_X * 2.0);
        panel_width = panel_width.max(recent_width + PADDING_X * 2.0);

        let mut palette_rows = 0usize;
        let mut palette_cols = 0usize;
        let mut palette_height = 0.0;
        if let Some((BoardPickerEditMode::Color, edit_index, _)) = edit_state
            && edit_index < board_count
            && self
                .boards
                .board_states()
                .get(edit_index)
                .map(|board| !board.spec.background.is_transparent())
                .unwrap_or(false)
        {
            let colors = board_palette_colors();
            if !colors.is_empty() {
                let available_width = panel_width - PADDING_X * 2.0;
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
            BOARD_PICKER_RECENT_LINE_HEIGHT
        } else {
            0.0
        };
        let footer_height = FOOTER_HEIGHT + recent_height;

        let panel_height = PADDING_Y * 2.0
            + HEADER_HEIGHT
            + ROW_HEIGHT * row_count as f64
            + palette_extra
            + footer_height;

        let mut origin_x = (screen_width as f64 - panel_width) * 0.5;
        let mut origin_y = (screen_height as f64 - panel_height) * 0.5;
        origin_x = origin_x.max(8.0);
        origin_y = origin_y.max(8.0);

        let palette_top = if palette_rows > 0 {
            origin_y + PADDING_Y + HEADER_HEIGHT + ROW_HEIGHT * row_count as f64 + PALETTE_TOP_GAP
        } else {
            0.0
        };

        self.board_picker_layout = Some(BoardPickerLayout {
            origin_x,
            origin_y,
            width: panel_width,
            height: panel_height,
            row_height: ROW_HEIGHT,
            header_height: HEADER_HEIGHT,
            footer_height,
            padding_x: PADDING_X,
            padding_y: PADDING_Y,
            swatch_size: SWATCH_SIZE,
            swatch_padding: SWATCH_PADDING,
            hint_width: max_hint_width,
            row_count,
            palette_top,
            palette_rows,
            palette_cols,
            recent_height,
        });

        if let Some(layout) = self.board_picker_layout {
            self.mark_board_picker_region(layout);
        }
    }

    pub(crate) fn board_picker_index_at(&self, x: i32, y: i32) -> Option<usize> {
        let layout = self.board_picker_layout?;
        let local_x = x as f64 - layout.origin_x;
        let local_y = y as f64 - layout.origin_y;
        if local_x < 0.0 || local_y < 0.0 || local_x > layout.width || local_y > layout.height {
            return None;
        }
        let rows_top = layout.origin_y + layout.padding_y + layout.header_height;
        let row_y = y as f64 - rows_top;
        if row_y < 0.0 {
            return None;
        }
        let row = (row_y / layout.row_height).floor() as isize;
        if row < 0 {
            return None;
        }
        let index = row as usize;
        if index >= layout.row_count {
            None
        } else {
            Some(index)
        }
    }

    pub(crate) fn board_picker_contains_point(&self, x: i32, y: i32) -> bool {
        let Some(layout) = self.board_picker_layout else {
            return false;
        };
        let local_x = x as f64 - layout.origin_x;
        let local_y = y as f64 - layout.origin_y;
        local_x >= 0.0 && local_y >= 0.0 && local_x <= layout.width && local_y <= layout.height
    }

    pub(crate) fn board_picker_swatch_index_at(&self, x: i32, y: i32) -> Option<usize> {
        let layout = self.board_picker_layout?;
        let board_count = self.boards.board_count();
        let rows_top = layout.origin_y + layout.padding_y + layout.header_height;
        for row in 0..board_count {
            let row_top = rows_top + layout.row_height * row as f64;
            let row_center = row_top + layout.row_height * 0.5;
            let swatch_x = layout.origin_x + layout.padding_x;
            let swatch_y = row_center - layout.swatch_size * 0.5;
            let within_x = (x as f64) >= swatch_x && (x as f64) <= swatch_x + layout.swatch_size;
            let within_y = (y as f64) >= swatch_y && (y as f64) <= swatch_y + layout.swatch_size;
            if within_x && within_y {
                return Some(row);
            }
        }
        None
    }

    pub(crate) fn board_picker_palette_color_at(&self, x: i32, y: i32) -> Option<Color> {
        let layout = self.board_picker_layout?;
        if layout.palette_rows == 0 || layout.palette_cols == 0 {
            return None;
        }
        let palette = board_palette_colors();
        if palette.is_empty() {
            return None;
        }
        let origin_x = layout.origin_x + layout.padding_x;
        let origin_y = layout.palette_top;
        let local_x = x as f64 - origin_x;
        let local_y = y as f64 - origin_y;
        if local_x < 0.0 || local_y < 0.0 {
            return None;
        }
        let cell = PALETTE_SWATCH_SIZE + PALETTE_SWATCH_GAP;
        let col = (local_x / cell).floor() as usize;
        let row = (local_y / cell).floor() as usize;
        if col >= layout.palette_cols || row >= layout.palette_rows {
            return None;
        }
        let within_x = local_x - col as f64 * cell <= PALETTE_SWATCH_SIZE;
        let within_y = local_y - row as f64 * cell <= PALETTE_SWATCH_SIZE;
        if !within_x || !within_y {
            return None;
        }
        let index = row * layout.palette_cols + col;
        palette.get(index).copied()
    }

    pub(crate) fn update_board_picker_hover_from_pointer(&mut self, x: i32, y: i32) {
        if !self.is_board_picker_open() {
            return;
        }
        let hover = self.board_picker_index_at(x, y);
        if let BoardPickerState::Open { hover_index, .. } = &mut self.board_picker_state
            && *hover_index != hover
        {
            *hover_index = hover;
            self.needs_redraw = true;
        }
    }

    pub(super) fn mark_board_picker_region(&mut self, layout: BoardPickerLayout) {
        let x = layout.origin_x.floor() as i32;
        let y = layout.origin_y.floor() as i32;
        let width = layout.width.ceil() as i32 + 2;
        let height = layout.height.ceil() as i32 + 2;
        if let Some(rect) = Rect::new(x, y, width.max(1), height.max(1)) {
            self.dirty_tracker.mark_rect(rect);
        } else {
            self.dirty_tracker.mark_full();
        }
    }
}

fn text_width(ctx: &CairoContext, text: &str) -> f64 {
    match ctx.text_extents(text) {
        Ok(extents) => extents.width(),
        Err(_) => text.len() as f64 * BODY_FONT_SIZE * 0.5,
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
