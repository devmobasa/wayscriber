use iced::widget::{column, pick_list, row, scrollable, text};
use iced::{Element, Length};

use crate::messages::Message;
use crate::models::{TextField, ToggleField};

use super::super::state::ConfiguratorApp;
use super::widgets::{
    BUFFER_PICKER_WIDTH, labeled_control, labeled_input_with_feedback, toggle_row,
    validate_u32_range,
};

impl ConfiguratorApp {
    pub(super) fn performance_tab(&self) -> Element<'_, Message> {
        let buffer_pick = pick_list(
            vec![2u32, 3, 4],
            Some(self.draft.performance_buffer_count),
            Message::BufferCountChanged,
        );
        let buffer_control = row![
            buffer_pick.width(Length::Fixed(BUFFER_PICKER_WIDTH)),
            text(self.draft.performance_buffer_count.to_string())
        ]
        .spacing(12)
        .align_items(iced::Alignment::Center)
        .into();

        scrollable(
            column![
                text("Performance").size(20),
                labeled_control(
                    "Buffer count (2-4)",
                    buffer_control,
                    self.defaults.performance_buffer_count.to_string(),
                    self.draft.performance_buffer_count != self.defaults.performance_buffer_count,
                ),
                labeled_input_with_feedback(
                    "UI animation FPS (0 = unlimited)",
                    &self.draft.performance_ui_animation_fps,
                    &self.defaults.performance_ui_animation_fps,
                    TextField::PerformanceUiAnimationFps,
                    Some("Range: 0-240"),
                    validate_u32_range(&self.draft.performance_ui_animation_fps, 0, 240),
                ),
                toggle_row(
                    "Enable VSync",
                    self.draft.performance_enable_vsync,
                    self.defaults.performance_enable_vsync,
                    ToggleField::PerformanceVsync,
                )
            ]
            .spacing(12),
        )
        .into()
    }
}
