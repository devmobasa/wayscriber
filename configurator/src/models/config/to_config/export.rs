use super::super::draft::ConfigDraft;
use crate::models::error::FormError;
use crate::models::util::parse_f64;
use wayscriber::config::{Config, PdfLabelContentMode, validate_pdf_label_template};

impl ConfigDraft {
    pub(super) fn apply_export(&self, config: &mut Config, errors: &mut Vec<FormError>) {
        config.export.pdf.filename_template = non_empty(self.export_pdf_filename_template.clone());
        config.export.pdf.all_boards_filename_template =
            non_empty(self.export_pdf_all_boards_filename_template.clone());
        config.export.pdf.page_size = self.export_pdf_page_size.to_config();
        config.export.pdf.orientation = self.export_pdf_orientation.to_config();
        config.export.pdf.fit = self.export_pdf_fit.to_config();
        parse_pdf_number(
            &self.export_pdf_custom_width,
            "export.pdf.custom_width",
            1.0,
            14_400.0,
            errors,
            |value| config.export.pdf.custom_width = value,
        );
        parse_pdf_number(
            &self.export_pdf_custom_height,
            "export.pdf.custom_height",
            1.0,
            14_400.0,
            errors,
            |value| config.export.pdf.custom_height = value,
        );
        parse_pdf_number(
            &self.export_pdf_content_source_padding,
            "export.pdf.content_source_padding",
            0.0,
            4_096.0,
            errors,
            |value| config.export.pdf.content_source_padding = value,
        );

        config.export.pdf.labels.enabled = self.export_pdf_labels_enabled;
        config.export.pdf.labels.position = self.export_pdf_label_position.to_config();
        let label_content = self.export_pdf_label_content.to_config();
        config.export.pdf.labels.content = label_content;
        match validate_pdf_label_template_for_content(
            &self.export_pdf_label_template,
            label_content,
        ) {
            Ok(true) => {
                config.export.pdf.labels.template = self.export_pdf_label_template.clone();
            }
            Ok(false) => {}
            Err(err) => {
                errors.push(FormError::new("export.pdf.labels.template", err));
            }
        }
        config.export.pdf.labels.font_family = self.export_pdf_label_font_family.clone();
        parse_pdf_number(
            &self.export_pdf_label_font_size,
            "export.pdf.labels.font_size",
            1.0,
            72.0,
            errors,
            |value| config.export.pdf.labels.font_size = value,
        );
        parse_pdf_number(
            &self.export_pdf_label_margin,
            "export.pdf.labels.margin",
            0.0,
            240.0,
            errors,
            |value| config.export.pdf.labels.margin = value,
        );
        parse_pdf_number(
            &self.export_pdf_label_padding_x,
            "export.pdf.labels.padding_x",
            0.0,
            120.0,
            errors,
            |value| config.export.pdf.labels.padding_x = value,
        );
        parse_pdf_number(
            &self.export_pdf_label_padding_y,
            "export.pdf.labels.padding_y",
            0.0,
            120.0,
            errors,
            |value| config.export.pdf.labels.padding_y = value,
        );
        match self
            .export_pdf_label_text_color
            .to_array("export.pdf.labels.text_color")
        {
            Ok(values) => config.export.pdf.labels.text_color = values,
            Err(err) => errors.push(err),
        }
        config.export.pdf.labels.background_enabled = self.export_pdf_label_background_enabled;
        match self
            .export_pdf_label_background_color
            .to_array("export.pdf.labels.background_color")
        {
            Ok(values) => config.export.pdf.labels.background_color = values,
            Err(err) => errors.push(err),
        }
    }
}

fn non_empty(value: String) -> Option<String> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

fn validate_pdf_label_template_for_content(
    template: &str,
    content: PdfLabelContentMode,
) -> Result<bool, String> {
    let is_custom = content == PdfLabelContentMode::CustomTemplate;
    if is_custom && template.trim().is_empty() {
        return Err("Expected a non-empty template".to_string());
    }
    match validate_pdf_label_template(template) {
        Ok(()) => Ok(true),
        Err(err) if is_custom => Err(err),
        Err(_) => Ok(false),
    }
}

fn parse_pdf_number(
    value: &str,
    field: &'static str,
    min: f64,
    max: f64,
    errors: &mut Vec<FormError>,
    apply: impl FnOnce(f64),
) {
    match parse_f64(value.trim()) {
        Ok(parsed) if !parsed.is_finite() => {
            errors.push(FormError::new(field, "Expected a finite numeric value"));
        }
        Ok(parsed) if parsed < min || parsed > max => {
            errors.push(FormError::new(field, format!("Expected {min}-{max}")));
        }
        Ok(parsed) => apply(parsed),
        Err(err) => errors.push(FormError::new(field, err)),
    }
}
