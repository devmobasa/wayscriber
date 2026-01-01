use super::super::base::{InputState, PresetAction, ZoomAction};
use crate::config::Action;

impl InputState {
    /// Takes and clears any pending capture action.
    pub fn take_pending_capture_action(&mut self) -> Option<Action> {
        self.pending_capture_action.take()
    }

    /// Stores a capture action for retrieval by the backend.
    pub(crate) fn set_pending_capture_action(&mut self, action: Action) {
        self.pending_capture_action = Some(action);
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
}
