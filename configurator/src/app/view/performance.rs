use iced::widget::{column, pick_list, row, scrollable, text};
use iced::{Element, Length};
use wayscriber::config::PERFORMANCE_FIELDS;

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
        let buffer_field = PERFORMANCE_FIELDS.buffer_count();
        let vsync_field = PERFORMANCE_FIELDS.enable_vsync();
        let max_fps_field = PERFORMANCE_FIELDS.max_fps_no_vsync();
        let animation_field = PERFORMANCE_FIELDS.ui_animation_fps();
        let buffer_metadata = buffer_field.presentation();
        let vsync_metadata = vsync_field.presentation();
        let max_fps_metadata = max_fps_field.presentation();
        let animation_metadata = animation_field.presentation();
        let max_fps_range = max_fps_field.bounds();
        let animation_range = animation_field.bounds();
        let buffer_pick = pick_list(
            buffer_field.choices(),
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
                    buffer_metadata.label(),
                    buffer_control,
                    self.defaults.performance_buffer_count.to_string(),
                    self.draft.performance_buffer_count != self.defaults.performance_buffer_count,
                ))
                .push(text(buffer_metadata.help()).size(12))
                .push(toggle_row(
                    vsync_metadata.label(),
                    self.draft.performance_enable_vsync,
                    self.defaults.performance_enable_vsync,
                    ToggleField::PerformanceVsync,
                ))
                .push(text(vsync_metadata.help()).size(12))
                .push(labeled_input_with_feedback(
                    max_fps_metadata.label(),
                    &self.draft.performance_max_fps_no_vsync,
                    &self.defaults.performance_max_fps_no_vsync,
                    TextField::PerformanceMaxFpsNoVsync,
                    Some(max_fps_metadata.help()),
                    validate_u32_range(
                        &self.draft.performance_max_fps_no_vsync,
                        max_fps_range.0,
                        max_fps_range.1,
                    ),
                ));
        }

        if show_animation {
            content = content
                .push(text("Animations").size(16))
                .push(labeled_input_with_feedback(
                    animation_metadata.label(),
                    &self.draft.performance_ui_animation_fps,
                    &self.defaults.performance_ui_animation_fps,
                    TextField::PerformanceUiAnimationFps,
                    Some(animation_metadata.help()),
                    validate_u32_range(
                        &self.draft.performance_ui_animation_fps,
                        animation_range.0,
                        animation_range.1,
                    ),
                ));
        }

        scrollable(content).id(CONTENT_SCROLL_ID).into()
    }
}
