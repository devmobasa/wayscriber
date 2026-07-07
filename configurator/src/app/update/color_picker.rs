use iced::Task;

use crate::messages::Message;
use crate::models::color::{
    hex_from_rgb, hex_from_rgba, parse_hex, parse_quad_values, parse_triplet_values,
};
use crate::models::util::format_float;
use crate::models::{ColorPickerId, ColorPickerValue, QuadField};

use super::super::state::{ConfiguratorApp, StatusMessage};

impl ConfiguratorApp {
    pub(super) fn handle_color_picker_toggled(&mut self, id: ColorPickerId) -> Task<Message> {
        if self.color_picker_open == Some(id) {
            self.color_picker_open = None;
            return Task::none();
        }

        self.color_picker_open = Some(id);
        self.sync_color_picker_hex_for_id(id);
        Task::none()
    }

    pub(super) fn handle_color_picker_advanced_toggled(
        &mut self,
        id: ColorPickerId,
        value: bool,
    ) -> Task<Message> {
        if value {
            self.color_picker_advanced.insert(id);
        } else {
            self.color_picker_advanced.remove(&id);
        }
        Task::none()
    }

    pub(super) fn handle_color_picker_hex_changed(
        &mut self,
        id: ColorPickerId,
        value: String,
    ) -> Task<Message> {
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
        Task::none()
    }

    pub(super) fn handle_color_picker_changed(
        &mut self,
        id: ColorPickerId,
        value: ColorPickerValue,
    ) -> Task<Message> {
        self.status = StatusMessage::idle();
        self.apply_color_picker_value(id, value.rgb, value.alpha);
        self.refresh_dirty_flag();
        Task::none()
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

    pub(super) fn sync_render_profile_color_picker_hex(&mut self) {
        for profile_index in 0..self.draft.render_profiles.profiles.len() {
            let mapping_len = self.draft.render_profiles.profiles[profile_index]
                .mappings
                .len();
            for mapping_index in 0..mapping_len {
                self.sync_color_picker_hex_for_id(ColorPickerId::RenderProfileMappingFrom(
                    profile_index,
                    mapping_index,
                ));
                self.sync_color_picker_hex_for_id(ColorPickerId::RenderProfileMappingTo(
                    profile_index,
                    mapping_index,
                ));
            }
        }
    }

    pub(crate) fn sync_all_color_picker_hex(&mut self) {
        self.sync_board_color_picker_hex();
        self.sync_render_profile_color_picker_hex();
        for id in [
            ColorPickerId::DrawingColor,
            ColorPickerId::StatusBarBg,
            ColorPickerId::StatusBarText,
            ColorPickerId::HighlightFill,
            ColorPickerId::HighlightOutline,
            ColorPickerId::HelpBg,
            ColorPickerId::HelpBorder,
            ColorPickerId::HelpText,
            ColorPickerId::ExportPdfLabelText,
            ColorPickerId::ExportPdfLabelBackground,
        ] {
            self.sync_color_picker_hex_for_id(id);
        }
        for index in 0..self.draft.drawing_quick_colors.entries.len() {
            self.sync_color_picker_hex_for_id(ColorPickerId::QuickColor(index));
        }
    }

    fn apply_color_picker_value(&mut self, id: ColorPickerId, rgb: [f64; 3], alpha: Option<f64>) {
        let values = rgb.map(format_float);
        match id {
            ColorPickerId::DrawingColor => {
                let values = rgb.map(format_rgb255);
                for (component, value) in values.iter().enumerate() {
                    if let Some(slot) = self.draft.drawing_color.rgb.get_mut(component) {
                        *slot = value.to_string();
                    }
                }
            }
            ColorPickerId::QuickColor(index) => {
                let values = rgb.map(format_rgb255);
                if let Some(entry) = self.draft.drawing_quick_colors.get_mut(index) {
                    for (component, value) in values.iter().enumerate() {
                        if let Some(slot) = entry.color.rgb.get_mut(component) {
                            *slot = value.to_string();
                        }
                    }
                }
            }
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
            ColorPickerId::RenderProfileMappingFrom(profile_index, mapping_index)
            | ColorPickerId::RenderProfileMappingTo(profile_index, mapping_index) => {
                if let Some(mapping) = self
                    .draft
                    .render_profiles
                    .profiles
                    .get_mut(profile_index)
                    .and_then(|profile| profile.mappings.get_mut(mapping_index))
                {
                    let hex = hex_from_rgb(rgb);
                    match id {
                        ColorPickerId::RenderProfileMappingFrom(_, _) => mapping.from = hex,
                        ColorPickerId::RenderProfileMappingTo(_, _) => mapping.to = hex,
                        _ => {}
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
            ColorPickerId::ExportPdfLabelText => {
                self.apply_quad_rgb(QuadField::ExportPdfLabelText, values, alpha);
            }
            ColorPickerId::ExportPdfLabelBackground => {
                self.apply_quad_rgb(QuadField::ExportPdfLabelBackground, values, alpha);
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
            ColorPickerId::DrawingColor => {
                normalized_drawing_rgb(&self.draft.drawing_color.rgb).map(|rgb| (rgb, None))
            }
            ColorPickerId::QuickColor(index) => {
                let entry = self.draft.drawing_quick_colors.get(index)?;
                normalized_drawing_rgb(&entry.color.rgb).map(|rgb| (rgb, None))
            }
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
            ColorPickerId::RenderProfileMappingFrom(profile_index, mapping_index)
            | ColorPickerId::RenderProfileMappingTo(profile_index, mapping_index) => {
                let mapping = self
                    .draft
                    .render_profiles
                    .profiles
                    .get(profile_index)
                    .and_then(|profile| profile.mappings.get(mapping_index))?;
                let value = match id {
                    ColorPickerId::RenderProfileMappingFrom(_, _) => &mapping.from,
                    ColorPickerId::RenderProfileMappingTo(_, _) => &mapping.to,
                    _ => return None,
                };
                parse_hex(value).map(|(rgb, _)| (rgb, None))
            }
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
            ColorPickerId::ExportPdfLabelText => {
                let values = parse_quad_values(&self.draft.export_pdf_label_text_color.components);
                Some(([values[0], values[1], values[2]], Some(values[3])))
            }
            ColorPickerId::ExportPdfLabelBackground => {
                let values =
                    parse_quad_values(&self.draft.export_pdf_label_background_color.components);
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
                | ColorPickerId::ExportPdfLabelText
                | ColorPickerId::ExportPdfLabelBackground
        )
    }
}

fn normalized_drawing_rgb(values: &[String; 3]) -> Option<[f64; 3]> {
    let mut rgb = [0.0; 3];
    for (index, value) in values.iter().enumerate() {
        let parsed = value.trim().parse::<f64>().ok()?;
        if !(0.0..=255.0).contains(&parsed) {
            return None;
        }
        rgb[index] = parsed / 255.0;
    }
    Some(rgb)
}

fn format_rgb255(value: f64) -> String {
    let value = if value.is_nan() { 0.0 } else { value };
    ((value.clamp(0.0, 1.0) * 255.0).round() as u8).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{ColorMode, ColorPickerValue};

    #[test]
    fn drawing_color_picker_writes_rgb255_components_and_hex() {
        let (mut app, _cmd) = ConfiguratorApp::new_app();
        app.draft.drawing_color.mode = ColorMode::Rgb;

        let _ = app.handle_color_picker_changed(
            ColorPickerId::DrawingColor,
            ColorPickerValue {
                rgb: [0.0, 0.5, 1.0],
                alpha: None,
            },
        );

        assert_eq!(app.draft.drawing_color.rgb, ["0", "128", "255"]);
        assert_eq!(
            app.color_picker_hex
                .get(&ColorPickerId::DrawingColor)
                .map(String::as_str),
            Some("#0080FF")
        );
    }

    #[test]
    fn drawing_color_picker_hex_writes_rgb255_components() {
        let (mut app, _cmd) = ConfiguratorApp::new_app();
        app.draft.drawing_color.mode = ColorMode::Rgb;

        let _ =
            app.handle_color_picker_hex_changed(ColorPickerId::DrawingColor, "#00FF80".to_string());

        assert_eq!(app.draft.drawing_color.rgb, ["0", "255", "128"]);
    }

    #[test]
    fn quick_color_picker_hex_writes_slot_rgb255_components() {
        let (mut app, _cmd) = ConfiguratorApp::new_app();
        app.draft.drawing_quick_colors.entries[1].color.mode = ColorMode::Rgb;

        let _ = app
            .handle_color_picker_hex_changed(ColorPickerId::QuickColor(1), "#123456".to_string());

        assert_eq!(
            app.draft.drawing_quick_colors.entries[1].color.rgb,
            ["18", "52", "86"]
        );
    }
}
