use cairo::Context as CairoContext;

use crate::config::Action;

use super::super::super::super::base::InputState;
use super::super::super::BoardPickerEditMode;
use super::super::super::COLUMN_GAP;
use super::{BoardPickerContentMetrics, BoardPickerLayoutConfig};

impl InputState {
    pub(super) fn compute_board_picker_content_metrics(
        &self,
        ctx: &CairoContext,
        row_count: usize,
        board_count: usize,
        max_count: usize,
        config: &BoardPickerLayoutConfig,
        edit_state: Option<(BoardPickerEditMode, usize, &str)>,
    ) -> BoardPickerContentMetrics {
        let title = self.board_picker_title(board_count, max_count);
        let footer = self.board_picker_footer_text();
        let recent_label = self.board_picker_recent_label();

        let _ = ctx.save();
        let (title_width, footer_width, recent_width) = self.measure_board_picker_static_widths(
            ctx,
            config,
            &title,
            &footer,
            recent_label.as_deref(),
        );

        let show_hints = !self.board_picker_is_quick();
        let (max_name_width, max_hint_width) = self.measure_board_picker_widths(
            ctx,
            row_count,
            board_count,
            max_count,
            config.body_font_size,
            show_hints,
            edit_state,
        );

        let _ = ctx.restore();

        let content_width =
            self.compute_board_picker_content_width(config, max_name_width, max_hint_width);

        let list_width = self.compute_board_picker_list_width(
            config,
            content_width,
            title_width,
            footer_width,
            recent_width,
        );

        let recent_height = if recent_label.is_some() {
            config.recent_line_height
        } else {
            0.0
        };

        BoardPickerContentMetrics {
            list_width,
            max_hint_width,
            footer_height: config.base_footer_height + recent_height,
            recent_height,
        }
    }

    pub(super) fn measure_board_picker_static_widths(
        &self,
        ctx: &CairoContext,
        config: &BoardPickerLayoutConfig,
        title: &str,
        footer: &str,
        recent: Option<&str>,
    ) -> (f64, f64, f64) {
        ctx.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Bold);
        ctx.set_font_size(config.title_font_size);
        let title_width = text_width(ctx, title, config.title_font_size);
        ctx.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Normal);
        ctx.set_font_size(config.footer_font_size);
        let footer_width = text_width(ctx, footer, config.footer_font_size);
        let recent_width = recent
            .map(|label| text_width(ctx, label, config.footer_font_size))
            .unwrap_or(0.0);
        (title_width, footer_width, recent_width)
    }

    pub(super) fn compute_board_picker_content_width(
        &self,
        config: &BoardPickerLayoutConfig,
        max_name_width: f64,
        max_hint_width: f64,
    ) -> f64 {
        let mut content_width = config.swatch_size + config.swatch_padding + max_name_width;
        if max_hint_width > 0.0 {
            content_width += COLUMN_GAP + max_hint_width;
        }
        self.extend_content_for_action_controls(config, content_width)
    }

    pub(super) fn extend_content_for_action_controls(
        &self,
        config: &BoardPickerLayoutConfig,
        mut content_width: f64,
    ) -> f64 {
        if config.handle_width > 0.0 {
            content_width += config.handle_gap + config.handle_width;
            if config.open_icon_size > 0.0 {
                content_width += config.open_icon_gap + config.open_icon_size;
            }
        }
        content_width
    }

    pub(super) fn compute_board_picker_list_width(
        &self,
        config: &BoardPickerLayoutConfig,
        content_width: f64,
        title_width: f64,
        footer_width: f64,
        recent_width: f64,
    ) -> f64 {
        let mut list_width = config.padding_x * 2.0 + content_width;
        list_width = list_width.max(title_width + config.padding_x * 2.0);
        list_width = list_width.max(footer_width + config.padding_x * 2.0);
        list_width = list_width.max(recent_width + config.padding_x * 2.0);
        list_width
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn measure_board_picker_widths(
        &self,
        ctx: &CairoContext,
        row_count: usize,
        board_count: usize,
        max_count: usize,
        body_font_size: f64,
        show_hints: bool,
        edit_state: Option<(BoardPickerEditMode, usize, &str)>,
    ) -> (f64, f64) {
        let mut max_name_width: f64 = 0.0;
        let mut max_hint_width: f64 = 0.0;

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

        (max_name_width, max_hint_width)
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
