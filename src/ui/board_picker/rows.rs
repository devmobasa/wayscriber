use crate::draw::Color;
use crate::input::state::{BoardPickerEditMode, BoardPickerLayout};
use crate::input::{BoardBackground, InputState};
use crate::ui::primitives::{draw_rounded_rect, text_extents_for};
use crate::ui::theme::Rgba;
use crate::ui_text::{UiTextStyle, draw_text_baseline};

use super::constants::{
    self, ACCENT_PRIMARY, BG_SELECTED_INDICATOR, BG_SELECTION, DIVIDER_LIGHT, ICON_PIN_ACTIVE,
    ICON_PIN_INACTIVE, INPUT_CARET, TEXT_ACTIVE, TEXT_HINT, TEXT_SECONDARY,
};
use super::helpers::{
    SWATCH_EDGE, board_slot_hint, draw_drag_handle, draw_open_icon, draw_pin_icon,
};

const SWATCH_TRANSPARENT_OUTLINE: Rgba = (0.62, 0.68, 0.76, 0.85);

pub(super) fn render_board_rows(
    ctx: &cairo::Context,
    input_state: &InputState,
    layout: &BoardPickerLayout,
    board_count: usize,
    max_count: usize,
) {
    BoardRowsRenderer::new(ctx, input_state, layout, board_count, max_count).render();
}

struct BoardRowsRenderer<'a> {
    ctx: &'a cairo::Context,
    input: &'a InputState,
    layout: &'a BoardPickerLayout,
    board_count: usize,
    max_count: usize,
    rows_top: f64,
    name_x: f64,
    list_right: f64,
    handle_x: Option<f64>,
    open_icon_x: Option<f64>,
    hint_x: Option<f64>,
    highlight_index: Option<usize>,
    selected_index: Option<usize>,
    active_board_index: usize,
    edit_state: Option<(BoardPickerEditMode, usize, &'a str)>,
    pinned_count: usize,
    body_style: UiTextStyle<'static>,
}

impl<'a> BoardRowsRenderer<'a> {
    fn new(
        ctx: &'a cairo::Context,
        input: &'a InputState,
        layout: &'a BoardPickerLayout,
        board_count: usize,
        max_count: usize,
    ) -> Self {
        let rows_top = layout.origin_y + layout.padding_y + layout.header_height;
        let name_x =
            layout.origin_x + layout.padding_x + layout.swatch_size + layout.swatch_padding;
        let list_right = layout.origin_x + layout.list_width;
        let handle_x = (layout.handle_width > 0.0)
            .then_some(list_right - layout.padding_x - layout.handle_width);
        let open_icon_x = (layout.open_icon_size > 0.0)
            .then(|| handle_x.map(|x| x - layout.open_icon_gap - layout.open_icon_size))
            .flatten();
        let hint_right_edge = open_icon_x
            .map(|x| x - layout.handle_gap)
            .or_else(|| handle_x.map(|x| x - layout.handle_gap))
            .unwrap_or(list_right - layout.padding_x);
        let hint_x = (layout.hint_width > 0.0).then_some(hint_right_edge - layout.hint_width);
        Self {
            ctx,
            input,
            layout,
            board_count,
            max_count,
            rows_top,
            name_x,
            list_right,
            handle_x,
            open_icon_x,
            hint_x,
            highlight_index: input.board_picker_active_index(),
            selected_index: input.board_picker_selected_index(),
            active_board_index: input.boards.active_index(),
            edit_state: input.board_picker_edit_state(),
            pinned_count: input.board_picker_pinned_count(),
            body_style: UiTextStyle {
                family: "Sans",
                slant: cairo::FontSlant::Normal,
                weight: cairo::FontWeight::Normal,
                size: layout.body_font_size,
            },
        }
    }

    fn render(&self) {
        self.render_section_header();
        for row in 0..self.layout.row_count {
            self.render_row(row);
        }
    }

    fn render_section_header(&self) {
        if self.pinned_count > 0 && !self.input.board_picker_is_quick() {
            let style = UiTextStyle {
                size: self.layout.footer_font_size * 0.9,
                ..self.body_style
            };
            constants::set_color(self.ctx, constants::with_alpha(TEXT_HINT, 0.6));
            draw_text_baseline(
                self.ctx,
                style,
                "Pinned",
                self.layout.origin_x + self.layout.padding_x,
                self.rows_top - self.layout.footer_font_size * 0.4,
                None,
            );
        }
        if self.pinned_count > 0 && self.pinned_count < self.board_count {
            let y = self.rows_top + self.layout.row_height * self.pinned_count as f64;
            constants::set_color(self.ctx, DIVIDER_LIGHT);
            self.ctx.set_line_width(1.0);
            self.ctx
                .move_to(self.layout.origin_x + self.layout.padding_x, y);
            self.ctx.line_to(self.list_right - self.layout.padding_x, y);
            let _ = self.ctx.stroke();
        }
    }

    fn render_row(&self, row: usize) {
        let row_top = self.rows_top + self.layout.row_height * row as f64;
        let row_center = row_top + self.layout.row_height * 0.5;
        let highlighted = self.highlight_index == Some(row);
        let selected = self.selected_index == Some(row);
        self.render_row_selection(row_top, highlighted, selected);

        let is_new = row >= self.board_count;
        if is_new && row > 0 {
            self.render_new_row_divider(row_top);
        }
        let swatch_x = self.layout.origin_x + self.layout.padding_x;
        let swatch_y = row_center - self.layout.swatch_size * 0.5;
        if is_new {
            self.render_new_row(swatch_x, swatch_y, row_center);
            return;
        }
        let board_index = self
            .input
            .board_picker_board_index_for_row(row)
            .unwrap_or(row);
        let active = board_index == self.active_board_index;
        self.render_board_swatch(board_index, swatch_x, swatch_y, active);
        self.render_existing_row(
            row,
            board_index,
            row_center,
            swatch_x,
            highlighted,
            selected,
            active,
        );
    }

    fn render_row_selection(&self, row_top: f64, highlighted: bool, selected: bool) {
        if highlighted {
            constants::set_color(self.ctx, BG_SELECTION);
            self.ctx.rectangle(
                self.layout.origin_x + 6.0,
                row_top,
                self.layout.list_width - 12.0,
                self.layout.row_height,
            );
            let _ = self.ctx.fill();
        }
        if selected {
            constants::set_color(self.ctx, BG_SELECTED_INDICATOR);
            self.ctx.rectangle(
                self.layout.origin_x + 6.0,
                row_top,
                3.0,
                self.layout.row_height,
            );
            let _ = self.ctx.fill();
        }
    }

    fn render_new_row_divider(&self, row_top: f64) {
        constants::set_color(self.ctx, DIVIDER_LIGHT);
        self.ctx.set_line_width(0.5);
        self.ctx
            .move_to(self.layout.origin_x + self.layout.padding_x, row_top);
        self.ctx
            .line_to(self.list_right - self.layout.padding_x, row_top);
        let _ = self.ctx.stroke();
    }

    fn render_new_row(&self, swatch_x: f64, swatch_y: f64, row_center: f64) {
        constants::set_color(self.ctx, TEXT_HINT);
        draw_rounded_rect(
            self.ctx,
            swatch_x,
            swatch_y,
            self.layout.swatch_size,
            self.layout.swatch_size,
            3.5,
        );
        let _ = self.ctx.stroke();
        self.ctx.set_line_width(1.5);
        let mid_x = swatch_x + self.layout.swatch_size * 0.5;
        let mid_y = swatch_y + self.layout.swatch_size * 0.5;
        self.ctx.move_to(mid_x - 4.0, mid_y);
        self.ctx.line_to(mid_x + 4.0, mid_y);
        self.ctx.move_to(mid_x, mid_y - 4.0);
        self.ctx.line_to(mid_x, mid_y + 4.0);
        let _ = self.ctx.stroke();
        let label = if self.board_count >= self.max_count {
            "New board (max reached)"
        } else {
            "New board"
        };
        constants::set_color(self.ctx, TEXT_HINT);
        draw_text_baseline(
            self.ctx,
            self.body_style,
            label,
            self.name_x,
            self.text_baseline(row_center),
            None,
        );
    }

    fn render_board_swatch(&self, board_index: usize, x: f64, y: f64, active: bool) {
        let board = &self.input.boards.board_states()[board_index];
        match board.spec.background {
            BoardBackground::Transparent => {
                constants::set_color(self.ctx, SWATCH_TRANSPARENT_OUTLINE);
                draw_rounded_rect(
                    self.ctx,
                    x,
                    y,
                    self.layout.swatch_size,
                    self.layout.swatch_size,
                    3.5,
                );
                let _ = self.ctx.stroke();
                self.ctx.move_to(x, y);
                self.ctx
                    .line_to(x + self.layout.swatch_size, y + self.layout.swatch_size);
                self.ctx.move_to(x + self.layout.swatch_size, y);
                self.ctx.line_to(x, y + self.layout.swatch_size);
                let _ = self.ctx.stroke();
            }
            BoardBackground::Solid(color) => {
                self.ctx.set_source_rgba(color.r, color.g, color.b, 1.0);
                draw_rounded_rect(
                    self.ctx,
                    x,
                    y,
                    self.layout.swatch_size,
                    self.layout.swatch_size,
                    3.5,
                );
                let _ = self.ctx.fill();
                constants::set_color(self.ctx, SWATCH_EDGE);
                draw_rounded_rect(
                    self.ctx,
                    x,
                    y,
                    self.layout.swatch_size,
                    self.layout.swatch_size,
                    3.5,
                );
                let _ = self.ctx.stroke();
            }
        }
        if active {
            constants::set_color(self.ctx, ACCENT_PRIMARY);
            draw_rounded_rect(
                self.ctx,
                x - 2.0,
                y - 2.0,
                self.layout.swatch_size + 4.0,
                self.layout.swatch_size + 4.0,
                4.0,
            );
            let _ = self.ctx.stroke();
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn render_existing_row(
        &self,
        row: usize,
        board_index: usize,
        row_center: f64,
        swatch_x: f64,
        highlighted: bool,
        selected: bool,
        active: bool,
    ) {
        self.render_pin(board_index, row_center, swatch_x, highlighted, selected);
        let (name, hint_override) = self.edited_board_text(row, board_index);
        self.render_board_name(board_index, row_center, active, &name);
        self.render_edit_caret(
            row,
            BoardPickerEditMode::Name,
            &name,
            self.name_x,
            row_center,
        );
        self.render_board_hint(row, board_index, row_center, hint_override);
        self.render_row_controls(row_center, highlighted, selected);
    }

    fn render_pin(
        &self,
        board_index: usize,
        row_center: f64,
        swatch_x: f64,
        highlighted: bool,
        selected: bool,
    ) {
        let pinned = self.input.boards.board_states()[board_index].spec.pinned;
        if !pinned && !highlighted && !selected {
            return;
        }
        let rgba = if pinned {
            ICON_PIN_ACTIVE
        } else {
            ICON_PIN_INACTIVE
        };
        draw_pin_icon(
            self.ctx,
            swatch_x - self.layout.swatch_padding * 0.6,
            row_center,
            self.layout.body_font_size,
            Color {
                r: rgba.0,
                g: rgba.1,
                b: rgba.2,
                a: rgba.3,
            },
            pinned,
        );
    }

    fn edited_board_text(&self, row: usize, board_index: usize) -> (String, Option<String>) {
        let mut name = self.input.boards.board_states()[board_index]
            .spec
            .name
            .clone();
        let mut hint = None;
        if let Some((mode, edit_index, buffer)) = self.edit_state
            && edit_index == row
        {
            match mode {
                BoardPickerEditMode::Name => name = buffer.to_string(),
                BoardPickerEditMode::Color => hint = Some(buffer.to_string()),
            }
        }
        (name, hint)
    }

    fn render_board_name(&self, board_index: usize, row_center: f64, active: bool, name: &str) {
        constants::set_color(self.ctx, if active { TEXT_ACTIVE } else { TEXT_SECONDARY });
        draw_text_baseline(
            self.ctx,
            self.body_style,
            name,
            self.name_x,
            self.text_baseline(row_center),
            None,
        );
        let page_count = self.input.boards.board_states()[board_index]
            .pages
            .page_count();
        if page_count <= 1 || self.layout.page_panel_enabled {
            return;
        }
        let extents = self.text_extents(name);
        constants::set_color(self.ctx, constants::with_alpha(TEXT_HINT, 0.85));
        draw_text_baseline(
            self.ctx,
            self.body_style,
            &format!(" ({page_count} pages)"),
            self.name_x + extents.width(),
            self.text_baseline(row_center),
            None,
        );
    }

    fn render_board_hint(
        &self,
        row: usize,
        board_index: usize,
        row_center: f64,
        hint_override: Option<String>,
    ) {
        let Some(hint_x) = self.hint_x else {
            return;
        };
        let Some(hint) = hint_override.or_else(|| board_slot_hint(self.input, board_index)) else {
            return;
        };
        constants::set_color(self.ctx, TEXT_HINT);
        draw_text_baseline(
            self.ctx,
            self.body_style,
            &hint,
            hint_x,
            self.text_baseline(row_center),
            None,
        );
        self.render_edit_caret(row, BoardPickerEditMode::Color, &hint, hint_x, row_center);
    }

    fn render_edit_caret(
        &self,
        row: usize,
        mode: BoardPickerEditMode,
        text: &str,
        x: f64,
        row_center: f64,
    ) {
        if !self
            .edit_state
            .is_some_and(|(edit_mode, edit_index, _)| edit_mode == mode && edit_index == row)
        {
            return;
        }
        let advance = self.text_extents(text).x_advance();
        constants::set_color(self.ctx, INPUT_CARET);
        self.ctx.set_line_width(1.0);
        self.ctx.move_to(
            x + advance + 2.0,
            row_center - self.layout.body_font_size * 0.5,
        );
        self.ctx.line_to(
            x + advance + 2.0,
            row_center + self.layout.body_font_size * 0.5,
        );
        let _ = self.ctx.stroke();
        self.ctx
            .move_to(x, row_center + self.layout.body_font_size * 0.55);
        self.ctx.line_to(
            x + advance + 6.0,
            row_center + self.layout.body_font_size * 0.55,
        );
        let _ = self.ctx.stroke();
    }

    fn render_row_controls(&self, row_center: f64, highlighted: bool, selected: bool) {
        if self.input.board_picker_is_quick() {
            return;
        }
        if let Some(x) = self.open_icon_x {
            let alpha = if highlighted || selected { 0.95 } else { 0.6 };
            draw_open_icon(
                self.ctx,
                x + self.layout.open_icon_size * 0.5,
                row_center,
                self.layout.open_icon_size,
                alpha,
            );
        }
        if let Some(x) = self.handle_x {
            draw_drag_handle(self.ctx, x, row_center, self.layout.handle_width);
        }
    }

    fn text_extents(&self, text: &str) -> cairo::TextExtents {
        text_extents_for(
            self.ctx,
            "Sans",
            cairo::FontSlant::Normal,
            cairo::FontWeight::Normal,
            self.layout.body_font_size,
            text,
        )
    }

    fn text_baseline(&self, row_center: f64) -> f64 {
        row_center + self.layout.body_font_size * 0.35
    }
}
