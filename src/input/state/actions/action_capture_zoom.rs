use crate::domain::Action;
use crate::input::state::{RegionPurposeTag, Toast, ToastPriority};
use crate::input::{OutputFocusAction, ZoomAction};

use super::super::{InputState, PendingBackendAction};

impl InputState {
    pub(in crate::input::state) fn handle_capture_zoom_action(&mut self, action: Action) -> bool {
        if self.refuse_region_capture_while_screen_modal_engaged(action) {
            return true;
        }
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
                self.set_pending_backend_action(PendingBackendAction::Screenshot(action));

                // Native region selection needs the compositor-synced Shift
                // state when it arms. Focus loss still resets modifiers for
                // slurp/portal handoffs, where releases may be missed.
                if !action.is_region_capture() {
                    self.reset_modifiers();
                }
                true
            }
            Action::ExportCanvasFile
            | Action::ExportCanvasClipboard
            | Action::ExportCanvasClipboardAndFile => {
                log::debug!("Canvas export action {:?} pending for backend", action);
                self.set_pending_backend_action(PendingBackendAction::CanvasExport(action));

                // Clear modifiers to prevent them from being "stuck" after capture
                // (portal dialog causes key releases to be missed or focus to flicker)
                self.reset_modifiers();
                true
            }
            Action::ExportBoardPdfFile | Action::ExportAllBoardsPdfFile => {
                log::debug!("Board PDF export action {:?} pending for backend", action);
                self.set_pending_backend_action(PendingBackendAction::BoardPdfExport(action));

                // Clear modifiers to prevent them from being "stuck" after capture
                // (portal dialog causes key releases to be missed or focus to flicker)
                self.reset_modifiers();
                true
            }
            Action::CopyTextFromScreen => {
                // The backend owns capture ownership and the region selector,
                // so this only records the intent. It selects no tool and
                // touches no drawing state.
                log::debug!("Copy text from screen requested");
                self.request_copy_text_from_screen();
                true
            }
            Action::ToggleFrozenMode => {
                log::info!("Toggle frozen mode requested");
                self.request_frozen_toggle();
                self.reset_modifiers();
                true
            }
            // Zoom stays within the focused overlay. Preserve physically held modifiers so
            // consecutive zoom shortcuts work until their real release events arrive.
            Action::ZoomIn => {
                self.request_zoom_action(ZoomAction::In);
                true
            }
            Action::ZoomOut => {
                self.request_zoom_action(ZoomAction::Out);
                true
            }
            Action::ResetZoom => {
                self.request_zoom_action(ZoomAction::Reset);
                true
            }
            Action::ToggleZoomLock => {
                self.request_zoom_action(ZoomAction::ToggleLock);
                true
            }
            Action::RefreshZoomCapture => {
                self.request_zoom_action(ZoomAction::RefreshCapture);
                true
            }
            Action::FocusNextOutput => {
                self.request_output_focus_action(OutputFocusAction::Next);
                self.reset_modifiers();
                true
            }
            Action::FocusPrevOutput => {
                self.request_output_focus_action(OutputFocusAction::Prev);
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

    pub(crate) fn refuse_region_capture_while_screen_modal_engaged(
        &mut self,
        action: Action,
    ) -> bool {
        if !self.screen_modal_is_engaged() || !action.is_region_capture() {
            return false;
        }
        if self
            .region_state()
            .purpose()
            .is_some_and(RegionPurposeTag::is_capture)
        {
            // Capture-owned selectors pass their immutable intent to the
            // backend, which decides same-opener cancellation versus refusal.
            return false;
        }
        self.push_toast(
            ToastPriority::Info,
            "capture.region.refused",
            Toast::info("Finish or cancel the current screen selection first."),
        );
        true
    }

    pub(crate) fn capture_region_action_reaches_backend(&self, action: Action) -> bool {
        self.region_state()
            .purpose()
            .is_some_and(RegionPurposeTag::is_capture)
            && action.is_region_capture()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::state::test_support::make_test_input_state;
    use crate::input::state::{EyedropperCaptureSource, RegionPurposeTag, ScreenCaptureSource};

    #[test]
    fn region_actions_refuse_without_replacing_an_engaged_screen_modal() {
        for action in [
            Action::CaptureSelection,
            Action::CaptureClipboardSelection,
            Action::CaptureFileSelection,
            Action::CaptureClipboardRegion,
            Action::CaptureFileRegion,
        ] {
            for modal in ["ocr", "eyedropper"] {
                let mut state = make_test_input_state();
                if modal == "ocr" {
                    state.set_region_pending_capture(
                        RegionPurposeTag::Ocr,
                        1,
                        ScreenCaptureSource::Frozen,
                    );
                } else {
                    state.set_eyedropper_pending_capture(EyedropperCaptureSource::Frozen);
                    state.activate_eyedropper(None);
                }

                assert!(state.handle_capture_zoom_action(action));

                assert!(state.take_pending_backend_action().is_none());
                assert!(state.screen_modal_is_engaged());
                assert_eq!(state.test_toast_count(), 1);
                assert_eq!(
                    state.test_active_toast_message(),
                    Some("Finish or cancel the current screen selection first.")
                );
            }
        }
    }

    #[test]
    fn region_actions_reach_the_backend_when_a_capture_selector_owns_the_modal() {
        let mut state = make_test_input_state();
        state.set_region_pending_capture(
            RegionPurposeTag::CaptureDeliver,
            1,
            ScreenCaptureSource::Frozen,
        );

        assert!(state.handle_capture_zoom_action(Action::CaptureClipboardSelection));

        assert_eq!(
            state.take_pending_backend_action(),
            Some(PendingBackendAction::Screenshot(
                Action::CaptureClipboardSelection
            ))
        );
        assert_eq!(state.test_toast_count(), 0);
    }
}
