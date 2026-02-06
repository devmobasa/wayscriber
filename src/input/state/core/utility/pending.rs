use super::super::base::{InputState, OutputFocusAction, PresetAction, ZoomAction};
use crate::config::{Action, BoardsConfig};

impl InputState {
    /// Takes and clears any pending capture action.
    pub fn take_pending_capture_action(&mut self) -> Option<Action> {
        self.pending_capture_action.take()
    }

    /// Stores a capture action for retrieval by the backend.
    pub(crate) fn set_pending_capture_action(&mut self, action: Action) {
        self.pending_capture_action = Some(action);
    }

    /// Stores an output focus action for retrieval by the backend.
    pub(crate) fn request_output_focus_action(&mut self, action: OutputFocusAction) {
        self.pending_output_focus_action = Some(action);
    }

    /// Takes and clears any pending output focus action.
    pub fn take_pending_output_focus_action(&mut self) -> Option<OutputFocusAction> {
        self.pending_output_focus_action.take()
    }

    /// Stores a zoom action for retrieval by the backend.
    pub(crate) fn request_zoom_action(&mut self, action: ZoomAction) {
        self.pending_zoom_action = Some(action);
    }

    /// Takes and clears any pending zoom action.
    pub fn take_pending_zoom_action(&mut self) -> Option<ZoomAction> {
        self.pending_zoom_action.take()
    }

    /// Takes and clears any pending preset save/clear action.
    pub fn take_pending_preset_action(&mut self) -> Option<PresetAction> {
        self.pending_preset_action.take()
    }

    /// Takes and clears any pending board config update.
    pub fn take_pending_board_config(&mut self) -> Option<BoardsConfig> {
        self.pending_board_config.take()
    }
}
