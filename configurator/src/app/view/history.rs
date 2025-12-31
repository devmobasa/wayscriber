use iced::Element;
use iced::theme;
use iced::widget::{column, container, row, scrollable, text};

use crate::messages::Message;
use crate::models::{TextField, ToggleField};

use super::super::state::ConfiguratorApp;
use super::widgets::{
    labeled_input_state, labeled_input_with_feedback, toggle_row, validate_u64_range,
    validate_usize_range,
};

impl ConfiguratorApp {
    pub(super) fn history_tab(&self) -> Element<'_, Message> {
        let custom_enabled = self.draft.history_custom_section_enabled;
        let custom_section = container(
            column![
                text("Custom section").size(16),
                row![
                    labeled_input_state(
                        "Custom undo delay (ms)",
                        &self.draft.history_custom_undo_delay_ms,
                        &self.defaults.history_custom_undo_delay_ms,
                        TextField::HistoryCustomUndoDelayMs,
                        custom_enabled,
                        Some("Range: 50-5000 ms"),
                        if custom_enabled {
                            validate_u64_range(&self.draft.history_custom_undo_delay_ms, 50, 5000)
                        } else {
                            None
                        },
                    ),
                    labeled_input_state(
                        "Custom redo delay (ms)",
                        &self.draft.history_custom_redo_delay_ms,
                        &self.defaults.history_custom_redo_delay_ms,
                        TextField::HistoryCustomRedoDelayMs,
                        custom_enabled,
                        Some("Range: 50-5000 ms"),
                        if custom_enabled {
                            validate_u64_range(&self.draft.history_custom_redo_delay_ms, 50, 5000)
                        } else {
                            None
                        },
                    )
                ]
                .spacing(12),
                row![
                    labeled_input_state(
                        "Custom undo steps",
                        &self.draft.history_custom_undo_steps,
                        &self.defaults.history_custom_undo_steps,
                        TextField::HistoryCustomUndoSteps,
                        custom_enabled,
                        Some("Range: 1-500"),
                        if custom_enabled {
                            validate_usize_range(&self.draft.history_custom_undo_steps, 1, 500)
                        } else {
                            None
                        },
                    ),
                    labeled_input_state(
                        "Custom redo steps",
                        &self.draft.history_custom_redo_steps,
                        &self.defaults.history_custom_redo_steps,
                        TextField::HistoryCustomRedoSteps,
                        custom_enabled,
                        Some("Range: 1-500"),
                        if custom_enabled {
                            validate_usize_range(&self.draft.history_custom_redo_steps, 1, 500)
                        } else {
                            None
                        },
                    )
                ]
                .spacing(12),
            ]
            .spacing(12),
        )
        .padding(12)
        .style(theme::Container::Box);

        scrollable(
            column![
                text("History").size(20),
                row![
                    labeled_input_with_feedback(
                        "Undo all delay (ms)",
                        &self.draft.history_undo_all_delay_ms,
                        &self.defaults.history_undo_all_delay_ms,
                        TextField::HistoryUndoAllDelayMs,
                        Some("Range: 50-5000 ms"),
                        validate_u64_range(&self.draft.history_undo_all_delay_ms, 50, 5000),
                    ),
                    labeled_input_with_feedback(
                        "Redo all delay (ms)",
                        &self.draft.history_redo_all_delay_ms,
                        &self.defaults.history_redo_all_delay_ms,
                        TextField::HistoryRedoAllDelayMs,
                        Some("Range: 50-5000 ms"),
                        validate_u64_range(&self.draft.history_redo_all_delay_ms, 50, 5000),
                    )
                ]
                .spacing(12),
                toggle_row(
                    "Enable custom undo/redo section",
                    self.draft.history_custom_section_enabled,
                    self.defaults.history_custom_section_enabled,
                    ToggleField::HistoryCustomSectionEnabled,
                ),
                custom_section,
            ]
            .spacing(12),
        )
        .into()
    }
}
