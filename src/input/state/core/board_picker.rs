#![allow(dead_code)]

use cairo::Context as CairoContext;

use crate::config::Action;
use crate::draw::{Color, BLACK, BLUE, GREEN, ORANGE, PINK, RED, WHITE, YELLOW};
use crate::input::BoardBackground;
use crate::util::Rect;

use super::base::{InputState, UiToastKind};

const TITLE_FONT_SIZE: f64 = 17.0;
const BODY_FONT_SIZE: f64 = 14.0;
const ROW_HEIGHT: f64 = 32.0;
const HEADER_HEIGHT: f64 = 28.0;
const FOOTER_HEIGHT: f64 = 22.0;
const PADDING_X: f64 = 16.0;
const PADDING_Y: f64 = 14.0;
const SWATCH_SIZE: f64 = 14.0;
const SWATCH_PADDING: f64 = 10.0;
const COLUMN_GAP: f64 = 12.0;
const MAX_BOARD_NAME_LEN: usize = 40;
const PALETTE_SWATCH_SIZE: f64 = 18.0;
const PALETTE_SWATCH_GAP: f64 = 6.0;
const PALETTE_TOP_GAP: f64 = 8.0;
const PALETTE_BOTTOM_GAP: f64 = 8.0;

#[derive(Debug, Clone)]
pub enum BoardPickerState {
    Hidden,
    Open {
        selected: usize,
        hover_index: Option<usize>,
        edit: Option<BoardPickerEdit>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoardPickerEditMode {
    Name,
    Color,
}

#[derive(Debug, Clone)]
pub struct BoardPickerEdit {
    pub mode: BoardPickerEditMode,
    pub buffer: String,
}

#[derive(Debug, Clone, Copy)]
pub struct BoardPickerLayout {
    pub origin_x: f64,
    pub origin_y: f64,
    pub width: f64,
    pub height: f64,
    pub row_height: f64,
    pub header_height: f64,
    pub footer_height: f64,
    pub padding_x: f64,
    pub padding_y: f64,
    pub swatch_size: f64,
    pub swatch_padding: f64,
    pub hint_width: f64,
    pub row_count: usize,
    pub palette_top: f64,
    pub palette_rows: usize,
    pub palette_cols: usize,
}

impl InputState {
    pub(crate) fn is_board_picker_open(&self) -> bool {
        matches!(self.board_picker_state, BoardPickerState::Open { .. })
    }

    pub(crate) fn open_board_picker(&mut self) {
        if self.show_help {
            self.toggle_help_overlay();
        }
        self.cancel_active_interaction();
        self.close_context_menu();
        self.close_properties_panel();
        self.board_picker_state = BoardPickerState::Open {
            selected: self.boards.active_index(),
            hover_index: None,
            edit: None,
        };
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
    }

    pub(crate) fn close_board_picker(&mut self) {
        if let Some(layout) = self.board_picker_layout {
            self.mark_board_picker_region(layout);
        }
        self.board_picker_state = BoardPickerState::Hidden;
        self.board_picker_layout = None;
        self.needs_redraw = true;
    }

    pub(crate) fn toggle_board_picker(&mut self) {
        if self.is_board_picker_open() {
            self.close_board_picker();
        } else {
            self.open_board_picker();
        }
    }

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

        let title = format!("Boards ({}/{})", board_count, max_count);
        let footer =
            "Enter: switch  N: new  R: rename  C: color  Swatch: palette  Del: delete  Esc: close";

        let _ = ctx.save();
        ctx.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Bold);
        ctx.set_font_size(TITLE_FONT_SIZE);
        let title_width = text_width(ctx, &title);
        ctx.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Normal);
        ctx.set_font_size(BODY_FONT_SIZE);
        let footer_width = text_width(ctx, footer);

        let mut max_name_width: f64 = 0.0;
        let mut max_hint_width: f64 = 0.0;

        let edit_state = self.board_picker_edit_state();

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
                let hint = match edit_state {
                    Some((BoardPickerEditMode::Color, edit_index, buffer))
                        if edit_index == index =>
                    {
                        Some(buffer.to_string())
                    }
                    _ => board_slot_hint(self, index),
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
                let max_cols =
                    ((available_width + PALETTE_SWATCH_GAP) / unit).floor() as usize;
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

        let panel_height = PADDING_Y * 2.0
            + HEADER_HEIGHT
            + ROW_HEIGHT * row_count as f64
            + palette_extra
            + FOOTER_HEIGHT;

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
            footer_height: FOOTER_HEIGHT,
            padding_x: PADDING_X,
            padding_y: PADDING_Y,
            swatch_size: SWATCH_SIZE,
            swatch_padding: SWATCH_PADDING,
            hint_width: max_hint_width,
            row_count,
            palette_top,
            palette_rows,
            palette_cols,
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

    pub(crate) fn board_picker_swatch_index_at(&self, x: i32, y: i32) -> Option<usize> {
        let layout = self.board_picker_layout?;
        let board_count = self.boards.board_count();
        let rows_top = layout.origin_y + layout.padding_y + layout.header_height;
        for row in 0..board_count {
            let row_top = rows_top + layout.row_height * row as f64;
            let row_center = row_top + layout.row_height * 0.5;
            let swatch_x = layout.origin_x + layout.padding_x;
            let swatch_y = row_center - layout.swatch_size * 0.5;
            let within_x = (x as f64) >= swatch_x
                && (x as f64) <= swatch_x + layout.swatch_size;
            let within_y = (y as f64) >= swatch_y
                && (y as f64) <= swatch_y + layout.swatch_size;
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

    pub(crate) fn board_picker_active_index(&self) -> Option<usize> {
        match &self.board_picker_state {
            BoardPickerState::Open {
                hover_index,
                selected,
                ..
            } => hover_index.or(Some(*selected)),
            BoardPickerState::Hidden => None,
        }
    }

    pub(crate) fn board_picker_selected_index(&self) -> Option<usize> {
        match &self.board_picker_state {
            BoardPickerState::Open { selected, .. } => Some(*selected),
            BoardPickerState::Hidden => None,
        }
    }

    pub(crate) fn board_picker_set_selected(&mut self, index: usize) {
        let row_count = self.board_picker_row_count().max(1);
        let next = index.min(row_count.saturating_sub(1));
        if let BoardPickerState::Open { selected, .. } = &mut self.board_picker_state {
            *selected = next;
            self.needs_redraw = true;
        }
    }

    pub(crate) fn board_picker_clear_edit(&mut self) {
        if let BoardPickerState::Open { edit, .. } = &mut self.board_picker_state {
            *edit = None;
        }
    }

    pub(crate) fn board_picker_start_edit(&mut self, mode: BoardPickerEditMode, buffer: String) {
        if let BoardPickerState::Open { edit, .. } = &mut self.board_picker_state {
            *edit = Some(BoardPickerEdit { mode, buffer });
        }
    }

    pub(crate) fn board_picker_edit_state(&self) -> Option<(BoardPickerEditMode, usize, &str)> {
        let BoardPickerState::Open { selected, edit, .. } = &self.board_picker_state else {
            return None;
        };
        let edit = edit.as_ref()?;
        Some((edit.mode, *selected, edit.buffer.as_str()))
    }

    pub(crate) fn board_picker_edit_buffer_mut(&mut self) -> Option<&mut BoardPickerEdit> {
        let BoardPickerState::Open { edit, .. } = &mut self.board_picker_state else {
            return None;
        };
        edit.as_mut()
    }

    pub(crate) fn board_picker_row_count(&self) -> usize {
        let board_count = self.boards.board_count();
        board_count + 1
    }

    pub(crate) fn board_picker_is_new_row(&self, index: usize) -> bool {
        index >= self.boards.board_count()
    }

    pub(crate) fn board_picker_activate_row(&mut self, index: usize) {
        let board_count = self.boards.board_count();
        if index < board_count {
            self.switch_board_slot(index);
            self.close_board_picker();
        } else {
            self.board_picker_create_new();
        }
    }

    pub(crate) fn board_picker_create_new(&mut self) {
        if !self.create_board() {
            self.set_ui_toast(UiToastKind::Warning, "Board limit reached.");
            return;
        }
        let index = self.boards.active_index();
        self.board_picker_set_selected(index);
        let name = self.boards.active_board_name().to_string();
        self.board_picker_start_edit(BoardPickerEditMode::Name, name);
        self.needs_redraw = true;
    }

    pub(crate) fn board_picker_rename_selected(&mut self) {
        let Some(index) = self.board_picker_selected_index() else {
            return;
        };
        if self.board_picker_is_new_row(index) {
            self.board_picker_create_new();
            return;
        }
        if let Some(board) = self.boards.board_states().get(index) {
            self.board_picker_start_edit(BoardPickerEditMode::Name, board.spec.name.clone());
        }
    }

    pub(crate) fn board_picker_edit_color_selected(&mut self) {
        let Some(index) = self.board_picker_selected_index() else {
            return;
        };
        if self.board_picker_is_new_row(index) {
            self.board_picker_create_new();
            return;
        }
        let Some(board) = self.boards.board_states().get(index) else {
            return;
        };
        if board.spec.background.is_transparent() {
            self.set_ui_toast(UiToastKind::Info, "Overlay board has no background color.");
            return;
        }
        let buffer = match &board.spec.background {
            BoardBackground::Solid(color) => color_to_hex(*color),
            BoardBackground::Transparent => String::new(),
        };
        self.board_picker_start_edit(BoardPickerEditMode::Color, buffer);
        self.needs_redraw = true;
    }

    pub(crate) fn board_picker_commit_edit(&mut self) -> bool {
        let Some((mode, index, buffer)) = self.board_picker_edit_state() else {
            return false;
        };
        if self.board_picker_is_new_row(index) {
            self.board_picker_clear_edit();
            return false;
        }

        let buffer = buffer.to_string();
        let trimmed = buffer.trim();
        match mode {
            BoardPickerEditMode::Name => {
                if !self.set_board_name(index, trimmed.to_string()) {
                    return false;
                }
            }
            BoardPickerEditMode::Color => {
                let Some(color) = parse_hex_color(trimmed) else {
                    self.set_ui_toast(
                        UiToastKind::Warning,
                        "Invalid color. Use #RRGGBB or RRGGBB.",
                    );
                    return false;
                };
                if !self.set_board_background_color(index, color) {
                    return false;
                }
            }
        }

        self.board_picker_clear_edit();
        true
    }

    pub(crate) fn board_picker_cancel_edit(&mut self) {
        self.board_picker_clear_edit();
        self.needs_redraw = true;
    }

    pub(crate) fn board_picker_edit_backspace(&mut self) {
        if let Some(edit) = self.board_picker_edit_buffer_mut() {
            edit.buffer.pop();
            self.needs_redraw = true;
        }
    }

    pub(crate) fn board_picker_edit_append(&mut self, ch: char) {
        let Some(edit) = self.board_picker_edit_buffer_mut() else {
            return;
        };
        match edit.mode {
            BoardPickerEditMode::Name => {
                if edit.buffer.len() >= MAX_BOARD_NAME_LEN {
                    return;
                }
                if !ch.is_control() {
                    edit.buffer.push(ch);
                    self.needs_redraw = true;
                }
            }
            BoardPickerEditMode::Color => {
                let max_len = if edit.buffer.starts_with('#') { 7 } else { 6 };
                if edit.buffer.len() >= max_len {
                    return;
                }
                if ch == '#' && edit.buffer.is_empty() {
                    edit.buffer.push(ch);
                    self.needs_redraw = true;
                    return;
                }
                if ch.is_ascii_hexdigit() {
                    edit.buffer.push(ch.to_ascii_uppercase());
                    self.needs_redraw = true;
                }
            }
        }
    }

    pub(crate) fn board_picker_apply_palette_color(&mut self, color: Color) -> bool {
        let Some(index) = self.board_picker_selected_index() else {
            return false;
        };
        if self.board_picker_is_new_row(index) {
            return false;
        }
        if !self.set_board_background_color(index, color) {
            return false;
        }
        if let Some(edit) = self.board_picker_edit_buffer_mut()
            && edit.mode == BoardPickerEditMode::Color
        {
            edit.buffer = color_to_hex(color);
        }
        self.needs_redraw = true;
        true
    }

    pub(crate) fn board_picker_delete_selected(&mut self) {
        let Some(index) = self.board_picker_selected_index() else {
            return;
        };
        if self.board_picker_is_new_row(index) {
            return;
        }
        if self.boards.active_index() != index {
            self.switch_board_slot(index);
        }
        self.delete_active_board();
        self.board_picker_set_selected(self.boards.active_index());
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

const BOARD_PALETTE: [Color; 11] = [
    RED,
    GREEN,
    BLUE,
    YELLOW,
    WHITE,
    BLACK,
    ORANGE,
    PINK,
    Color {
        r: 0.0,
        g: 1.0,
        b: 1.0,
        a: 1.0,
    },
    Color {
        r: 0.6,
        g: 0.4,
        b: 0.8,
        a: 1.0,
    },
    Color {
        r: 0.4,
        g: 0.4,
        b: 0.4,
        a: 1.0,
    },
];

fn board_palette_colors() -> &'static [Color] {
    &BOARD_PALETTE
}

fn parse_hex_color(value: &str) -> Option<Color> {
    let mut hex = value.trim().trim_start_matches("0x");
    if hex.starts_with('#') {
        hex = &hex[1..];
    }
    if hex.len() != 6 && hex.len() != 3 {
        return None;
    }
    let expanded = if hex.len() == 3 {
        let mut out = String::new();
        for ch in hex.chars() {
            out.push(ch);
            out.push(ch);
        }
        out
    } else {
        hex.to_string()
    };
    let r = u8::from_str_radix(&expanded[0..2], 16).ok()?;
    let g = u8::from_str_radix(&expanded[2..4], 16).ok()?;
    let b = u8::from_str_radix(&expanded[4..6], 16).ok()?;
    Some(Color {
        r: r as f64 / 255.0,
        g: g as f64 / 255.0,
        b: b as f64 / 255.0,
        a: 1.0,
    })
}

fn color_to_hex(color: Color) -> String {
    format!(
        "#{:02X}{:02X}{:02X}",
        (color.r * 255.0).round() as u8,
        (color.g * 255.0).round() as u8,
        (color.b * 255.0).round() as u8
    )
}

fn contrast_color(background: Color) -> Color {
    let luminance = 0.2126 * background.r + 0.7152 * background.g + 0.0722 * background.b;
    if luminance > 0.5 {
        Color {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        }
    } else {
        Color {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: 1.0,
        }
    }
}
