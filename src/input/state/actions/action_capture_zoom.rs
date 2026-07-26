use crate::domain::Action;
use crate::input::{OutputFocusAction, ZoomAction};

use super::super::{InputState, PendingBackendAction, Toast, ToastPriority};

impl InputState {
    pub(in crate::input::state) fn handle_capture_zoom_action(&mut self, action: Action) -> bool {
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

                // Clear modifiers to prevent them from being "stuck" after capture
                // (portal dialog causes key releases to be missed or focus to flicker)
                self.reset_modifiers();
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
            Action::ToggleStepCapture => {
                let armed = self.toggle_step_capture();
                if armed {
                    self.push_toast(
                        ToastPriority::Info,
                        "steps",
                        Toast::info(format!(
                            "Step capture armed - next step {}. Run Capture Step for each step.",
                            self.next_step_number()
                        )),
                    );
                } else {
                    self.push_toast(
                        ToastPriority::Info,
                        "steps",
                        Toast::info("Step capture off - review the Steps board"),
                    );
                }
                true
            }
            Action::CaptureStep => {
                if !self.step_capture_armed() {
                    self.push_toast(
                        ToastPriority::Info,
                        "steps",
                        Toast::info("Step capture is not armed")
                            .action("Arm", Action::ToggleStepCapture),
                    );
                    return true;
                }
                log::debug!("Step capture pending for backend");
                self.set_pending_backend_action(PendingBackendAction::StepCapture);
                self.reset_modifiers();
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
}
