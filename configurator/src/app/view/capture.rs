use iced::widget::{column, pick_list, scrollable, text};
use iced::{Element, Length};

use crate::messages::Message;
use crate::models::{
    ColorPickerId, PdfFitModeOption, PdfLabelContentModeOption, PdfLabelPositionOption,
    PdfOrientationOption, PdfPageSizeOption, PdfTransparentBackgroundOption, QuadField, TextField,
    ToggleField,
};

use super::super::search::{SearchArea, TabSearchSummary};
use super::super::state::ConfiguratorApp;
use super::widgets::{
    ColorPickerUi, color_quad_picker, labeled_control, labeled_input, labeled_input_with_feedback,
    toggle_row, validate_f64_range,
};

impl ConfiguratorApp {
    pub(super) fn capture_tab(&self, search: Option<&TabSearchSummary>) -> Element<'_, Message> {
        let show_files = search.is_none_or(|search| search.area_matches(SearchArea::CaptureFiles));
        let show_pdf = search.is_none_or(|search| search.area_matches(SearchArea::CapturePdf));
        let mut content = column![text("Capture Settings").size(20)].spacing(12);

        if show_files {
            content = content
                .push(toggle_row(
                    "Enable capture shortcuts",
                    self.draft.capture_enabled,
                    self.defaults.capture_enabled,
                    ToggleField::CaptureEnabled,
                ))
                .push(labeled_input(
                    "Save directory",
                    &self.draft.capture_save_directory,
                    &self.defaults.capture_save_directory,
                    TextField::CaptureSaveDirectory,
                ))
                .push(labeled_input(
                    "Filename template",
                    &self.draft.capture_filename_template,
                    &self.defaults.capture_filename_template,
                    TextField::CaptureFilename,
                ))
                .push(labeled_input(
                    "Format (png, jpg, ...)",
                    &self.draft.capture_format,
                    &self.defaults.capture_format,
                    TextField::CaptureFormat,
                ))
                .push(toggle_row(
                    "Copy to clipboard",
                    self.draft.capture_copy_to_clipboard,
                    self.defaults.capture_copy_to_clipboard,
                    ToggleField::CaptureCopyToClipboard,
                ))
                .push(toggle_row(
                    "Always exit overlay after capture",
                    self.draft.capture_exit_after,
                    self.defaults.capture_exit_after,
                    ToggleField::CaptureExitAfter,
                ));
        }

        if show_pdf {
            content = content
                .push(text("PDF Export").size(20))
                .push(labeled_input(
                    "PDF filename template (blank = capture template)",
                    &self.draft.export_pdf_filename_template,
                    &self.defaults.export_pdf_filename_template,
                    TextField::ExportPdfFilenameTemplate,
                ))
                .push(labeled_input(
                    "All boards PDF filename template",
                    &self.draft.export_pdf_all_boards_filename_template,
                    &self.defaults.export_pdf_all_boards_filename_template,
                    TextField::ExportPdfAllBoardsFilenameTemplate,
                ))
                .push(labeled_control(
                    "Page size",
                    pick_list(
                        PdfPageSizeOption::list(),
                        Some(self.draft.export_pdf_page_size),
                        Message::ExportPdfPageSizeChanged,
                    )
                    .width(Length::Fill)
                    .into(),
                    self.defaults.export_pdf_page_size.label().to_string(),
                    self.draft.export_pdf_page_size != self.defaults.export_pdf_page_size,
                ))
                .push(labeled_control(
                    "Orientation",
                    pick_list(
                        PdfOrientationOption::list(),
                        Some(self.draft.export_pdf_orientation),
                        Message::ExportPdfOrientationChanged,
                    )
                    .width(Length::Fill)
                    .into(),
                    self.defaults.export_pdf_orientation.label().to_string(),
                    self.draft.export_pdf_orientation != self.defaults.export_pdf_orientation,
                ))
                .push(labeled_control(
                    "Fit",
                    pick_list(
                        PdfFitModeOption::list(),
                        Some(self.draft.export_pdf_fit),
                        Message::ExportPdfFitChanged,
                    )
                    .width(Length::Fill)
                    .into(),
                    self.defaults.export_pdf_fit.label().to_string(),
                    self.draft.export_pdf_fit != self.defaults.export_pdf_fit,
                ))
                .push(labeled_control(
                    "Transparent page background",
                    pick_list(
                        PdfTransparentBackgroundOption::list(),
                        Some(self.draft.export_pdf_transparent_background),
                        Message::ExportPdfTransparentBackgroundChanged,
                    )
                    .width(Length::Fill)
                    .into(),
                    self.defaults
                        .export_pdf_transparent_background
                        .label()
                        .to_string(),
                    self.draft.export_pdf_transparent_background
                        != self.defaults.export_pdf_transparent_background,
                ))
                .push(labeled_input_with_feedback(
                    "Custom width (PDF points)",
                    &self.draft.export_pdf_custom_width,
                    &self.defaults.export_pdf_custom_width,
                    TextField::ExportPdfCustomWidth,
                    Some("Range: 1-14400"),
                    validate_f64_range(&self.draft.export_pdf_custom_width, 1.0, 14400.0),
                ))
                .push(labeled_input_with_feedback(
                    "Custom height (PDF points)",
                    &self.draft.export_pdf_custom_height,
                    &self.defaults.export_pdf_custom_height,
                    TextField::ExportPdfCustomHeight,
                    Some("Range: 1-14400"),
                    validate_f64_range(&self.draft.export_pdf_custom_height, 1.0, 14400.0),
                ))
                .push(labeled_input_with_feedback(
                    "Content source padding",
                    &self.draft.export_pdf_content_source_padding,
                    &self.defaults.export_pdf_content_source_padding,
                    TextField::ExportPdfContentSourcePadding,
                    Some("Range: 0-4096"),
                    validate_f64_range(&self.draft.export_pdf_content_source_padding, 0.0, 4096.0),
                ))
                .push(toggle_row(
                    "Show PDF page labels",
                    self.draft.export_pdf_labels_enabled,
                    self.defaults.export_pdf_labels_enabled,
                    ToggleField::ExportPdfLabelsEnabled,
                ))
                .push(labeled_control(
                    "Label position",
                    pick_list(
                        PdfLabelPositionOption::list(),
                        Some(self.draft.export_pdf_label_position),
                        Message::ExportPdfLabelPositionChanged,
                    )
                    .width(Length::Fill)
                    .into(),
                    self.defaults.export_pdf_label_position.label().to_string(),
                    self.draft.export_pdf_label_position != self.defaults.export_pdf_label_position,
                ))
                .push(labeled_control(
                    "Label content",
                    pick_list(
                        PdfLabelContentModeOption::list(),
                        Some(self.draft.export_pdf_label_content),
                        Message::ExportPdfLabelContentChanged,
                    )
                    .width(Length::Fill)
                    .into(),
                    self.defaults.export_pdf_label_content.label().to_string(),
                    self.draft.export_pdf_label_content != self.defaults.export_pdf_label_content,
                ))
                .push(labeled_input(
                    "Label template",
                    &self.draft.export_pdf_label_template,
                    &self.defaults.export_pdf_label_template,
                    TextField::ExportPdfLabelTemplate,
                ))
                .push(labeled_input(
                    "Label font family",
                    &self.draft.export_pdf_label_font_family,
                    &self.defaults.export_pdf_label_font_family,
                    TextField::ExportPdfLabelFontFamily,
                ))
                .push(labeled_input_with_feedback(
                    "Label font size",
                    &self.draft.export_pdf_label_font_size,
                    &self.defaults.export_pdf_label_font_size,
                    TextField::ExportPdfLabelFontSize,
                    Some("Range: 1-72"),
                    validate_f64_range(&self.draft.export_pdf_label_font_size, 1.0, 72.0),
                ))
                .push(labeled_input_with_feedback(
                    "Label margin",
                    &self.draft.export_pdf_label_margin,
                    &self.defaults.export_pdf_label_margin,
                    TextField::ExportPdfLabelMargin,
                    Some("Range: 0-240"),
                    validate_f64_range(&self.draft.export_pdf_label_margin, 0.0, 240.0),
                ))
                .push(labeled_input_with_feedback(
                    "Label horizontal padding",
                    &self.draft.export_pdf_label_padding_x,
                    &self.defaults.export_pdf_label_padding_x,
                    TextField::ExportPdfLabelPaddingX,
                    Some("Range: 0-120"),
                    validate_f64_range(&self.draft.export_pdf_label_padding_x, 0.0, 120.0),
                ))
                .push(labeled_input_with_feedback(
                    "Label vertical padding",
                    &self.draft.export_pdf_label_padding_y,
                    &self.defaults.export_pdf_label_padding_y,
                    TextField::ExportPdfLabelPaddingY,
                    Some("Range: 0-120"),
                    validate_f64_range(&self.draft.export_pdf_label_padding_y, 0.0, 120.0),
                ))
                .push(color_quad_picker(
                    "Label text RGBA (0-1)",
                    ColorPickerUi {
                        id: ColorPickerId::ExportPdfLabelText,
                        is_open: self.color_picker_open == Some(ColorPickerId::ExportPdfLabelText),
                        show_advanced: self
                            .color_picker_advanced
                            .contains(&ColorPickerId::ExportPdfLabelText),
                        hex_value: self
                            .color_picker_hex
                            .get(&ColorPickerId::ExportPdfLabelText)
                            .map(String::as_str)
                            .unwrap_or(""),
                    },
                    &self.draft.export_pdf_label_text_color,
                    &self.defaults.export_pdf_label_text_color,
                    QuadField::ExportPdfLabelText,
                ))
                .push(toggle_row(
                    "Label solid background",
                    self.draft.export_pdf_label_background_enabled,
                    self.defaults.export_pdf_label_background_enabled,
                    ToggleField::ExportPdfLabelBackgroundEnabled,
                ))
                .push(color_quad_picker(
                    "Label background RGBA (0-1)",
                    ColorPickerUi {
                        id: ColorPickerId::ExportPdfLabelBackground,
                        is_open: self.color_picker_open
                            == Some(ColorPickerId::ExportPdfLabelBackground),
                        show_advanced: self
                            .color_picker_advanced
                            .contains(&ColorPickerId::ExportPdfLabelBackground),
                        hex_value: self
                            .color_picker_hex
                            .get(&ColorPickerId::ExportPdfLabelBackground)
                            .map(String::as_str)
                            .unwrap_or(""),
                    },
                    &self.draft.export_pdf_label_background_color,
                    &self.defaults.export_pdf_label_background_color,
                    QuadField::ExportPdfLabelBackground,
                ));
        }

        scrollable(content).into()
    }
}
