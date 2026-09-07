use crate::models::error::FormError;
use crate::models::fields::RegionPickerOption;
use wayscriber::config::{
    CaptureConfig, validate_capture_format, validate_filename_template, validate_ocr_languages,
};

/// Editable capture values, including intermediate or invalid text.
#[derive(Debug, Clone, PartialEq)]
pub struct CaptureDraft {
    pub enabled: bool,
    pub save_directory: String,
    pub filename_template: String,
    pub format: String,
    pub copy_to_clipboard: bool,
    pub include_drawings: bool,
    pub exit_after: bool,
    pub ocr_languages: String,
    pub region_picker: RegionPickerOption,
    pub region_show_size_readout: bool,
    pub region_show_loupe: bool,
    pub region_show_legend: bool,
}

impl CaptureDraft {
    pub(super) fn from_config(config: &CaptureConfig) -> Self {
        Self {
            enabled: config.enabled,
            save_directory: config.save_directory.clone(),
            filename_template: config.filename_template.clone(),
            format: config.format.clone(),
            copy_to_clipboard: config.copy_to_clipboard,
            include_drawings: config.include_drawings,
            exit_after: config.exit_after_capture,
            ocr_languages: config.ocr_languages.clone(),
            region_picker: RegionPickerOption::from_picker(config.region.picker),
            region_show_size_readout: config.region.show_size_readout,
            region_show_loupe: config.region.show_loupe,
            region_show_legend: config.region.show_legend,
        }
    }

    pub(super) fn apply_to(&self, config: &mut CaptureConfig, errors: &mut Vec<FormError>) {
        config.enabled = self.enabled;
        config.save_directory = self.save_directory.clone();
        match validate_filename_template(&self.filename_template) {
            Ok(()) => config.filename_template = self.filename_template.clone(),
            Err(reason) => errors.push(FormError::new(
                "capture.filename_template",
                format!("Filename template: {reason}."),
            )),
        }
        match validate_capture_format(&self.format) {
            Ok(format) => config.format = format,
            Err(reason) => errors.push(FormError::new(
                "capture.format",
                format!("Image format: {reason}."),
            )),
        }
        config.copy_to_clipboard = self.copy_to_clipboard;
        config.include_drawings = self.include_drawings;
        config.exit_after_capture = self.exit_after;
        config.region.picker = self.region_picker.to_picker();
        config.region.show_size_readout = self.region_show_size_readout;
        config.region.show_loupe = self.region_show_loupe;
        config.region.show_legend = self.region_show_legend;
        match validate_ocr_languages(&self.ocr_languages) {
            Ok(languages) => config.ocr_languages = languages,
            Err(reason) => errors.push(FormError::new(
                "capture.ocr_languages",
                format!("OCR languages: {reason}."),
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn capture_round_trip_and_invalid_text_preserve_input() {
        let original = CaptureConfig::default();
        let mut draft = CaptureDraft::from_config(&original);
        let mut saved = original.clone();
        let mut errors = Vec::new();
        draft.apply_to(&mut saved, &mut errors);
        assert!(errors.is_empty());
        assert_eq!(CaptureDraft::from_config(&saved), draft);
        draft.format = "bad-format".into();
        draft.filename_template = "../escape".into();
        draft.ocr_languages = "!invalid!".into();
        draft.apply_to(&mut saved, &mut errors);
        assert_eq!(errors.len(), 3);
        assert_eq!(draft.format, "bad-format");
        assert_eq!(saved.format, original.format);
        assert_eq!(saved.filename_template, original.filename_template);
        assert_eq!(saved.ocr_languages, original.ocr_languages);
    }
}
