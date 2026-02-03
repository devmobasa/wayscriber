use cairo::Context as CairoContext;

use crate::config::Action;
use crate::draw::Color;
use crate::util::Rect;

use super::super::base::InputState;
use super::{
    BOARD_PICKER_RECENT_LINE_HEIGHT, BOARD_PICKER_RECENT_LINE_HEIGHT_COMPACT, BODY_FONT_SIZE,
    BoardPickerCursorHint, BoardPickerEditMode, BoardPickerLayout, BoardPickerState, COLUMN_GAP,
    COMPACT_BODY_FONT_SIZE, COMPACT_FOOTER_FONT_SIZE, COMPACT_FOOTER_HEIGHT, COMPACT_HEADER_HEIGHT,
    COMPACT_PADDING_X, COMPACT_PADDING_Y, COMPACT_ROW_HEIGHT, COMPACT_SWATCH_PADDING,
    COMPACT_SWATCH_SIZE, COMPACT_TITLE_FONT_SIZE, FOOTER_FONT_SIZE, FOOTER_HEIGHT, HANDLE_GAP,
    HANDLE_WIDTH, HEADER_HEIGHT, OPEN_ICON_GAP, OPEN_ICON_SIZE, PADDING_X, PADDING_Y,
    PAGE_PANEL_GAP, PAGE_PANEL_MAX_COLS, PAGE_PANEL_MAX_ROWS, PAGE_PANEL_PADDING_X, PAGE_THUMB_GAP,
    PAGE_THUMB_HEIGHT, PAGE_THUMB_MAX_WIDTH, PAGE_THUMB_MIN_WIDTH, PALETTE_BOTTOM_GAP,
    PALETTE_SWATCH_GAP, PALETTE_SWATCH_SIZE, PALETTE_TOP_GAP, PIN_OFFSET_FACTOR, ROW_HEIGHT,
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
            content_width += COLUMN_GAP + max_hint_width;
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
            if page_count > 0 {
                let aspect = screen_width as f64 / screen_height as f64;
                page_thumb_width =
                    (page_thumb_height * aspect).clamp(PAGE_THUMB_MIN_WIDTH, PAGE_THUMB_MAX_WIDTH);
                let max_panel_width = (screen_width as f64 - 32.0).max(list_width);
                for cols in (1..=PAGE_PANEL_MAX_COLS).rev() {
                    let candidate_width = PAGE_PANEL_PADDING_X * 2.0
                        + page_thumb_width * cols as f64
                        + page_thumb_gap * (cols.saturating_sub(1) as f64);
                    let total_width = list_width + PAGE_PANEL_GAP + candidate_width;
                    if total_width <= max_panel_width || cols == 1 {
                        page_cols = cols;
                        page_panel_width = candidate_width;
                        break;
                    }
                }
                let max_rows = PAGE_PANEL_MAX_ROWS.max(1);
                page_rows = page_count.div_ceil(page_cols).min(max_rows);
                page_panel_height = page_rows as f64 * page_thumb_height
                    + page_thumb_gap * (page_rows.saturating_sub(1) as f64);
                page_visible_count = (page_rows * page_cols).min(page_count);
                page_panel_enabled = true;
                page_board_index = Some(board_index);
            }
        }

        if page_panel_enabled {
            panel_height = panel_height
                .max(padding_y * 2.0 + header_height + page_panel_height + footer_height);
        }

        let mut panel_width = list_width;
        if page_panel_enabled {
            panel_width = list_width + PAGE_PANEL_GAP + page_panel_width;
        }

        let mut origin_x = (screen_width as f64 - panel_width) * 0.5;
        let mut origin_y = (screen_height as f64 - panel_height) * 0.5;
        origin_x = origin_x.max(8.0);
        origin_y = origin_y.max(8.0);

        let palette_top = if palette_rows > 0 {
            origin_y + padding_y + header_height + row_height * row_count as f64 + PALETTE_TOP_GAP
        } else {
            0.0
        };

        self.board_picker_layout = Some(BoardPickerLayout {
            origin_x,
            origin_y,
            width: panel_width,
            height: panel_height,
            list_width,
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
            palette_top,
            palette_rows,
            palette_cols,
            recent_height,
            handle_width,
            handle_gap,
            open_icon_size,
            open_icon_gap,
            page_panel_enabled,
            page_panel_x: if page_panel_enabled {
                origin_x + list_width + PAGE_PANEL_GAP
            } else {
                0.0
            },
            page_panel_y: if page_panel_enabled {
                origin_y + padding_y + header_height
            } else {
                0.0
            },
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

        if let Some(layout) = self.board_picker_layout {
            self.mark_board_picker_region(layout);
        }
    }

    pub(crate) fn board_picker_index_at(&self, x: i32, y: i32) -> Option<usize> {
        let layout = self.board_picker_layout?;
        let list_right = layout.origin_x + layout.list_width;
        if (x as f64) > list_right {
            return None;
        }
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

    pub(crate) fn board_picker_page_index_at(&self, x: i32, y: i32) -> Option<usize> {
        let layout = self.board_picker_layout?;
        if !layout.page_panel_enabled {
            return None;
        }
        let board_index = layout.page_board_index?;
        let page_count = self
            .boards
            .board_states()
            .get(board_index)
            .map(|board| board.pages.page_count())
            .unwrap_or(0);
        if page_count == 0 {
            return None;
        }
        let cols = layout.page_cols.max(1);
        let max_rows = layout.page_max_rows.max(1);
        let rows = page_count.div_ceil(cols).min(max_rows);
        let visible = page_count.min(rows.saturating_mul(cols));
        if visible == 0 {
            return None;
        }
        let local_x = x as f64 - layout.page_panel_x - PAGE_PANEL_PADDING_X;
        let local_y = y as f64 - layout.page_panel_y;
        let grid_width = (layout.page_panel_width - PAGE_PANEL_PADDING_X * 2.0).max(0.0);
        if local_x < 0.0
            || local_y < 0.0
            || local_x > grid_width
            || local_y > layout.page_panel_height
        {
            return None;
        }
        let cell_w = layout.page_thumb_width + layout.page_thumb_gap;
        let cell_h = layout.page_thumb_height + layout.page_thumb_gap;
        let col = (local_x / cell_w).floor() as usize;
        let row = (local_y / cell_h).floor() as usize;
        if col >= cols || row >= rows {
            return None;
        }
        let within_x = local_x - col as f64 * cell_w <= layout.page_thumb_width;
        let within_y = local_y - row as f64 * cell_h <= layout.page_thumb_height;
        if !within_x || !within_y {
            return None;
        }
        let index = row * cols + col;
        if index < visible { Some(index) } else { None }
    }

    pub(crate) fn board_picker_page_handle_index_at(&self, x: i32, y: i32) -> Option<usize> {
        let layout = self.board_picker_layout?;
        if !layout.page_panel_enabled {
            return None;
        }
        let board_index = layout.page_board_index?;
        let page_count = self
            .boards
            .board_states()
            .get(board_index)
            .map(|board| board.pages.page_count())
            .unwrap_or(0);
        if page_count == 0 {
            return None;
        }
        let cols = layout.page_cols.max(1);
        let max_rows = layout.page_max_rows.max(1);
        let rows = page_count.div_ceil(cols).min(max_rows);
        let visible = page_count.min(rows.saturating_mul(cols));
        if visible == 0 {
            return None;
        }
        let handle_size = (layout.page_thumb_height * 0.22).clamp(8.0, 12.0);
        for index in 0..visible {
            let col = index % cols;
            let row = index / cols;
            if row >= rows {
                continue;
            }
            let thumb_x = layout.page_panel_x
                + PAGE_PANEL_PADDING_X
                + col as f64 * (layout.page_thumb_width + layout.page_thumb_gap);
            let thumb_y = layout.page_panel_y
                + row as f64 * (layout.page_thumb_height + layout.page_thumb_gap);
            let handle_x = thumb_x + layout.page_thumb_width - handle_size - 4.0;
            let handle_y = thumb_y + 4.0;
            let within_x = (x as f64) >= handle_x && (x as f64) <= handle_x + handle_size;
            let within_y = (y as f64) >= handle_y && (y as f64) <= handle_y + handle_size;
            if within_x && within_y {
                return Some(index);
            }
        }
        None
    }

    pub(crate) fn board_picker_handle_index_at(&self, x: i32, y: i32) -> Option<usize> {
        let layout = self.board_picker_layout?;
        if layout.handle_width <= 0.0 || self.board_picker_is_quick() {
            return None;
        }
        let list_right = layout.origin_x + layout.list_width;
        let board_count = self.boards.board_count();
        if board_count == 0 {
            return None;
        }
        let rows_top = layout.origin_y + layout.padding_y + layout.header_height;
        let handle_x = list_right - layout.padding_x - layout.handle_width;
        let within_x = (x as f64) >= handle_x && (x as f64) <= handle_x + layout.handle_width;
        if !within_x {
            return None;
        }
        for row in 0..board_count {
            let row_top = rows_top + layout.row_height * row as f64;
            let row_bottom = row_top + layout.row_height;
            if (y as f64) >= row_top && (y as f64) <= row_bottom {
                return Some(row);
            }
        }
        None
    }

    pub(crate) fn board_picker_open_icon_index_at(&self, x: i32, y: i32) -> Option<usize> {
        let layout = self.board_picker_layout?;
        if layout.open_icon_size <= 0.0 || self.board_picker_is_quick() {
            return None;
        }
        let list_right = layout.origin_x + layout.list_width;
        let board_count = self.boards.board_count();
        if board_count == 0 {
            return None;
        }
        let handle_x = list_right - layout.padding_x - layout.handle_width;
        let icon_x = handle_x - layout.open_icon_gap - layout.open_icon_size;
        let within_x = (x as f64) >= icon_x && (x as f64) <= icon_x + layout.open_icon_size;
        if !within_x {
            return None;
        }
        let rows_top = layout.origin_y + layout.padding_y + layout.header_height;
        for row in 0..board_count {
            let row_top = rows_top + layout.row_height * row as f64;
            let row_bottom = row_top + layout.row_height;
            if (y as f64) >= row_top && (y as f64) <= row_bottom {
                return Some(row);
            }
        }
        None
    }

    pub(crate) fn board_picker_pin_index_at(&self, x: i32, y: i32) -> Option<usize> {
        let layout = self.board_picker_layout?;
        let board_count = self.boards.board_count();
        if board_count == 0 {
            return None;
        }
        let rows_top = layout.origin_y + layout.padding_y + layout.header_height;
        let swatch_x = layout.origin_x + layout.padding_x;
        let pin_x = swatch_x - layout.swatch_padding * PIN_OFFSET_FACTOR;
        let radius = (layout.body_font_size * 0.45).max(4.0);
        for row in 0..board_count {
            let row_center = rows_top + layout.row_height * row as f64 + layout.row_height * 0.5;
            let dx = x as f64 - pin_x;
            let dy = y as f64 - row_center;
            if dx * dx + dy * dy <= radius * radius {
                return Some(row);
            }
        }
        None
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

    /// Determine the cursor type for a given point within the board picker.
    /// Returns `None` if the board picker is not open or the point is outside.
    pub(crate) fn board_picker_cursor_hint_at(
        &self,
        x: i32,
        y: i32,
    ) -> Option<BoardPickerCursorHint> {
        if !self.is_board_picker_open() {
            return None;
        }
        let layout = self.board_picker_layout?;

        // Check if point is within the panel
        if !self.board_picker_contains_point(x, y) {
            return None;
        }

        // Check if currently dragging a board (grabbing)
        if self.board_picker_drag.is_some() || self.board_picker_page_drag.is_some() {
            return Some(BoardPickerCursorHint::Grabbing);
        }

        // Check drag handles first (grab cursor)
        if self.board_picker_handle_index_at(x, y).is_some() {
            return Some(BoardPickerCursorHint::Grab);
        }
        if self.board_picker_page_handle_index_at(x, y).is_some() {
            return Some(BoardPickerCursorHint::Grab);
        }
        if self.board_picker_open_icon_index_at(x, y).is_some() {
            return Some(BoardPickerCursorHint::Pointer);
        }

        // Check if in edit mode and hovering over the edit row
        if let Some((edit_mode, edit_index, _)) = self.board_picker_edit_state()
            && let Some(row_index) = self.board_picker_index_at(x, y)
            && row_index == edit_index
        {
            // If editing name or hex, show text cursor over the row
            match edit_mode {
                BoardPickerEditMode::Name => return Some(BoardPickerCursorHint::Text),
                BoardPickerEditMode::Color => {
                    // Check palette first
                    if layout.palette_rows > 0 && self.board_picker_palette_color_at(x, y).is_some()
                    {
                        return Some(BoardPickerCursorHint::Pointer);
                    }
                    // In color edit mode, text cursor for hex input
                    return Some(BoardPickerCursorHint::Text);
                }
            }
        }

        // Check palette swatches (pointer)
        if layout.palette_rows > 0 && self.board_picker_palette_color_at(x, y).is_some() {
            return Some(BoardPickerCursorHint::Pointer);
        }

        // Check color swatches (pointer for color edit)
        if self.board_picker_swatch_index_at(x, y).is_some() {
            return Some(BoardPickerCursorHint::Pointer);
        }

        // Check page thumbnails (pointer)
        if self.board_picker_page_index_at(x, y).is_some() {
            return Some(BoardPickerCursorHint::Pointer);
        }

        // Check pin icons (pointer)
        if self.board_picker_pin_index_at(x, y).is_some() {
            return Some(BoardPickerCursorHint::Pointer);
        }

        // Check board rows (pointer for selection)
        if self.board_picker_index_at(x, y).is_some() {
            return Some(BoardPickerCursorHint::Pointer);
        }

        // Default within the picker panel
        Some(BoardPickerCursorHint::Default)
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
