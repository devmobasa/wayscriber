use crate::input::{InputState, ZoomAction};

impl InputState {
    pub(super) fn apply_toolbar_undo(&mut self) -> bool {
        self.toolbar_undo();
        true
    }

    pub(super) fn apply_toolbar_redo(&mut self) -> bool {
        self.toolbar_redo();
        true
    }

    pub(super) fn apply_toolbar_undo_all(&mut self) -> bool {
        self.undo_all_immediate();
        true
    }

    pub(super) fn apply_toolbar_redo_all(&mut self) -> bool {
        self.redo_all_immediate();
        true
    }

    pub(super) fn apply_toolbar_undo_all_delayed(&mut self) -> bool {
        self.start_undo_all_delayed(self.undo_all_delay_ms);
        true
    }

    pub(super) fn apply_toolbar_redo_all_delayed(&mut self) -> bool {
        self.start_redo_all_delayed(self.redo_all_delay_ms);
        true
    }

    pub(super) fn apply_toolbar_custom_undo(&mut self) -> bool {
        self.start_custom_undo(self.custom_undo_delay_ms, self.custom_undo_steps);
        true
    }

    pub(super) fn apply_toolbar_custom_redo(&mut self) -> bool {
        self.start_custom_redo(self.custom_redo_delay_ms, self.custom_redo_steps);
        true
    }

    pub(super) fn apply_toolbar_clear_canvas(&mut self) -> bool {
        self.toolbar_clear();
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
