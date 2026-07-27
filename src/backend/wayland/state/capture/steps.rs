//! Step-capture backend: one desktop-backdrop capture per armed step,
//! appended to the Steps board as a numbered guide page.

use super::super::*;
use crate::backend::wayland::capture::{PendingStepCapture, PendingStepClick};
use crate::canvas_export::{GuideStep, guide_image_file_stem, render_guide_markdown};
use crate::capture::{DocumentAttachment, DocumentDeliveryRequest, RenderedDocument};
use crate::draw::EmbeddedImage;
use crate::input::state::{
    STEP_CAPTURE_BOARD_ID, StepCaptureFrame, StepPageOutcome, Toast, ToastPriority,
};
use smithay_client_toolkit::seat::pointer::BTN_LEFT;
use wayland_client::protocol::wl_pointer;

impl WaylandState {
    /// Captures the screen as the next step page. The pointer position is
    /// recorded before suppression so the marker lands where the user was
    /// pointing, not where the cursor drifted during the capture.
    pub(in crate::backend::wayland) fn handle_step_capture_action(&mut self) {
        self.begin_step_capture(None);
    }

    /// Guide-mode variant: the press that reached the canvas becomes the
    /// step, and the click is re-sent beneath the overlay after capture at
    /// the position it was intercepted.
    ///
    /// Returns whether the press was taken. A press this refuses has not been
    /// swallowed, so the caller must let it act as an ordinary canvas press
    /// rather than dropping it on the floor.
    pub(in crate::backend::wayland) fn handle_step_capture_click(
        &mut self,
        x: i32,
        y: i32,
    ) -> bool {
        self.begin_step_capture(Some((x, y)))
    }

    fn begin_step_capture(&mut self, forward_click: Option<(i32, i32)>) -> bool {
        // Step capture drives the same full-screen source as the capture
        // actions, so the switch that turns those off has to stop it too.
        if !self.config.capture.enabled {
            log::warn!("Step capture requested but capture is disabled in config");
            self.input_state.push_toast(
                ToastPriority::Info,
                "steps",
                Toast::warning("Screen capture is disabled in the config."),
            );
            return false;
        }

        if self.capture.is_in_progress() {
            log::warn!("Step capture requested while another image operation is running; ignoring");
            self.input_state.push_toast(
                ToastPriority::Info,
                "steps",
                Toast::warning("Another capture is still running."),
            );
            return false;
        }

        let resolved_forward_click = match forward_click {
            Some((x, y)) => {
                let Some(target) = self.virtual_pointer_absolute(x, y) else {
                    log::warn!(
                        "Step click capture requires output geometry for safe click forwarding"
                    );
                    self.input_state.push_toast(
                        ToastPriority::Info,
                        "steps.forward",
                        Toast::warning(
                            "Step click capture is unavailable until output geometry is known.",
                        ),
                    );
                    return false;
                };
                Some(target)
            }
            None => None,
        };

        let operation = ImageOperationKind::StepCapture;
        let request = self.desktop_backdrop_capture_request(operation);
        if !self.enter_overlay_suppression(OverlaySuppression::DesktopBackdrop) {
            log::warn!("Step capture requested while overlay is suppressed; ignoring");
            self.input_state.push_toast(
                ToastPriority::Info,
                "steps",
                Toast::warning("Step capture is already preparing another overlay operation."),
            );
            return false;
        }

        // The intercepted press is authoritative for its own step; only the
        // keybinding route has to fall back to the last seen pointer.
        let marker = forward_click.or_else(|| self.input_state.pointer_position_if_seen());
        self.capture.mark_in_progress();
        self.capture.set_pending_step_capture(PendingStepCapture {
            marker,
            logical_width: self.surface.width() as i32,
            logical_height: self.surface.height() as i32,
            forward_click: resolved_forward_click,
        });
        log::info!(
            "Queued step capture (step {}); waiting for suppression frame",
            self.input_state.next_step_number()
        );
        self.capture
            .queue_preflight(CapturePreflightRequest::DesktopBackdrop(request));
        true
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
            StepPageOutcome::Appended(receipt) => {
                log::info!("Captured step {} onto the Steps board", receipt.step);
                self.input_state.push_toast(
                    ToastPriority::Info,
                    "steps",
                    Toast::info(format!("Step {} captured", receipt.step)),
                );
            }
            StepPageOutcome::BoardLimit => {
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
            StepPageOutcome::SessionLimit => {
                // The preflight already explained itself in its own toast;
                // it refused rather than let the session become unsaveable.
                log::warn!("Step capture refused: the session save limit would be exceeded");
            }
            StepPageOutcome::ShapeLimit => {
                // The input-state guard already supplied the actionable toast.
                log::warn!("Step capture refused: the per-frame shape limit is too low");
            }
        }
    }

    /// Re-sends an intercepted click through the compositor's virtual pointer,
    /// so the application beneath still gets the press the overlay swallowed.
    ///
    /// Must run while the empty input region committed for suppression is
    /// still current: restoring the overlay's region first would route the
    /// synthetic press back into Wayscriber. Every terminal capture path calls
    /// this before [`WaylandState::show_overlay`], including the failure and
    /// cancellation paths — the real press is already gone, so a transient
    /// capture failure must not swallow the user's click as well.
    ///
    /// The press is replayed at the recorded position rather than wherever the
    /// cursor sits now: the capture is asynchronous, and a pointer that drifted
    /// in the meantime would otherwise activate a different control than the
    /// one the step marker names.
    pub(in crate::backend::wayland) fn forward_pending_step_click(&mut self) {
        let Some(target) = self.capture.take_pending_step_click() else {
            return;
        };
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
        pointer.motion_absolute(time, target.x, target.y, target.x_extent, target.y_extent);
        pointer.frame();
        pointer.button(time, BTN_LEFT, wl_pointer::ButtonState::Pressed);
        pointer.frame();
        pointer.button(
            time.saturating_add(1),
            BTN_LEFT,
            wl_pointer::ButtonState::Released,
        );
        pointer.frame();
    }

    /// Maps a surface-local logical position onto the coordinate frame a
    /// virtual pointer's absolute motion uses: the whole output layout, since
    /// the pointer is created without an output binding. Returns the position
    /// and the extents it is expressed against.
    fn virtual_pointer_absolute(&self, x: i32, y: i32) -> Option<PendingStepClick> {
        let output = self.surface.current_output()?;
        let (origin_x, origin_y) = self.output_state.info(&output)?.logical_position?;

        let mut min_x = i32::MAX;
        let mut min_y = i32::MAX;
        let mut max_x = i32::MIN;
        let mut max_y = i32::MIN;
        for output in self.output_state.outputs() {
            let info = self.output_state.info(&output)?;
            let (logical_x, logical_y) = info.logical_position?;
            let (width, height) = info.logical_size?;
            if width <= 0 || height <= 0 {
                return None;
            }
            min_x = min_x.min(logical_x);
            min_y = min_y.min(logical_y);
            max_x = max_x.max(logical_x.saturating_add(width));
            max_y = max_y.max(logical_y.saturating_add(height));
        }

        let x_extent = u32::try_from(max_x.checked_sub(min_x)?).ok()?;
        let y_extent = u32::try_from(max_y.checked_sub(min_y)?).ok()?;
        if x_extent == 0 || y_extent == 0 {
            return None;
        }
        let layout_x = origin_x.saturating_add(x).saturating_sub(min_x);
        let layout_y = origin_y.saturating_add(y).saturating_sub(min_y);
        Some(PendingStepClick {
            x: (layout_x.max(0) as u32).min(x_extent),
            y: (layout_y.max(0) as u32).min(y_extent),
            x_extent,
            y_extent,
        })
    }

    /// Exports the Steps board as a Markdown guide bundle: one directory
    /// with `guide.md` plus a rendered PNG per page. Page snapshots are handed
    /// to the capture worker so rendering, encoding, and file IO stay off the
    /// dispatch thread.
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
        let (attachments, steps) = {
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
            for (index, page) in board.pages.pages().iter().enumerate() {
                // Each page is rendered through the viewport it was captured
                // with, not the overlay's current one: a guide recorded before
                // the surface moved output or changed resolution would
                // otherwise be clipped, padded, or rescaled page by page.
                let (logical_width, logical_height, scale, physical_size) =
                    match step_page_viewport(page) {
                        Some((logical_width, logical_height, physical_width, physical_height)) => (
                            logical_width,
                            logical_height,
                            1,
                            Some((physical_width, physical_height)),
                        ),
                        None => (
                            self.surface.width(),
                            self.surface.height(),
                            self.surface.scale(),
                            None,
                        ),
                    };
                let snapshot = crate::canvas_export::CanvasExportSnapshot {
                    viewport: crate::canvas_export::CanvasExportViewport {
                        logical_width,
                        logical_height,
                        scale,
                        physical_size,
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
                attachments.push(DocumentAttachment::canvas_png(
                    guide_image_file_stem(index + 1),
                    snapshot,
                ));
                steps.push(GuideStep {
                    title: page.page_name.clone(),
                });
            }
            (attachments, steps)
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

/// Recovers the viewport a Steps page was captured with from its locked
/// backdrop: the image spans the captured logical size, and its pixel width
/// over that size is the scale the frame was taken at. Returns `None` for a
/// page without a capture backdrop, which the caller renders at the overlay's
/// current viewport instead.
fn step_page_viewport(page: &crate::draw::Frame) -> Option<(u32, u32, u32, u32)> {
    page.shapes.iter().find_map(|drawn| match &drawn.shape {
        crate::draw::Shape::Image {
            x: 0,
            y: 0,
            w,
            h,
            data,
        } if drawn.locked => {
            let logical_width = u32::try_from(*w).ok().filter(|width| *width > 0)?;
            let logical_height = u32::try_from(*h).ok().filter(|height| *height > 0)?;
            if data.width == 0 || data.height == 0 {
                return None;
            }
            Some((logical_width, logical_height, data.width, data.height))
        }
        _ => None,
    })
}

#[cfg(test)]
mod tests {
    use super::step_page_viewport;
    use crate::draw::{EmbeddedImage, Frame, RED, Shape};

    fn backdrop_page(logical: (i32, i32), pixels: (u32, u32), locked: bool) -> Frame {
        let mut page = Frame::new();
        let id = page.add_shape(Shape::Image {
            x: 0,
            y: 0,
            w: logical.0,
            h: logical.1,
            data: EmbeddedImage {
                mime_type: "image/png".to_string(),
                width: pixels.0,
                height: pixels.1,
                bytes: vec![0, 1, 2],
            },
        });
        if let Some(shape) = page.shape_mut(id) {
            shape.locked = locked;
        }
        page
    }

    #[test]
    fn a_captured_page_reports_its_logical_and_pixel_size() {
        let page = backdrop_page((1280, 720), (2560, 1440), true);
        assert_eq!(step_page_viewport(&page), Some((1280, 720, 2560, 1440)));
    }

    #[test]
    fn an_unscaled_capture_reports_matching_logical_and_pixel_size() {
        let page = backdrop_page((3840, 2160), (3840, 2160), true);
        assert_eq!(step_page_viewport(&page), Some((3840, 2160, 3840, 2160)));
    }

    #[test]
    fn a_fractional_scale_capture_preserves_its_exact_pixel_size() {
        let page = backdrop_page((1707, 960), (2560, 1440), true);

        assert_eq!(step_page_viewport(&page), Some((1707, 960, 2560, 1440)));
    }

    #[test]
    fn a_page_without_a_capture_backdrop_has_no_recorded_viewport() {
        // An unlocked image is something the user pasted, not a captured step.
        assert_eq!(
            step_page_viewport(&backdrop_page((800, 600), (800, 600), false)),
            None
        );

        let mut page = Frame::new();
        page.add_shape(Shape::Freehand {
            points: vec![(1, 1), (2, 2)],
            color: RED,
            thick: 2.0,
        });
        assert_eq!(step_page_viewport(&page), None);
    }
}
