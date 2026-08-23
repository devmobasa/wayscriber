//! The scan overlay that runs while screen text recognition works.
//!
//! Recognition happens on a worker and can take a second or more, with nothing
//! on screen to say the region was even read. A band sweeps the selected
//! rectangle while the worker runs, and a short card reports the outcome when
//! it finishes.
//!
//! The card carries the outcome, never the recognized text: `src/ocr` keeps
//! recognized text out of application state entirely, and drawing it here would
//! put screen contents into the overlay's own UI. See `src/ocr/AGENTS.md`.

use std::time::{Duration, Instant};

use super::super::base::InputState;
use crate::ocr::OcrRequestId;
use crate::util::Rect;

/// One top-to-bottom pass of the scan band.
pub(crate) const SWEEP: Duration = Duration::from_millis(1200);
/// How long the outcome card stays up once the sweep lets it through.
const RESULT_LIFETIME: Duration = Duration::from_millis(4500);
/// The card fades over the last of its lifetime.
pub(crate) const RESULT_FADE: Duration = Duration::from_millis(450);

/// What recognition produced, in the terms the card is allowed to show.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OcrScanOutcome {
    Copied {
        character_count: usize,
        replaced_invalid_utf8: bool,
    },
    NoTextFound,
    Failed,
}

impl OcrScanOutcome {
    pub(crate) fn headline(self) -> &'static str {
        match self {
            Self::Copied {
                replaced_invalid_utf8: false,
                ..
            } => "Copied to clipboard",
            Self::Copied {
                replaced_invalid_utf8: true,
                ..
            } => "Copied — some characters were unreadable",
            Self::NoTextFound => "No text found",
            Self::Failed => "Recognition failed",
        }
    }

    /// The detail line. Deliberately a count, not the text itself.
    pub(crate) fn detail(self) -> Option<String> {
        match self {
            Self::Copied {
                character_count: 1, ..
            } => Some("1 character".to_string()),
            Self::Copied {
                character_count, ..
            } => Some(format!("{character_count} characters")),
            Self::NoTextFound | Self::Failed => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OcrScanPhase {
    /// The worker is running, or it has finished but the band has not reached
    /// the end of its current pass.
    Scanning {
        /// The outcome and the instant the band may hand over, measured from
        /// `started`. Fixed when recognition settles rather than recomputed per
        /// tick: a deadline derived from the current elapsed time moves along
        /// with it and is only ever reached on an exact sweep boundary, so the
        /// band sweeps forever.
        settled: Option<(OcrScanOutcome, Duration)>,
    },
    Showing {
        outcome: OcrScanOutcome,
    },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct OcrScan {
    /// The recognition this overlay is waiting on. A completion for anything
    /// else belongs to a request whose overlay is already gone.
    request: OcrRequestId,
    /// The scanned rectangle in logical surface pixels.
    region: Rect,
    started: Instant,
    phase: OcrScanPhase,
}

impl OcrScan {
    pub(crate) const fn region(self) -> Rect {
        self.region
    }

    /// Whether recognition is still being waited on, and so the region should
    /// be marked as being read.
    pub(crate) const fn is_scanning(self) -> bool {
        matches!(self.phase, OcrScanPhase::Scanning { .. })
    }

    /// Progress of the band through its current pass, in `0.0..1.0`. `None`
    /// under `[ui] reduced_motion`: the region is marked with a static
    /// indicator instead of a moving band (WCAG 2.3.3).
    pub(crate) fn sweep_progress(self, now: Instant) -> Option<f64> {
        self.sweep_progress_for(now, crate::ui::anim::motion_enabled())
    }

    /// Split from the accessor so both motion settings can be exercised
    /// without writing the process-wide flag, which parallel tests share.
    pub(crate) fn sweep_progress_for(self, now: Instant, motion: bool) -> Option<f64> {
        if !motion {
            return None;
        }
        self.is_scanning().then(|| {
            let elapsed = now.saturating_duration_since(self.started).as_secs_f64();
            let period = SWEEP.as_secs_f64();
            (elapsed % period) / period
        })
    }

    /// The settled outcome and how long it has been up, once the band has
    /// finished its pass.
    pub(crate) fn result(self, now: Instant) -> Option<(OcrScanOutcome, Duration)> {
        match self.phase {
            OcrScanPhase::Showing { outcome } => {
                Some((outcome, now.saturating_duration_since(self.started)))
            }
            OcrScanPhase::Scanning { .. } => None,
        }
    }
}

impl InputState {
    /// Start the sweep over `region` (logical surface pixels).
    pub(crate) fn begin_ocr_scan(&mut self, request: OcrRequestId, region: Rect, now: Instant) {
        self.ocr_scan = Some(OcrScan {
            request,
            region,
            started: now,
            phase: OcrScanPhase::Scanning { settled: None },
        });
        self.needs_redraw = true;
    }

    /// Record what recognition produced. The card does not appear yet: the band
    /// finishes the pass it is on first, and always at least one full pass, so
    /// a fast recognition cannot cut the sweep off mid-screen.
    pub(crate) fn settle_ocr_scan(
        &mut self,
        request: OcrRequestId,
        outcome: OcrScanOutcome,
        now: Instant,
    ) {
        self.settle_ocr_scan_for(request, outcome, now, crate::ui::anim::motion_enabled());
    }

    pub(crate) fn settle_ocr_scan_for(
        &mut self,
        request: OcrRequestId,
        outcome: OcrScanOutcome,
        now: Instant,
        motion: bool,
    ) {
        let Some(scan) = self.ocr_scan.as_mut() else {
            return;
        };
        if scan.request != request {
            return;
        }
        let elapsed = now.saturating_duration_since(scan.started);
        if let OcrScanPhase::Scanning { settled } = &mut scan.phase
            && settled.is_none()
        {
            *settled = Some((outcome, completed_sweep(elapsed, motion)));
            self.needs_redraw = true;
        }
    }

    /// Take away a finished outcome card. A sweep still waiting on the worker
    /// is left alone: it is progress feedback for work that is still running,
    /// and dropping it on a stray keystroke would also discard the result that
    /// recognition is about to report.
    pub(crate) fn dismiss_ocr_scan_result(&mut self) -> bool {
        let Some(scan) = self.ocr_scan else {
            return false;
        };
        if scan.is_scanning() {
            return false;
        }
        self.ocr_scan = None;
        self.needs_redraw = true;
        true
    }

    pub(crate) const fn ocr_scan(&self) -> Option<OcrScan> {
        self.ocr_scan
    }

    /// When the overlay next changes on its own, for a loop that is not
    /// already ticking for animation.
    ///
    /// `None` while something is moving — the animation tick covers that — and
    /// `None` during a still scan, whose next change is the worker completing,
    /// which wakes the loop itself. A still card is the one case that needs a
    /// deadline: nothing else will wake the loop to expire it.
    pub(crate) fn ocr_scan_wake_after(&self, now: Instant) -> Option<Duration> {
        self.ocr_scan_wake_after_for(now, crate::ui::anim::motion_enabled())
    }

    pub(crate) fn ocr_scan_wake_after_for(&self, now: Instant, motion: bool) -> Option<Duration> {
        if motion {
            return None;
        }
        let scan = self.ocr_scan?;
        let (_, shown) = scan.result(now)?;
        Some(RESULT_LIFETIME.saturating_sub(shown))
    }

    /// Whether a still card has outlived its deadline and needs the frame that
    /// takes it away.
    pub(crate) fn ocr_scan_due(&self, now: Instant) -> bool {
        self.ocr_scan_wake_after(now) == Some(Duration::ZERO)
    }

    /// Advance the overlay, reporting whether it needs *continuous* frames.
    ///
    /// Only movement answers yes. Under `[ui] reduced_motion` the overlay is
    /// static, so it is rendered and damaged like any other chrome but does not
    /// pin a repaint at the animation frame rate for the length of a
    /// recognition; `ocr_scan_wake_after` supplies the one deadline it needs.
    pub fn advance_ocr_scan(&mut self, now: Instant) -> bool {
        self.advance_ocr_scan_for(now, crate::ui::anim::motion_enabled())
    }

    pub(crate) fn advance_ocr_scan_for(&mut self, now: Instant, motion: bool) -> bool {
        let Some(scan) = self.ocr_scan else {
            return false;
        };
        let elapsed = now.saturating_duration_since(scan.started);
        let animating = motion;
        match scan.phase {
            OcrScanPhase::Scanning { settled: None } => animating,
            OcrScanPhase::Scanning {
                settled: Some((outcome, deadline)),
            } => {
                if elapsed < deadline {
                    return true;
                }
                self.ocr_scan = Some(OcrScan {
                    request: scan.request,
                    region: scan.region,
                    started: now,
                    phase: OcrScanPhase::Showing { outcome },
                });
                self.needs_redraw = true;
                animating
            }
            OcrScanPhase::Showing { .. } => {
                if elapsed < RESULT_LIFETIME {
                    return animating;
                }
                self.ocr_scan = None;
                self.needs_redraw = true;
                false
            }
        }
    }
}

/// The end of the pass `elapsed` falls in, never earlier than one full pass.
///
/// Zero under `[ui] reduced_motion`: with no band sweeping there is nothing to
/// let finish, so the outcome is shown as soon as it arrives.
fn completed_sweep(elapsed: Duration, motion: bool) -> Duration {
    if !motion {
        return Duration::ZERO;
    }
    let sweep = SWEEP.as_millis().max(1);
    let elapsed_ms = elapsed.as_millis();
    let passes = elapsed_ms.div_ceil(sweep).max(1);
    Duration::from_millis(u64::try_from(passes * sweep).unwrap_or(u64::MAX))
}

/// Opacity of the outcome card `shown` into its lifetime.
pub(crate) fn result_opacity(shown: Duration) -> f64 {
    result_opacity_for(shown, crate::ui::anim::motion_enabled())
}

/// Split from the accessor so both motion settings can be exercised without
/// writing the process-wide flag, which every parallel test shares.
///
/// A card past its lifetime is gone either way; only the gradual fade into
/// that is animation, and reduced motion skips it by cutting straight from
/// fully opaque to nothing.
pub(crate) fn result_opacity_for(shown: Duration, motion: bool) -> f64 {
    let remaining = RESULT_LIFETIME.saturating_sub(shown);
    if remaining.is_zero() {
        return 0.0;
    }
    if !motion || remaining >= RESULT_FADE {
        return 1.0;
    }
    (remaining.as_secs_f64() / RESULT_FADE.as_secs_f64()).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::state::test_support::make_test_input_state;

    fn request() -> OcrRequestId {
        OcrRequestId::for_test(1)
    }

    fn region() -> Rect {
        Rect::new(100, 80, 240, 160).expect("a scan region")
    }

    #[test]
    fn the_band_sweeps_while_recognition_runs() {
        let mut state = make_test_input_state();
        let start = Instant::now();
        state.begin_ocr_scan(request(), region(), start);

        let scan = state.ocr_scan().expect("a scan is up");
        assert_eq!(scan.region(), region());
        assert_eq!(scan.sweep_progress(start), Some(0.0));
        assert_eq!(scan.result(start), None, "nothing to report yet");

        let half = start + SWEEP / 2;
        let progress = scan.sweep_progress(half).expect("still scanning");
        assert!((progress - 0.5).abs() < 1e-6, "half way down: {progress}");

        // The band restarts rather than stopping: the worker sets the pace.
        let next = start + SWEEP + SWEEP / 4;
        let progress = scan.sweep_progress(next).expect("still scanning");
        assert!((progress - 0.25).abs() < 1e-6, "second pass: {progress}");

        assert!(state.advance_ocr_scan(half), "keeps asking for frames");
    }

    #[test]
    fn a_fast_recognition_still_gets_a_whole_sweep_before_the_card() {
        let mut state = make_test_input_state();
        let start = Instant::now();
        state.begin_ocr_scan(request(), region(), start);
        state.settle_ocr_scan(
            request(),
            OcrScanOutcome::Copied {
                character_count: 12,
                replaced_invalid_utf8: false,
            },
            start,
        );

        // Recognition finished almost immediately; the card must wait.
        let early = start + Duration::from_millis(80);
        assert!(state.advance_ocr_scan(early));
        let scan = state.ocr_scan().expect("still scanning");
        assert_eq!(scan.result(early), None, "a card here would cut the sweep");
        assert!(scan.sweep_progress(early).is_some());

        // One full pass later it is allowed through.
        let settled = start + SWEEP;
        assert!(state.advance_ocr_scan(settled));
        let scan = state.ocr_scan().expect("the card is up");
        assert_eq!(scan.sweep_progress(settled), None, "the band is done");
        let (outcome, shown) = scan.result(settled).expect("an outcome");
        assert_eq!(
            outcome,
            OcrScanOutcome::Copied {
                character_count: 12,
                replaced_invalid_utf8: false,
            }
        );
        assert_eq!(shown, Duration::ZERO);
    }

    #[test]
    fn a_slow_recognition_finishes_the_pass_it_lands_in() {
        let mut state = make_test_input_state();
        let start = Instant::now();
        state.begin_ocr_scan(request(), region(), start);

        // Settles a third of the way through the third pass.
        let landed = start + SWEEP * 2 + SWEEP / 3;
        state.settle_ocr_scan(request(), OcrScanOutcome::NoTextFound, landed);
        assert!(state.advance_ocr_scan(landed));
        assert_eq!(
            state.ocr_scan().and_then(|scan| scan.result(landed)),
            None,
            "the third pass is still running"
        );

        let completed = start + SWEEP * 3;
        assert!(state.advance_ocr_scan(completed));
        assert!(
            state
                .ocr_scan()
                .and_then(|scan| scan.result(completed))
                .is_some(),
            "the card appears at the end of that pass, not the next one"
        );
    }

    #[test]
    fn the_band_hands_over_on_a_real_tick_not_only_on_an_exact_boundary() {
        // The event loop ticks on a frame clock, so `elapsed` lands wherever it
        // lands. A hand-over deadline recomputed from the current elapsed time
        // moves along with it and is only ever met on an exact multiple of the
        // sweep, which a real tick almost never hits — the band would sweep on
        // forever. Drive it the way the loop does and require it to settle.
        let mut state = make_test_input_state();
        let start = Instant::now();
        state.begin_ocr_scan(request(), region(), start);
        state.settle_ocr_scan(
            request(),
            OcrScanOutcome::Copied {
                character_count: 7,
                replaced_invalid_utf8: false,
            },
            start + Duration::from_millis(137),
        );

        let tick = Duration::from_millis(17);
        let mut now = start;
        let mut handed_over = None;
        for _ in 0..500 {
            now += tick;
            state.advance_ocr_scan(now);
            if let Some(scan) = state.ocr_scan()
                && scan.result(now).is_some()
            {
                handed_over = Some(now.saturating_duration_since(start));
                break;
            }
        }

        let handed_over = handed_over.expect("the band must stop sweeping and show the card");
        assert!(
            handed_over >= SWEEP,
            "at least one full pass: {handed_over:?}"
        );
        assert!(
            handed_over < SWEEP * 2,
            "and no more than the pass it settled in: {handed_over:?}"
        );
    }

    #[test]
    fn the_card_expires_on_its_own_and_stops_asking_for_frames() {
        let mut state = make_test_input_state();
        let start = Instant::now();
        state.begin_ocr_scan(request(), region(), start);
        state.settle_ocr_scan(request(), OcrScanOutcome::NoTextFound, start);
        let settled = start + SWEEP;
        assert!(state.advance_ocr_scan(settled));

        assert!(state.advance_ocr_scan(settled + RESULT_LIFETIME / 2));
        assert!(state.ocr_scan().is_some());

        assert!(!state.advance_ocr_scan(settled + RESULT_LIFETIME));
        assert!(state.ocr_scan().is_none(), "it clears itself");
        assert!(!state.advance_ocr_scan(settled + RESULT_LIFETIME));
    }

    #[test]
    fn interaction_takes_the_finished_card_but_leaves_a_running_sweep() {
        let mut state = make_test_input_state();
        let start = Instant::now();
        state.begin_ocr_scan(request(), region(), start);

        // Recognition is still running: the sweep is progress feedback for work
        // in flight, and dropping it would also discard the result about to
        // arrive. A stray keystroke must not do that.
        assert!(!state.dismiss_ocr_scan_result(), "the sweep is not a card");
        assert!(state.ocr_scan().is_some(), "and it stays up");

        state.settle_ocr_scan(request(), OcrScanOutcome::NoTextFound, start);
        assert!(
            !state.dismiss_ocr_scan_result(),
            "still sweeping until the pass completes"
        );

        let settled = start + SWEEP;
        assert!(state.advance_ocr_scan(settled));
        assert!(
            state.dismiss_ocr_scan_result(),
            "the card can be taken away"
        );
        assert!(state.ocr_scan().is_none());
        assert!(!state.dismiss_ocr_scan_result(), "nothing left to dismiss");
    }

    #[test]
    fn a_completion_for_another_request_leaves_this_sweep_alone() {
        // Capacity one makes overlapping requests unlikely, but a completion
        // that outlives its own overlay must not settle a newer one with a
        // stale outcome.
        let mut state = make_test_input_state();
        let start = Instant::now();
        state.begin_ocr_scan(request(), region(), start);
        state.settle_ocr_scan(OcrRequestId::for_test(99), OcrScanOutcome::Failed, start);

        assert!(state.advance_ocr_scan(start + SWEEP * 2));
        assert_eq!(
            state
                .ocr_scan()
                .and_then(|scan| scan.result(start + SWEEP * 2)),
            None,
            "the sweep is still waiting on its own request"
        );
    }

    #[test]
    fn settling_without_a_scan_is_inert() {
        // A recognition can outlive its overlay when the user dismisses first.
        let mut state = make_test_input_state();
        state.settle_ocr_scan(request(), OcrScanOutcome::Failed, Instant::now());
        assert!(state.ocr_scan().is_none());
        assert!(!state.advance_ocr_scan(Instant::now()));
    }

    #[test]
    fn a_still_overlay_asks_for_no_frames_and_shows_its_result_at_once() {
        // Reduced motion is passed in rather than written to the process-wide
        // flag, which every parallel test shares.
        const STILL: bool = false;
        let mut state = make_test_input_state();
        let start = Instant::now();
        state.begin_ocr_scan(request(), region(), start);

        // Nothing is moving, so nothing should pin a repaint while the worker
        // runs; the completion is what wakes the loop.
        assert!(
            !state.advance_ocr_scan_for(start, STILL),
            "no frames for a still scan"
        );
        assert_eq!(
            state.ocr_scan_wake_after_for(start, STILL),
            None,
            "and no deadline either"
        );
        assert_eq!(
            state
                .ocr_scan()
                .and_then(|scan| scan.sweep_progress_for(start, STILL)),
            None,
            "no band travels across the region"
        );
        assert!(state.ocr_scan().is_some_and(|scan| scan.is_scanning()));

        // With no sweep to finish, the outcome is shown as soon as it arrives
        // rather than waiting out an animation that never ran.
        state.settle_ocr_scan_for(request(), OcrScanOutcome::NoTextFound, start, STILL);
        assert!(!state.advance_ocr_scan_for(start, STILL));
        assert!(
            state
                .ocr_scan()
                .and_then(|scan| scan.result(start))
                .is_some(),
            "no pass to wait for"
        );

        // The one thing a still card needs is a deadline to be taken away on.
        let wake = state
            .ocr_scan_wake_after_for(start, STILL)
            .expect("a deadline");
        assert!(wake > Duration::ZERO && wake <= RESULT_LIFETIME);
        assert!(
            state.ocr_scan_wake_after_for(start + RESULT_LIFETIME, STILL) == Some(Duration::ZERO),
            "due at the end of its life"
        );
        assert!(!state.advance_ocr_scan_for(start + RESULT_LIFETIME, STILL));
        assert!(state.ocr_scan().is_none(), "and it is taken away");
    }

    #[test]
    fn the_card_fades_over_the_end_of_its_life_and_never_shows_text() {
        // Both settings are passed in rather than written to the process-wide
        // flag, which every parallel test shares.
        assert_eq!(result_opacity_for(Duration::ZERO, true), 1.0);
        assert_eq!(result_opacity_for(RESULT_LIFETIME - RESULT_FADE, true), 1.0);
        let mid = result_opacity_for(RESULT_LIFETIME - RESULT_FADE / 2, true);
        assert!(
            (0.0..1.0).contains(&mid),
            "part way through the fade: {mid}"
        );

        // Reduced motion cuts straight from opaque to gone: no gradual fade,
        // but the card still stops being painted once its life is over.
        assert_eq!(
            result_opacity_for(RESULT_LIFETIME - RESULT_FADE / 2, false),
            1.0
        );
        for motion in [true, false] {
            assert_eq!(
                result_opacity_for(RESULT_LIFETIME, motion),
                0.0,
                "an expired card paints nothing either way"
            );
        }

        // The card's own words: an outcome and a count, never a transcript.
        let copied = OcrScanOutcome::Copied {
            character_count: 42,
            replaced_invalid_utf8: false,
        };
        assert_eq!(copied.headline(), "Copied to clipboard");
        assert_eq!(copied.detail().as_deref(), Some("42 characters"));
        assert_eq!(
            OcrScanOutcome::Copied {
                character_count: 1,
                replaced_invalid_utf8: false,
            }
            .detail()
            .as_deref(),
            Some("1 character")
        );
        assert_eq!(OcrScanOutcome::NoTextFound.detail(), None);
        assert_eq!(OcrScanOutcome::Failed.headline(), "Recognition failed");
    }
}
