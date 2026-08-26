mod boards;
mod color_picker;
mod config;
mod daemon;
mod fields;
mod presets;
mod render_profiles;
mod session_catalog;
mod shortcuts;
mod tabs;

use crate::messages::{CommandMessage, Message};

use super::effects::Effect;
use super::state::ConfiguratorApp;

pub(crate) use config::migration_offer_text;

impl ConfiguratorApp {
    /// Dispatch for what a finished effect reports back.
    ///
    /// Separate from [`Self::update_message`] only because the payloads are:
    /// a command result carries values that are moved into the model rather
    /// than copied into a widget closure.
    pub(crate) fn update_command(&mut self, message: CommandMessage) -> Vec<Effect> {
        match message {
            CommandMessage::ConfigLoaded(result) => self.handle_config_loaded(result),
            CommandMessage::ConfigSaved(result) => self.handle_config_saved(result),
            CommandMessage::DaemonStatusLoaded(request_id, result) => {
                self.handle_daemon_status_loaded(request_id, result)
            }
            CommandMessage::DaemonActionCompleted(result) => {
                self.handle_daemon_action_completed(result)
            }
            CommandMessage::SessionCatalogLoaded(result) => {
                self.handle_session_catalog_loaded(result)
            }
            CommandMessage::SessionCatalogActionCompleted(result) => {
                self.handle_session_catalog_action_completed(result)
            }
        }
    }

    pub(crate) fn update_message(&mut self, message: Message) -> Vec<Effect> {
        match message {
            Message::ReloadRequested => self.handle_reload_requested(),
            Message::ResetToDefaultsRequested => self.handle_reset_to_defaults_requested(),
            Message::ResetToDefaultsConfirmed => self.handle_reset_to_defaults_confirmed(),
            Message::ResetToDefaultsCanceled => self.handle_reset_to_defaults_canceled(),
            Message::WindowEscapePressed => self.handle_window_escape_pressed(),
            Message::SaveRequested => self.handle_save_requested(),
            Message::MigrationApplyRequested => self.handle_migration_apply_requested(),
            Message::MigrationDismissed => self.handle_migration_dismissed(),
            Message::DaemonShortcutInputChanged(value) => {
                self.handle_daemon_shortcut_input_changed(value)
            }
            Message::DaemonActionRequested(action) => self.handle_daemon_action_requested(action),
            Message::SessionCatalogRefreshRequested => {
                self.handle_session_catalog_refresh_requested()
            }
            Message::SessionCatalogForgetRequested(id) => {
                self.handle_session_catalog_forget_requested(id)
            }
            Message::SessionCatalogRenameInputChanged(id, value) => {
                self.handle_session_catalog_rename_input_changed(id, value)
            }
            Message::SessionCatalogRenameRequested(id) => {
                self.handle_session_catalog_rename_requested(id)
            }
            Message::SessionCatalogDuplicateInputChanged(id, value) => {
                self.handle_session_catalog_duplicate_input_changed(id, value)
            }
            Message::SessionCatalogDuplicateRequested(id) => {
                self.handle_session_catalog_duplicate_requested(id)
            }
            Message::SessionCatalogMoveInputChanged(id, value) => {
                self.handle_session_catalog_move_input_changed(id, value)
            }
            Message::SessionCatalogMoveRequested(id) => {
                self.handle_session_catalog_move_requested(id)
            }
            Message::SessionCatalogRevealRequested(id) => {
                self.handle_session_catalog_reveal_requested(id)
            }
            Message::SessionCatalogClearToolStateRequested(id) => {
                self.handle_session_catalog_clear_tool_state_requested(id)
            }
            Message::SessionCatalogClearRequested(id) => {
                self.handle_session_catalog_clear_requested(id)
            }
            Message::SessionCatalogClearConfirmed(id) => {
                self.handle_session_catalog_clear_confirmed(id)
            }
            Message::SessionCatalogClearCanceled(id) => {
                self.handle_session_catalog_clear_canceled(id)
            }
            Message::SearchChanged(value) => self.handle_search_changed(value),
            Message::SearchCleared => self.handle_search_cleared(),
            Message::SearchFocusRequested => self.handle_search_focus_requested(),
            Message::StartupInteractionObserved => self.handle_startup_interaction_observed(),
            Message::TabSelected(tab) => self.handle_tab_selected(tab),
            Message::UiTabSelected(tab) => self.handle_ui_tab_selected(tab),
            Message::KeybindingsTabSelected(tab) => self.handle_keybindings_tab_selected(tab),
            Message::ShortcutManagerShowAll => self.handle_shortcut_manager_show_all(),
            Message::ShortcutManagerFilterChanged(filter) => {
                self.handle_shortcut_manager_filter_changed(filter)
            }
            Message::ShortcutManagerSortChanged(sort) => {
                self.handle_shortcut_manager_sort_changed(sort)
            }
            Message::ShortcutManagerRowSelected(field) => {
                self.handle_shortcut_manager_row_selected(field)
            }
            Message::ShortcutManagerJumpTo(field) => self.handle_shortcut_manager_jump_to(field),
            Message::ShortcutResetVisibleRequested => {
                self.handle_shortcut_reset_visible_requested()
            }
            Message::ShortcutResetVisibleConfirmed => {
                self.handle_shortcut_reset_visible_confirmed()
            }
            Message::ShortcutResetAllRequested => self.handle_shortcut_reset_all_requested(),
            Message::ShortcutResetAllConfirmed => self.handle_shortcut_reset_all_confirmed(),
            Message::ShortcutResetCanceled => self.handle_shortcut_reset_canceled(),
            Message::ShortcutConflictReviewStarted => {
                self.handle_shortcut_conflict_review_started()
            }
            Message::ToggleChanged(field, value) => self.handle_toggle_changed(field, value),
            Message::TextChanged(field, value) => self.handle_text_changed(field, value),
            Message::ColorPickerHexChanged(id, value) => {
                self.handle_color_picker_hex_changed(id, value)
            }
            Message::ColorModeChanged(mode) => self.handle_color_mode_changed(mode),
            Message::NamedColorSelected(option) => self.handle_named_color_selected(option),
            Message::QuickColorAdded => self.handle_quick_color_added(),
            Message::QuickColorRemoved(index) => self.handle_quick_color_removed(index),
            Message::QuickColorMoved(index, delta) => self.handle_quick_color_moved(index, delta),
            Message::FontCycleAdded => self.handle_font_cycle_added(),
            Message::FontCycleRemoved(index) => self.handle_font_cycle_removed(index),
            Message::FontCycleMoved(index, delta) => self.handle_font_cycle_moved(index, delta),
            Message::FontCycleChanged(index, family) => {
                self.handle_font_cycle_changed(index, family)
            }
            Message::QuickColorModeChanged(index, mode) => {
                self.handle_quick_color_mode_changed(index, mode)
            }
            Message::QuickNamedColorSelected(index, option) => {
                self.handle_quick_named_color_selected(index, option)
            }
            Message::EraserModeChanged(option) => self.handle_eraser_mode_changed(option),
            Message::ArrowStyleChanged(option) => self.handle_arrow_style_changed(option),
            Message::DrawingDragMappingSectionToggled(button) => {
                self.handle_drawing_drag_mapping_section_toggled(button)
            }
            Message::DrawingMouseDragToolChanged(button, field, option) => {
                self.handle_drawing_mouse_drag_tool_changed(button, field, option)
            }
            Message::DrawingMouseDragColorChanged(button, field, option) => {
                self.handle_drawing_mouse_drag_color_changed(button, field, option)
            }
            Message::StatusPositionChanged(option) => self.handle_status_position_changed(option),
            Message::InputHudModeChanged(option) => self.handle_input_hud_mode_changed(option),
            Message::InputHudPositionChanged(option) => {
                self.handle_input_hud_position_changed(option)
            }
            Message::UiThemeChanged(option) => self.handle_ui_theme_changed(option),
            Message::UiReducedMotionChanged(option) => {
                self.handle_ui_reduced_motion_changed(option)
            }
            Message::ToolbarLayoutModeChanged(option) => {
                self.handle_toolbar_layout_mode_changed(option)
            }
            Message::ToolbarZoomChipDisplayChanged(option) => {
                self.handle_toolbar_zoom_chip_display_changed(option)
            }
            Message::ToolbarRebindModifierChanged(option) => {
                self.handle_toolbar_rebind_modifier_changed(option)
            }
            Message::ToolbarOverrideModeChanged(option) => {
                self.handle_toolbar_override_mode_changed(option)
            }
            Message::ToolbarOverrideChanged(field, option) => {
                self.handle_toolbar_override_changed(field, option)
            }
            Message::ToolbarItemVisibilityChanged(id, visible) => {
                self.handle_toolbar_item_visibility_changed(id, visible)
            }
            Message::ToolbarItemMoveRequested(group, id, delta) => {
                self.handle_toolbar_item_move_requested(group, id, delta)
            }
            Message::ToolbarItemOrderReset(group) => self.handle_toolbar_item_order_reset(group),
            Message::BoardsAddItem => self.handle_boards_add_item(),
            Message::BoardsRemoveItem(index) => self.handle_boards_remove_item(index),
            Message::BoardsMoveItemUp(index) => self.handle_boards_move_item(index, true),
            Message::BoardsMoveItemDown(index) => self.handle_boards_move_item(index, false),
            Message::BoardsDuplicateItem(index) => self.handle_boards_duplicate_item(index),
            Message::BoardsCollapseToggled(index) => self.handle_boards_collapse_toggled(index),
            Message::BoardsDefaultChanged(value) => self.handle_boards_default_changed(value),
            Message::BoardsItemTextChanged(index, field, value) => {
                self.handle_boards_item_text_changed(index, field, value)
            }
            Message::BoardsBackgroundKindChanged(index, value) => {
                self.handle_boards_background_kind_changed(index, value)
            }
            Message::BoardsBackgroundColorChanged(index, component, value) => {
                self.handle_boards_background_color_changed(index, component, value)
            }
            Message::BoardsDefaultPenEnabledChanged(index, value) => {
                self.handle_boards_default_pen_enabled_changed(index, value)
            }
            Message::BoardsDefaultPenColorChanged(index, component, value) => {
                self.handle_boards_default_pen_color_changed(index, component, value)
            }
            Message::BoardsItemToggleChanged(index, field, value) => {
                self.handle_boards_item_toggle_changed(index, field, value)
            }
            Message::RenderProfileAdd => self.handle_render_profile_add(),
            Message::RenderProfileRemove(index) => self.handle_render_profile_remove(index),
            Message::RenderProfileDuplicate(index) => self.handle_render_profile_duplicate(index),
            Message::RenderProfileTextChanged(index, field, value) => {
                self.handle_render_profile_text_changed(index, field, value)
            }
            Message::RenderProfileActiveChanged(value) => {
                self.handle_render_profile_active_changed(value)
            }
            Message::RenderProfileExportChanged(value) => {
                self.handle_render_profile_export_changed(value)
            }
            Message::RenderProfileExportProfileChanged(value) => {
                self.handle_render_profile_export_profile_changed(value)
            }
            Message::RenderProfileApplyCanvasChanged(value) => {
                self.handle_render_profile_apply_canvas_changed(value)
            }
            Message::RenderProfileApplyUiChanged(value) => {
                self.handle_render_profile_apply_ui_changed(value)
            }
            Message::RenderProfileMappingAdd(index) => {
                self.handle_render_profile_mapping_add(index)
            }
            Message::RenderProfileMappingRemove(profile, mapping) => {
                self.handle_render_profile_mapping_remove(profile, mapping)
            }
            Message::RenderProfileMappingColorChanged(profile, mapping, side, value) => {
                self.handle_render_profile_mapping_color_changed(profile, mapping, side, value)
            }
            Message::SessionStorageModeChanged(option) => {
                self.handle_session_storage_mode_changed(option)
            }
            Message::SessionCompressionChanged(option) => {
                self.handle_session_compression_changed(option)
            }
            Message::PresenterToolBehaviorChanged(option) => {
                self.handle_presenter_tool_behavior_changed(option)
            }
            Message::PresenterToolbarModeChanged(option) => {
                self.handle_presenter_toolbar_mode_changed(option)
            }
            Message::CaptureRegionPickerChanged(option) => {
                self.handle_capture_region_picker_changed(option)
            }
            Message::ExportPdfPageSizeChanged(option) => {
                self.handle_export_pdf_page_size_changed(option)
            }
            Message::ExportPdfOrientationChanged(option) => {
                self.handle_export_pdf_orientation_changed(option)
            }
            Message::ExportPdfFitChanged(option) => self.handle_export_pdf_fit_changed(option),
            Message::ExportPdfTransparentBackgroundChanged(option) => {
                self.handle_export_pdf_transparent_background_changed(option)
            }
            Message::ExportPdfLabelPositionChanged(option) => {
                self.handle_export_pdf_label_position_changed(option)
            }
            Message::ExportPdfLabelContentChanged(option) => {
                self.handle_export_pdf_label_content_changed(option)
            }
            Message::BufferCountChanged(count) => self.handle_buffer_count_changed(count),
            Message::ShortcutRecordingStarted(field) => {
                self.handle_shortcut_recording_started(field)
            }
            Message::ShortcutSequenceRecordingStarted(field) => {
                self.handle_shortcut_sequence_recording_started(field)
            }
            Message::ShortcutRecordingCanceled(field) => {
                self.handle_shortcut_recording_canceled(field)
            }
            Message::ShortcutRecorderKey(keyval, modifiers) => {
                self.handle_shortcut_recorder_key(keyval, modifiers)
            }
            Message::ShortcutRecorderButton(button, kind, modifiers) => {
                self.handle_shortcut_recorder_button(button, kind, modifiers)
            }
            Message::ShortcutSequenceFinish => self.handle_shortcut_sequence_finish(),
            Message::ShortcutSequenceRemoveLastStep => {
                self.handle_shortcut_sequence_remove_last_step()
            }
            Message::ShortcutRemoved(field, binding) => {
                self.handle_shortcut_removed(field, binding)
            }
            Message::ShortcutResetRequested(field) => self.handle_shortcut_reset_requested(field),
            Message::ShortcutTextEditStarted(field) => {
                self.handle_shortcut_text_edit_started(field)
            }
            Message::ShortcutTextEditChanged(value) => {
                self.handle_shortcut_text_edit_changed(value)
            }
            Message::ShortcutTextEditApplied => self.handle_shortcut_text_edit_applied(),
            Message::ShortcutTextEditCanceled(field) => {
                self.handle_shortcut_text_edit_canceled(field)
            }
            Message::ShortcutConflictReplaceConfirmed => {
                self.handle_shortcut_conflict_replace_confirmed()
            }
            Message::ShortcutConflictCanceled => self.handle_shortcut_conflict_canceled(),
            Message::FontStyleOptionSelected(option) => {
                self.handle_font_style_option_selected(option)
            }
            Message::FontWeightOptionSelected(option) => {
                self.handle_font_weight_option_selected(option)
            }
            #[cfg(feature = "tablet-input")]
            Message::TabletPressureEditModeChanged(option) => {
                self.handle_tablet_pressure_edit_mode_changed(option)
            }
            #[cfg(feature = "tablet-input")]
            Message::TabletPressureEntryModeChanged(option) => {
                self.handle_tablet_pressure_entry_mode_changed(option)
            }
            Message::PresetSlotCountChanged(count) => self.handle_preset_slot_count_changed(count),
            Message::PresetSlotEnabledChanged(slot_index, enabled) => {
                self.handle_preset_slot_enabled_changed(slot_index, enabled)
            }
            Message::PresetCollapseToggled(slot_index) => {
                self.handle_preset_collapse_toggled(slot_index)
            }
            Message::PresetResetSlot(slot_index) => self.handle_preset_reset_slot(slot_index),
            Message::PresetDuplicateSlot(slot_index) => {
                self.handle_preset_duplicate_slot(slot_index)
            }
            Message::PresetToolChanged(slot_index, tool) => {
                self.handle_preset_tool_changed(slot_index, tool)
            }
            Message::PresetColorModeChanged(slot_index, mode) => {
                self.handle_preset_color_mode_changed(slot_index, mode)
            }
            Message::PresetNamedColorSelected(slot_index, option) => {
                self.handle_preset_named_color_selected(slot_index, option)
            }
            Message::PresetColorComponentChanged(slot_index, component, value) => {
                self.handle_preset_color_component_changed(slot_index, component, value)
            }
            Message::PresetTextChanged(slot_index, field, value) => {
                self.handle_preset_text_changed(slot_index, field, value)
            }
            Message::PresetToggleOptionChanged(slot_index, field, value) => {
                self.handle_preset_toggle_option_changed(slot_index, field, value)
            }
            Message::PresetEraserKindChanged(slot_index, value) => {
                self.handle_preset_eraser_kind_changed(slot_index, value)
            }
            Message::PresetEraserModeChanged(slot_index, value) => {
                self.handle_preset_eraser_mode_changed(slot_index, value)
            }
        }
    }
}
