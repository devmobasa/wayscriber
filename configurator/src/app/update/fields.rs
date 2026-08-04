use wayscriber::config::{PerformanceFieldId, ToolbarItemId, ToolbarItemOrderGroup};

use crate::models::{
    ColorMode, ColorPickerId, DragColorOption, DragMouseButton, DragToolField, DragToolOption,
    EraserModeOption, FontStyleOption, FontWeightOption, InputHudModeOption,
    InputHudPositionOption, KeybindingField, NamedColorOption, OverrideOption, PdfFitModeOption,
    PdfLabelContentModeOption, PdfLabelPositionOption, PdfOrientationOption, PdfPageSizeOption,
    PdfTransparentBackgroundOption, PresenterToolBehaviorOption, PresenterToolbarModeOption,
    ReducedMotionOption, SessionCompressionOption, SessionStorageModeOption, StatusPositionOption,
    TextField, ToggleField, ToolbarLayoutModeOption, ToolbarOverrideField,
    ToolbarRebindModifierOption, ToolbarSideLayoutOption, UiThemeOption, ZoomChipDisplayOption,
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

    pub(super) fn handle_color_mode_changed(&mut self, mode: ColorMode) -> Vec<Effect> {
        self.status = StatusMessage::idle();
        if matches!(mode, ColorMode::Rgb) {
            self.draft.drawing_color.sync_rgb_from_preview();
        }
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
        Vec::new()
    }

    pub(super) fn handle_named_color_selected(&mut self, option: NamedColorOption) -> Vec<Effect> {
        self.status = StatusMessage::idle();
        self.draft.drawing_color.selected_named = option;
        if option != NamedColorOption::Custom {
            self.draft.drawing_color.name = option.as_value().to_string();
        }
        self.refresh_dirty_flag();
        Vec::new()
    }

    pub(super) fn handle_quick_color_mode_changed(
        &mut self,
        index: usize,
        mode: ColorMode,
    ) -> Vec<Effect> {
        self.status = StatusMessage::idle();
        let Some(entry) = self.draft.drawing_quick_colors.get_mut(index) else {
            return Vec::new();
        };
        let quick_color = &mut entry.color;
        if matches!(mode, ColorMode::Rgb) {
            quick_color.sync_rgb_from_preview();
        }
        quick_color.mode = mode;
        if matches!(mode, ColorMode::Named) {
            if quick_color.name.trim().is_empty() {
                quick_color.selected_named = NamedColorOption::Red;
                quick_color.name = quick_color.selected_named.as_value().to_string();
            } else {
                quick_color.update_named_from_current();
            }
        }
        self.sync_color_picker_hex_for_id(ColorPickerId::QuickColor(index));
        self.refresh_dirty_flag();
        Vec::new()
    }

    pub(super) fn handle_quick_named_color_selected(
        &mut self,
        index: usize,
        option: NamedColorOption,
    ) -> Vec<Effect> {
        self.status = StatusMessage::idle();
        let Some(entry) = self.draft.drawing_quick_colors.get_mut(index) else {
            return Vec::new();
        };
        let quick_color = &mut entry.color;
        quick_color.selected_named = option;
        if option != NamedColorOption::Custom {
            quick_color.name = option.as_value().to_string();
        }
        self.refresh_dirty_flag();
        Vec::new()
    }

    pub(super) fn handle_quick_color_added(&mut self) -> Vec<Effect> {
        self.status = StatusMessage::idle();
        let new_index = self.draft.drawing_quick_colors.entries.len();
        self.draft.drawing_quick_colors.add_entry();
        self.remap_quick_color_pickers(|index| (index < new_index).then_some(index));
        self.refresh_dirty_flag();
        Vec::new()
    }

    pub(super) fn handle_quick_color_removed(&mut self, index: usize) -> Vec<Effect> {
        self.status = StatusMessage::idle();
        if self.draft.drawing_quick_colors.remove_entry(index) {
            self.remap_quick_color_pickers(|picker_index| match picker_index.cmp(&index) {
                std::cmp::Ordering::Less => Some(picker_index),
                std::cmp::Ordering::Equal => None,
                std::cmp::Ordering::Greater => picker_index.checked_sub(1),
            });
            self.refresh_dirty_flag();
        }
        Vec::new()
    }

    pub(super) fn handle_quick_color_moved(&mut self, index: usize, delta: isize) -> Vec<Effect> {
        self.status = StatusMessage::idle();
        let Some(target) = index.checked_add_signed(delta) else {
            return Vec::new();
        };
        if self.draft.drawing_quick_colors.move_entry(index, delta) {
            self.remap_quick_color_pickers(|picker_index| {
                if picker_index == target {
                    Some(index)
                } else if picker_index == index {
                    Some(target)
                } else {
                    Some(picker_index)
                }
            });
            self.refresh_dirty_flag();
        }
        Vec::new()
    }

    /// Carries each surviving quick-color edit buffer to its row's new index.
    ///
    /// The buffer can be half-typed and therefore absent from the draft. Add,
    /// move, and removal must preserve it with the row that survives, while a
    /// deleted row's buffer must disappear so it cannot hold Save hostage.
    fn remap_quick_color_pickers(&mut self, remap: impl Fn(usize) -> Option<usize>) {
        let count = self.draft.drawing_quick_colors.entries.len();
        let mut preserved = Vec::new();
        self.color_picker_hex.retain(|id, text| match id {
            ColorPickerId::QuickColor(index) => {
                if let Some(next) = remap(*index).filter(|next| *next < count) {
                    preserved.push((next, text.clone()));
                }
                false
            }
            _ => true,
        });
        for (index, text) in preserved {
            self.color_picker_hex
                .insert(ColorPickerId::QuickColor(index), text);
        }
        for index in 0..count {
            let id = ColorPickerId::QuickColor(index);
            if !self.color_picker_hex.contains_key(&id) {
                self.sync_color_picker_hex_for_id(id);
            }
        }
    }

    pub(super) fn handle_eraser_mode_changed(&mut self, option: EraserModeOption) -> Vec<Effect> {
        self.status = StatusMessage::idle();
        self.draft.drawing_default_eraser_mode = option;
        self.refresh_dirty_flag();
        Vec::new()
    }

    pub(super) fn handle_drawing_mouse_drag_tool_changed(
        &mut self,
        button: DragMouseButton,
        field: DragToolField,
        option: DragToolOption,
    ) -> Vec<Effect> {
        self.status = StatusMessage::idle();
        self.draft.set_mouse_drag_tool(button, field, option);
        self.refresh_dirty_flag();
        Vec::new()
    }

    pub(super) fn handle_drawing_mouse_drag_color_changed(
        &mut self,
        button: DragMouseButton,
        field: DragToolField,
        option: DragColorOption,
    ) -> Vec<Effect> {
        self.status = StatusMessage::idle();
        self.draft.set_mouse_drag_color(button, field, option);
        self.refresh_dirty_flag();
        Vec::new()
    }

    pub(super) fn handle_status_position_changed(
        &mut self,
        option: StatusPositionOption,
    ) -> Vec<Effect> {
        self.status = StatusMessage::idle();
        self.draft.ui_status_position = option;
        self.refresh_dirty_flag();
        Vec::new()
    }

    pub(super) fn handle_input_hud_mode_changed(
        &mut self,
        option: InputHudModeOption,
    ) -> Vec<Effect> {
        self.status = StatusMessage::idle();
        self.draft.input_hud_mode = option;
        self.refresh_dirty_flag();
        Vec::new()
    }

    pub(super) fn handle_input_hud_position_changed(
        &mut self,
        option: InputHudPositionOption,
    ) -> Vec<Effect> {
        self.status = StatusMessage::idle();
        self.draft.input_hud_position = option;
        self.refresh_dirty_flag();
        Vec::new()
    }

    pub(super) fn handle_ui_theme_changed(&mut self, option: UiThemeOption) -> Vec<Effect> {
        self.status = StatusMessage::idle();
        self.draft.ui_theme = option;
        self.refresh_dirty_flag();
        Vec::new()
    }

    pub(super) fn handle_ui_reduced_motion_changed(
        &mut self,
        option: ReducedMotionOption,
    ) -> Vec<Effect> {
        self.status = StatusMessage::idle();
        self.draft.ui_reduced_motion = option;
        self.refresh_dirty_flag();
        Vec::new()
    }

    pub(super) fn handle_toolbar_layout_mode_changed(
        &mut self,
        option: ToolbarLayoutModeOption,
    ) -> Vec<Effect> {
        self.status = StatusMessage::idle();
        self.draft.apply_toolbar_layout_mode(option);
        self.refresh_dirty_flag();
        Vec::new()
    }

    pub(super) fn handle_toolbar_side_layout_changed(
        &mut self,
        option: ToolbarSideLayoutOption,
    ) -> Vec<Effect> {
        self.status = StatusMessage::idle();
        self.draft.ui_toolbar_side_layout = option;
        self.refresh_dirty_flag();
        Vec::new()
    }

    pub(super) fn handle_toolbar_zoom_chip_display_changed(
        &mut self,
        option: ZoomChipDisplayOption,
    ) -> Vec<Effect> {
        self.status = StatusMessage::idle();
        self.draft.ui_toolbar_zoom_chip_display = option;
        self.refresh_dirty_flag();
        Vec::new()
    }

    pub(super) fn handle_toolbar_rebind_modifier_changed(
        &mut self,
        option: ToolbarRebindModifierOption,
    ) -> Vec<Effect> {
        self.status = StatusMessage::idle();
        self.draft.ui_toolbar_rebind_modifier = option;
        self.refresh_dirty_flag();
        Vec::new()
    }

    pub(super) fn handle_toolbar_override_mode_changed(
        &mut self,
        option: ToolbarLayoutModeOption,
    ) -> Vec<Effect> {
        self.override_mode = option;
        Vec::new()
    }

    pub(super) fn handle_toolbar_override_changed(
        &mut self,
        field: ToolbarOverrideField,
        option: OverrideOption,
    ) -> Vec<Effect> {
        self.status = StatusMessage::idle();
        self.draft
            .set_toolbar_override(self.override_mode, field, option);
        self.refresh_dirty_flag();
        Vec::new()
    }

    pub(super) fn handle_toolbar_item_visibility_changed(
        &mut self,
        id: ToolbarItemId,
        visible: bool,
    ) -> Vec<Effect> {
        self.status = StatusMessage::idle();
        self.draft.set_toolbar_item_visible(id, visible);
        self.refresh_dirty_flag();
        Vec::new()
    }

    pub(super) fn handle_toolbar_item_move_requested(
        &mut self,
        group: ToolbarItemOrderGroup,
        id: ToolbarItemId,
        delta: isize,
    ) -> Vec<Effect> {
        self.status = StatusMessage::idle();
        self.draft.move_toolbar_item(group, id, delta);
        self.refresh_dirty_flag();
        Vec::new()
    }

    pub(super) fn handle_toolbar_item_order_reset(
        &mut self,
        group: ToolbarItemOrderGroup,
    ) -> Vec<Effect> {
        self.status = StatusMessage::idle();
        self.draft.reset_toolbar_item_order(group);
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

#[cfg(test)]
mod tests {
    use super::*;
    use wayscriber::config::{ColorSpec, Config};

    #[test]
    fn quick_color_mode_change_to_rgb_materializes_named_hex_preview() {
        let (mut app, _effects) = ConfiguratorApp::new_app();
        let entry = &mut app.draft.drawing_quick_colors.entries[1];
        entry.color.mode = ColorMode::Named;
        entry.color.selected_named = NamedColorOption::Custom;
        entry.color.name = "#123456".to_string();

        let _ = app.handle_quick_color_mode_changed(1, ColorMode::Rgb);

        assert_eq!(
            app.draft.drawing_quick_colors.entries[1].color.rgb,
            ["18", "52", "86"]
        );

        let saved = app
            .draft
            .to_config(&Config::default())
            .expect("expected quick color RGB to save");

        assert_eq!(
            saved.drawing.quick_colors.entries[1].color,
            ColorSpec::Rgb([18, 52, 86])
        );
    }

    /// Deleting a quick color takes its picker's editing text with it.
    ///
    /// Without the prune the removed slot's text stays in the map with no row
    /// left to show it, and text the save gate refuses keeps Save disabled
    /// over a field the user can no longer reach.
    #[test]
    fn removing_the_last_quick_color_drops_the_hex_text_it_left_behind() {
        let (mut app, _effects) = ConfiguratorApp::new_app();
        let _ = app.handle_quick_color_added();
        let last = app.draft.drawing_quick_colors.entries.len() - 1;
        let _ = app
            .handle_color_picker_hex_changed(ColorPickerId::QuickColor(last), "#12zz".to_string());
        assert_eq!(app.invalid_color_hex_count(), 1);

        let _ = app.handle_quick_color_removed(last);

        assert!(
            !app.color_picker_hex
                .contains_key(&ColorPickerId::QuickColor(last)),
            "the removed slot's picker text must go with the row"
        );
        assert_eq!(app.invalid_color_hex_count(), 0);
    }

    #[test]
    fn removing_a_different_quick_color_preserves_a_half_typed_hex() {
        let (mut app, _effects) = ConfiguratorApp::new_app();
        let _ = app.handle_quick_color_added();
        let last = app.draft.drawing_quick_colors.entries.len() - 1;
        let _ =
            app.handle_color_picker_hex_changed(ColorPickerId::QuickColor(0), "#12zz".to_string());

        let _ = app.handle_quick_color_removed(last);

        assert_eq!(
            app.color_picker_hex
                .get(&ColorPickerId::QuickColor(0))
                .map(String::as_str),
            Some("#12zz")
        );
        assert_eq!(app.invalid_color_hex_count(), 1);
    }

    /// Reordering remaps every surviving slot, so the editing text follows the
    /// row it was moved with rather than staying at its old position.
    #[test]
    fn moving_a_quick_color_resyncs_the_pickers_that_survive() {
        let (mut app, _effects) = ConfiguratorApp::new_app();
        app.draft.drawing_quick_colors.entries[0].color.mode = ColorMode::Rgb;
        app.draft.drawing_quick_colors.entries[0].color.rgb =
            ["1".to_string(), "2".to_string(), "3".to_string()];
        app.draft.drawing_quick_colors.entries[1].color.mode = ColorMode::Rgb;
        app.draft.drawing_quick_colors.entries[1].color.rgb =
            ["4".to_string(), "5".to_string(), "6".to_string()];
        app.sync_all_color_picker_hex();
        let first = app
            .color_picker_hex
            .get(&ColorPickerId::QuickColor(0))
            .cloned();

        let _ = app.handle_quick_color_moved(0, 1);

        assert_eq!(
            app.color_picker_hex.get(&ColorPickerId::QuickColor(1)),
            first.as_ref()
        );
    }

    #[test]
    fn adding_a_quick_color_preserves_a_half_typed_surviving_hex() {
        let (mut app, _effects) = ConfiguratorApp::new_app();
        let _ =
            app.handle_color_picker_hex_changed(ColorPickerId::QuickColor(0), "#12zz".to_string());

        let _ = app.handle_quick_color_added();

        assert_eq!(
            app.color_picker_hex
                .get(&ColorPickerId::QuickColor(0))
                .map(String::as_str),
            Some("#12zz")
        );
        assert_eq!(app.invalid_color_hex_count(), 1);
    }

    #[test]
    fn moving_a_quick_color_carries_its_half_typed_hex_with_it() {
        let (mut app, _effects) = ConfiguratorApp::new_app();
        let _ =
            app.handle_color_picker_hex_changed(ColorPickerId::QuickColor(0), "#12zz".to_string());

        let _ = app.handle_quick_color_moved(0, 1);

        assert_eq!(
            app.color_picker_hex
                .get(&ColorPickerId::QuickColor(1))
                .map(String::as_str),
            Some("#12zz")
        );
        assert_ne!(
            app.color_picker_hex
                .get(&ColorPickerId::QuickColor(0))
                .map(String::as_str),
            Some("#12zz")
        );
        assert_eq!(app.invalid_color_hex_count(), 1);
    }

    #[test]
    fn quick_color_label_edit_does_not_change_slot_colors() {
        let (mut app, _effects) = ConfiguratorApp::new_app();

        let _ = app.handle_text_changed(TextField::QuickColorLabel(0), "RedNew".to_string());

        // The built-in defaults are named colors resolving to the tuned
        // palette, so the slots stay on their named values after a label edit.
        assert_eq!(app.draft.drawing_quick_colors.entries[0].color.name, "red");
        assert_eq!(
            app.draft.drawing_quick_colors.entries[1].color.name,
            "green"
        );
        assert_eq!(app.draft.drawing_quick_colors.entries[2].color.name, "blue");
        assert_eq!(
            app.draft.drawing_quick_colors.entries[0]
                .color
                .selected_named,
            NamedColorOption::Red
        );
    }
}
