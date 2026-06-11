use serde::{Deserialize, Serialize};

use super::capture::CaptureConfig;

pub const PDF_LABEL_DEFAULT_TEMPLATE: &str =
    "{board_name} - {page_name} ({document_page}/{document_pages})";
pub const PDF_LABEL_APP_BOARD: &str = "app_board";
pub const PDF_LABEL_APP_BOARDS: &str = "app_boards";
pub const PDF_LABEL_EXPORT_BOARD: &str = "export_board";
pub const PDF_LABEL_EXPORT_BOARDS: &str = "export_boards";
pub const PDF_LABEL_PAGE: &str = "page";
pub const PDF_LABEL_PAGES: &str = "pages";
pub const PDF_LABEL_DOCUMENT_PAGE: &str = "document_page";
pub const PDF_LABEL_DOCUMENT_PAGES: &str = "document_pages";
pub const PDF_LABEL_BOARD_NAME: &str = "board_name";
pub const PDF_LABEL_PAGE_NAME: &str = "page_name";
pub const PDF_LABEL_PLACEHOLDERS: &[&str] = &[
    PDF_LABEL_APP_BOARD,
    PDF_LABEL_APP_BOARDS,
    PDF_LABEL_EXPORT_BOARD,
    PDF_LABEL_EXPORT_BOARDS,
    PDF_LABEL_PAGE,
    PDF_LABEL_PAGES,
    PDF_LABEL_DOCUMENT_PAGE,
    PDF_LABEL_DOCUMENT_PAGES,
    PDF_LABEL_BOARD_NAME,
    PDF_LABEL_PAGE_NAME,
];

#[cfg_attr(feature = "config-schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default, deny_unknown_fields)]
pub struct ExportConfig {
    pub pdf: PdfExportConfig,
}

#[cfg_attr(feature = "config-schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PdfExportConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filename_template: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub all_boards_filename_template: Option<String>,
    pub page_size: PdfPageSize,
    pub orientation: PdfOrientation,
    pub fit: PdfFitMode,
    pub transparent_background: PdfTransparentBackground,
    pub custom_width: f64,
    pub custom_height: f64,
    pub content_source_padding: f64,
    pub labels: PdfLabelConfig,
}

impl Default for PdfExportConfig {
    fn default() -> Self {
        Self {
            filename_template: None,
            all_boards_filename_template: None,
            page_size: PdfPageSize::Viewport,
            orientation: PdfOrientation::Auto,
            fit: PdfFitMode::Viewport,
            transparent_background: PdfTransparentBackground::None,
            custom_width: 800.0,
            custom_height: 600.0,
            content_source_padding: 24.0,
            labels: PdfLabelConfig::default(),
        }
    }
}

impl PdfExportConfig {
    pub fn resolved_filename_template(&self, capture: &CaptureConfig) -> String {
        self.filename_template
            .as_deref()
            .map(str::trim)
            .filter(|template| !template.is_empty())
            .unwrap_or(&capture.filename_template)
            .to_string()
    }

    pub fn resolved_all_boards_filename_template(&self, capture: &CaptureConfig) -> String {
        self.all_boards_filename_template
            .as_deref()
            .map(str::trim)
            .filter(|template| !template.is_empty())
            .or_else(|| {
                self.filename_template
                    .as_deref()
                    .map(str::trim)
                    .filter(|template| !template.is_empty())
            })
            .unwrap_or(&capture.filename_template)
            .to_string()
    }
}

#[cfg_attr(feature = "config-schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum PdfPageSize {
    #[default]
    Viewport,
    A4,
    Letter,
    Custom,
}

#[cfg_attr(feature = "config-schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum PdfOrientation {
    #[default]
    Auto,
    Portrait,
    Landscape,
}

#[cfg_attr(feature = "config-schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum PdfFitMode {
    #[default]
    Viewport,
    FitViewportToPage,
    FitContentToPage,
}

#[cfg_attr(feature = "config-schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum PdfTransparentBackground {
    #[default]
    None,
    Desktop,
}

#[cfg_attr(feature = "config-schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum PdfLabelPosition {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
    #[default]
    BottomCenter,
}

#[cfg_attr(feature = "config-schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum PdfLabelContentMode {
    #[default]
    CustomTemplate,
    BoardAndPage,
    DocumentPage,
    BoardName,
    PageName,
}

#[cfg_attr(feature = "config-schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PdfLabelConfig {
    pub enabled: bool,
    pub position: PdfLabelPosition,
    pub content: PdfLabelContentMode,
    pub template: String,
    pub font_family: String,
    pub font_size: f64,
    pub margin: f64,
    pub padding_x: f64,
    pub padding_y: f64,
    pub text_color: [f64; 4],
    pub background_enabled: bool,
    pub background_color: [f64; 4],
}

impl Default for PdfLabelConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            position: PdfLabelPosition::default(),
            content: PdfLabelContentMode::default(),
            template: PDF_LABEL_DEFAULT_TEMPLATE.to_string(),
            font_family: "Sans".to_string(),
            font_size: 10.0,
            margin: 12.0,
            padding_x: 6.0,
            padding_y: 3.0,
            text_color: [0.1, 0.1, 0.1, 1.0],
            background_enabled: true,
            background_color: [1.0, 1.0, 1.0, 0.85],
        }
    }
}

pub fn validate_pdf_label_template(template: &str) -> Result<(), String> {
    let mut chars = template.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '{' if chars.peek() == Some(&'{') => {
                chars.next();
            }
            '}' if chars.peek() == Some(&'}') => {
                chars.next();
            }
            '{' => {
                let mut name = String::new();
                let mut closed = false;
                for next in chars.by_ref() {
                    if next == '}' {
                        closed = true;
                        break;
                    }
                    if next == '{' {
                        return Err("Nested label template braces are not supported".to_string());
                    }
                    name.push(next);
                }
                if !closed {
                    return Err("Unclosed label template placeholder".to_string());
                }
                if name.is_empty() {
                    return Err("Empty label template placeholder".to_string());
                }
                if !PDF_LABEL_PLACEHOLDERS.contains(&name.as_str()) {
                    return Err(format!("Unknown label template placeholder: {name}"));
                }
            }
            '}' => return Err("Unmatched label template closing brace".to_string()),
            _ => {}
        }
    }
    Ok(())
}
