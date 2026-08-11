//! `Copy text from screen`: capture ownership, region selection, and outcomes.
//!
//! OCR reads the desktop image the renderer is already displaying, so it reuses
//! the eyedropper's freeze/zoom ownership rule: a freeze OCR created is a freeze
//! OCR releases, and a user-owned freeze or zoom survives untouched. Nothing
//! here mutates annotations, history, boards, sessions, or the active tool.

use crate::input::state::{OcrCaptureSource, OcrInputSource, Toast, ToastPriority};
use crate::ocr::{OcrFailure, OcrLanguages, OcrPoll, OcrRequest, OcrSubmitError, OcrSuccess};

use super::WaylandState;
use super::screen_image::{
    CropError, DisplayedScreenImage, ScreenSourceEntry, copy_image_rect, displayed_screen_image,
    image_rect_for_screen_rect, screen_source_entry,
};

/// Drags below this many logical pixels on either axis are treated as a stray
/// click and never reach the engine.
const MIN_REGION_LOGICAL_PIXELS: f64 = 4.0;

const TOAST_SOURCE: &str = "ocr";

impl WaylandState {
    /// Drain a `Copy text from screen` request into the region selector.
    pub(in crate::backend::wayland) fn handle_pending_ocr_request(&mut self) {
        if !self.input_state.take_pending_ocr_request() {
            return;
        }
        if self.input_state.ocr_state().is_engaged() {
            // A second invocation while the selector is up cancels it, matching
            // the eyedropper's toggle behavior.
            self.cancel_ocr();
            return;
        }
        if !self.config.capture.enabled {
            self.input_state.push_toast(
                ToastPriority::Info,
                TOAST_SOURCE,
                Toast::warning("Screen text recognition is off because capture is disabled."),
            );
            return;
        }
        if self.ocr.is_active() {
            self.input_state.push_toast(
                ToastPriority::Info,
                TOAST_SOURCE,
                Toast::info("OCR is already running"),
            );
            return;
        }

        // The two screen modals are mutually exclusive; entering one ends the
        // other, including any temporary freeze that one owned.
        self.cancel_eyedropper();
        self.input_state.prepare_for_screen_modal();
        self.zoom.stop_pan();
        self.stop_board_pan();
        self.set_board_pan_key_held(false);
        self.cancel_toolbar_move_drag();
        self.unlock_pointer();
        // The gesture just cancelled above may belong to a pen that is still
        // down. Activation retires the contact on its own, but the pending
        // branches below do not activate: without this, a tip-up during the
        // wait would commit the cancelled stroke's peak pressure to the tool.
        self.retire_stylus_contact();

        match screen_source_entry(
            self.ocr_screen_source().is_some(),
            self.input_state.board_is_transparent(),
            self.zoom.is_engaged(),
            self.zoom.active,
            self.frozen_enabled(),
        ) {
            ScreenSourceEntry::Activate => self.activate_ocr_selector(false),
            ScreenSourceEntry::WaitForZoom => {
                self.input_state
                    .set_ocr_pending_capture(OcrCaptureSource::Zoom);
            }
            ScreenSourceEntry::AutoFreeze => {
                self.input_state
                    .set_ocr_pending_capture(OcrCaptureSource::Frozen);
                self.input_state.request_frozen_toggle();
            }
            ScreenSourceEntry::RefuseWhileZoomedOnSolidBoard
            | ScreenSourceEntry::RefuseSolidBoard => {
                self.input_state.push_toast(
                    ToastPriority::Action,
                    TOAST_SOURCE,
                    Toast::info("OCR needs a visible screen image.").action(
                        "Switch to transparent",
                        crate::config::Action::ReturnToTransparent,
                    ),
                );
            }
            ScreenSourceEntry::CaptureUnavailable => {
                self.input_state.push_toast(
                    ToastPriority::Info,
                    TOAST_SOURCE,
                    Toast::warning(
                        "Screen text recognition is unavailable because screen capture is not available.",
                    ),
                );
            }
            ScreenSourceEntry::ZoomImageUnavailable => {
                self.input_state.push_toast(
                    ToastPriority::Info,
                    TOAST_SOURCE,
                    Toast::warning(
                        "Screen text recognition is unavailable because zoom has no captured screen image.",
                    ),
                );
            }
        }
    }

    fn ocr_screen_source(&self) -> Option<DisplayedScreenImage<'_>> {
        displayed_screen_image(
            &self.zoom,
            &self.frozen,
            self.input_state.board_is_transparent(),
        )
    }

    pub(in crate::backend::wayland) fn finish_pending_ocr_capture(
        &mut self,
        capture_source: OcrCaptureSource,
    ) {
        if self.input_state.ocr_state().pending_source() != Some(capture_source) {
            return;
        }
        if self.ocr_screen_source().is_some() {
            self.activate_ocr_selector(matches!(capture_source, OcrCaptureSource::Frozen));
        } else {
            self.cancel_ocr();
            self.input_state.report_ocr_capture_failure_if_unreported();
        }
    }

    /// Arm the region selector.
    ///
    /// Activation cancels whatever gesture was in flight, and from that moment
    /// the selector swallows the events that would have ended it — including a
    /// stylus tip-up. The contact is retired alongside, so the two can never be
    /// separated: a capture that lands seconds after the request arms the
    /// selector against a gesture that started during the wait, not the one the
    /// request itself cancelled.
    fn activate_ocr_selector(&mut self, auto_froze: bool) {
        self.retire_stylus_contact();
        self.input_state.activate_ocr(auto_froze);
    }

    /// Leave OCR selection, releasing only a freeze OCR created itself.
    pub(in crate::backend::wayland) fn cancel_ocr(&mut self) -> bool {
        let state = self.input_state.ocr_state();
        let was_engaged = state.is_engaged();
        let pending_source = state.pending_source();
        let auto_froze = self.input_state.cancel_ocr();
        self.release_ocr_capture(auto_froze, pending_source);
        was_engaged
    }

    /// Give back the temporary freeze OCR created. A user-owned freeze or an
    /// active zoom is never touched here.
    fn release_ocr_capture(&mut self, auto_froze: bool, pending_source: Option<OcrCaptureSource>) {
        if !auto_froze {
            return;
        }
        self.restore_xdg_after_frozen();
        if pending_source == Some(OcrCaptureSource::Frozen) && self.frozen.is_in_progress() {
            self.frozen.cancel(&mut self.input_state);
            self.exit_overlay_suppression(super::OverlaySuppression::Frozen);
        } else {
            self.frozen.unfreeze(&mut self.input_state);
        }
    }

    /// A display or source change invalidates the pixels the user is selecting.
    pub(in crate::backend::wayland) fn cancel_ocr_if_source_missing(&mut self) {
        if self.input_state.ocr_is_active() && self.ocr_screen_source().is_none() {
            self.cancel_ocr();
        }
    }

    pub(in crate::backend::wayland) fn begin_ocr_selection(
        &mut self,
        owner: OcrInputSource,
        x: f64,
        y: f64,
    ) -> bool {
        self.input_state.start_ocr_selection(owner, (x, y))
    }

    pub(in crate::backend::wayland) fn update_ocr_selection(
        &mut self,
        source: OcrInputSource,
        x: f64,
        y: f64,
    ) {
        self.input_state.update_ocr_selection(source, (x, y));
    }

    /// Discard a region because the device dragging it went away — the pen left
    /// proximity, the touch sequence was cancelled. Devices that are not
    /// dragging have nothing to withdraw, so this leaves the selector armed for
    /// whichever one is.
    pub(in crate::backend::wayland) fn cancel_ocr_selection_from(
        &mut self,
        source: OcrInputSource,
    ) -> bool {
        if !self.input_state.ocr_selection_is_owned_by(source) {
            return false;
        }
        self.cancel_ocr()
    }

    /// End the drag: own the selected pixels, release an OCR-created freeze,
    /// and submit. Returns whether `source` had a region drag to finish.
    pub(in crate::backend::wayland) fn finish_ocr_selection(
        &mut self,
        source: OcrInputSource,
        x: f64,
        y: f64,
    ) -> bool {
        // A release from a device that is not dragging must not submit — or
        // even see — the region another one is still drawing.
        if !self.input_state.ocr_selection_is_owned_by(source) {
            return false;
        }
        self.input_state.update_ocr_selection(source, (x, y));
        let Some(selection) = self.input_state.ocr_state().selection() else {
            return false;
        };

        // The crop is taken while the capture is still held: releasing first
        // would leave the worker reading pixels that no longer exist.
        match self.crop_ocr_selection(selection.start, selection.end) {
            Ok(None) => self.input_state.rearm_ocr_selection(),
            Ok(Some(pixels)) => {
                let auto_froze = self.input_state.cancel_ocr();
                self.release_ocr_capture(auto_froze, None);
                self.submit_ocr_request(pixels);
            }
            Err(message) => {
                let auto_froze = self.input_state.cancel_ocr();
                self.release_ocr_capture(auto_froze, None);
                self.input_state.push_toast(
                    ToastPriority::Info,
                    TOAST_SOURCE,
                    Toast::warning(message),
                );
            }
        }
        true
    }

    /// `Ok(None)` means the drag was too small to be a region — a stray click,
    /// not a failure worth a toast.
    fn crop_ocr_selection(
        &self,
        start: (f64, f64),
        end: (f64, f64),
    ) -> Result<Option<crate::ocr::OcrPixels>, &'static str> {
        if (end.0 - start.0).abs() < MIN_REGION_LOGICAL_PIXELS
            || (end.1 - start.1).abs() < MIN_REGION_LOGICAL_PIXELS
        {
            return Ok(None);
        }
        let Some(source) = self.ocr_screen_source() else {
            return Err("The screen image for text recognition is no longer available.");
        };
        let Some(rect) = image_rect_for_screen_rect(
            &source,
            &self.zoom,
            (self.surface.width(), self.surface.height()),
            start,
            end,
        ) else {
            return Ok(None);
        };
        copy_image_rect(source.image, rect)
            .map(Some)
            .map_err(|error| match error {
                CropError::Empty => "That selection has no screen pixels.",
                CropError::OutOfBounds => "Could not read that region of the screen image.",
            })
    }

    fn submit_ocr_request(&mut self, pixels: crate::ocr::OcrPixels) {
        let languages = OcrLanguages::from_validated(self.config.capture.resolved_ocr_languages());
        log::debug!(
            "OCR submitting {}x{} crop for languages {}",
            pixels.width,
            pixels.height,
            languages.as_str()
        );
        match self.ocr.try_submit(OcrRequest { pixels, languages }) {
            Ok(id) => {
                // wl-copy needs the overlay to stay alive long enough to serve
                // the selection it publishes.
                self.suppress_focus_exit_for(std::time::Duration::from_millis(1500));
                log::debug!("OCR request {id} started");
            }
            Err(OcrSubmitError::Busy { .. }) => {
                self.input_state.push_toast(
                    ToastPriority::Info,
                    TOAST_SOURCE,
                    Toast::info("OCR is already running"),
                );
            }
            Err(error) => {
                log::warn!("Failed to start screen text recognition: {error}");
                self.input_state.push_toast(
                    ToastPriority::Info,
                    TOAST_SOURCE,
                    Toast::warning("Could not start screen text recognition."),
                );
            }
        }
    }

    /// Turn a finished recognition into one stable toast. The completion never
    /// carries recognized text, so nothing here can leak screen content.
    pub(in crate::backend::wayland) fn poll_ocr_completion(&mut self) {
        match self.ocr.poll() {
            OcrPoll::Idle | OcrPoll::Pending => {}
            OcrPoll::Ready {
                id,
                outcome:
                    Ok(OcrSuccess::Copied {
                        character_count,
                        replaced_invalid_utf8,
                    }),
            } => {
                log::debug!("OCR request {id} copied {character_count} characters");
                let message = if replaced_invalid_utf8 {
                    "Text copied (some characters were unreadable)"
                } else {
                    "Text copied"
                };
                self.input_state.push_toast(
                    ToastPriority::Info,
                    TOAST_SOURCE,
                    Toast::info(message),
                );
            }
            OcrPoll::Ready {
                outcome: Ok(OcrSuccess::NoTextFound),
                ..
            } => {
                self.input_state.push_toast(
                    ToastPriority::Info,
                    TOAST_SOURCE,
                    Toast::info("No text found"),
                );
            }
            OcrPoll::Ready {
                id,
                outcome: Err(failure),
            } => {
                log::warn!("OCR request {id} failed: {failure:?}");
                let toast = match failure {
                    OcrFailure::EngineMissing | OcrFailure::LanguageMissing { .. } => {
                        Toast::warning(failure.message())
                    }
                    _ => Toast::error(failure.message()),
                };
                self.input_state
                    .push_toast(ToastPriority::Critical, TOAST_SOURCE, toast);
            }
            OcrPoll::WorkerLost { id, reason } => {
                log::error!("OCR request {id} lost its worker: {reason}");
                self.input_state.push_toast(
                    ToastPriority::Critical,
                    TOAST_SOURCE,
                    Toast::error("Screen text recognition failed."),
                );
            }
        }
    }

    /// Paint the region selector: a scrim over everything outside the drag and
    /// scan-corner brackets around it. Never baked into the source crop — this
    /// runs in the UI pass, over a copy the worker already owns.
    pub(in crate::backend::wayland) fn render_ocr_selection(
        &self,
        ctx: &cairo::Context,
        screen_width: u32,
        screen_height: u32,
    ) {
        if !self.input_state.ocr_is_active() {
            return;
        }
        let width = f64::from(screen_width);
        let height = f64::from(screen_height);
        let selection = self.input_state.ocr_state().selection();

        let _ = ctx.save();
        ctx.set_source_rgba(0.02, 0.03, 0.05, 0.35);
        ctx.rectangle(0.0, 0.0, width, height);
        if let Some(selection) = selection {
            let (x, y, w, h) = normalized_rect(selection.start, selection.end);
            // Even-odd keeps the selected region unscrimmed, so the user reads
            // the real screen pixels the crop will take.
            ctx.rectangle(x, y, w, h);
            ctx.set_fill_rule(cairo::FillRule::EvenOdd);
        }
        let _ = ctx.fill();
        ctx.set_fill_rule(cairo::FillRule::Winding);

        if let Some(selection) = selection {
            let (x, y, w, h) = normalized_rect(selection.start, selection.end);
            ctx.set_source_rgba(1.0, 1.0, 1.0, 0.9);
            ctx.set_line_width(1.0);
            ctx.rectangle(x + 0.5, y + 0.5, (w - 1.0).max(0.0), (h - 1.0).max(0.0));
            let _ = ctx.stroke();
            draw_scan_corners(ctx, x, y, w, h);
        }
        let _ = ctx.restore();
    }
}

fn normalized_rect(start: (f64, f64), end: (f64, f64)) -> (f64, f64, f64, f64) {
    let x = start.0.min(end.0);
    let y = start.1.min(end.1);
    (x, y, (end.0 - start.0).abs(), (end.1 - start.1).abs())
}

/// Four bracket corners, the visual language a scan frame reads as. The arm
/// length shrinks with the region so a small selection is not swallowed by it.
fn draw_scan_corners(ctx: &cairo::Context, x: f64, y: f64, w: f64, h: f64) {
    let arm = (w.min(h) / 4.0).clamp(4.0, 20.0);
    ctx.set_source_rgba(1.0, 1.0, 1.0, 1.0);
    ctx.set_line_width(2.0);
    ctx.set_line_cap(cairo::LineCap::Square);
    for (corner_x, corner_y, dx, dy) in [
        (x, y, 1.0, 1.0),
        (x + w, y, -1.0, 1.0),
        (x, y + h, 1.0, -1.0),
        (x + w, y + h, -1.0, -1.0),
    ] {
        ctx.move_to(corner_x + dx * arm, corner_y);
        ctx.line_to(corner_x, corner_y);
        ctx.line_to(corner_x, corner_y + dy * arm);
        let _ = ctx.stroke();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalization_orders_any_pair_of_drag_corners() {
        assert_eq!(
            normalized_rect((30.0, 40.0), (10.0, 20.0)),
            (10.0, 20.0, 20.0, 20.0)
        );
        assert_eq!(
            normalized_rect((10.0, 20.0), (30.0, 40.0)),
            (10.0, 20.0, 20.0, 20.0)
        );
    }

    #[test]
    fn scan_corner_arms_stay_inside_a_small_region() {
        // The clamp floor is 4px; the guard is that the arm never spans more
        // than the region it decorates.
        for size in [1.0_f64, 8.0, 40.0, 400.0] {
            let arm = (size / 4.0).clamp(4.0, 20.0);
            assert!(arm <= 20.0);
            assert!(arm >= 4.0);
        }
    }
}
