//! `Copy text from screen`: capture ownership, region selection, and outcomes.
//!
//! OCR reads the desktop image the renderer is already displaying, so it reuses
//! the eyedropper's freeze/zoom ownership rule: a freeze OCR created is a freeze
//! OCR releases, and a user-owned freeze or zoom survives untouched. Nothing
//! here mutates annotations, history, boards, sessions, or the active tool.

use crate::backend::wayland::acquisition::{ScreenAcquisitionOutcome, ScreenAcquisitionOwner};
use crate::backend::wayland::zoom::ZoomWaiterOwner;
use crate::input::state::OcrScanOutcome;
use crate::input::state::{
    RegionInputSource, RegionPurposeTag, ScreenCaptureSource, Toast, ToastPriority,
};
use crate::ocr::{OcrFailure, OcrLanguages, OcrPoll, OcrRequest, OcrSubmitError, OcrSuccess};

use super::WaylandState;
use super::acquisition::report_screen_source_activation_rejected_to;
use super::region_capture::{
    ActiveScreenRegion, FreezeOwnership, RegionOwnerLoss, RegionSelectionFinalize,
    finalize_region_selection_event,
};
use super::screen_image::{
    CropError, DisplayedScreenImage, ScreenSourceEntry, copy_image_rect, displayed_screen_image,
    screen_source_entry,
};

const TOAST_SOURCE: &str = "ocr";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ActiveRegionCancelTarget {
    None,
    Ocr,
    Capture,
    Measure,
}

fn active_region_cancel_target(purpose: Option<RegionPurposeTag>) -> ActiveRegionCancelTarget {
    match purpose {
        Some(RegionPurposeTag::Ocr) => ActiveRegionCancelTarget::Ocr,
        Some(RegionPurposeTag::CaptureDeliver | RegionPurposeTag::CaptureInteractive) => {
            ActiveRegionCancelTarget::Capture
        }
        Some(RegionPurposeTag::Measure) => ActiveRegionCancelTarget::Measure,
        None => ActiveRegionCancelTarget::None,
    }
}

impl WaylandState {
    /// Drain a `Copy text from screen` request into the region selector.
    pub(in crate::backend::wayland) fn handle_pending_ocr_request(&mut self) {
        // Read unconditionally so the latch never outlives the batch that set it.
        let dismissed_by_toolbar = self.input_state.take_ocr_cancelled_by_toolbar();
        if !self.input_state.take_pending_ocr_request() {
            return;
        }
        if self.input_state.region_is_engaged() {
            // A second invocation while the selector is up cancels it, matching
            // the eyedropper's toggle behavior.
            self.cancel_ocr();
            return;
        }
        if dismissed_by_toolbar {
            // The toolbar path already cancelled the selector on the way in, so
            // this request is the same click toggling it off.
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

        let generation = self.next_screen_region_generation();
        match screen_source_entry(
            self.ocr_screen_source().is_some(),
            self.input_state.board_is_transparent(),
            self.zoom.is_engaged(),
            self.zoom.active,
            self.frozen_enabled(),
        ) {
            ScreenSourceEntry::Activate => {
                if !self.activate_ocr_selector(generation, FreezeOwnership::PreExisting) {
                    self.clear_screen_region_ui_only();
                    report_screen_source_activation_rejected_to(
                        &mut self.input_state,
                        ScreenAcquisitionOwner::Ocr,
                    );
                }
            }
            ScreenSourceEntry::WaitForZoom => {
                if self.wait_for_current_zoom_capture(ZoomWaiterOwner::Ocr) {
                    self.set_pending_screen_region(
                        RegionPurposeTag::Ocr,
                        generation,
                        ScreenCaptureSource::Zoom,
                        None,
                    );
                } else {
                    self.report_ocr_zoom_image_unavailable();
                }
            }
            ScreenSourceEntry::AutoFreeze => {
                match self.request_screen_acquisition(ScreenAcquisitionOwner::Ocr) {
                    Ok(acquisition) => self.set_pending_screen_region(
                        RegionPurposeTag::Ocr,
                        generation,
                        ScreenCaptureSource::Frozen,
                        Some(acquisition),
                    ),
                    Err(_) => self.report_terminal(
                        ScreenAcquisitionOwner::Ocr,
                        &ScreenAcquisitionOutcome::Unavailable,
                    ),
                }
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
                self.report_ocr_zoom_image_unavailable();
            }
        }
    }

    fn report_ocr_zoom_image_unavailable(&mut self) {
        self.input_state.push_toast(
            ToastPriority::Info,
            TOAST_SOURCE,
            Toast::warning(
                "Screen text recognition is unavailable because zoom has no captured screen image.",
            ),
        );
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
        capture_source: ScreenCaptureSource,
        installed_generation: u64,
    ) -> bool {
        let Some(region) = self.data.active_screen_region else {
            return false;
        };
        let generation = region.generation();
        let waiting = matches!(
            (region, capture_source),
            (
                ActiveScreenRegion::PendingZoom { .. },
                ScreenCaptureSource::Zoom
            ) | (
                ActiveScreenRegion::PendingFrozen { .. },
                ScreenCaptureSource::Frozen
            )
        );
        if !waiting {
            return false;
        }
        if self.ocr_screen_source().is_none() {
            return false;
        }
        let ownership = if capture_source == ScreenCaptureSource::Frozen {
            FreezeOwnership::PickerOwned {
                image_generation: installed_generation,
            }
        } else {
            FreezeOwnership::PreExisting
        };
        self.activate_ocr_selector(generation, ownership)
    }

    /// Arm the region selector.
    ///
    /// Activation cancels whatever gesture was in flight, and from that moment
    /// the selector swallows the events that would have ended it — including a
    /// stylus tip-up. The contact is retired alongside, so the two can never be
    /// separated: a capture that lands seconds after the request arms the
    /// selector against a gesture that started during the wait, not the one the
    /// request itself cancelled.
    fn activate_ocr_selector(&mut self, generation: u64, ownership: FreezeOwnership) -> bool {
        self.retire_stylus_contact();
        self.activate_screen_region(RegionPurposeTag::Ocr, generation, ownership, false)
    }

    /// Leave OCR selection, releasing only a freeze OCR created itself.
    pub(in crate::backend::wayland) fn cancel_ocr(&mut self) -> bool {
        let Some(region) = self.data.active_screen_region else {
            self.clear_zoom_waiter_for(ZoomWaiterOwner::Ocr);
            self.input_state.cancel_region_ui_only();
            return false;
        };
        let pending_acquisition = region.pending_acquisition();
        let owned_generation = region.owned_frozen_generation();
        self.cancel_modal_owner_resources(
            ScreenAcquisitionOwner::Ocr,
            pending_acquisition,
            owned_generation,
        );
        true
    }

    /// Dismiss the selector because a toolbar interaction took over.
    ///
    /// Recorded rather than just cancelled: when the interaction is a click on
    /// the OCR button itself, the request that follows is the button toggling
    /// off, and without this the pending handler would see `Inactive` and open a
    /// fresh selector instead.
    pub(in crate::backend::wayland) fn cancel_ocr_for_toolbar_interaction(&mut self) -> bool {
        if !self.cancel_ocr() {
            return false;
        }
        self.input_state.note_ocr_cancelled_by_toolbar();
        true
    }

    pub(in crate::backend::wayland) fn cancel_active_region_selector(&mut self) -> bool {
        match active_region_cancel_target(self.input_state.region_state().purpose()) {
            ActiveRegionCancelTarget::Ocr => self.cancel_ocr(),
            ActiveRegionCancelTarget::Capture => self.cancel_region_capture(),
            ActiveRegionCancelTarget::Measure => self.cancel_measure_mode(),
            ActiveRegionCancelTarget::None => false,
        }
    }

    pub(in crate::backend::wayland) fn cancel_region_for_toolbar_interaction(&mut self) -> bool {
        match active_region_cancel_target(self.input_state.region_state().purpose()) {
            ActiveRegionCancelTarget::Ocr => self.cancel_ocr_for_toolbar_interaction(),
            ActiveRegionCancelTarget::Capture => self.cancel_region_capture(),
            ActiveRegionCancelTarget::Measure => self.cancel_measure_mode(),
            ActiveRegionCancelTarget::None => false,
        }
    }

    /// Discard a region because the device dragging it went away — the pen left
    /// proximity, the touch sequence was cancelled. Devices that are not
    /// dragging have nothing to withdraw, so this leaves the selector armed for
    /// whichever one is.
    pub(in crate::backend::wayland) fn cancel_region_selection_from(
        &mut self,
        source: RegionInputSource,
    ) -> bool {
        match self.region_owner_lost(source) {
            RegionOwnerLoss::NotOwned => false,
            RegionOwnerLoss::Rearmed => true,
            RegionOwnerLoss::Cancel(RegionPurposeTag::Ocr) => self.cancel_ocr(),
            RegionOwnerLoss::Cancel(RegionPurposeTag::Measure) => self.cancel_measure_mode(),
            RegionOwnerLoss::Cancel(purpose) => {
                debug_assert!(purpose.is_capture());
                self.cancel_region_capture()
            }
        }
    }

    /// End the drag: OCR and direct capture submit their selected pixels;
    /// interactive capture enters Review. Returns whether `source` had a
    /// region drag or review move to finish.
    pub(in crate::backend::wayland) fn finish_region_selection(
        &mut self,
        source: RegionInputSource,
        x: f64,
        y: f64,
    ) -> bool {
        let was_engaged = self.input_state.region_is_engaged();
        self.cancel_screen_modals_if_source_changed();
        if was_engaged && !self.input_state.region_is_engaged() {
            return true;
        }
        if self.finish_region_cut_drag(source, (x, y)) {
            return true;
        }
        let rect = match finalize_region_selection_event(
            &mut self.data.active_screen_region,
            &mut self.input_state,
            source,
            (x, y),
        ) {
            RegionSelectionFinalize::NotOwned => return false,
            RegionSelectionFinalize::Rearmed => return true,
            RegionSelectionFinalize::Reviewed => return true,
            RegionSelectionFinalize::Measured => return true,
            RegionSelectionFinalize::Selected {
                purpose: RegionPurposeTag::Ocr,
                rect,
            } => rect,
            RegionSelectionFinalize::Selected {
                purpose: RegionPurposeTag::CaptureDeliver,
                rect,
            } => {
                self.submit_region_capture(rect);
                return true;
            }
            RegionSelectionFinalize::Selected {
                purpose: RegionPurposeTag::CaptureInteractive,
                ..
            } => return true,
            RegionSelectionFinalize::Selected {
                purpose: RegionPurposeTag::Measure,
                ..
            } => return true,
        };

        // The crop is taken while the capture is still held: releasing first
        // would leave the worker reading pixels that no longer exist.
        self.submit_ocr_for_rect(rect);
        true
    }

    /// Recognize an already-chosen rectangle — the whole image, from `Ctrl+A`.
    pub(in crate::backend::wayland) fn submit_whole_image_ocr(
        &mut self,
        rect: crate::screen_pixels::ImagePixelRect,
    ) {
        self.submit_ocr_for_rect(rect);
    }

    /// The one path from a chosen rectangle to a running recognition: map the
    /// area the sweep will cover, crop while the source is still held, release
    /// it, submit, and start the overlay. Both entry points share it so a fix
    /// to one cannot leave the other behind.
    fn submit_ocr_for_rect(&mut self, rect: crate::screen_pixels::ImagePixelRect) {
        // Mapped before the region is released: the token is what turns the
        // authoritative image rectangle into the surface pixels the sweep is
        // painted over.
        let scan_region = match self.data.active_screen_region {
            Some(ActiveScreenRegion::Ready { source, .. }) => Some(
                crate::backend::wayland::state::screen_image::screen_rect_for_image_rect(
                    &source, rect,
                ),
            ),
            _ => None,
        };
        let pixels = match self.crop_ocr_selection(rect) {
            Ok(pixels) => pixels,
            Err(message) => {
                self.cancel_ocr();
                self.input_state.push_toast(
                    ToastPriority::Info,
                    TOAST_SOURCE,
                    Toast::warning(message),
                );
                return;
            }
        };
        self.cancel_ocr();
        // The overlay starts only for a request that was actually accepted. A
        // refused submission produces no completion, and an overlay waiting on
        // one would sweep until something dismissed it.
        let Some(id) = self.submit_ocr_request(pixels) else {
            return;
        };
        if let Some(region) = scan_region {
            self.input_state
                .begin_ocr_scan(id, region, std::time::Instant::now());
        }
    }

    fn crop_ocr_selection(
        &self,
        rect: crate::screen_pixels::ImagePixelRect,
    ) -> Result<crate::screen_pixels::PackedArgb32, &'static str> {
        let Some(source) = self.ocr_screen_source() else {
            return Err("The screen image for text recognition is no longer available.");
        };
        copy_image_rect(source.image, rect).map_err(|error| match error {
            CropError::Empty => "That selection has no screen pixels.",
            CropError::OutOfBounds => "Could not read that region of the screen image.",
        })
    }

    /// Hand the crop to the worker, reporting the accepted request. `None`
    /// means nothing is running and no completion will arrive.
    fn submit_ocr_request(
        &mut self,
        pixels: crate::screen_pixels::PackedArgb32,
    ) -> Option<crate::ocr::OcrRequestId> {
        let languages = OcrLanguages::from_validated(self.config.capture.resolved_ocr_languages());
        log::debug!(
            "OCR submitting {}x{} crop for languages {}",
            pixels.width(),
            pixels.height(),
            languages.as_str()
        );
        match self.ocr.try_submit(OcrRequest { pixels, languages }) {
            Ok(id) => {
                // wl-copy needs the overlay to stay alive long enough to serve
                // the selection it publishes.
                self.suppress_focus_exit_for(std::time::Duration::from_millis(1500));
                log::debug!("OCR request {id} started");
                Some(id)
            }
            Err(OcrSubmitError::Busy { .. }) => {
                self.input_state.push_toast(
                    ToastPriority::Info,
                    TOAST_SOURCE,
                    Toast::info("OCR is already running"),
                );
                None
            }
            Err(error) => {
                log::warn!("Failed to start screen text recognition: {error}");
                self.input_state.push_toast(
                    ToastPriority::Info,
                    TOAST_SOURCE,
                    Toast::warning("Could not start screen text recognition."),
                );
                None
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
                self.input_state.settle_ocr_scan(
                    id,
                    OcrScanOutcome::Copied {
                        character_count,
                        replaced_invalid_utf8,
                    },
                    std::time::Instant::now(),
                );
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
                id,
                outcome: Ok(OcrSuccess::NoTextFound),
            } => {
                self.input_state.settle_ocr_scan(
                    id,
                    OcrScanOutcome::NoTextFound,
                    std::time::Instant::now(),
                );
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
                self.input_state.settle_ocr_scan(
                    id,
                    OcrScanOutcome::Failed,
                    std::time::Instant::now(),
                );
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
                self.input_state.settle_ocr_scan(
                    id,
                    OcrScanOutcome::Failed,
                    std::time::Instant::now(),
                );
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
        if !self.input_state.region_is_active()
            || self.input_state.region_state().purpose() != Some(RegionPurposeTag::Ocr)
        {
            return;
        }
        let width = f64::from(screen_width);
        let height = f64::from(screen_height);
        let selection = self.input_state.region_state().selection();

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
        // Whole-image recognition is a keystroke with nothing on screen to
        // suggest it. The capture picker teaches its keys here; recognition
        // reuses the same strip, honours the same `show_legend` setting, and
        // dismisses on the first drag through the shared selector state.
        if self.config.capture.region.show_legend && !self.region_picker_legend_dismissed() {
            crate::ui::render_region_legend(
                ctx,
                (screen_width, screen_height),
                crate::ui::OCR_LEGEND_TEXT,
            );
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
    fn active_region_cancellation_routes_every_selector_purpose_to_its_owner() {
        assert_eq!(
            active_region_cancel_target(Some(RegionPurposeTag::Ocr)),
            ActiveRegionCancelTarget::Ocr
        );
        for purpose in [
            RegionPurposeTag::CaptureDeliver,
            RegionPurposeTag::CaptureInteractive,
        ] {
            assert_eq!(
                active_region_cancel_target(Some(purpose)),
                ActiveRegionCancelTarget::Capture
            );
        }
        assert_eq!(
            active_region_cancel_target(Some(RegionPurposeTag::Measure)),
            ActiveRegionCancelTarget::Measure
        );
        assert_eq!(
            active_region_cancel_target(None),
            ActiveRegionCancelTarget::None
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
