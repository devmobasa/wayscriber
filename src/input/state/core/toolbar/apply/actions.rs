use crate::config::Action;
use crate::input::{InputState, ZoomAction};

impl InputState {
    pub(super) fn apply_toolbar_undo_with_resources(
        &mut self,
        resources: crate::input::state::InputTextResources<'_>,
    ) -> bool {
        self.toolbar_undo_with_resources(resources);
        true
    }

    pub(super) fn apply_toolbar_redo_with_resources(
        &mut self,
        resources: crate::input::state::InputTextResources<'_>,
    ) -> bool {
        self.toolbar_redo_with_resources(resources);
        true
    }

    pub(super) fn apply_toolbar_undo_all_with_measurer(
        &mut self,
        measurer: &crate::draw::TextMeasurer,
    ) -> bool {
        self.undo_all_immediate_with_measurer(measurer);
        true
    }

    pub(super) fn apply_toolbar_redo_all_with_measurer(
        &mut self,
        measurer: &crate::draw::TextMeasurer,
    ) -> bool {
        self.redo_all_immediate_with_measurer(measurer);
        true
    }

    pub(super) fn apply_toolbar_undo_all_delayed(&mut self) -> bool {
        self.start_undo_all_delayed(self.history_limits.undo_all_delay_ms());
        true
    }

    pub(super) fn apply_toolbar_redo_all_delayed(&mut self) -> bool {
        self.start_redo_all_delayed(self.history_limits.redo_all_delay_ms());
        true
    }

    pub(super) fn apply_toolbar_custom_undo(&mut self) -> bool {
        self.start_custom_undo(
            self.history_limits.custom_undo_delay_ms(),
            self.history_limits.custom_undo_steps(),
        );
        true
    }

    pub(super) fn apply_toolbar_custom_redo(&mut self) -> bool {
        self.start_custom_redo(
            self.history_limits.custom_redo_delay_ms(),
            self.history_limits.custom_redo_steps(),
        );
        true
    }

    pub(super) fn apply_toolbar_clear_canvas_with_resources(
        &mut self,
        resources: crate::input::state::InputTextResources<'_>,
        instant: bool,
    ) -> bool {
        if instant {
            self.toolbar_clear_with_resources(resources);
        } else {
            self.toolbar_clear_with_undo_toast_with_resources(resources);
        }
        true
    }

    pub(super) fn apply_toolbar_capture_screenshot_with_resources(
        &mut self,
        resources: crate::input::state::InputTextResources<'_>,
    ) -> bool {
        self.handle_action_with_resources(resources, Action::CaptureSelection);
        self.close_top_toolbar_menus();
        true
    }

    pub(super) fn apply_toolbar_copy_text_from_screen_with_resources(
        &mut self,
        resources: crate::input::state::InputTextResources<'_>,
    ) -> bool {
        self.handle_action_with_resources(resources, Action::CopyTextFromScreen);
        self.close_top_toolbar_menus();
        true
    }

    pub(super) fn apply_toolbar_open_command_palette_with_resources(
        &mut self,
        resources: crate::input::state::InputTextResources<'_>,
    ) -> bool {
        self.handle_action_with_resources(resources, Action::ToggleCommandPalette);
        self.close_top_toolbar_menus();
        true
    }

    pub(super) fn apply_toolbar_toggle_freeze(&mut self) -> bool {
        self.request_frozen_toggle();
        self.needs_redraw = true;
        true
    }

    pub(super) fn apply_toolbar_zoom_in(&mut self) -> bool {
        self.request_zoom_action(ZoomAction::In);
        true
    }

    pub(super) fn apply_toolbar_zoom_out(&mut self) -> bool {
        self.request_zoom_action(ZoomAction::Out);
        true
    }

    pub(super) fn apply_toolbar_reset_zoom(&mut self) -> bool {
        self.request_zoom_action(ZoomAction::Reset);
        true
    }

    pub(super) fn apply_toolbar_toggle_zoom_lock(&mut self) -> bool {
        self.request_zoom_action(ZoomAction::ToggleLock);
        true
    }

    pub(super) fn apply_toolbar_refresh_zoom_capture(&mut self) -> bool {
        self.request_zoom_action(ZoomAction::RefreshCapture);
        true
    }
}

#[cfg(test)]
mod tests {
    use crate::input::state::test_support::make_test_input_state;
    use crate::ui::toolbar::ToolbarEvent;

    #[test]
    fn open_command_palette_event_toggles_the_palette() {
        let mut state = make_test_input_state();
        assert!(!state.command_palette_is_engaged());

        let opened = state.apply_toolbar_event(ToolbarEvent::OpenCommandPalette);
        assert!(opened);
        assert!(state.command_palette_is_engaged());

        // The toolbar hook reuses the same toggle as the Ctrl+K shortcut.
        let closed = state.apply_toolbar_event(ToolbarEvent::OpenCommandPalette);
        assert!(closed);
        assert!(!state.command_palette_is_engaged());
    }
}
