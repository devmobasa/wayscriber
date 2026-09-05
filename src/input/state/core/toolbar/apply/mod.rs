mod actions;
mod boards;
mod delays;
mod layout;
mod pages;
mod tools;

use crate::input::InputState;

use crate::ui::toolbar::ToolbarEvent;

impl InputState {
    /// Applies a toolbar-originated event to the input state.
    ///
    /// Returns true if the event resulted in a state change.
    pub fn apply_toolbar_event(&mut self, event: ToolbarEvent) -> bool {
        crate::input::state::with_scoped_text_resources(|resources| {
            self.apply_toolbar_event_with_resources(resources, event)
        })
    }

    pub(crate) fn apply_toolbar_event_with_resources(
        &mut self,
        resources: crate::input::state::InputTextResources<'_>,
        event: ToolbarEvent,
    ) -> bool {
        // Resolve the keyboard-action equivalent before the event is consumed
        // so the shortcut coach can learn from toolbar use (the slow path the
        // palette also feeds).
        let coach_action = event.action();
        // Toolbar page and board switches never reach `handle_action`, so the
        // gesture is closed here as well. A wheel adjustment must not outlive
        // the frame it started on: shape ids restart per frame.
        self.flush_spotlight_magnification_gesture();
        // Same barrier for a held bend. Touch, tablet, and the GTK toolbar all
        // deliver events while a pointer-held gesture is running, and Undo All
        // can delete the arrow outright — after which the release finds no
        // shape and drops the bend without a trace.
        self.finish_active_arrow_bend();
        let changed = self.apply_toolbar_event_inner_with_resources(resources, event);
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

    fn apply_toolbar_event_inner_with_resources(
        &mut self,
        resources: crate::input::state::InputTextResources<'_>,
        event: ToolbarEvent,
    ) -> bool {
        match event {
            ToolbarEvent::SelectTool(tool) => {
                self.apply_toolbar_select_tool_with(resources.measurer, tool)
            }
            ToolbarEvent::SetColor(color) => {
                self.apply_toolbar_set_color_with_measurer(resources.measurer, color)
            }
            ToolbarEvent::SetQuickColor { color, .. } => {
                self.apply_toolbar_set_color_with_measurer(resources.measurer, color)
            }
            ToolbarEvent::EditQuickColor { index } => {
                self.apply_toolbar_edit_quick_color_with(resources.measurer, index)
            }
            ToolbarEvent::SetThickness(value) => {
                self.apply_toolbar_set_thickness_with(resources.measurer, value)
            }
            ToolbarEvent::SetMarkerOpacity(value) => self.apply_toolbar_set_marker_opacity(value),
            ToolbarEvent::SetSpotlightMagnification(value) => {
                self.apply_toolbar_set_spotlight_magnification(value)
            }
            ToolbarEvent::SetPenSmoothing(level) => self.apply_toolbar_set_pen_smoothing(level),
            ToolbarEvent::OpenFontPicker => self.apply_toolbar_open_font_picker(),
            ToolbarEvent::SetEraserMode(mode) => self.apply_toolbar_set_eraser_mode(mode),
            ToolbarEvent::SetFont(descriptor) => self.apply_toolbar_set_font(descriptor),
            ToolbarEvent::SetFontBold(bold) => {
                self.apply_toolbar_set_font_bold_with(resources.measurer, bold)
            }
            ToolbarEvent::SetFontSize(size) => self.apply_toolbar_set_font_size(size),
            ToolbarEvent::NudgeFontSize(delta) => {
                self.apply_toolbar_set_font_size(self.style.current_font_size + delta)
            }
            ToolbarEvent::ToggleFill(enable) => self.apply_toolbar_toggle_fill(enable),
            ToolbarEvent::SetPolygonSides(sides) => self.apply_toolbar_set_polygon_sides(sides),
            ToolbarEvent::NudgePolygonSides(delta) => self.apply_toolbar_nudge_polygon_sides(delta),
            ToolbarEvent::ToggleArrowLabels(enable) => {
                self.apply_toolbar_toggle_arrow_labels(enable)
            }
            ToolbarEvent::CycleArrowStyle => self.apply_toolbar_cycle_arrow_style(),
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
            ToolbarEvent::NudgeThickness(delta) => {
                self.apply_toolbar_nudge_thickness_with(resources.measurer, delta)
            }
            ToolbarEvent::NudgeMarkerOpacity(delta) => {
                self.apply_toolbar_nudge_marker_opacity(delta)
            }
            ToolbarEvent::Undo => self.apply_toolbar_undo_with_resources(resources),
            ToolbarEvent::Redo => self.apply_toolbar_redo_with_resources(resources),
            ToolbarEvent::UndoAll => self.apply_toolbar_undo_all_with_measurer(resources.measurer),
            ToolbarEvent::RedoAll => self.apply_toolbar_redo_all_with_measurer(resources.measurer),
            ToolbarEvent::UndoAllDelayed => self.apply_toolbar_undo_all_delayed(),
            ToolbarEvent::RedoAllDelayed => self.apply_toolbar_redo_all_delayed(),
            ToolbarEvent::CustomUndo => self.apply_toolbar_custom_undo(),
            ToolbarEvent::CustomRedo => self.apply_toolbar_custom_redo(),
            ToolbarEvent::ClearCanvas { instant } => {
                self.apply_toolbar_clear_canvas_with_resources(resources, instant)
            }
            ToolbarEvent::CaptureScreenshot => {
                self.apply_toolbar_capture_screenshot_with_resources(resources)
            }
            ToolbarEvent::CopyTextFromScreen => {
                self.apply_toolbar_copy_text_from_screen_with_resources(resources)
            }
            ToolbarEvent::PagePrev => self.apply_toolbar_page_prev_with(resources.measurer),
            ToolbarEvent::PageNext => self.apply_toolbar_page_next_with(resources.measurer),
            ToolbarEvent::PageNew => self.apply_toolbar_page_new_with(resources.measurer),
            ToolbarEvent::PageDuplicate => {
                self.apply_toolbar_page_duplicate_with(resources.measurer)
            }
            ToolbarEvent::PageDelete => self.apply_toolbar_page_delete_with(resources.measurer),
            ToolbarEvent::BoardPrev => self.apply_toolbar_board_prev_with(resources.measurer),
            ToolbarEvent::BoardNext => self.apply_toolbar_board_next_with(resources.measurer),
            ToolbarEvent::BoardNew => self.apply_toolbar_board_new_with(resources.measurer),
            ToolbarEvent::BoardDelete => self.apply_toolbar_board_delete_with(resources.measurer),
            ToolbarEvent::BoardDuplicate => {
                self.apply_toolbar_board_duplicate_with(resources.measurer)
            }
            ToolbarEvent::BoardRename => self.apply_toolbar_board_rename_with(resources.measurer),
            ToolbarEvent::ToggleBoardPicker => {
                self.apply_toolbar_toggle_board_picker_with(resources.measurer)
            }
            ToolbarEvent::EnterTextMode => {
                self.apply_toolbar_enter_text_mode_with_resources(resources)
            }
            ToolbarEvent::EnterStickyNoteMode => {
                self.apply_toolbar_enter_sticky_note_mode_with_resources(resources)
            }
            ToolbarEvent::ToggleAllHighlight(enable) => {
                self.apply_toolbar_toggle_all_highlight_with(resources.measurer, enable)
            }
            ToolbarEvent::ToggleHighlightToolRing(enable) => {
                self.apply_toolbar_toggle_highlight_tool_ring(enable)
            }
            ToolbarEvent::ToggleInputHud(enable) => self.apply_toolbar_toggle_input_hud(enable),
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
            ToolbarEvent::OpenAbout => self.apply_toolbar_open_about(),
            ToolbarEvent::RequestRuntimeUiReset
            | ToolbarEvent::ConfirmUnsupportedRuntimeUiReset
            | ToolbarEvent::CancelUnsupportedRuntimeUiReset
            | ToolbarEvent::RetryRuntimeUiPersistence
            | ToolbarEvent::DiscardPendingRuntimeUiAndAdoptDisk
            | ToolbarEvent::RequestPreserveInvalidRuntimeUiReset
            | ToolbarEvent::ConfirmPreserveInvalidRuntimeUiReset
            | ToolbarEvent::CancelPreserveInvalidRuntimeUiReset
            | ToolbarEvent::CancelRuntimeUiRecovery => false,
            ToolbarEvent::OpenCommandPalette => {
                self.apply_toolbar_open_command_palette_with_resources(resources)
            }
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
            ToolbarEvent::SetTopDisplayMode(mode) => {
                self.apply_toolbar_set_top_display_mode_with_engine(resources.ui_engine, mode)
            }
            ToolbarEvent::CloseTopToolbar => self.apply_toolbar_set_top_minimized(true),
            ToolbarEvent::PinTopToolbar(pin) => self.apply_toolbar_pin_top_toolbar(pin),
            ToolbarEvent::ToggleIconMode(use_icons) => {
                self.apply_toolbar_toggle_icon_mode(use_icons)
            }
            ToolbarEvent::ToggleMoreColors(show) => self.apply_toolbar_toggle_more_colors(show),
            ToolbarEvent::CopyHexColor => self.apply_toolbar_copy_hex_color(),
            ToolbarEvent::PasteHexColor => self.apply_toolbar_paste_hex_color(),
            ToolbarEvent::EditHexColor => {
                self.apply_toolbar_edit_hex_color_with(resources.measurer)
            }
            ToolbarEvent::OpenColorPickerPopup => {
                self.apply_toolbar_open_color_picker_popup_with(resources.measurer)
            }
            ToolbarEvent::AdjustSelectionProperty { kind, direction } => {
                self.adjust_selection_property_kind_with(resources.measurer, kind, direction)
            }
            ToolbarEvent::OpenPrecisionEntry(target) => {
                self.apply_toolbar_open_precision_entry(target)
            }
            ToolbarEvent::CommitPrecisionEntry { target, value } => {
                self.apply_toolbar_commit_precision_entry_with(resources.measurer, target, value)
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
            ToolbarEvent::ToggleIdleFade(enable) => self.apply_toolbar_toggle_idle_fade(enable),
            ToolbarEvent::ToggleToolPreview(show) => self.apply_toolbar_toggle_tool_preview(show),
            ToolbarEvent::ToggleStatusBar(show) => {
                self.apply_toolbar_toggle_status_bar_with_engine(resources.ui_engine, show)
            }
            ToolbarEvent::SetStatusBarInteractive(interactive) => {
                self.apply_toolbar_set_status_bar_interactive(interactive)
            }
            ToolbarEvent::SetStatusBarItemVisible(item, visible) => self
                .apply_toolbar_set_status_bar_item_visible_with_engine(
                    resources.ui_engine,
                    item,
                    visible,
                ),
            ToolbarEvent::ToggleStatusBoardBadge(show) => {
                self.apply_toolbar_toggle_status_board_badge_with_engine(resources.ui_engine, show)
            }
            ToolbarEvent::ToggleStatusPageBadge(show) => {
                self.apply_toolbar_toggle_status_page_badge_with_engine(resources.ui_engine, show)
            }
            ToolbarEvent::ToggleFloatingBadgeAlways(show) => {
                self.apply_toolbar_toggle_floating_badge_always(show)
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
            ToolbarEvent::SetStatusBarContentsOpen(open) => {
                self.apply_toolbar_set_status_bar_contents_open(open)
            }
            ToolbarEvent::ToggleShapePicker(open) => self.apply_toolbar_toggle_shape_picker(open),
            ToolbarEvent::ApplyPreset(slot) => {
                self.apply_toolbar_apply_preset_with(resources.measurer, slot)
            }
            ToolbarEvent::SavePreset(slot) => self.apply_toolbar_save_preset(slot),
            ToolbarEvent::ClearPreset(slot) => self.apply_toolbar_clear_preset(slot),
            ToolbarEvent::OpenSession
            | ToolbarEvent::OpenRecentSession(_)
            | ToolbarEvent::SaveSessionAs
            | ToolbarEvent::SaveSessionAsConfirm(_)
            | ToolbarEvent::SaveSessionAsCancel
            | ToolbarEvent::SessionInfo
            | ToolbarEvent::ClearSession => false,
            ToolbarEvent::MoveTopToolbar { .. } => false,
        }
    }
}

#[cfg(test)]
mod coach_tests {
    use crate::config::{Action, Shortcut};
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
    fn canvas_popover_action_with_shortcut_records_coach_slow_path() {
        // The M8 "Canvas…" overflow popover dispatches its boards/pages/zoom/
        // undo-all/freeze controls as ToolbarEvents, so they route through
        // apply_toolbar_event and feed the shortcut coach the same slow-path
        // signal as any other toolbar use. Zoom In is a canvas-popover control
        // with a default shortcut, so activating it must nudge toward the key.
        let mut state = make_test_input_state();
        assert!(
            state.shortcut_for_action(Action::ZoomIn).is_some(),
            "test relies on ZoomIn having a default shortcut"
        );
        assert_eq!(
            ToolbarEvent::ZoomIn.action(),
            Some(Action::ZoomIn),
            "the canvas-popover Zoom In maps to the ZoomIn action"
        );
        assert!(!state.pending_onboarding_usage.used_zoom_control);

        assert!(state.apply_toolbar_event(ToolbarEvent::ZoomIn));

        assert!(state.pending_onboarding_usage.used_zoom_control);
        assert_eq!(
            state.pending_onboarding_usage.shortcut_slow_path_action,
            Some(Action::ZoomIn),
            "canvas-popover Zoom In feeds the coach slow path"
        );
        assert_eq!(state.pending_onboarding_usage.shortcut_slow_path_repeats, 1);
    }

    #[test]
    fn toolbar_action_without_shortcut_does_not_coach() {
        // Undo explicitly bound to nothing: it resolves to no shortcut, so
        // there is nothing to coach — the coach must not record a slow path it
        // cannot name. (An empty map would fall back to the default action map,
        // which still binds Undo, so the override must be an explicit empty
        // binding list.)
        let bindings: HashMap<Action, Vec<Shortcut>> = HashMap::from([(Action::Undo, Vec::new())]);
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
