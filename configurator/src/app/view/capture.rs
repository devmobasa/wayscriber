use iced::Element;
use iced::widget::{column, scrollable, text};

use crate::messages::Message;
use crate::models::{TextField, ToggleField};

use super::super::state::ConfiguratorApp;
use super::widgets::{labeled_input, toggle_row};

impl ConfiguratorApp {
    pub(super) fn capture_tab(&self) -> Element<'_, Message> {
        scrollable(
            column![
                text("Capture Settings").size(20),
                toggle_row(
                    "Enable capture shortcuts",
                    self.draft.capture_enabled,
                    self.defaults.capture_enabled,
                    ToggleField::CaptureEnabled,
                ),
                labeled_input(
                    "Save directory",
                    &self.draft.capture_save_directory,
                    &self.defaults.capture_save_directory,
                    TextField::CaptureSaveDirectory,
                ),
                labeled_input(
                    "Filename template",
                    &self.draft.capture_filename_template,
                    &self.defaults.capture_filename_template,
                    TextField::CaptureFilename,
                ),
                labeled_input(
                    "Format (png, jpg, ...)",
                    &self.draft.capture_format,
                    &self.defaults.capture_format,
                    TextField::CaptureFormat,
                ),
                toggle_row(
                    "Copy to clipboard",
                    self.draft.capture_copy_to_clipboard,
                    self.defaults.capture_copy_to_clipboard,
                    ToggleField::CaptureCopyToClipboard,
                ),
                toggle_row(
                    "Always exit overlay after capture",
                    self.draft.capture_exit_after,
                    self.defaults.capture_exit_after,
                    ToggleField::CaptureExitAfter,
                )
            ]
            .spacing(12),
        )
        .into()
    }
}
