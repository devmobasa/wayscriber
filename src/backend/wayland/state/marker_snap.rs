//! Backend half of the marker's snap-to-text mode.
//!
//! Owns the one thing the input layer cannot: turning the displayed screen
//! image into rows of text. A scan is submitted once per capture, its result is
//! mapped from image pixels into logical screen coordinates, and the rows are
//! handed to `InputState`, which does all the snapping itself.
//!
//! The rows are tied to the screen source they were read from. When that source
//! changes — a re-freeze, a zoom, an output swap — the rows describe pixels that
//! are no longer on screen, so they are dropped and a new scan is asked for
//! rather than being stretched onto the new image.

use crate::input::state::{MarkerSnapBlocker, MarkerSnapState, Toast, ToastPriority};
use crate::input::text_snap::{TextSnapLine, TextSnapMap};
use crate::ocr::{
    OcrFailure, OcrLanguages, TextLayoutPoll, TextLayoutRequest, TextLayoutSubmitError,
};
use crate::screen_pixels::ImagePixelRect;

use super::WaylandState;
use super::screen_image::{
    ScreenSourceToken, copy_image_rect, current_screen_source_token, displayed_screen_image,
    screen_rect_for_image_rect,
};

const TOAST_SOURCE: &str = "marker.snap";

impl WaylandState {
    /// Start a scan when one was asked for, and drop rows that have gone stale.
    ///
    /// Called once per event-loop batch. Both halves run every batch because
    /// the screen source can change without anyone requesting anything.
    pub(in crate::backend::wayland) fn handle_marker_snap_scan(&mut self) {
        let requested = self.input_state.take_pending_marker_snap_scan();
        if !self.input_state.marker_snap_to_text() {
            self.data.marker_snap_scan_token = None;
            self.data.marker_snap_freeze_requested = false;
            return;
        }

        self.expire_stale_marker_snap_rows();

        if !requested && !self.marker_snap_needs_scan() {
            return;
        }
        self.submit_marker_snap_scan();
    }

    /// Whether the mode wants rows it does not have.
    fn marker_snap_needs_scan(&self) -> bool {
        matches!(
            self.input_state.marker_snap_state(),
            MarkerSnapState::AwaitingScreen
        ) && !self.text_layout.is_active()
    }

    /// Drop rows whose screen image is gone, and ask for a replacement.
    fn expire_stale_marker_snap_rows(&mut self) {
        let Some(scanned) = self.data.marker_snap_scan_token else {
            return;
        };
        if self.current_marker_snap_token() == Some(scanned) {
            return;
        }
        self.data.marker_snap_scan_token = None;
        self.input_state.invalidate_marker_text_snap();
        self.input_state.request_marker_snap_scan();
    }

    /// The identity of the screen image a scan would read right now.
    fn current_marker_snap_token(&self) -> Option<ScreenSourceToken> {
        let source = displayed_screen_image(
            &self.zoom,
            &self.frozen,
            self.input_state.board_is_transparent(),
        )?;
        current_screen_source_token(
            &source,
            &self.zoom,
            &self.frozen,
            (self.surface.width(), self.surface.height()),
        )
    }

    fn submit_marker_snap_scan(&mut self) {
        // Nothing is scanned, blocked, or reported until the marker is the tool
        // in hand. Snapping enabled in config must not freeze the screen at
        // startup, and a blocker the user cannot act on yet is just noise.
        if self.input_state.active_tool() != crate::input::Tool::Marker {
            return;
        }
        if self.text_layout.is_active() {
            // The active scan will land and be checked against the current
            // source; if it no longer matches, the drift check asks again.
            return;
        }
        if !self.config.capture.enabled {
            self.block_marker_snap(MarkerSnapBlocker::CaptureDisabled);
            return;
        }
        // Text under an opaque board is text the user cannot see, so snapping
        // to it would move the highlight somewhere with nothing to highlight.
        if !self.input_state.board_is_transparent() {
            self.block_marker_snap(MarkerSnapBlocker::OpaqueBoard);
            return;
        }

        let Some(source) = displayed_screen_image(
            &self.zoom,
            &self.frozen,
            self.input_state.board_is_transparent(),
        ) else {
            // Nothing to read yet. Freezing is what produces one, and the blur
            // tool already takes the same liberty when it needs a backdrop.
            self.input_state
                .set_marker_snap_state(MarkerSnapState::AwaitingScreen);
            self.request_marker_snap_freeze();
            return;
        };
        let Some(token) = current_screen_source_token(
            &source,
            &self.zoom,
            &self.frozen,
            (self.surface.width(), self.surface.height()),
        ) else {
            self.input_state
                .set_marker_snap_state(MarkerSnapState::AwaitingScreen);
            return;
        };
        let Some(rect) = ImagePixelRect::whole((source.image.width, source.image.height)) else {
            self.input_state
                .set_marker_snap_state(MarkerSnapState::AwaitingScreen);
            return;
        };
        let pixels = match copy_image_rect(source.image, rect) {
            Ok(pixels) => pixels,
            Err(error) => {
                log::warn!("Marker snap could not read the screen image: {error:?}");
                self.block_marker_snap(MarkerSnapBlocker::ScanFailed);
                return;
            }
        };

        let languages = OcrLanguages::from_validated(self.config.capture.resolved_ocr_languages());
        log::debug!(
            "Marker snap scanning {}x{} screen image for languages {}",
            pixels.width(),
            pixels.height(),
            languages.as_str()
        );
        match self
            .text_layout
            .try_submit(TextLayoutRequest { pixels, languages })
        {
            Ok(id) => {
                log::debug!("Marker snap layout scan {id} started");
                self.data.marker_snap_scan_token = Some(token);
                self.input_state
                    .set_marker_snap_state(MarkerSnapState::Scanning);
            }
            Err(TextLayoutSubmitError::Busy) => {}
            Err(error) => {
                log::warn!("Failed to start a marker snap layout scan: {error}");
                self.block_marker_snap(MarkerSnapBlocker::ScanFailed);
            }
        }
    }

    /// Ask for the freeze that produces a screen image, at most once per enable.
    ///
    /// The latch matters more than it looks: without it, unfreezing while
    /// holding the marker would re-freeze on the very next batch, and a failed
    /// freeze would ask again forever.
    fn request_marker_snap_freeze(&mut self) {
        if self.data.marker_snap_freeze_requested
            || self.input_state.frozen_active()
            || self.input_state.pending_frozen_toggle()
        {
            return;
        }
        if !self.frozen_enabled() {
            self.block_marker_snap(MarkerSnapBlocker::CaptureDisabled);
            return;
        }
        self.data.marker_snap_freeze_requested = true;
        self.input_state.request_frozen_toggle();
        self.input_state.push_toast(
            ToastPriority::Info,
            TOAST_SOURCE,
            Toast::info("Reading screen text for the marker..."),
        );
    }

    /// Record a blocker and say so once, rather than on every batch.
    fn block_marker_snap(&mut self, blocker: MarkerSnapBlocker) {
        let state = MarkerSnapState::Unavailable(blocker);
        if self.input_state.marker_snap_state() == state {
            return;
        }
        self.input_state.set_marker_snap_state(state);
        if let Some(message) = marker_snap_blocker_message(blocker) {
            self.input_state
                .push_toast(ToastPriority::Info, TOAST_SOURCE, Toast::warning(message));
        }
    }

    /// Turn a finished scan into rows, or into a stable reason there are none.
    pub(in crate::backend::wayland) fn poll_marker_snap_completion(&mut self) {
        match self.text_layout.poll() {
            TextLayoutPoll::Idle | TextLayoutPoll::Pending => {}
            TextLayoutPoll::Ready {
                id,
                outcome: Ok(lines),
            } => {
                let Some(token) = self.data.marker_snap_scan_token else {
                    log::debug!("Marker snap scan {id} finished after its request was dropped");
                    return;
                };
                // The scan describes one specific capture. If that capture is
                // gone, its rows are wrong everywhere, so they are discarded
                // rather than mapped onto whatever is on screen now.
                if self.current_marker_snap_token() != Some(token) {
                    log::debug!("Marker snap scan {id} finished for a screen image that is gone");
                    self.data.marker_snap_scan_token = None;
                    self.input_state.invalidate_marker_text_snap();
                    self.input_state.request_marker_snap_scan();
                    return;
                }
                let rows = lines.len();
                let map = TextSnapMap::new(lines.into_iter().filter_map(|line| {
                    let rect = ImagePixelRect::new(
                        u32::try_from(line.left.max(0)).ok()?,
                        u32::try_from(line.top.max(0)).ok()?,
                        u32::try_from(line.width.max(0)).ok()?,
                        u32::try_from(line.height.max(0)).ok()?,
                        token.image_size,
                    )?;
                    let screen = screen_rect_for_image_rect(&token, rect);
                    Some(TextSnapLine {
                        left: f64::from(screen.x),
                        top: f64::from(screen.y),
                        right: f64::from(screen.x.saturating_add(screen.width)),
                        bottom: f64::from(screen.y.saturating_add(screen.height)),
                    })
                }));
                log::debug!(
                    "Marker snap scan {id} produced {rows} rows, {} usable",
                    map.len()
                );
                self.input_state.install_marker_text_snap(map);
            }
            TextLayoutPoll::Ready {
                id,
                outcome: Err(failure),
            } => {
                log::warn!("Marker snap scan {id} failed: {failure:?}");
                self.data.marker_snap_scan_token = None;
                self.block_marker_snap(blocker_for_failure(&failure));
            }
            TextLayoutPoll::WorkerLost { id, reason } => {
                log::warn!("Marker snap scan {id} worker lost: {reason}");
                self.data.marker_snap_scan_token = None;
                self.block_marker_snap(MarkerSnapBlocker::ScanFailed);
            }
        }
    }
}

/// Which blocker an engine failure reduces to.
///
/// Only a missing engine is worth its own wording: it is the one the user can
/// act on, and it is permanent until they install something. Everything else is
/// a scan that did not work this time.
fn blocker_for_failure(failure: &OcrFailure) -> MarkerSnapBlocker {
    match failure {
        OcrFailure::EngineMissing => MarkerSnapBlocker::EngineMissing,
        _ => MarkerSnapBlocker::ScanFailed,
    }
}

/// The one-off toast for a blocker, or `None` to stay silent.
///
/// Snapping degrades to freehand rather than failing, so most blockers are a
/// status-line matter. Only the two the user can do something about interrupt.
fn marker_snap_blocker_message(blocker: MarkerSnapBlocker) -> Option<&'static str> {
    match blocker {
        MarkerSnapBlocker::EngineMissing => {
            Some("Install Tesseract to snap the marker to text. Drawing freehand.")
        }
        MarkerSnapBlocker::CaptureDisabled => {
            Some("Marker snap needs screen capture enabled. Drawing freehand.")
        }
        MarkerSnapBlocker::OpaqueBoard | MarkerSnapBlocker::ScanFailed => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_missing_engine_gets_its_own_blocker() {
        assert_eq!(
            blocker_for_failure(&OcrFailure::EngineMissing),
            MarkerSnapBlocker::EngineMissing
        );
        for failure in [
            OcrFailure::EngineFailed,
            OcrFailure::TimedOut,
            OcrFailure::OutputTooLarge,
            OcrFailure::EncodeFailed,
            OcrFailure::EngineUnavailable,
            OcrFailure::LanguageMissing {
                languages: "eng".to_string(),
            },
        ] {
            assert_eq!(
                blocker_for_failure(&failure),
                MarkerSnapBlocker::ScanFailed,
                "{failure:?} is a scan that did not work, not a distinct condition"
            );
        }
    }

    #[test]
    fn only_actionable_blockers_interrupt_with_a_toast() {
        assert!(marker_snap_blocker_message(MarkerSnapBlocker::EngineMissing).is_some());
        assert!(marker_snap_blocker_message(MarkerSnapBlocker::CaptureDisabled).is_some());
        assert!(
            marker_snap_blocker_message(MarkerSnapBlocker::OpaqueBoard).is_none(),
            "switching to a solid board is not an error the user needs told about"
        );
        assert!(marker_snap_blocker_message(MarkerSnapBlocker::ScanFailed).is_none());
    }

    #[test]
    fn every_blocker_still_names_itself_in_the_status_line() {
        for blocker in [
            MarkerSnapBlocker::EngineMissing,
            MarkerSnapBlocker::CaptureDisabled,
            MarkerSnapBlocker::OpaqueBoard,
            MarkerSnapBlocker::ScanFailed,
        ] {
            assert!(
                MarkerSnapState::Unavailable(blocker)
                    .status_text()
                    .is_some(),
                "{blocker:?} is silent in a toast, so the status line must carry it"
            );
        }
    }
}
