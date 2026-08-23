mod drawing;
#[cfg(test)]
mod tests;
mod ui;

use wayscriber::config::PerformanceFieldId;

use crate::models::{
    FontStyleOption, FontWeightOption, KeybindingField, PdfFitModeOption,
    PdfLabelContentModeOption, PdfLabelPositionOption, PdfOrientationOption, PdfPageSizeOption,
    PdfTransparentBackgroundOption, PresenterToolBehaviorOption, PresenterToolbarModeOption,
    RegionPickerOption, SessionCompressionOption, SessionStorageModeOption, TextField, ToggleField,
};
#[cfg(feature = "tablet-input")]
use crate::models::{PressureThicknessEditModeOption, PressureThicknessEntryModeOption};

use super::super::effects::Effect;
use super::super::state::{ConfiguratorApp, StatusMessage};

impl ConfiguratorApp {
    pub(super) fn handle_toggle_changed(&mut self, field: ToggleField, value: bool) -> Vec<Effect> {
        self.status = StatusMessage::idle();
        self.draft.set_toggle(field, value);
        self.refresh_dirty_flag();
        Vec::new()
    }

    pub(super) fn handle_text_changed(&mut self, field: TextField, value: String) -> Vec<Effect> {
        self.status = StatusMessage::idle();
        self.draft.set_text(field, value);
        self.refresh_dirty_flag();
        Vec::new()
    }

    pub(super) fn handle_session_storage_mode_changed(
        &mut self,
        option: SessionStorageModeOption,
    ) -> Vec<Effect> {
        self.status = StatusMessage::idle();
        self.draft.session_storage_mode = option;
        self.refresh_dirty_flag();
        Vec::new()
    }

    pub(super) fn handle_session_compression_changed(
        &mut self,
        option: SessionCompressionOption,
    ) -> Vec<Effect> {
        self.status = StatusMessage::idle();
        self.draft.session_compression = option;
        self.refresh_dirty_flag();
        Vec::new()
    }

    pub(super) fn handle_presenter_tool_behavior_changed(
        &mut self,
        option: PresenterToolBehaviorOption,
    ) -> Vec<Effect> {
        self.status = StatusMessage::idle();
        self.draft.presenter_tool_behavior = option;
        self.refresh_dirty_flag();
        Vec::new()
    }

    pub(super) fn handle_presenter_toolbar_mode_changed(
        &mut self,
        option: PresenterToolbarModeOption,
    ) -> Vec<Effect> {
        self.status = StatusMessage::idle();
        self.draft.presenter_toolbar_mode = option;
        self.refresh_dirty_flag();
        Vec::new()
    }

    pub(super) fn handle_capture_region_picker_changed(
        &mut self,
        option: RegionPickerOption,
    ) -> Vec<Effect> {
        self.status = StatusMessage::idle();
        self.draft.capture_region_picker = option;
        self.refresh_dirty_flag();
        Vec::new()
    }

    pub(super) fn handle_export_pdf_page_size_changed(
        &mut self,
        option: PdfPageSizeOption,
    ) -> Vec<Effect> {
        self.status = StatusMessage::idle();
        self.draft.export_pdf_page_size = option;
        self.refresh_dirty_flag();
        Vec::new()
    }

    pub(super) fn handle_export_pdf_orientation_changed(
        &mut self,
        option: PdfOrientationOption,
    ) -> Vec<Effect> {
        self.status = StatusMessage::idle();
        self.draft.export_pdf_orientation = option;
        self.refresh_dirty_flag();
        Vec::new()
    }

    pub(super) fn handle_export_pdf_fit_changed(
        &mut self,
        option: PdfFitModeOption,
    ) -> Vec<Effect> {
        self.status = StatusMessage::idle();
        self.draft.export_pdf_fit = option;
        self.refresh_dirty_flag();
        Vec::new()
    }

    pub(super) fn handle_export_pdf_transparent_background_changed(
        &mut self,
        option: PdfTransparentBackgroundOption,
    ) -> Vec<Effect> {
        self.status = StatusMessage::idle();
        self.draft.export_pdf_transparent_background = option;
        self.refresh_dirty_flag();
        Vec::new()
    }

    pub(super) fn handle_export_pdf_label_position_changed(
        &mut self,
        option: PdfLabelPositionOption,
    ) -> Vec<Effect> {
        self.status = StatusMessage::idle();
        self.draft.export_pdf_label_position = option;
        self.refresh_dirty_flag();
        Vec::new()
    }

    pub(super) fn handle_export_pdf_label_content_changed(
        &mut self,
        option: PdfLabelContentModeOption,
    ) -> Vec<Effect> {
        self.status = StatusMessage::idle();
        self.draft.export_pdf_label_content = option;
        self.refresh_dirty_flag();
        Vec::new()
    }

    pub(super) fn handle_buffer_count_changed(&mut self, count: u32) -> Vec<Effect> {
        self.status = StatusMessage::idle();
        self.draft
            .set_performance_choice(PerformanceFieldId::BufferCount, count);
        self.refresh_dirty_flag();
        Vec::new()
    }

    pub(super) fn handle_keybinding_changed(
        &mut self,
        field: KeybindingField,
        value: String,
    ) -> Vec<Effect> {
        self.status = StatusMessage::idle();
        self.draft.keybindings.set(field, value);
        self.refresh_dirty_flag();
        Vec::new()
    }

    pub(super) fn handle_font_style_option_selected(
        &mut self,
        option: FontStyleOption,
    ) -> Vec<Effect> {
        self.status = StatusMessage::idle();
        self.draft.drawing_font_style_option = option;
        if option != FontStyleOption::Custom {
            self.draft.drawing_font_style = option.canonical_value().to_string();
        }
        self.refresh_dirty_flag();
        Vec::new()
    }

    pub(super) fn handle_font_weight_option_selected(
        &mut self,
        option: FontWeightOption,
    ) -> Vec<Effect> {
        self.status = StatusMessage::idle();
        self.draft.drawing_font_weight_option = option;
        if option != FontWeightOption::Custom {
            self.draft.drawing_font_weight = option.canonical_value().to_string();
        }
        self.refresh_dirty_flag();
        Vec::new()
    }

    #[cfg(feature = "tablet-input")]
    pub(super) fn handle_tablet_pressure_edit_mode_changed(
        &mut self,
        option: PressureThicknessEditModeOption,
    ) -> Vec<Effect> {
        self.status = StatusMessage::idle();
        self.draft.tablet_pressure_thickness_edit_mode = option;
        self.refresh_dirty_flag();
        Vec::new()
    }

    #[cfg(feature = "tablet-input")]
    pub(super) fn handle_tablet_pressure_entry_mode_changed(
        &mut self,
        option: PressureThicknessEntryModeOption,
    ) -> Vec<Effect> {
        self.status = StatusMessage::idle();
        self.draft.tablet_pressure_thickness_entry_mode = option;
        self.refresh_dirty_flag();
        Vec::new()
    }
}
