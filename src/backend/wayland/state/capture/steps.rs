//! Step-capture backend: one desktop-backdrop capture per armed step,
//! appended to the Steps board as a numbered guide page.

use super::super::*;
use crate::backend::wayland::capture::PendingStepCapture;
use crate::draw::EmbeddedImage;
use crate::input::state::{StepCaptureFrame, Toast, ToastPriority};

impl WaylandState {
    /// Captures the screen as the next step page. The pointer position is
    /// recorded before suppression so the marker lands where the user was
    /// pointing, not where the cursor drifted during the capture.
    pub(in crate::backend::wayland) fn handle_step_capture_action(&mut self) {
        if self.capture.is_in_progress() {
            log::warn!("Step capture requested while another image operation is running; ignoring");
            self.input_state.push_toast(
                ToastPriority::Info,
                "steps",
                Toast::warning("Another capture is still running."),
            );
            return;
        }

        let operation = ImageOperationKind::StepCapture;
        let request = self.desktop_backdrop_capture_request(operation);
        if !self.enter_overlay_suppression(OverlaySuppression::DesktopBackdrop) {
            log::warn!("Step capture requested while overlay is suppressed; ignoring");
            self.input_state.push_toast(
                ToastPriority::Info,
                "steps",
                Toast::warning("Step capture is already preparing another overlay operation."),
            );
            return;
        }

        let marker = self.input_state.pointer_position_if_seen();
        self.capture.mark_in_progress();
        self.capture.set_pending_step_capture(PendingStepCapture {
            marker,
            logical_width: self.surface.width() as i32,
            logical_height: self.surface.height() as i32,
        });
        log::info!(
            "Queued step capture (step {}); waiting for suppression frame",
            self.input_state.next_step_number()
        );
        self.capture
            .queue_preflight(CapturePreflightRequest::DesktopBackdrop(request));
    }

    /// Applies a completed step-capture frame: appends the Steps page with
    /// the encoded frame as its locked backdrop and toasts the step number.
    pub(in crate::backend::wayland) fn finish_pending_step_capture(
        &mut self,
        backdrop: DesktopBackdropCaptureResult,
    ) {
        let Some(pending) = self.capture.take_pending_step_capture() else {
            let message = "Step capture failed: desktop backdrop completed without a pending step"
                .to_string();
            log::error!("{message}");
            self.input_state
                .push_toast(ToastPriority::Critical, "steps", Toast::error(message));
            return;
        };
        let Some(bytes) = backdrop.encoded_png else {
            let message = "Step capture failed: the captured frame was not encoded".to_string();
            log::error!("{message}");
            self.input_state
                .push_toast(ToastPriority::Critical, "steps", Toast::error(message));
            return;
        };

        let frame = StepCaptureFrame {
            image: EmbeddedImage {
                mime_type: "image/png".to_string(),
                width: backdrop.width.max(0) as u32,
                height: backdrop.height.max(0) as u32,
                bytes,
            },
            logical_width: pending.logical_width,
            logical_height: pending.logical_height,
            marker: pending.marker,
        };

        match self.input_state.append_step_page(frame) {
            Some(receipt) => {
                log::info!("Captured step {} onto the Steps board", receipt.step);
                self.input_state.push_toast(
                    ToastPriority::Info,
                    "steps",
                    Toast::info(format!("Step {} captured", receipt.step)),
                );
            }
            None => {
                let message =
                    "Step capture failed: the board limit prevents creating the Steps board"
                        .to_string();
                log::error!("{message}");
                self.input_state.push_toast(
                    ToastPriority::Critical,
                    "steps",
                    Toast::error(message),
                );
            }
        }
    }
}
