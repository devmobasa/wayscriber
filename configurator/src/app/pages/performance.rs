//! Performance page: rendering and animation settings.
//!
//! The exemplar for page ports: every control is one `PageBuilder` row
//! sending the same message the Iced view sent, groups map to the same
//! `SearchArea`s, and metadata-driven labels/help come from the shared
//! `performance_field_metadata` table.

use relm4::prelude::*;
use wayscriber::config::{PerformanceFieldId, performance_field_metadata};

use crate::messages::Message;
use crate::models::{TabId, TextField, ToggleField};

use super::super::search::SearchArea;
use super::super::state::ConfiguratorApp;
use super::{BuiltPage, PageBuilder, validate_u32_range};

pub(super) fn build(sender: &ComponentSender<ConfiguratorApp>) -> BuiltPage {
    let buffer_metadata = performance_field_metadata(PerformanceFieldId::BufferCount);
    let vsync_metadata = performance_field_metadata(PerformanceFieldId::EnableVsync);
    let max_fps_metadata = performance_field_metadata(PerformanceFieldId::MaxFpsNoVsync);
    let animation_metadata = performance_field_metadata(PerformanceFieldId::UiAnimationFps);

    let buffer_counts: Vec<u32> = buffer_metadata
        .constraint
        .unsigned_choices()
        .into_iter()
        .flatten()
        .copied()
        .collect();
    let buffer_labels: Vec<String> = buffer_counts.iter().map(u32::to_string).collect();
    let max_fps_range = max_fps_metadata.constraint.unsigned_range();
    let animation_range = animation_metadata.constraint.unsigned_range();

    let mut page = PageBuilder::new(sender, TabId::Performance);

    page.group_in_area("Rendering", SearchArea::PerformanceRendering)
        .combo_row(
            buffer_metadata.label,
            buffer_metadata.help,
            buffer_counts,
            buffer_labels,
            |app| app.draft.performance_buffer_count,
            Message::BufferCountChanged,
        )
        .switch_row(
            vsync_metadata.label,
            vsync_metadata.help,
            |app| app.draft.performance_enable_vsync,
            |value| Message::ToggleChanged(ToggleField::PerformanceVsync, value),
        )
        .entry_row_validated(
            max_fps_metadata.label,
            |app| app.draft.performance_max_fps_no_vsync.clone(),
            |value| Message::TextChanged(TextField::PerformanceMaxFpsNoVsync, value),
            move |app| {
                let (min, max) = max_fps_range?;
                validate_u32_range(&app.draft.performance_max_fps_no_vsync, min, max)
            },
        );

    page.group_in_area("Animations", SearchArea::PerformanceAnimations)
        .entry_row_validated(
            animation_metadata.label,
            |app| app.draft.performance_ui_animation_fps.clone(),
            |value| Message::TextChanged(TextField::PerformanceUiAnimationFps, value),
            move |app| {
                let (min, max) = animation_range?;
                validate_u32_range(&app.draft.performance_ui_animation_fps, min, max)
            },
        );

    page.finish()
}
