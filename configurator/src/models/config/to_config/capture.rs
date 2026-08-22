use super::super::draft::ConfigDraft;
use crate::models::error::FormError;
use wayscriber::config::{
    Config, validate_capture_format, validate_filename_template, validate_ocr_languages,
};

impl ConfigDraft {
    pub(super) fn apply_capture(&self, config: &mut Config, errors: &mut Vec<FormError>) {
        config.capture.enabled = self.capture_enabled;
        config.capture.save_directory = self.capture_save_directory.clone();
        match validate_filename_template(&self.capture_filename_template) {
            Ok(()) => config.capture.filename_template = self.capture_filename_template.clone(),
            Err(reason) => errors.push(FormError::new(
                "capture.filename_template",
                format!("Filename template: {reason}."),
            )),
        }
        match validate_capture_format(&self.capture_format) {
            Ok(format) => config.capture.format = format,
            Err(reason) => errors.push(FormError::new(
                "capture.format",
                format!("Image format: {reason}."),
            )),
        }
        config.capture.copy_to_clipboard = self.capture_copy_to_clipboard;
        config.capture.exit_after_capture = self.capture_exit_after;
        config.capture.region.picker = self.capture_region_picker.to_picker();
        config.capture.region.show_size_readout = self.capture_region_show_size_readout;
        config.capture.region.show_loupe = self.capture_region_show_loupe;
        config.capture.region.show_legend = self.capture_region_show_legend;
        match validate_ocr_languages(&self.capture_ocr_languages) {
            Ok(languages) => config.capture.ocr_languages = languages,
            Err(reason) => errors.push(FormError::new(
                "capture.ocr_languages",
                format!("OCR languages: {reason}."),
            )),
        }
    }
}
