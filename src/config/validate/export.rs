use super::super::{
    Config, PDF_LABEL_DEFAULT_TEMPLATE, PdfLabelConfig, PdfLabelContentMode,
    validate_pdf_label_template,
};

const PDF_DIMENSION_MIN: f64 = 1.0;
const PDF_DIMENSION_MAX: f64 = 14_400.0;
const PDF_CONTENT_SOURCE_PADDING_MAX: f64 = 4_096.0;
const PDF_LABEL_FONT_MIN: f64 = 1.0;
const PDF_LABEL_FONT_MAX: f64 = 72.0;
const PDF_LABEL_MARGIN_MAX: f64 = 240.0;
const PDF_LABEL_PADDING_MAX: f64 = 120.0;

impl Config {
    pub(super) fn validate_export(&mut self) {
        self.export.pdf.custom_width = sanitize_range(
            self.export.pdf.custom_width,
            PDF_DIMENSION_MIN,
            PDF_DIMENSION_MAX,
            800.0,
            "export.pdf.custom_width",
        );
        self.export.pdf.custom_height = sanitize_range(
            self.export.pdf.custom_height,
            PDF_DIMENSION_MIN,
            PDF_DIMENSION_MAX,
            600.0,
            "export.pdf.custom_height",
        );
        self.export.pdf.content_source_padding = sanitize_range(
            self.export.pdf.content_source_padding,
            0.0,
            PDF_CONTENT_SOURCE_PADDING_MAX,
            24.0,
            "export.pdf.content_source_padding",
        );
        validate_pdf_labels(&mut self.export.pdf.labels);
    }
}

fn validate_pdf_labels(labels: &mut PdfLabelConfig) {
    if labels.content == PdfLabelContentMode::CustomTemplate {
        if labels.template.trim().is_empty() {
            labels.template = PDF_LABEL_DEFAULT_TEMPLATE.to_string();
        } else if let Err(err) = validate_pdf_label_template(&labels.template) {
            log::warn!(
                "Invalid export.pdf.labels.template ({}); resetting to default",
                err
            );
            labels.template = PDF_LABEL_DEFAULT_TEMPLATE.to_string();
        }
    }

    if labels.font_family.trim().is_empty() {
        labels.font_family = "Sans".to_string();
    } else {
        labels.font_family = labels.font_family.trim().to_string();
    }

    labels.font_size = sanitize_range(
        labels.font_size,
        PDF_LABEL_FONT_MIN,
        PDF_LABEL_FONT_MAX,
        10.0,
        "export.pdf.labels.font_size",
    );
    labels.margin = sanitize_range(
        labels.margin,
        0.0,
        PDF_LABEL_MARGIN_MAX,
        12.0,
        "export.pdf.labels.margin",
    );
    labels.padding_x = sanitize_range(
        labels.padding_x,
        0.0,
        PDF_LABEL_PADDING_MAX,
        6.0,
        "export.pdf.labels.padding_x",
    );
    labels.padding_y = sanitize_range(
        labels.padding_y,
        0.0,
        PDF_LABEL_PADDING_MAX,
        3.0,
        "export.pdf.labels.padding_y",
    );
    sanitize_color(&mut labels.text_color, [0.1, 0.1, 0.1, 1.0]);
    sanitize_color(&mut labels.background_color, [1.0, 1.0, 1.0, 0.85]);
}

fn sanitize_range(value: f64, min: f64, max: f64, fallback: f64, field: &str) -> f64 {
    if !value.is_finite() {
        log::warn!("Invalid {field}: {value}; resetting to {fallback}");
        return fallback;
    }
    let clamped = value.clamp(min, max);
    if (clamped - value).abs() > f64::EPSILON {
        log::warn!("Clamping {field} from {value} to {clamped}");
    }
    clamped
}

fn sanitize_color(color: &mut [f64; 4], fallback: [f64; 4]) {
    for (index, component) in color.iter_mut().enumerate() {
        if !component.is_finite() {
            *component = fallback[index];
        } else {
            *component = component.clamp(0.0, 1.0);
        }
    }
}
