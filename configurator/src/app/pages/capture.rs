//! Capture page: screenshot destinations and the PDF export block.
//!
//! Both label colors route through the shared color row, so the hex text and
//! the color dialog feed the same `ColorPickerHexChanged` the Iced picker's
//! hex field sent; the four RGBA component boxes it drew are gone with it.

use relm4::prelude::*;

use crate::messages::Message;
use crate::models::color::parse_quad_values;
use crate::models::util::format_float;
use crate::models::{
    ColorPickerId, ColorQuadInput, PdfFitModeOption, PdfLabelContentModeOption,
    PdfLabelPositionOption, PdfOrientationOption, PdfPageSizeOption,
    PdfTransparentBackgroundOption, TabId, TextField, ToggleField,
};

use super::super::search::SearchArea;
use super::super::state::ConfiguratorApp;
use super::color_rows::{ResolvedColor, color_row};
use super::{BuiltPage, PageBuilder};

pub(super) fn build(sender: &ComponentSender<ConfiguratorApp>) -> BuiltPage {
    let mut page = PageBuilder::new(sender, TabId::Capture);

    page.group_in_area("Capture", SearchArea::CaptureFiles)
        .switch_row(
            "Enable capture shortcuts",
            "",
            |app| app.draft.capture_enabled,
            |value| Message::ToggleChanged(ToggleField::CaptureEnabled, value),
        )
        .entry_row(
            "Save directory",
            |app| app.draft.capture_save_directory.clone(),
            |value| Message::TextChanged(TextField::CaptureSaveDirectory, value),
        )
        .entry_row(
            "Filename template",
            |app| app.draft.capture_filename_template.clone(),
            |value| Message::TextChanged(TextField::CaptureFilename, value),
        )
        .entry_row(
            "Format (png, jpg, ...)",
            |app| app.draft.capture_format.clone(),
            |value| Message::TextChanged(TextField::CaptureFormat, value),
        )
        .switch_row(
            "Copy to clipboard",
            "",
            |app| app.draft.capture_copy_to_clipboard,
            |value| Message::ToggleChanged(ToggleField::CaptureCopyToClipboard, value),
        )
        .switch_row(
            "Always exit overlay after capture",
            "",
            |app| app.draft.capture_exit_after,
            |value| Message::ToggleChanged(ToggleField::CaptureExitAfter, value),
        );

    page.group_in_area("PDF export", SearchArea::CapturePdf)
        .entry_row(
            "PDF filename template (blank = capture template)",
            |app| app.draft.export_pdf_filename_template.clone(),
            |value| Message::TextChanged(TextField::ExportPdfFilenameTemplate, value),
        )
        .entry_row(
            "All boards PDF filename template",
            |app| app.draft.export_pdf_all_boards_filename_template.clone(),
            |value| Message::TextChanged(TextField::ExportPdfAllBoardsFilenameTemplate, value),
        )
        .combo_row(
            "Page size",
            "",
            PdfPageSizeOption::list(),
            labels(PdfPageSizeOption::list(), PdfPageSizeOption::label),
            |app| app.draft.export_pdf_page_size,
            Message::ExportPdfPageSizeChanged,
        )
        .combo_row(
            "Orientation",
            "",
            PdfOrientationOption::list(),
            labels(PdfOrientationOption::list(), PdfOrientationOption::label),
            |app| app.draft.export_pdf_orientation,
            Message::ExportPdfOrientationChanged,
        )
        .combo_row(
            "Fit",
            "",
            PdfFitModeOption::list(),
            labels(PdfFitModeOption::list(), PdfFitModeOption::label),
            |app| app.draft.export_pdf_fit,
            Message::ExportPdfFitChanged,
        )
        .combo_row(
            "Transparent page background",
            "",
            PdfTransparentBackgroundOption::list(),
            labels(
                PdfTransparentBackgroundOption::list(),
                PdfTransparentBackgroundOption::label,
            ),
            |app| app.draft.export_pdf_transparent_background,
            Message::ExportPdfTransparentBackgroundChanged,
        )
        .entry_row_validated(
            "Custom width (PDF points)",
            |app| app.draft.export_pdf_custom_width.clone(),
            |value| Message::TextChanged(TextField::ExportPdfCustomWidth, value),
            |app| validate_f64_range(&app.draft.export_pdf_custom_width, 1.0, 14400.0),
        )
        .entry_row_validated(
            "Custom height (PDF points)",
            |app| app.draft.export_pdf_custom_height.clone(),
            |value| Message::TextChanged(TextField::ExportPdfCustomHeight, value),
            |app| validate_f64_range(&app.draft.export_pdf_custom_height, 1.0, 14400.0),
        )
        .entry_row_validated(
            "Content source padding",
            |app| app.draft.export_pdf_content_source_padding.clone(),
            |value| Message::TextChanged(TextField::ExportPdfContentSourcePadding, value),
            |app| validate_f64_range(&app.draft.export_pdf_content_source_padding, 0.0, 4096.0),
        )
        .switch_row(
            "Show PDF page labels",
            "",
            |app| app.draft.export_pdf_labels_enabled,
            |value| Message::ToggleChanged(ToggleField::ExportPdfLabelsEnabled, value),
        )
        .combo_row(
            "Label position",
            "",
            PdfLabelPositionOption::list(),
            labels(
                PdfLabelPositionOption::list(),
                PdfLabelPositionOption::label,
            ),
            |app| app.draft.export_pdf_label_position,
            Message::ExportPdfLabelPositionChanged,
        )
        .combo_row(
            "Label content",
            "",
            PdfLabelContentModeOption::list(),
            labels(
                PdfLabelContentModeOption::list(),
                PdfLabelContentModeOption::label,
            ),
            |app| app.draft.export_pdf_label_content,
            Message::ExportPdfLabelContentChanged,
        )
        .entry_row(
            "Label template",
            |app| app.draft.export_pdf_label_template.clone(),
            |value| Message::TextChanged(TextField::ExportPdfLabelTemplate, value),
        )
        .entry_row(
            "Label font family",
            |app| app.draft.export_pdf_label_font_family.clone(),
            |value| Message::TextChanged(TextField::ExportPdfLabelFontFamily, value),
        )
        .entry_row_validated(
            "Label font size",
            |app| app.draft.export_pdf_label_font_size.clone(),
            |value| Message::TextChanged(TextField::ExportPdfLabelFontSize, value),
            |app| validate_f64_range(&app.draft.export_pdf_label_font_size, 1.0, 72.0),
        )
        .entry_row_validated(
            "Label margin",
            |app| app.draft.export_pdf_label_margin.clone(),
            |value| Message::TextChanged(TextField::ExportPdfLabelMargin, value),
            |app| validate_f64_range(&app.draft.export_pdf_label_margin, 0.0, 240.0),
        )
        .entry_row_validated(
            "Label horizontal padding",
            |app| app.draft.export_pdf_label_padding_x.clone(),
            |value| Message::TextChanged(TextField::ExportPdfLabelPaddingX, value),
            |app| validate_f64_range(&app.draft.export_pdf_label_padding_x, 0.0, 120.0),
        )
        .entry_row_validated(
            "Label vertical padding",
            |app| app.draft.export_pdf_label_padding_y.clone(),
            |value| Message::TextChanged(TextField::ExportPdfLabelPaddingY, value),
            |app| validate_f64_range(&app.draft.export_pdf_label_padding_y, 0.0, 120.0),
        );

    color_row(
        &mut page,
        "Label text color",
        ColorPickerId::ExportPdfLabelText,
        |app| quad_color(&app.draft.export_pdf_label_text_color),
    );

    page.switch_row(
        "Label solid background",
        "",
        |app| app.draft.export_pdf_label_background_enabled,
        |value| Message::ToggleChanged(ToggleField::ExportPdfLabelBackgroundEnabled, value),
    );

    color_row(
        &mut page,
        "Label background color",
        ColorPickerId::ExportPdfLabelBackground,
        |app| quad_color(&app.draft.export_pdf_label_background_color),
    );

    page.finish()
}

/// Error text for a fractional field constrained to `min..=max`, `None`
/// while the input is acceptable.
///
/// The tablet page validates the same way; `pages/mod.rs` owns the
/// whole-number twin of this helper.
pub(super) fn validate_f64_range(value: &str, min: f64, max: f64) -> Option<String> {
    match value.trim().parse::<f64>() {
        Ok(parsed) if parsed.is_finite() && (min..=max).contains(&parsed) => None,
        _ => Some(format!(
            "Enter a number between {} and {}.",
            format_float(min),
            format_float(max)
        )),
    }
}

/// The dialog/swatch seed for a `0.0..=1.0` RGBA quad.
fn quad_color(input: &ColorQuadInput) -> ResolvedColor {
    let [red, green, blue, alpha] = parse_quad_values(&input.components);
    Some((red, green, blue, alpha))
}

/// Combo labels in the same order as the values they name.
fn labels<O: Copy>(values: Vec<O>, label: impl Fn(O) -> &'static str) -> Vec<String> {
    values
        .into_iter()
        .map(|value| label(value).to_string())
        .collect()
}
