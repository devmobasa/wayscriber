use iced::widget::{Space, pick_list, row};
use iced::{Element, Length};

use crate::messages::Message;
use crate::models::{
    PresetEraserKindOption, PresetEraserModeOption, PresetTextField, PresetToggleField, ToolOption,
};

use super::super::super::widgets::{labeled_control, preset_input, preset_override_control};
use crate::app::state::ConfiguratorApp;

impl ConfiguratorApp {
    pub(super) fn preset_slot_tool_row(&self, slot_index: usize) -> Element<'_, Message> {
        let Some(slot) = self.draft.presets.slot(slot_index) else {
            return Space::new(Length::Shrink, Length::Shrink).into();
        };
        let Some(default_slot) = self.defaults.presets.slot(slot_index) else {
            return Space::new(Length::Shrink, Length::Shrink).into();
        };

        let tool_picker = pick_list(ToolOption::list(), Some(slot.tool), move |opt| {
            Message::PresetToolChanged(slot_index, opt)
        })
        .width(Length::Fill);

        row![
            preset_input(
                "Label",
                &slot.name,
                &default_slot.name,
                slot_index,
                PresetTextField::Name,
                true,
            ),
            labeled_control(
                "Tool",
                tool_picker.into(),
                default_slot.tool.label(),
                slot.tool != default_slot.tool,
            )
        ]
        .spacing(12)
        .into()
    }

    pub(super) fn preset_slot_size_row(&self, slot_index: usize) -> Element<'_, Message> {
        let Some(slot) = self.draft.presets.slot(slot_index) else {
            return Space::new(Length::Shrink, Length::Shrink).into();
        };
        let Some(default_slot) = self.defaults.presets.slot(slot_index) else {
            return Space::new(Length::Shrink, Length::Shrink).into();
        };

        row![
            preset_input(
                "Size (px)",
                &slot.size,
                &default_slot.size,
                slot_index,
                PresetTextField::Size,
                false,
            ),
            preset_input(
                "Marker opacity (0.05-0.9)",
                &slot.marker_opacity,
                &default_slot.marker_opacity,
                slot_index,
                PresetTextField::MarkerOpacity,
                true,
            )
        ]
        .spacing(12)
        .into()
    }

    pub(super) fn preset_slot_eraser_row(&self, slot_index: usize) -> Element<'_, Message> {
        let Some(slot) = self.draft.presets.slot(slot_index) else {
            return Space::new(Length::Shrink, Length::Shrink).into();
        };
        let Some(default_slot) = self.defaults.presets.slot(slot_index) else {
            return Space::new(Length::Shrink, Length::Shrink).into();
        };

        row![
            labeled_control(
                "Eraser kind",
                pick_list(
                    PresetEraserKindOption::list(),
                    Some(slot.eraser_kind),
                    move |opt| Message::PresetEraserKindChanged(slot_index, opt),
                )
                .width(Length::Fill)
                .into(),
                default_slot.eraser_kind.label(),
                slot.eraser_kind != default_slot.eraser_kind,
            ),
            labeled_control(
                "Eraser mode",
                pick_list(
                    PresetEraserModeOption::list(),
                    Some(slot.eraser_mode),
                    move |opt| Message::PresetEraserModeChanged(slot_index, opt),
                )
                .width(Length::Fill)
                .into(),
                default_slot.eraser_mode.label(),
                slot.eraser_mode != default_slot.eraser_mode,
            )
        ]
        .spacing(12)
        .into()
    }

    pub(super) fn preset_slot_fill_row(&self, slot_index: usize) -> Element<'_, Message> {
        let Some(slot) = self.draft.presets.slot(slot_index) else {
            return Space::new(Length::Shrink, Length::Shrink).into();
        };
        let Some(default_slot) = self.defaults.presets.slot(slot_index) else {
            return Space::new(Length::Shrink, Length::Shrink).into();
        };

        row![
            preset_override_control(
                "Fill enabled",
                slot.fill_enabled,
                default_slot.fill_enabled,
                slot_index,
                PresetToggleField::FillEnabled,
            ),
            preset_override_control(
                "Text background",
                slot.text_background_enabled,
                default_slot.text_background_enabled,
                slot_index,
                PresetToggleField::TextBackgroundEnabled,
            )
        ]
        .spacing(12)
        .into()
    }

    pub(super) fn preset_slot_font_row(&self, slot_index: usize) -> Element<'_, Message> {
        let Some(slot) = self.draft.presets.slot(slot_index) else {
            return Space::new(Length::Shrink, Length::Shrink).into();
        };
        let Some(default_slot) = self.defaults.presets.slot(slot_index) else {
            return Space::new(Length::Shrink, Length::Shrink).into();
        };

        row![
            preset_input(
                "Font size (pt)",
                &slot.font_size,
                &default_slot.font_size,
                slot_index,
                PresetTextField::FontSize,
                true,
            ),
            preset_input(
                "Arrow length (px)",
                &slot.arrow_length,
                &default_slot.arrow_length,
                slot_index,
                PresetTextField::ArrowLength,
                true,
            )
        ]
        .spacing(12)
        .into()
    }

    pub(super) fn preset_slot_arrow_row(&self, slot_index: usize) -> Element<'_, Message> {
        let Some(slot) = self.draft.presets.slot(slot_index) else {
            return Space::new(Length::Shrink, Length::Shrink).into();
        };
        let Some(default_slot) = self.defaults.presets.slot(slot_index) else {
            return Space::new(Length::Shrink, Length::Shrink).into();
        };

        row![
            preset_input(
                "Arrow angle (deg)",
                &slot.arrow_angle,
                &default_slot.arrow_angle,
                slot_index,
                PresetTextField::ArrowAngle,
                true,
            ),
            preset_override_control(
                "Arrow head at end",
                slot.arrow_head_at_end,
                default_slot.arrow_head_at_end,
                slot_index,
                PresetToggleField::ArrowHeadAtEnd,
            )
        ]
        .spacing(12)
        .into()
    }

    pub(super) fn preset_slot_status_row(&self, slot_index: usize) -> Element<'_, Message> {
        let Some(slot) = self.draft.presets.slot(slot_index) else {
            return Space::new(Length::Shrink, Length::Shrink).into();
        };
        let Some(default_slot) = self.defaults.presets.slot(slot_index) else {
            return Space::new(Length::Shrink, Length::Shrink).into();
        };

        row![preset_override_control(
            "Show status bar",
            slot.show_status_bar,
            default_slot.show_status_bar,
            slot_index,
            PresetToggleField::ShowStatusBar,
        )]
        .into()
    }
}
