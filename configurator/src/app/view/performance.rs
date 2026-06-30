use iced::widget::{column, pick_list, row, scrollable, text};
use iced::{Element, Length};

use crate::app::scroll::CONTENT_SCROLL_ID;
use crate::messages::Message;
use crate::models::{TextField, ToggleField};

use super::super::search::{SearchArea, TabSearchSummary};
use super::super::state::ConfiguratorApp;
use super::widgets::{
    BUFFER_PICKER_WIDTH, labeled_control, labeled_input_with_feedback, toggle_row,
    validate_u32_range,
};

impl ConfiguratorApp {
    pub(super) fn performance_tab(
        &self,
        search: Option<&TabSearchSummary>,
    ) -> Element<'_, Message> {
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
        .align_y(iced::Alignment::Center)
        .into();

        let show_rendering =
            search.is_none_or(|search| search.area_matches(SearchArea::PerformanceRendering));
        let show_animation =
            search.is_none_or(|search| search.area_matches(SearchArea::PerformanceAnimations));
        let mut content = column![text("Performance").size(20)].spacing(12);

        if show_rendering {
            content = content
                .push(text("Rendering").size(16))
                .push(labeled_control(
                    "Buffer count (2-4)",
                    buffer_control,
                    self.defaults.performance_buffer_count.to_string(),
                    self.draft.performance_buffer_count != self.defaults.performance_buffer_count,
                ))
                .push(toggle_row(
                    "Enable VSync",
                    self.draft.performance_enable_vsync,
                    self.defaults.performance_enable_vsync,
                    ToggleField::PerformanceVsync,
                ))
                .push(text("Synchronizes rendering with display refresh. Prevents tearing but adds slight input latency.").size(12))
                .push(labeled_input_with_feedback(
                    "Max FPS (VSync off)",
                    &self.draft.performance_max_fps_no_vsync,
                    &self.defaults.performance_max_fps_no_vsync,
                    TextField::PerformanceMaxFpsNoVsync,
                    Some("Default 120; try 144/240 on high-refresh displays; 0 = unlimited"),
                    validate_u32_range(&self.draft.performance_max_fps_no_vsync, 0, 1000),
                ))
                .push(text("Caps frame rate when VSync is disabled. 120 FPS keeps drawing latency low without uncapped CPU/GPU usage; use 0 only for profiling.").size(12));
        }

        if show_animation {
            content = content
                .push(text("Animations").size(16))
                .push(labeled_input_with_feedback(
                    "UI Animation FPS",
                    &self.draft.performance_ui_animation_fps,
                    &self.defaults.performance_ui_animation_fps,
                    TextField::PerformanceUiAnimationFps,
                    Some("0 = unlimited, recommended: 30-60"),
                    validate_u32_range(&self.draft.performance_ui_animation_fps, 0, 1000),
                ))
                .push(text("Controls how often UI animations tick (fade effects, toasts, click highlights). Higher values = smoother animations but more CPU usage. Does not affect input responsiveness.").size(12));
        }

        scrollable(content).id(CONTENT_SCROLL_ID).into()
    }
}
