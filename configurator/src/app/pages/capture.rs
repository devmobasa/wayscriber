//! Capture page: screenshot destinations and the PDF export block.
//!
//! Both label colors route through the shared color row, so the hex text and
//! the color dialog feed the same `ColorPickerHexChanged` the Iced picker's
//! hex field sent; the four RGBA component boxes it drew are gone with it.

use wayscriber::i18n::{tr, trf};

use relm4::prelude::*;

use crate::messages::Message;
use crate::models::color::parse_quad_values;
use crate::models::util::format_float;
use crate::models::{
    ColorPickerId, ColorQuadInput, PdfFitModeOption, PdfLabelContentModeOption,
    PdfLabelPositionOption, PdfOrientationOption, PdfPageSizeOption,
    PdfTransparentBackgroundOption, RegionPickerOption, TabId, TextField, ToggleField,
};

use wayscriber::config::validate_ocr_languages;

use super::super::search::SearchArea;
use super::super::state::ConfiguratorApp;
use super::color_rows::{ResolvedColor, color_row};
use super::{BuiltPage, PageBuilder};

pub(super) fn build(sender: &ComponentSender<ConfiguratorApp>) -> BuiltPage {
    let mut page = PageBuilder::new(sender, TabId::Capture);

    page.group_in_area(tr("Capture"), SearchArea::CaptureFiles)
        .switch_row(
            tr("Enable capture shortcuts"),
            "",
            |app| app.draft.capture.enabled,
            |value| Message::ToggleChanged(ToggleField::CaptureEnabled, value),
        )
        .entry_row(
            tr("Save directory"),
            |app| app.draft.capture.save_directory.clone(),
            |value| Message::TextChanged(TextField::CaptureSaveDirectory, value),
        )
        .entry_row(
            tr("Filename template"),
            |app| app.draft.capture.filename_template.clone(),
            |value| Message::TextChanged(TextField::CaptureFilename, value),
        )
        .entry_row(
            tr("Format (png, jpg, ...)"),
            |app| app.draft.capture.format.clone(),
            |value| Message::TextChanged(TextField::CaptureFormat, value),
        )
        .switch_row(
            tr("Copy to clipboard"),
            "",
            |app| app.draft.capture.copy_to_clipboard,
            |value| Message::ToggleChanged(ToggleField::CaptureCopyToClipboard, value),
        )
        .switch_row(
            tr("Include drawings in screenshots"),
            tr("Composites the active board's committed annotations into full-screen and region captures by default. Interactive region capture can override this per screenshot."),
            |app| app.draft.capture.include_drawings,
            |value| Message::ToggleChanged(ToggleField::CaptureIncludeDrawings, value),
        )
        .switch_row(
            tr("Always exit overlay after capture"),
            "",
            |app| app.draft.capture.exit_after,
            |value| Message::ToggleChanged(ToggleField::CaptureExitAfter, value),
        )
        .entry_row_validated(
            tr("OCR languages (e.g. eng, eng+deu; needs the matching Tesseract packages)"),
            |app| app.draft.capture.ocr_languages.clone(),
            |value| Message::TextChanged(TextField::CaptureOcrLanguages, value),
            |app| {
                validate_ocr_languages(&app.draft.capture.ocr_languages)
                    .err()
                    .map(|reason| {
                        format!("{reason}. Use Tesseract codes joined by '+', such as eng+deu.")
                    })
            },
        );

    page.group_in_area(tr("Region picker"), SearchArea::CaptureRegion)
        .combo_row(
            tr("Selection frontend"),
            tr("Native draws the picker over a frozen desktop image. Slurp uses the external selector."),
            RegionPickerOption::list(),
            labels(RegionPickerOption::list(), RegionPickerOption::label),
            |app| app.draft.capture.region_picker,
            Message::CaptureRegionPickerChanged,
        )
        .switch_row(
            tr("Show pointer position and selection size"),
            tr("Shows coordinates while idle and the selected export size while dragging."),
            |app| app.draft.capture.region_show_size_readout,
            |value| Message::ToggleChanged(ToggleField::CaptureRegionShowSizeReadout, value),
        )
        .switch_row(
            tr("Show magnified pixel loupe"),
            tr("Shows a magnified pixel grid beside the pointer while selecting or reviewing."),
            |app| app.draft.capture.region_show_loupe,
            |value| Message::ToggleChanged(ToggleField::CaptureRegionShowLoupe, value),
        )
        .switch_row(
            tr("Show hotkey legend"),
            tr("Shows a short picker hotkey guide until the first drag."),
            |app| app.draft.capture.region_show_legend,
            |value| Message::ToggleChanged(ToggleField::CaptureRegionShowLegend, value),
        );

    page.group_in_area(tr("PDF export"), SearchArea::CapturePdf)
        .entry_row(
            tr("PDF filename template (blank = capture template)"),
            |app| app.draft.export_pdf_filename_template.clone(),
            |value| Message::TextChanged(TextField::ExportPdfFilenameTemplate, value),
        )
        .entry_row(
            tr("All boards PDF filename template"),
            |app| app.draft.export_pdf_all_boards_filename_template.clone(),
            |value| Message::TextChanged(TextField::ExportPdfAllBoardsFilenameTemplate, value),
        )
        .combo_row(
            tr("Page size"),
            "",
            PdfPageSizeOption::list(),
            labels(PdfPageSizeOption::list(), PdfPageSizeOption::label),
            |app| app.draft.export_pdf_page_size,
            Message::ExportPdfPageSizeChanged,
        )
        .combo_row(
            tr("Orientation"),
            "",
            PdfOrientationOption::list(),
            labels(PdfOrientationOption::list(), PdfOrientationOption::label),
            |app| app.draft.export_pdf_orientation,
            Message::ExportPdfOrientationChanged,
        )
        .combo_row(
            tr("Fit"),
            "",
            PdfFitModeOption::list(),
            labels(PdfFitModeOption::list(), PdfFitModeOption::label),
            |app| app.draft.export_pdf_fit,
            Message::ExportPdfFitChanged,
        )
        .combo_row(
            tr("Transparent page background"),
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
            tr("Custom width (PDF points)"),
            |app| app.draft.export_pdf_custom_width.clone(),
            |value| Message::TextChanged(TextField::ExportPdfCustomWidth, value),
            |app| validate_f64_range(&app.draft.export_pdf_custom_width, 1.0, 14400.0),
        )
        .entry_row_validated(
            tr("Custom height (PDF points)"),
            |app| app.draft.export_pdf_custom_height.clone(),
            |value| Message::TextChanged(TextField::ExportPdfCustomHeight, value),
            |app| validate_f64_range(&app.draft.export_pdf_custom_height, 1.0, 14400.0),
        )
        .entry_row_validated(
            tr("Content source padding"),
            |app| app.draft.export_pdf_content_source_padding.clone(),
            |value| Message::TextChanged(TextField::ExportPdfContentSourcePadding, value),
            |app| validate_f64_range(&app.draft.export_pdf_content_source_padding, 0.0, 4096.0),
        )
        .switch_row(
            tr("Show PDF page labels"),
            "",
            |app| app.draft.export_pdf_labels_enabled,
            |value| Message::ToggleChanged(ToggleField::ExportPdfLabelsEnabled, value),
        )
        .combo_row(
            tr("Label position"),
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
            tr("Label content"),
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
            tr("Label template"),
            |app| app.draft.export_pdf_label_template.clone(),
            |value| Message::TextChanged(TextField::ExportPdfLabelTemplate, value),
        )
        .entry_row(
            tr("Label font family"),
            |app| app.draft.export_pdf_label_font_family.clone(),
            |value| Message::TextChanged(TextField::ExportPdfLabelFontFamily, value),
        )
        .entry_row_validated(
            tr("Label font size"),
            |app| app.draft.export_pdf_label_font_size.clone(),
            |value| Message::TextChanged(TextField::ExportPdfLabelFontSize, value),
            |app| validate_f64_range(&app.draft.export_pdf_label_font_size, 1.0, 72.0),
        )
        .entry_row_validated(
            tr("Label margin"),
            |app| app.draft.export_pdf_label_margin.clone(),
            |value| Message::TextChanged(TextField::ExportPdfLabelMargin, value),
            |app| validate_f64_range(&app.draft.export_pdf_label_margin, 0.0, 240.0),
        )
        .entry_row_validated(
            tr("Label horizontal padding"),
            |app| app.draft.export_pdf_label_padding_x.clone(),
            |value| Message::TextChanged(TextField::ExportPdfLabelPaddingX, value),
            |app| validate_f64_range(&app.draft.export_pdf_label_padding_x, 0.0, 120.0),
        )
        .entry_row_validated(
            tr("Label vertical padding"),
            |app| app.draft.export_pdf_label_padding_y.clone(),
            |value| Message::TextChanged(TextField::ExportPdfLabelPaddingY, value),
            |app| validate_f64_range(&app.draft.export_pdf_label_padding_y, 0.0, 120.0),
        );

    color_row(
        &mut page,
        tr("Label text color"),
        ColorPickerId::ExportPdfLabelText,
        |app| quad_color(&app.draft.export_pdf_label_text_color),
    );

    page.switch_row(
        tr("Label solid background"),
        "",
        |app| app.draft.export_pdf_label_background_enabled,
        |value| Message::ToggleChanged(ToggleField::ExportPdfLabelBackgroundEnabled, value),
    );

    color_row(
        &mut page,
        tr("Label background color"),
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
        _ => Some(trf(
            tr("Enter a number between {} and {}."),
            &[&format_float(min), &format_float(max)],
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
