use iced::Task;

use crate::messages::Message;
use crate::models::{
    ColorMode, ColorPickerId, DragColorOption, DragMouseButton, DragToolField, DragToolOption,
    EraserModeOption, FontStyleOption, FontWeightOption, KeybindingField, NamedColorOption,
    OverrideOption, PdfFitModeOption, PdfLabelContentModeOption, PdfLabelPositionOption,
    PdfOrientationOption, PdfPageSizeOption, PdfTransparentBackgroundOption,
    PresenterToolBehaviorOption, QuadField, SessionCompressionOption, SessionStorageModeOption,
    StatusPositionOption, TextField, ToggleField, ToolbarLayoutModeOption, ToolbarOverrideField,
    TripletField,
};
#[cfg(feature = "tablet-input")]
use crate::models::{PressureThicknessEditModeOption, PressureThicknessEntryModeOption};

use super::super::state::{ConfiguratorApp, StatusMessage};

impl ConfiguratorApp {
    pub(super) fn handle_toggle_changed(
        &mut self,
        field: ToggleField,
        value: bool,
    ) -> Task<Message> {
        self.status = StatusMessage::idle();
        self.draft.set_toggle(field, value);
        self.refresh_dirty_flag();
        Task::none()
    }

    pub(super) fn handle_text_changed(&mut self, field: TextField, value: String) -> Task<Message> {
        self.status = StatusMessage::idle();
        self.draft.set_text(field, value);
        self.refresh_dirty_flag();
        Task::none()
    }

    pub(super) fn handle_triplet_changed(
        &mut self,
        field: TripletField,
        index: usize,
        value: String,
    ) -> Task<Message> {
        self.status = StatusMessage::idle();
        self.draft.set_triplet(field, index, value);
        let id = triplet_field_picker_id(field);
        self.sync_color_picker_hex_for_id(id);
        self.refresh_dirty_flag();
        Task::none()
    }

    pub(super) fn handle_quad_changed(
        &mut self,
        field: QuadField,
        index: usize,
        value: String,
    ) -> Task<Message> {
        self.status = StatusMessage::idle();
        self.draft.set_quad(field, index, value);
        if let Some(id) = quad_field_picker_id(field) {
            self.sync_color_picker_hex_for_id(id);
        }
        self.refresh_dirty_flag();
        Task::none()
    }

    pub(super) fn handle_color_mode_changed(&mut self, mode: ColorMode) -> Task<Message> {
        self.status = StatusMessage::idle();
        self.draft.drawing_color.mode = mode;
        if matches!(mode, ColorMode::Named) {
            if self.draft.drawing_color.name.trim().is_empty() {
                self.draft.drawing_color.selected_named = NamedColorOption::Red;
                self.draft.drawing_color.name = self
                    .draft
                    .drawing_color
                    .selected_named
                    .as_value()
                    .to_string();
            } else {
                self.draft.drawing_color.update_named_from_current();
            }
        }
        self.sync_color_picker_hex_for_id(ColorPickerId::DrawingColor);
        self.refresh_dirty_flag();
        Task::none()
    }

    pub(super) fn handle_named_color_selected(
        &mut self,
        option: NamedColorOption,
    ) -> Task<Message> {
        self.status = StatusMessage::idle();
        self.draft.drawing_color.selected_named = option;
        if option != NamedColorOption::Custom {
            self.draft.drawing_color.name = option.as_value().to_string();
        }
        self.refresh_dirty_flag();
        Task::none()
    }

    pub(super) fn handle_eraser_mode_changed(&mut self, option: EraserModeOption) -> Task<Message> {
        self.status = StatusMessage::idle();
        self.draft.drawing_default_eraser_mode = option;
        self.refresh_dirty_flag();
        Task::none()
    }

    pub(super) fn handle_drawing_mouse_drag_tool_changed(
        &mut self,
        button: DragMouseButton,
        field: DragToolField,
        option: DragToolOption,
    ) -> Task<Message> {
        self.status = StatusMessage::idle();
        self.draft.set_mouse_drag_tool(button, field, option);
        self.refresh_dirty_flag();
        Task::none()
    }

    pub(super) fn handle_drawing_mouse_drag_color_changed(
        &mut self,
        button: DragMouseButton,
        field: DragToolField,
        option: DragColorOption,
    ) -> Task<Message> {
        self.status = StatusMessage::idle();
        self.draft.set_mouse_drag_color(button, field, option);
        self.refresh_dirty_flag();
        Task::none()
    }

    pub(super) fn handle_status_position_changed(
        &mut self,
        option: StatusPositionOption,
    ) -> Task<Message> {
        self.status = StatusMessage::idle();
        self.draft.ui_status_position = option;
        self.refresh_dirty_flag();
        Task::none()
    }

    pub(super) fn handle_toolbar_layout_mode_changed(
        &mut self,
        option: ToolbarLayoutModeOption,
    ) -> Task<Message> {
        self.status = StatusMessage::idle();
        self.draft.apply_toolbar_layout_mode(option);
        self.refresh_dirty_flag();
        Task::none()
    }

    pub(super) fn handle_toolbar_override_mode_changed(
        &mut self,
        option: ToolbarLayoutModeOption,
    ) -> Task<Message> {
        self.override_mode = option;
        Task::none()
    }

    pub(super) fn handle_toolbar_override_changed(
        &mut self,
        field: ToolbarOverrideField,
        option: OverrideOption,
    ) -> Task<Message> {
        self.status = StatusMessage::idle();
        self.draft
            .set_toolbar_override(self.override_mode, field, option);
        self.refresh_dirty_flag();
        Task::none()
    }

    pub(super) fn handle_session_storage_mode_changed(
        &mut self,
        option: SessionStorageModeOption,
    ) -> Task<Message> {
        self.status = StatusMessage::idle();
        self.draft.session_storage_mode = option;
        self.refresh_dirty_flag();
        Task::none()
    }

    pub(super) fn handle_session_compression_changed(
        &mut self,
        option: SessionCompressionOption,
    ) -> Task<Message> {
        self.status = StatusMessage::idle();
        self.draft.session_compression = option;
        self.refresh_dirty_flag();
        Task::none()
    }

    pub(super) fn handle_presenter_tool_behavior_changed(
        &mut self,
        option: PresenterToolBehaviorOption,
    ) -> Task<Message> {
        self.status = StatusMessage::idle();
        self.draft.presenter_tool_behavior = option;
        self.refresh_dirty_flag();
        Task::none()
    }

    pub(super) fn handle_export_pdf_page_size_changed(
        &mut self,
        option: PdfPageSizeOption,
    ) -> Task<Message> {
        self.status = StatusMessage::idle();
        self.draft.export_pdf_page_size = option;
        self.refresh_dirty_flag();
        Task::none()
    }

    pub(super) fn handle_export_pdf_orientation_changed(
        &mut self,
        option: PdfOrientationOption,
    ) -> Task<Message> {
        self.status = StatusMessage::idle();
        self.draft.export_pdf_orientation = option;
        self.refresh_dirty_flag();
        Task::none()
    }

    pub(super) fn handle_export_pdf_fit_changed(
        &mut self,
        option: PdfFitModeOption,
    ) -> Task<Message> {
        self.status = StatusMessage::idle();
        self.draft.export_pdf_fit = option;
        self.refresh_dirty_flag();
        Task::none()
    }

    pub(super) fn handle_export_pdf_transparent_background_changed(
        &mut self,
        option: PdfTransparentBackgroundOption,
    ) -> Task<Message> {
        self.status = StatusMessage::idle();
        self.draft.export_pdf_transparent_background = option;
        self.refresh_dirty_flag();
        Task::none()
    }

    pub(super) fn handle_export_pdf_label_position_changed(
        &mut self,
        option: PdfLabelPositionOption,
    ) -> Task<Message> {
        self.status = StatusMessage::idle();
        self.draft.export_pdf_label_position = option;
        self.refresh_dirty_flag();
        Task::none()
    }

    pub(super) fn handle_export_pdf_label_content_changed(
        &mut self,
        option: PdfLabelContentModeOption,
    ) -> Task<Message> {
        self.status = StatusMessage::idle();
        self.draft.export_pdf_label_content = option;
        self.refresh_dirty_flag();
        Task::none()
    }

    pub(super) fn handle_buffer_count_changed(&mut self, count: u32) -> Task<Message> {
        self.status = StatusMessage::idle();
        self.draft.performance_buffer_count = count;
        self.refresh_dirty_flag();
        Task::none()
    }

    pub(super) fn handle_keybinding_changed(
        &mut self,
        field: KeybindingField,
        value: String,
    ) -> Task<Message> {
        self.status = StatusMessage::idle();
        self.draft.keybindings.set(field, value);
        self.refresh_dirty_flag();
        Task::none()
    }

    pub(super) fn handle_font_style_option_selected(
        &mut self,
        option: FontStyleOption,
    ) -> Task<Message> {
        self.status = StatusMessage::idle();
        self.draft.drawing_font_style_option = option;
        if option != FontStyleOption::Custom {
            self.draft.drawing_font_style = option.canonical_value().to_string();
        }
        self.refresh_dirty_flag();
        Task::none()
    }

    pub(super) fn handle_font_weight_option_selected(
        &mut self,
        option: FontWeightOption,
    ) -> Task<Message> {
        self.status = StatusMessage::idle();
        self.draft.drawing_font_weight_option = option;
        if option != FontWeightOption::Custom {
            self.draft.drawing_font_weight = option.canonical_value().to_string();
        }
        self.refresh_dirty_flag();
        Task::none()
    }

    #[cfg(feature = "tablet-input")]
    pub(super) fn handle_tablet_pressure_edit_mode_changed(
        &mut self,
        option: PressureThicknessEditModeOption,
    ) -> Task<Message> {
        self.status = StatusMessage::idle();
        self.draft.tablet_pressure_thickness_edit_mode = option;
        self.refresh_dirty_flag();
        Task::none()
    }

    #[cfg(feature = "tablet-input")]
    pub(super) fn handle_tablet_pressure_entry_mode_changed(
        &mut self,
        option: PressureThicknessEntryModeOption,
    ) -> Task<Message> {
        self.status = StatusMessage::idle();
        self.draft.tablet_pressure_thickness_entry_mode = option;
        self.refresh_dirty_flag();
        Task::none()
    }
}

fn quad_field_picker_id(field: QuadField) -> Option<ColorPickerId> {
    match field {
        QuadField::StatusBarBg => Some(ColorPickerId::StatusBarBg),
        QuadField::StatusBarText => Some(ColorPickerId::StatusBarText),
        QuadField::HighlightFill => Some(ColorPickerId::HighlightFill),
        QuadField::HighlightOutline => Some(ColorPickerId::HighlightOutline),
        QuadField::HelpBg => Some(ColorPickerId::HelpBg),
        QuadField::HelpBorder => Some(ColorPickerId::HelpBorder),
        QuadField::HelpText => Some(ColorPickerId::HelpText),
        QuadField::ExportPdfLabelText => Some(ColorPickerId::ExportPdfLabelText),
        QuadField::ExportPdfLabelBackground => Some(ColorPickerId::ExportPdfLabelBackground),
    }
}

fn triplet_field_picker_id(field: TripletField) -> ColorPickerId {
    match field {
        TripletField::DrawingColorRgb => ColorPickerId::DrawingColor,
    }
}
