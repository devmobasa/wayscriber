use super::super::draft::ConfigDraft;
use crate::models::error::FormError;
use wayscriber::config::Config;

impl ConfigDraft {
    pub(super) fn apply_capture(&self, config: &mut Config, _errors: &mut Vec<FormError>) {
        config.capture.enabled = self.capture_enabled;
        config.capture.save_directory = self.capture_save_directory.clone();
        config.capture.filename_template = self.capture_filename_template.clone();
        config.capture.format = self.capture_format.clone();
        config.capture.copy_to_clipboard = self.capture_copy_to_clipboard;
        config.capture.exit_after_capture = self.capture_exit_after;
    }
}
