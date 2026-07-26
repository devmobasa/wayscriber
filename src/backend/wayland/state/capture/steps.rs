//! Step-capture backend: one desktop-backdrop capture per armed step,
//! appended to the Steps board as a numbered guide page.

use super::super::*;
use crate::backend::wayland::capture::PendingStepCapture;
use crate::canvas_export::{GuideStep, guide_image_file_stem, render_guide_markdown};
use crate::capture::{DocumentAttachment, DocumentDeliveryRequest, RenderedDocument};
use crate::draw::EmbeddedImage;
use crate::input::state::{STEP_CAPTURE_BOARD_ID, StepCaptureFrame, Toast, ToastPriority};
use smithay_client_toolkit::seat::pointer::BTN_LEFT;
use wayland_client::protocol::wl_pointer;

impl WaylandState {
    /// Captures the screen as the next step page. The pointer position is
    /// recorded before suppression so the marker lands where the user was
    /// pointing, not where the cursor drifted during the capture.
    pub(in crate::backend::wayland) fn handle_step_capture_action(&mut self) {
        self.begin_step_capture(false);
    }

    /// Guide-mode variant: the press that reached the canvas becomes the
    /// step, and the click is re-sent beneath the overlay after capture.
    pub(in crate::backend::wayland) fn handle_step_capture_click(&mut self) {
        self.begin_step_capture(true);
    }

    fn begin_step_capture(&mut self, forward_click: bool) {
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
            forward_click,
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

        if pending.forward_click {
            self.forward_intercepted_click();
        }
    }

    /// Re-send the intercepted click through the compositor's virtual
    /// pointer. Runs right after capture completion: suppression has been
    /// released in state but the commit restoring the overlay's input region
    /// has not happened yet, so the compositor still routes the click to the
    /// application beneath. The pointer is physically at the intercepted
    /// position, so no motion is needed.
    fn forward_intercepted_click(&mut self) {
        let Some(pointer) = self.virtual_pointer.as_ref() else {
            log::warn!("Step click captured but the compositor offers no virtual pointer");
            self.input_state.push_toast(
                ToastPriority::Info,
                "steps.forward",
                Toast::info("Click forwarding is unavailable on this compositor"),
            );
            return;
        };
        let time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|elapsed| elapsed.as_millis() as u32)
            .unwrap_or_default();
        pointer.button(time, BTN_LEFT, wl_pointer::ButtonState::Pressed);
        pointer.frame();
        pointer.button(
            time.saturating_add(1),
            BTN_LEFT,
            wl_pointer::ButtonState::Released,
        );
        pointer.frame();
    }

    /// Exports the Steps board as a Markdown guide bundle: one directory
    /// with `guide.md` plus a rendered PNG per page, saved through the
    /// capture worker so file IO stays off the dispatch thread.
    pub(in crate::backend::wayland) fn handle_steps_guide_export_action(&mut self) {
        if self.capture.is_in_progress() {
            log::warn!("Guide export requested while another image operation is running; ignoring");
            self.input_state.push_toast(
                ToastPriority::Info,
                "steps",
                Toast::warning("Another capture is still running."),
            );
            return;
        }

        let operation = ImageOperationKind::StepsGuideExport;
        let rendered = {
            let Some(board) = self
                .input_state
                .boards
                .board_states()
                .iter()
                .find(|board| board.spec.id == STEP_CAPTURE_BOARD_ID)
                .filter(|board| {
                    board
                        .pages
                        .pages()
                        .iter()
                        .any(|page| !page.shapes.is_empty())
                })
            else {
                self.input_state.push_toast(
                    ToastPriority::Info,
                    "steps",
                    Toast::info("No steps to export yet").action("Arm", Action::ToggleStepCapture),
                );
                return;
            };

            let mut attachments = Vec::new();
            let mut steps = Vec::new();
            let mut render_error = None;
            for (index, page) in board.pages.pages().iter().enumerate() {
                let snapshot = crate::canvas_export::CanvasExportSnapshot {
                    viewport: crate::canvas_export::CanvasExportViewport {
                        logical_width: self.surface.width(),
                        logical_height: self.surface.height(),
                        scale: self.surface.scale(),
                        origin_x: page.view_offset().0,
                        origin_y: page.view_offset().1,
                    },
                    spotlight: crate::canvas_export::SpotlightPassSnapshot {
                        dim_opacity: self.input_state.spotlight_dim_opacity,
                        feather: self.input_state.spotlight_feather,
                    },
                    backdrop: match &board.spec.background {
                        crate::input::BoardBackground::Transparent => {
                            CanvasExportBackdropSnapshot::Transparent
                        }
                        crate::input::BoardBackground::Solid(color) => {
                            CanvasExportBackdropSnapshot::Solid(*color)
                        }
                    },
                    board: crate::canvas_export::BoardExportSnapshot {
                        frame: page.clone_without_history(),
                    },
                    render_profile: self.input_state.export_render_profile(),
                };
                match crate::canvas_export::render_canvas_png(&snapshot) {
                    Ok(image) => {
                        attachments.push(DocumentAttachment {
                            file_stem: guide_image_file_stem(index + 1),
                            extension: "png".to_string(),
                            bytes: image.bytes,
                        });
                        steps.push(GuideStep {
                            title: page.page_name.clone(),
                        });
                    }
                    Err(err) => {
                        render_error = Some(operation.format_error(&err));
                        break;
                    }
                }
            }
            match render_error {
                Some(message) => Err(message),
                None => Ok((attachments, steps)),
            }
        };

        let (attachments, steps) = match rendered {
            Ok(rendered) => rendered,
            Err(message) => {
                log::error!("{message}");
                self.input_state.push_toast(
                    ToastPriority::Critical,
                    "steps",
                    Toast::error(message),
                );
                return;
            }
        };

        let markdown = render_guide_markdown(&steps);
        let save_config = FileSaveConfig {
            save_directory: expand_tilde(&self.config.capture.save_directory),
            filename_template: "steps_guide_%Y-%m-%d_%H%M%S".to_string(),
            format: "md".to_string(),
        };

        self.capture.set_exit_on_success(false);
        self.capture.mark_in_progress();
        let request = DocumentDeliveryRequest {
            document: RenderedDocument {
                bytes: markdown.into_bytes(),
                extension: "md".to_string(),
                mime_type: "text/markdown".to_string(),
            },
            attachments,
            destination: CaptureDestination::FileOnly,
            save_config: Some(save_config),
            operation,
        };
        let submission = self
            .capture
            .manager_mut()
            .request_document_delivery(request);
        self.accept_capture_submission(submission, operation);
    }
}
