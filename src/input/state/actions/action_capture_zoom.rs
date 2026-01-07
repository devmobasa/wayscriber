use crate::config::Action;
use crate::input::ZoomAction;

use super::super::InputState;

impl InputState {
    pub(super) fn handle_capture_zoom_action(&mut self, action: Action) -> bool {
        match action {
            Action::CaptureFullScreen
            | Action::CaptureActiveWindow
            | Action::CaptureSelection
            | Action::CaptureClipboardFull
            | Action::CaptureFileFull
            | Action::CaptureClipboardSelection
            | Action::CaptureFileSelection
            | Action::CaptureClipboardRegion
            | Action::CaptureFileRegion => {
                // Capture actions are handled externally by WaylandState
                // since they require access to CaptureManager
                // Store the action for later retrieval
                log::debug!("Capture action {:?} pending for backend", action);
                self.set_pending_capture_action(action);

                // Clear modifiers to prevent them from being "stuck" after capture
                // (portal dialog causes key releases to be missed or focus to flicker)
                self.reset_modifiers();
                true
            }
            Action::ToggleFrozenMode => {
                log::info!("Toggle frozen mode requested");
                self.request_frozen_toggle();
                self.reset_modifiers();
                true
            }
            Action::ZoomIn => {
                self.request_zoom_action(ZoomAction::In);
                self.reset_modifiers();
                true
            }
            Action::ZoomOut => {
                self.request_zoom_action(ZoomAction::Out);
                self.reset_modifiers();
                true
            }
            Action::ResetZoom => {
                self.request_zoom_action(ZoomAction::Reset);
                self.reset_modifiers();
                true
            }
            Action::ToggleZoomLock => {
                self.request_zoom_action(ZoomAction::ToggleLock);
                self.reset_modifiers();
                true
            }
            Action::RefreshZoomCapture => {
                self.request_zoom_action(ZoomAction::RefreshCapture);
                self.reset_modifiers();
                true
            }
            Action::SavePendingToFile => {
                // This action is handled directly by InputState since we have the image data
                // and can use default save config
                self.save_pending_clipboard_to_file();
                true
            }
            _ => false,
        }
    }
}
