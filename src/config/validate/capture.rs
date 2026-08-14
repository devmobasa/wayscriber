use super::super::{CaptureConfig, Config, validate_capture_format, validate_filename_template};

impl Config {
    pub(super) fn validate_capture(&mut self) {
        if let Err(reason) = validate_filename_template(&self.capture.filename_template) {
            log::warn!("Invalid capture.filename_template ({reason}); resetting to default");
            self.capture.filename_template = CaptureConfig::default().filename_template;
        }
        match validate_capture_format(&self.capture.format) {
            Ok(format) => self.capture.format = format,
            Err(reason) => {
                log::warn!("Invalid capture.format ({reason}); resetting to png");
                self.capture.format = CaptureConfig::default().format;
            }
        }
    }
}
