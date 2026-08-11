use super::super::draft::ConfigDraft;
use crate::models::error::FormError;
use wayscriber::config::{Config, validate_ocr_languages};

impl ConfigDraft {
    pub(super) fn apply_capture(&self, config: &mut Config, errors: &mut Vec<FormError>) {
        config.capture.enabled = self.capture_enabled;
        config.capture.save_directory = self.capture_save_directory.clone();
        config.capture.filename_template = self.capture_filename_template.clone();
        config.capture.format = self.capture_format.clone();
        config.capture.copy_to_clipboard = self.capture_copy_to_clipboard;
        config.capture.exit_after_capture = self.capture_exit_after;
        match validate_ocr_languages(&self.capture_ocr_languages) {
            Ok(languages) => config.capture.ocr_languages = languages,
            Err(reason) => errors.push(FormError::new(
                "capture.ocr_languages",
                format!("OCR languages: {reason}."),
            )),
        }
    }
}
