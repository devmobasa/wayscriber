mod actions;
mod boards;
mod delays;
mod layout;
mod pages;
mod tools;

use crate::input::InputState;

use super::events::ToolbarEvent;

impl InputState {
    /// Applies a toolbar-originated event to the input state.
    ///
    /// Returns true if the event resulted in a state change.
    pub fn apply_toolbar_event(&mut self, event: ToolbarEvent) -> bool {
        // Resolve the keyboard-action equivalent before the event is consumed
        // so the shortcut coach can learn from toolbar use (the slow path the
        // palette also feeds).
        let coach_action = event.action();
        let changed = self.apply_toolbar_event_inner(event);
        self.note_toolbar_shortcut_slow_path(coach_action, changed);
        changed
    }

    /// Shortcut-coach slow-path signal for toolbar use: invoking a
    /// shortcut-bound action from the toolbar is the same "you could have
    /// pressed the key" case the command palette records. Only genuine state
    /// changes for actions that resolve to a shortcut count, so the coach can
    /// always name the key and no-op clicks never build a streak.
    fn note_toolbar_shortcut_slow_path(
        &mut self,
        coach_action: Option<crate::config::Action>,
        changed: bool,
    ) {
        if !changed {
            return;
        }
        if let Some(action) = coach_action
            && self.shortcut_for_action(action).is_some()
        {
            self.pending_onboarding_usage
                .note_shortcut_slow_path(action);
        }
    }

    fn apply_toolbar_event_inner(&mut self, event: ToolbarEvent) -> bool {
        match event {
            ToolbarEvent::SelectTool(tool) => self.apply_toolbar_select_tool(tool),
            ToolbarEvent::SetColor(color) => self.apply_toolbar_set_color(color),
            ToolbarEvent::SetQuickColor { color, .. } => self.apply_toolbar_set_color(color),
            ToolbarEvent::SetColorHsv { h, s, v } => self.apply_toolbar_set_color_hsv(h, s, v),
            ToolbarEvent::SetThickness(value) => self.apply_toolbar_set_thickness(value),
            ToolbarEvent::SetMarkerOpacity(value) => self.apply_toolbar_set_marker_opacity(value),
            ToolbarEvent::SetEraserMode(mode) => self.apply_toolbar_set_eraser_mode(mode),
            ToolbarEvent::SetFont(descriptor) => self.apply_toolbar_set_font(descriptor),
            ToolbarEvent::SetFontSize(size) => self.apply_toolbar_set_font_size(size),
            ToolbarEvent::NudgeFontSize(delta) => {
                self.apply_toolbar_set_font_size(self.current_font_size + delta)
            }
            ToolbarEvent::ToggleFill(enable) => self.apply_toolbar_toggle_fill(enable),
            ToolbarEvent::SetPolygonSides(sides) => self.apply_toolbar_set_polygon_sides(sides),
            ToolbarEvent::NudgePolygonSides(delta) => self.apply_toolbar_nudge_polygon_sides(delta),
            ToolbarEvent::ToggleArrowLabels(enable) => {
                self.apply_toolbar_toggle_arrow_labels(enable)
            }
            ToolbarEvent::ResetArrowLabelCounter => self.apply_toolbar_reset_arrow_label_counter(),
            ToolbarEvent::ResetStepMarkerCounter => self.apply_toolbar_reset_step_marker_counter(),
            ToolbarEvent::SetUndoDelay(delay_secs) => self.apply_toolbar_set_undo_delay(delay_secs),
            ToolbarEvent::SetRedoDelay(delay_secs) => self.apply_toolbar_set_redo_delay(delay_secs),
            ToolbarEvent::SetCustomUndoDelay(delay_secs) => {
                self.apply_toolbar_set_custom_undo_delay(delay_secs)
            }
            ToolbarEvent::SetCustomRedoDelay(delay_secs) => {
                self.apply_toolbar_set_custom_redo_delay(delay_secs)
            }
            ToolbarEvent::SetCustomUndoSteps(steps) => {
                self.apply_toolbar_set_custom_undo_steps(steps)
            }
            ToolbarEvent::SetCustomRedoSteps(steps) => {
                self.apply_toolbar_set_custom_redo_steps(steps)
            }
            ToolbarEvent::NudgeThickness(delta) => self.apply_toolbar_nudge_thickness(delta),
            ToolbarEvent::NudgeMarkerOpacity(delta) => {
                self.apply_toolbar_nudge_marker_opacity(delta)
            }
            ToolbarEvent::Undo => self.apply_toolbar_undo(),
            ToolbarEvent::Redo => self.apply_toolbar_redo(),
            ToolbarEvent::UndoAll => self.apply_toolbar_undo_all(),
            ToolbarEvent::RedoAll => self.apply_toolbar_redo_all(),
            ToolbarEvent::UndoAllDelayed => self.apply_toolbar_undo_all_delayed(),
            ToolbarEvent::RedoAllDelayed => self.apply_toolbar_redo_all_delayed(),
            ToolbarEvent::CustomUndo => self.apply_toolbar_custom_undo(),
            ToolbarEvent::CustomRedo => self.apply_toolbar_custom_redo(),
            ToolbarEvent::ClearCanvas { instant } => self.apply_toolbar_clear_canvas(instant),
            ToolbarEvent::CaptureScreenshot => self.apply_toolbar_capture_screenshot(),
            ToolbarEvent::PagePrev => self.apply_toolbar_page_prev(),
            ToolbarEvent::PageNext => self.apply_toolbar_page_next(),
            ToolbarEvent::PageNew => self.apply_toolbar_page_new(),
            ToolbarEvent::PageDuplicate => self.apply_toolbar_page_duplicate(),
            ToolbarEvent::PageDelete => self.apply_toolbar_page_delete(),
            ToolbarEvent::BoardPrev => self.apply_toolbar_board_prev(),
            ToolbarEvent::BoardNext => self.apply_toolbar_board_next(),
            ToolbarEvent::BoardNew => self.apply_toolbar_board_new(),
            ToolbarEvent::BoardDelete => self.apply_toolbar_board_delete(),
            ToolbarEvent::BoardDuplicate => self.apply_toolbar_board_duplicate(),
            ToolbarEvent::BoardRename => self.apply_toolbar_board_rename(),
            ToolbarEvent::ToggleBoardPicker => self.apply_toolbar_toggle_board_picker(),
            ToolbarEvent::EnterTextMode => self.apply_toolbar_enter_text_mode(),
            ToolbarEvent::EnterStickyNoteMode => self.apply_toolbar_enter_sticky_note_mode(),
            ToolbarEvent::ToggleAllHighlight(enable) => {
                self.apply_toolbar_toggle_all_highlight(enable)
            }
            ToolbarEvent::ToggleHighlightToolRing(enable) => {
                self.apply_toolbar_toggle_highlight_tool_ring(enable)
            }
            ToolbarEvent::ToggleFreeze => self.apply_toolbar_toggle_freeze(),
            ToolbarEvent::ZoomIn => self.apply_toolbar_zoom_in(),
            ToolbarEvent::ZoomOut => self.apply_toolbar_zoom_out(),
            ToolbarEvent::ResetZoom => self.apply_toolbar_reset_zoom(),
            ToolbarEvent::ToggleZoomLock => self.apply_toolbar_toggle_zoom_lock(),
            ToolbarEvent::RefreshZoomCapture => self.apply_toolbar_refresh_zoom_capture(),
            ToolbarEvent::ToggleCustomSection(enable) => {
                self.apply_toolbar_toggle_custom_section(enable)
            }
            ToolbarEvent::ToggleDelaySliders(show) => self.apply_toolbar_toggle_delay_sliders(show),
            ToolbarEvent::OpenConfigurator => self.apply_toolbar_open_configurator(),
            ToolbarEvent::OpenConfigFile => self.apply_toolbar_open_config_file(),
            ToolbarEvent::OpenCommandPalette => self.apply_toolbar_open_command_palette(),
            ToolbarEvent::ToggleTopOverflow(open) => self.apply_toolbar_toggle_top_overflow(open),
            ToolbarEvent::ToggleSessionPopover(open) => {
                self.apply_toolbar_toggle_session_popover(open)
            }
            ToolbarEvent::ToggleSettingsPopover(open) => {
                self.apply_toolbar_toggle_settings_popover(open)
            }
            ToolbarEvent::ToggleCanvasPopover(open) => {
                self.apply_toolbar_toggle_canvas_popover(open)
            }
            ToolbarEvent::ScrollTopPopover(offset) => self.apply_toolbar_scroll_top_popover(offset),
            ToolbarEvent::SetTopMinimized(minimized) => {
                self.apply_toolbar_set_top_minimized(minimized)
            }
            ToolbarEvent::SetTopDisplayMode(mode) => self.apply_toolbar_set_top_display_mode(mode),
            ToolbarEvent::SetSideMinimized(minimized) => {
                self.apply_toolbar_set_side_minimized(minimized)
            }
            ToolbarEvent::CloseTopToolbar => self.apply_toolbar_set_top_minimized(true),
            ToolbarEvent::CloseSideToolbar => self.apply_toolbar_set_side_minimized(true),
            ToolbarEvent::PinTopToolbar(pin) => self.apply_toolbar_pin_top_toolbar(pin),
            ToolbarEvent::PinSideToolbar(pin) => self.apply_toolbar_pin_side_toolbar(pin),
            ToolbarEvent::ToggleIconMode(use_icons) => {
                self.apply_toolbar_toggle_icon_mode(use_icons)
            }
            ToolbarEvent::ToggleMoreColors(show) => self.apply_toolbar_toggle_more_colors(show),
            ToolbarEvent::CopyHexColor => self.apply_toolbar_copy_hex_color(),
            ToolbarEvent::PasteHexColor => self.apply_toolbar_paste_hex_color(),
            ToolbarEvent::EditHexColor => self.apply_toolbar_edit_hex_color(),
            ToolbarEvent::OpenColorPickerPopup => self.apply_toolbar_open_color_picker_popup(),
            ToolbarEvent::AdjustSelectionProperty { kind, direction } => {
                self.adjust_selection_property_kind(kind, direction)
            }
            ToolbarEvent::OpenPrecisionEntry(target) => {
                self.apply_toolbar_open_precision_entry(target)
            }
            ToolbarEvent::CommitPrecisionEntry { target, value } => {
                self.apply_toolbar_commit_precision_entry(target, value)
            }
            ToolbarEvent::CancelPrecisionEntry => self.cancel_precision_entry(),
            ToolbarEvent::PickScreenColor => {
                self.request_eyedropper_toggle();
                true
            }
            ToolbarEvent::ToggleActionsSection(show) => {
                self.apply_toolbar_toggle_actions_section(show)
            }
            ToolbarEvent::ToggleActionsAdvanced(show) => {
                self.apply_toolbar_toggle_actions_advanced(show)
            }
            ToolbarEvent::ToggleZoomActions(show) => self.apply_toolbar_toggle_zoom_actions(show),
            ToolbarEvent::TogglePagesSection(show) => self.apply_toolbar_toggle_pages_section(show),
            ToolbarEvent::ToggleBoardsSection(show) => {
                self.apply_toolbar_toggle_boards_section(show)
            }
            ToolbarEvent::TogglePresets(show) => self.apply_toolbar_toggle_presets(show),
            ToolbarEvent::ToggleStepSection(show) => self.apply_toolbar_toggle_step_section(show),
            ToolbarEvent::ToggleTextControls(show) => self.apply_toolbar_toggle_text_controls(show),
            ToolbarEvent::ToggleContextAwareUi(enabled) => {
                self.apply_toolbar_toggle_context_aware_ui(enabled)
            }
            ToolbarEvent::TogglePresetToasts(show) => self.apply_toolbar_toggle_preset_toasts(show),
            ToolbarEvent::ToggleToolPreview(show) => self.apply_toolbar_toggle_tool_preview(show),
            ToolbarEvent::ToggleStatusBar(show) => self.apply_toolbar_toggle_status_bar(show),
            ToolbarEvent::ToggleStatusBoardBadge(show) => {
                self.apply_toolbar_toggle_status_board_badge(show)
            }
            ToolbarEvent::ToggleStatusPageBadge(show) => {
                self.apply_toolbar_toggle_status_page_badge(show)
            }
            ToolbarEvent::ToggleFloatingBadgeAlways(show) => {
                self.apply_toolbar_toggle_floating_badge_always(show)
            }
            ToolbarEvent::SetSidePane(pane) => self.apply_toolbar_set_side_pane(pane),
            ToolbarEvent::ScrollSidePane(offset) => self.apply_toolbar_scroll_side_pane(offset),
            ToolbarEvent::ToggleSideSectionCollapsed(section, collapsed) => {
                self.apply_toolbar_toggle_side_section_collapsed(section, collapsed)
            }
            ToolbarEvent::SetToolbarLayoutMode(mode) => self.apply_toolbar_set_layout_mode(mode),
            ToolbarEvent::SetToolbarItemHidden(id, hidden) => {
                self.apply_toolbar_set_item_hidden(id, hidden)
            }
            ToolbarEvent::MoveToolbarItem { group, id, delta } => {
                self.apply_toolbar_move_item(group, id, delta)
            }
            ToolbarEvent::StartToolbarItemDrag { group, id } => {
                self.apply_toolbar_start_item_drag(group, id)
            }
            ToolbarEvent::DragToolbarItemOver {
                group,
                target_index,
            } => self.apply_toolbar_drag_item_over(group, target_index),
            ToolbarEvent::ResetToolbarItemOrder(group) => {
                self.apply_toolbar_reset_item_order(group)
            }
            ToolbarEvent::ResetToolbarItemHiddenOverrides => {
                self.apply_toolbar_reset_item_hidden_overrides()
            }
            ToolbarEvent::SetToolbarItemCustomizationOpen(open) => {
                self.apply_toolbar_set_item_customization_open(open)
            }
            ToolbarEvent::SetToolbarItemCustomizationGroup(group) => {
                self.apply_toolbar_set_item_customization_group(group)
            }
            ToolbarEvent::ToggleShapePicker(open) => self.apply_toolbar_toggle_shape_picker(open),
            ToolbarEvent::ApplyPreset(slot) => self.apply_toolbar_apply_preset(slot),
            ToolbarEvent::SavePreset(slot) => self.apply_toolbar_save_preset(slot),
            ToolbarEvent::ClearPreset(slot) => self.apply_toolbar_clear_preset(slot),
            ToolbarEvent::OpenSession
            | ToolbarEvent::OpenRecentSession(_)
            | ToolbarEvent::SaveSessionAs
            | ToolbarEvent::SaveSessionAsConfirm(_)
            | ToolbarEvent::SaveSessionAsCancel
            | ToolbarEvent::SessionInfo
            | ToolbarEvent::ClearSession => false,
            ToolbarEvent::MoveTopToolbar { .. } | ToolbarEvent::MoveSideToolbar { .. } => false,
        }
    }
}

#[cfg(test)]
mod coach_tests {
    use crate::config::{Action, KeyBinding};
    use crate::draw::{Color, Shape};
    use crate::input::InputState;
    use crate::input::state::test_support::{
        make_test_input_state, make_test_input_state_with_action_bindings,
    };
    use crate::ui::toolbar::ToolbarEvent;
    use std::collections::HashMap;

    fn add_test_shape(state: &mut InputState) {
        state.boards.active_frame_mut().add_shape(Shape::Rect {
            x: 10,
            y: 10,
            w: 5,
            h: 5,
            fill: false,
            color: Color {
                r: 1.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
            thick: 1.0,
        });
    }

    #[test]
    fn toolbar_action_with_shortcut_records_coach_slow_path() {
        let mut state = make_test_input_state();
        add_test_shape(&mut state);
        assert!(
            state.shortcut_for_action(Action::Undo).is_some(),
            "test relies on Undo having a default shortcut"
        );

        assert!(state.apply_toolbar_event(ToolbarEvent::Undo));

        assert_eq!(
            state.pending_onboarding_usage.shortcut_slow_path_action,
            Some(Action::Undo),
            "toolbar-invoked shortcut-bound action feeds the coach slow path"
        );
        assert_eq!(state.pending_onboarding_usage.shortcut_slow_path_repeats, 1);

        // A second toolbar Undo accumulates the slow-path streak.
        add_test_shape(&mut state);
        assert!(state.apply_toolbar_event(ToolbarEvent::Undo));
        assert_eq!(state.pending_onboarding_usage.shortcut_slow_path_repeats, 2);
    }

    #[test]
    fn toolbar_action_without_shortcut_does_not_coach() {
        // Undo explicitly bound to nothing: it resolves to no shortcut, so
        // there is nothing to coach — the coach must not record a slow path it
        // cannot name. (An empty map would fall back to the default action map,
        // which still binds Undo, so the override must be an explicit empty
        // binding list.)
        let bindings: HashMap<Action, Vec<KeyBinding>> =
            HashMap::from([(Action::Undo, Vec::new())]);
        let mut state = make_test_input_state_with_action_bindings(bindings);
        add_test_shape(&mut state);
        assert!(state.shortcut_for_action(Action::Undo).is_none());

        assert!(state.apply_toolbar_event(ToolbarEvent::Undo));

        assert_eq!(
            state.pending_onboarding_usage.shortcut_slow_path_action,
            None
        );
        assert_eq!(state.pending_onboarding_usage.shortcut_slow_path_repeats, 0);
    }

    #[test]
    fn toolbar_layout_event_without_action_mapping_does_not_coach() {
        // A layout-only event has no keyboard-action equivalent, so it is never
        // a shortcut slow path regardless of whether it changed state.
        let mut state = make_test_input_state();
        assert_eq!(ToolbarEvent::ToggleStatusBar(false).action(), None);

        state.apply_toolbar_event(ToolbarEvent::ToggleStatusBar(false));

        assert_eq!(
            state.pending_onboarding_usage.shortcut_slow_path_action,
            None
        );
    }
}
