use iced::widget::{button, column, container, pick_list, row, rule, scrollable, text, text_input};
use iced::{Element, Length};

use crate::app::scroll::CONTENT_SCROLL_ID;
use crate::app::session_catalog::{
    session_artifact_status_label, session_clear_cached_status_blocker,
    session_duplicate_cached_status_blocker, session_move_cached_status_blocker,
};
use crate::app::view::theme;
use crate::messages::Message;
use crate::models::{
    SessionCatalogItem, SessionCompressionOption, SessionStorageModeOption, TextField, ToggleField,
};

use super::super::search::{SearchArea, TabSearchSummary};
use super::super::state::ConfiguratorApp;
use super::widgets::{
    labeled_control, labeled_input, labeled_input_with_feedback, toggle_row, validate_u64_min,
    validate_u64_range, validate_usize_min,
};

impl ConfiguratorApp {
    pub(super) fn session_tab(&self, search: Option<&TabSearchSummary>) -> Element<'_, Message> {
        let show_all = search.is_none_or(TabSearchSummary::show_all);
        let show_persistence =
            search.is_none_or(|search| search.area_matches(SearchArea::SessionPersistence));
        let show_catalog =
            search.is_none_or(|search| search.area_matches(SearchArea::SessionCatalog));
        let storage_pick = pick_list(
            SessionStorageModeOption::list(),
            Some(self.draft.session_storage_mode),
            Message::SessionStorageModeChanged,
        );
        let compression_pick = pick_list(
            SessionCompressionOption::list(),
            Some(self.draft.session_compression),
            Message::SessionCompressionChanged,
        );

        let mut column = column![text("Session Persistence").size(20)].spacing(12);

        if show_persistence || show_all {
            column = column
                .push(toggle_row(
                    "Persist transparent mode drawings",
                    self.draft.session_persist_transparent,
                    self.defaults.session_persist_transparent,
                    ToggleField::SessionPersistTransparent,
                ))
                .push(toggle_row(
                    "Persist whiteboard mode drawings",
                    self.draft.session_persist_whiteboard,
                    self.defaults.session_persist_whiteboard,
                    ToggleField::SessionPersistWhiteboard,
                ))
                .push(toggle_row(
                    "Persist blackboard mode drawings",
                    self.draft.session_persist_blackboard,
                    self.defaults.session_persist_blackboard,
                    ToggleField::SessionPersistBlackboard,
                ))
                .push(toggle_row(
                    "Persist undo/redo history",
                    self.draft.session_persist_history,
                    self.defaults.session_persist_history,
                    ToggleField::SessionPersistHistory,
                ))
                .push(toggle_row(
                    "Restore tool state on startup",
                    self.draft.session_restore_tool_state,
                    self.defaults.session_restore_tool_state,
                    ToggleField::SessionRestoreToolState,
                ))
                .push(toggle_row(
                    "Enable autosave",
                    self.draft.session_autosave_enabled,
                    self.defaults.session_autosave_enabled,
                    ToggleField::SessionAutosaveEnabled,
                ))
                .push(toggle_row(
                    "Per-output persistence",
                    self.draft.session_per_output,
                    self.defaults.session_per_output,
                    ToggleField::SessionPerOutput,
                ))
                .push(labeled_control(
                    "Storage mode",
                    storage_pick.width(Length::Fill).into(),
                    self.defaults.session_storage_mode.label().to_string(),
                    self.draft.session_storage_mode != self.defaults.session_storage_mode,
                ));

            if self.draft.session_storage_mode == SessionStorageModeOption::Custom {
                column = column.push(labeled_input(
                    "Custom directory",
                    &self.draft.session_custom_directory,
                    &self.defaults.session_custom_directory,
                    TextField::SessionCustomDirectory,
                ));
            }

            column = column.push(labeled_control(
                "Compression",
                compression_pick.width(Length::Fill).into(),
                self.defaults.session_compression.label().to_string(),
                self.draft.session_compression != self.defaults.session_compression,
            ));

            column = column.push(labeled_input_with_feedback(
                "Autosave idle (ms)",
                &self.draft.session_autosave_idle_ms,
                &self.defaults.session_autosave_idle_ms,
                TextField::SessionAutosaveIdleMs,
                Some("Minimum: 1000 ms"),
                validate_u64_min(&self.draft.session_autosave_idle_ms, 1000),
            ));
            column = column.push(labeled_input_with_feedback(
                "Autosave interval (ms)",
                &self.draft.session_autosave_interval_ms,
                &self.defaults.session_autosave_interval_ms,
                TextField::SessionAutosaveIntervalMs,
                Some("Minimum: 1000 ms"),
                validate_u64_min(&self.draft.session_autosave_interval_ms, 1000),
            ));
            column = column.push(labeled_input_with_feedback(
                "Autosave failure backoff (ms)",
                &self.draft.session_autosave_failure_backoff_ms,
                &self.defaults.session_autosave_failure_backoff_ms,
                TextField::SessionAutosaveFailureBackoffMs,
                Some("Minimum: 1000 ms"),
                validate_u64_min(&self.draft.session_autosave_failure_backoff_ms, 1000),
            ));
            column = column.push(labeled_input_with_feedback(
                "Max shapes per frame",
                &self.draft.session_max_shapes_per_frame,
                &self.defaults.session_max_shapes_per_frame,
                TextField::SessionMaxShapesPerFrame,
                Some("Minimum: 1"),
                validate_usize_min(&self.draft.session_max_shapes_per_frame, 1),
            ));
            column = column.push(labeled_input(
                "Max persisted undo depth (blank = runtime limit)",
                &self.draft.session_max_persisted_undo_depth,
                &self.defaults.session_max_persisted_undo_depth,
                TextField::SessionMaxPersistedUndoDepth,
            ));
            column = column.push(labeled_input_with_feedback(
                "Max file size (MB)",
                &self.draft.session_max_file_size_mb,
                &self.defaults.session_max_file_size_mb,
                TextField::SessionMaxFileSizeMb,
                Some("Range: 1-1024 MB"),
                validate_u64_range(&self.draft.session_max_file_size_mb, 1, 1024),
            ));
            column = column.push(labeled_input_with_feedback(
                "Auto-compress threshold (KB)",
                &self.draft.session_auto_compress_threshold_kb,
                &self.defaults.session_auto_compress_threshold_kb,
                TextField::SessionAutoCompressThresholdKb,
                Some("Minimum: 1 KB"),
                validate_u64_min(&self.draft.session_auto_compress_threshold_kb, 1),
            ));
            column = column.push(labeled_input(
                "Backup retention count",
                &self.draft.session_backup_retention,
                &self.defaults.session_backup_retention,
                TextField::SessionBackupRetention,
            ));
        }

        if show_catalog || show_all {
            column = column
                .push(rule::horizontal(1))
                .push(self.session_catalog_section(search));
        }

        scrollable(column).id(CONTENT_SCROLL_ID).into()
    }

    fn session_catalog_section(&self, search: Option<&TabSearchSummary>) -> Element<'_, Message> {
        let busy = self.session_catalog.busy || self.session_catalog.is_loading;
        let mut refresh = button("Refresh").style(theme::Button::Secondary);
        if !busy {
            refresh = refresh.on_press(Message::SessionCatalogRefreshRequested);
        }

        let mut section = column![
            row![text("Saved Sessions").size(20), refresh]
                .spacing(12)
                .align_y(iced::Alignment::Center),
            text("Recent named session files recorded from overlay Open and Save As actions.")
                .size(14),
        ]
        .spacing(10);

        if let Some(blocker) = session_clear_cached_status_blocker(self.daemon_status.as_ref()) {
            section = section.push(
                text(blocker)
                    .size(12)
                    .style(theme::Text::Color(iced::Color::from_rgb(0.95, 0.8, 0.3))),
            );
        }

        if self.session_catalog.is_loading {
            return section.push(text("Loading sessions...").size(14)).into();
        }

        if self.session_catalog.items.is_empty() {
            return section
                .push(text("No named sessions in the catalog yet.").size(14))
                .into();
        }

        let visible_items = self
            .session_catalog
            .items
            .iter()
            .filter(|item| search.is_none_or(|search| search.session_item_visible(&item.id)));

        for item in visible_items {
            section = section.push(self.session_catalog_item(item));
        }

        section.into()
    }

    fn session_catalog_item<'a>(&'a self, item: &'a SessionCatalogItem) -> Element<'a, Message> {
        let id = item.id.clone();
        let rename_value = self
            .session_catalog
            .rename_value(&item.id, &item.display_name);
        let rename_changed = rename_value.trim() != item.display_name.trim();
        let rename_valid = !rename_value.trim().is_empty();
        let duplicate_value = self.session_catalog.duplicate_value(&item.id, &item.path);
        let duplicate_valid = !duplicate_value.trim().is_empty();
        let move_value = self.session_catalog.move_value(&item.id, &item.path);
        let move_valid = !move_value.trim().is_empty();
        let busy = self.session_catalog.busy || self.session_catalog.is_loading;

        let mut rename_button = button("Save Name").style(theme::Button::Secondary);
        if !busy && rename_changed && rename_valid {
            rename_button =
                rename_button.on_press(Message::SessionCatalogRenameRequested(id.clone()));
        }

        let duplicate_blocker =
            session_duplicate_cached_status_blocker(self.daemon_status.as_ref());
        let mut duplicate_button = button("Duplicate").style(theme::Button::Secondary);
        if !busy && duplicate_blocker.is_none() && duplicate_valid {
            duplicate_button =
                duplicate_button.on_press(Message::SessionCatalogDuplicateRequested(id.clone()));
        }

        let move_blocker = session_move_cached_status_blocker(self.daemon_status.as_ref());
        let mut move_button = button("Move").style(theme::Button::Secondary);
        if !busy && move_blocker.is_none() && move_valid {
            move_button = move_button.on_press(Message::SessionCatalogMoveRequested(id.clone()));
        }

        let mut reveal_button = button("Reveal File").style(theme::Button::Secondary);
        if !busy {
            reveal_button =
                reveal_button.on_press(Message::SessionCatalogRevealRequested(id.clone()));
        }

        let mut forget_button = button("Forget").style(theme::Button::Subtle);
        if !busy {
            forget_button =
                forget_button.on_press(Message::SessionCatalogForgetRequested(id.clone()));
        }

        let clear_blocker = session_clear_cached_status_blocker(self.daemon_status.as_ref());
        let clear_controls: Element<'a, Message> =
            if self.session_catalog.pending_clear_id.as_deref() == Some(item.id.as_str()) {
                row![
                    button("Confirm Clear")
                        .style(theme::Button::Warning)
                        .on_press(Message::SessionCatalogClearConfirmed(id.clone())),
                    button("Cancel")
                        .style(theme::Button::Subtle)
                        .on_press(Message::SessionCatalogClearCanceled),
                ]
                .spacing(8)
                .into()
            } else {
                let mut clear_button = button("Clear Saved Data").style(theme::Button::Warning);
                if !busy && clear_blocker.is_none() {
                    clear_button =
                        clear_button.on_press(Message::SessionCatalogClearRequested(id.clone()));
                }
                clear_button.into()
            };

        let mut details = column![
            row![
                text(&item.display_name).size(16),
                text(session_artifact_status_label(item)).size(12)
            ]
            .spacing(12)
            .align_y(iced::Alignment::Center),
            text(&item.path_label).size(12),
        ]
        .spacing(4);
        if let Some(canonical) = item.canonical_path_label.as_deref() {
            details = details.push(text(format!("Canonical: {canonical}")).size(12));
        }
        details = details.push(
            row![
                text(format!("Created: {}", item.created_label)).size(12),
                text(format!("Opened: {}", item.last_opened_label)).size(12),
                text(format!("Saved: {}", item.last_saved_label)).size(12),
            ]
            .spacing(12),
        );

        let rename_input = text_input("Display name", &rename_value)
            .on_input(move |value| Message::SessionCatalogRenameInputChanged(id.clone(), value))
            .padding(8)
            .width(Length::Fill);

        let duplicate_id = item.id.clone();
        let duplicate_input = text_input("Duplicate target path", &duplicate_value)
            .on_input(move |value| {
                Message::SessionCatalogDuplicateInputChanged(duplicate_id.clone(), value)
            })
            .padding(8)
            .width(Length::Fill);

        let move_id = item.id.clone();
        let move_input = text_input("Move target path", &move_value)
            .on_input(move |value| Message::SessionCatalogMoveInputChanged(move_id.clone(), value))
            .padding(8)
            .width(Length::Fill);

        let rename_controls = row![rename_input, rename_button]
            .spacing(8)
            .align_y(iced::Alignment::Center);
        let duplicate_controls = row![duplicate_input, duplicate_button]
            .spacing(8)
            .align_y(iced::Alignment::Center);
        let move_controls = row![move_input, move_button]
            .spacing(8)
            .align_y(iced::Alignment::Center);

        let actions = row![reveal_button, clear_controls, forget_button]
            .spacing(8)
            .align_y(iced::Alignment::Center);

        container(
            column![
                details,
                rename_controls,
                duplicate_controls,
                move_controls,
                actions
            ]
            .spacing(8),
        )
        .padding(10)
        .width(Length::Fill)
        .style(theme::Container::Box)
        .into()
    }
}
