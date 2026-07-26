use super::super::draft::ConfigDraft;
use crate::models::error::FormError;
use wayscriber::config::Config;

impl ConfigDraft {
    pub(super) fn apply_capture(
        &self,
        config: &mut Config,
        errors: &mut Vec<FormError>,
        paths: &wayscriber::paths::PathResolver,
    ) {
        config.capture.enabled = self.capture_enabled;
        config.capture.save_directory = self.capture_save_directory.clone();
        config.capture.filename_template = self.capture_filename_template.clone();
        config.capture.format = self.capture_format.clone();
        config.capture.copy_to_clipboard = self.capture_copy_to_clipboard;
        config.capture.exit_after_capture = self.capture_exit_after;
        if let Err(error) = paths.require_absolute_user_path(
            self.capture_save_directory.trim(),
            wayscriber::paths::PathCapability::Pictures,
        ) {
            errors.push(FormError::new("capture.save_directory", error.to_string()));
        }
    }
}
