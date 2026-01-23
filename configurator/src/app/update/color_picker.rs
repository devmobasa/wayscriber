use iced::Command;

use crate::messages::Message;
use crate::models::color::{
    hex_from_rgb, hex_from_rgba, parse_hex, parse_quad_values, parse_triplet_values,
};
use crate::models::util::format_float;
use crate::models::{ColorPickerId, ColorPickerValue, QuadField};

use super::super::state::{ConfiguratorApp, StatusMessage};

impl ConfiguratorApp {
    pub(super) fn handle_color_picker_toggled(&mut self, id: ColorPickerId) -> Command<Message> {
        if self.color_picker_open == Some(id) {
            self.color_picker_open = None;
            return Command::none();
        }

        self.color_picker_open = Some(id);
        self.sync_color_picker_hex_for_id(id);
        Command::none()
    }

    pub(super) fn handle_color_picker_advanced_toggled(
        &mut self,
        id: ColorPickerId,
        value: bool,
    ) -> Command<Message> {
        if value {
            self.color_picker_advanced.insert(id);
        } else {
            self.color_picker_advanced.remove(&id);
        }
        Command::none()
    }

    pub(super) fn handle_color_picker_hex_changed(
        &mut self,
        id: ColorPickerId,
        value: String,
    ) -> Command<Message> {
        self.status = StatusMessage::idle();
        self.color_picker_hex.insert(id, value.clone());

        if let Some((rgb, alpha)) = parse_hex(&value) {
            let alpha = if self.color_picker_uses_alpha(id) {
                alpha.or_else(|| self.current_alpha_for_id(id))
            } else {
                None
            };
            self.apply_color_picker_value(id, rgb, alpha);
        }

        self.refresh_dirty_flag();
        Command::none()
    }

    pub(super) fn handle_color_picker_changed(
        &mut self,
        id: ColorPickerId,
        value: ColorPickerValue,
    ) -> Command<Message> {
        self.status = StatusMessage::idle();
        self.apply_color_picker_value(id, value.rgb, value.alpha);
        self.refresh_dirty_flag();
        Command::none()
    }

    pub(super) fn sync_color_picker_hex_for_id(&mut self, id: ColorPickerId) {
        if let Some((rgb, alpha)) = self.current_color_for_id(id) {
            let hex = if self.color_picker_uses_alpha(id) {
                let alpha = alpha.unwrap_or(1.0);
                hex_from_rgba([rgb[0], rgb[1], rgb[2], alpha])
            } else {
                hex_from_rgb(rgb)
            };
            self.color_picker_hex.insert(id, hex);
        }
    }

    pub(super) fn sync_board_color_picker_hex(&mut self) {
        let len = self.draft.boards.items.len();
        for index in 0..len {
            self.sync_color_picker_hex_for_id(ColorPickerId::BoardBackground(index));
            self.sync_color_picker_hex_for_id(ColorPickerId::BoardPen(index));
        }
    }

    pub(crate) fn sync_all_color_picker_hex(&mut self) {
        self.sync_board_color_picker_hex();
        for id in [
            ColorPickerId::StatusBarBg,
            ColorPickerId::StatusBarText,
            ColorPickerId::HighlightFill,
            ColorPickerId::HighlightOutline,
            ColorPickerId::HelpBg,
            ColorPickerId::HelpBorder,
            ColorPickerId::HelpText,
        ] {
            self.sync_color_picker_hex_for_id(id);
        }
    }

    fn apply_color_picker_value(&mut self, id: ColorPickerId, rgb: [f64; 3], alpha: Option<f64>) {
        let values = rgb.map(format_float);
        match id {
            ColorPickerId::BoardBackground(index) => {
                if let Some(item) = self.draft.boards.items.get_mut(index) {
                    for (component, value) in values.iter().enumerate() {
                        item.background_color
                            .set_component(component, value.to_string());
                    }
                }
            }
            ColorPickerId::BoardPen(index) => {
                if let Some(item) = self.draft.boards.items.get_mut(index) {
                    for (component, value) in values.iter().enumerate() {
                        item.default_pen_color
                            .color
                            .set_component(component, value.to_string());
                    }
                }
            }
            ColorPickerId::StatusBarBg => {
                self.apply_quad_rgb(QuadField::StatusBarBg, values, alpha);
            }
            ColorPickerId::StatusBarText => {
                self.apply_quad_rgb(QuadField::StatusBarText, values, alpha);
            }
            ColorPickerId::HighlightFill => {
                self.apply_quad_rgb(QuadField::HighlightFill, values, alpha);
            }
            ColorPickerId::HighlightOutline => {
                self.apply_quad_rgb(QuadField::HighlightOutline, values, alpha);
            }
            ColorPickerId::HelpBg => {
                self.apply_quad_rgb(QuadField::HelpBg, values, alpha);
            }
            ColorPickerId::HelpBorder => {
                self.apply_quad_rgb(QuadField::HelpBorder, values, alpha);
            }
            ColorPickerId::HelpText => {
                self.apply_quad_rgb(QuadField::HelpText, values, alpha);
            }
        }

        self.sync_color_picker_hex_for_id(id);
    }

    fn apply_quad_rgb(&mut self, field: QuadField, values: [String; 3], alpha: Option<f64>) {
        for (component, value) in values.iter().enumerate() {
            self.draft.set_quad(field, component, value.to_string());
        }
        if let Some(alpha) = alpha {
            self.draft.set_quad(field, 3, format_float(alpha));
        }
    }

    fn current_color_for_id(&self, id: ColorPickerId) -> Option<([f64; 3], Option<f64>)> {
        match id {
            ColorPickerId::BoardBackground(index) => {
                self.draft.boards.items.get(index).map(|item| {
                    (
                        parse_triplet_values(&item.background_color.components),
                        None,
                    )
                })
            }
            ColorPickerId::BoardPen(index) => self.draft.boards.items.get(index).map(|item| {
                (
                    parse_triplet_values(&item.default_pen_color.color.components),
                    None,
                )
            }),
            ColorPickerId::StatusBarBg => {
                let values = parse_quad_values(&self.draft.status_bar_bg_color.components);
                Some(([values[0], values[1], values[2]], Some(values[3])))
            }
            ColorPickerId::StatusBarText => {
                let values = parse_quad_values(&self.draft.status_bar_text_color.components);
                Some(([values[0], values[1], values[2]], Some(values[3])))
            }
            ColorPickerId::HighlightFill => {
                let values = parse_quad_values(&self.draft.click_highlight_fill_color.components);
                Some(([values[0], values[1], values[2]], Some(values[3])))
            }
            ColorPickerId::HighlightOutline => {
                let values =
                    parse_quad_values(&self.draft.click_highlight_outline_color.components);
                Some(([values[0], values[1], values[2]], Some(values[3])))
            }
            ColorPickerId::HelpBg => {
                let values = parse_quad_values(&self.draft.help_bg_color.components);
                Some(([values[0], values[1], values[2]], Some(values[3])))
            }
            ColorPickerId::HelpBorder => {
                let values = parse_quad_values(&self.draft.help_border_color.components);
                Some(([values[0], values[1], values[2]], Some(values[3])))
            }
            ColorPickerId::HelpText => {
                let values = parse_quad_values(&self.draft.help_text_color.components);
                Some(([values[0], values[1], values[2]], Some(values[3])))
            }
        }
    }

    fn current_alpha_for_id(&self, id: ColorPickerId) -> Option<f64> {
        self.current_color_for_id(id).and_then(|(_, alpha)| alpha)
    }

    fn color_picker_uses_alpha(&self, id: ColorPickerId) -> bool {
        matches!(
            id,
            ColorPickerId::StatusBarBg
                | ColorPickerId::StatusBarText
                | ColorPickerId::HighlightFill
                | ColorPickerId::HighlightOutline
                | ColorPickerId::HelpBg
                | ColorPickerId::HelpBorder
                | ColorPickerId::HelpText
        )
    }
}
