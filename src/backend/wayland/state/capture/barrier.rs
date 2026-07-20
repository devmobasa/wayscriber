use super::super::*;
use crate::input::state::{Toast, ToastPriority};
use std::time::{Duration, Instant};

const MAIN_SURFACE_FRAME_TIMEOUT: Duration = Duration::from_secs(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MainSurfaceCapturePhase {
    AwaitingRender,
    AwaitingFrame,
    Ready,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ActiveOverlayCaptureBarrier {
    generation: u64,
    reason: OverlaySuppression,
    main_surface_phase: MainSurfaceCapturePhase,
    main_surface_frame_deadline: Option<Instant>,
    gtk_paint_generation: Option<u64>,
    started_at: Instant,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct OverlayCaptureBarrierTimeout {
    generation: u64,
    reason: OverlaySuppression,
    elapsed: Duration,
}

/// Coordinates capture preflight across the main Wayland surface and the
/// optional GTK toolbar connection.
#[derive(Debug, Default)]
pub(in crate::backend::wayland::state) struct OverlayCaptureBarrier {
    next_generation: u64,
    active: Option<ActiveOverlayCaptureBarrier>,
}

impl OverlayCaptureBarrier {
    pub(in crate::backend::wayland::state) fn begin(
        &mut self,
        reason: OverlaySuppression,
        wait_for_gtk: bool,
    ) -> Option<u64> {
        self.next_generation = self.next_generation.wrapping_add(1).max(1);
        let generation = self.next_generation;
        let gtk_paint_generation = wait_for_gtk.then_some(generation);
        self.active = Some(ActiveOverlayCaptureBarrier {
            generation,
            reason,
            main_surface_phase: MainSurfaceCapturePhase::AwaitingRender,
            main_surface_frame_deadline: None,
            gtk_paint_generation,
            started_at: Instant::now(),
        });
        log::info!(
            "capture.preflight id={generation} component=barrier reason={reason:?} phase=begin gtk_wait={wait_for_gtk}"
        );
        gtk_paint_generation
    }

    pub(in crate::backend::wayland::state) fn gtk_paint_generation(&self) -> Option<u64> {
        self.active.and_then(|active| active.gtk_paint_generation)
    }

    /// Records the fresh main-surface commit that follows any required GTK
    /// transparent-paint acknowledgement.
    pub(in crate::backend::wayland::state) fn begin_main_surface_submission(
        &mut self,
    ) -> Option<u64> {
        self.begin_main_surface_submission_at(Instant::now())
    }

    fn begin_main_surface_submission_at(&mut self, now: Instant) -> Option<u64> {
        let active = self.active.as_mut()?;
        if active.gtk_paint_generation.is_some()
            || active.main_surface_phase != MainSurfaceCapturePhase::AwaitingRender
        {
            return None;
        }
        active.main_surface_phase = MainSurfaceCapturePhase::AwaitingFrame;
        active.main_surface_frame_deadline = Some(now + MAIN_SURFACE_FRAME_TIMEOUT);
        log::info!(
            "capture.preflight id={} component=main-surface reason={:?} phase=submitted-hidden-frame elapsed_ms={}",
            active.generation,
            active.reason,
            active.started_at.elapsed().as_millis()
        );
        Some(active.generation)
    }

    fn mark_main_surface_frame_ready(&mut self, generation: u64) {
        if let Some(active) = self.active.as_mut()
            && active.generation == generation
            && active.main_surface_phase == MainSurfaceCapturePhase::AwaitingFrame
        {
            active.main_surface_phase = MainSurfaceCapturePhase::Ready;
            active.main_surface_frame_deadline = None;
            log::info!(
                "capture.preflight id={} component=main-surface reason={:?} phase=frame-callback elapsed_ms={}",
                active.generation,
                active.reason,
                active.started_at.elapsed().as_millis()
            );
        }
    }

    fn acknowledge_gtk_paint(&mut self, generation: u64) -> bool {
        let Some(active) = self.active.as_mut() else {
            log::info!(
                "capture.preflight id={generation} component=gtk phase=ack-ignored cause=no-active-barrier"
            );
            return false;
        };
        if active.gtk_paint_generation != Some(generation) {
            log::info!(
                "capture.preflight id={generation} component=gtk reason={:?} phase=ack-ignored expected={:?}",
                active.reason,
                active.gtk_paint_generation
            );
            return false;
        }
        active.gtk_paint_generation = None;
        // A main-surface frame committed before the GTK acknowledgement may
        // have been composed while the bars still had visible buffers. Only a
        // fresh hidden commit can unlock capture.
        active.main_surface_phase = MainSurfaceCapturePhase::AwaitingRender;
        active.main_surface_frame_deadline = None;
        log::info!(
            "capture.preflight id={generation} component=gtk reason={:?} phase=ack-accepted elapsed_ms={}",
            active.reason,
            active.started_at.elapsed().as_millis()
        );
        true
    }

    fn reason_waiting_for_gtk_generation(&self, generation: u64) -> Option<OverlaySuppression> {
        let active = self.active?;
        (active.gtk_paint_generation == Some(generation)).then_some(active.reason)
    }

    fn take_ready(&mut self) -> Option<OverlaySuppression> {
        let active = self.active?;
        if active.main_surface_phase != MainSurfaceCapturePhase::Ready
            || active.gtk_paint_generation.is_some()
        {
            return None;
        }
        self.active = None;
        log::info!(
            "capture.preflight id={} component=barrier reason={:?} phase=ready elapsed_ms={}",
            active.generation,
            active.reason,
            active.started_at.elapsed().as_millis()
        );
        Some(active.reason)
    }

    fn frame_timeout(&self, now: Instant) -> Option<Duration> {
        let active = self.active?;
        if active.main_surface_phase != MainSurfaceCapturePhase::AwaitingFrame {
            return None;
        }
        active
            .main_surface_frame_deadline
            .map(|deadline| deadline.saturating_duration_since(now))
    }

    fn take_frame_timeout(&mut self, now: Instant) -> Option<OverlayCaptureBarrierTimeout> {
        let active = self.active?;
        let deadline = active.main_surface_frame_deadline?;
        if active.main_surface_phase != MainSurfaceCapturePhase::AwaitingFrame || now < deadline {
            return None;
        }
        self.active = None;
        Some(OverlayCaptureBarrierTimeout {
            generation: active.generation,
            reason: active.reason,
            elapsed: now.saturating_duration_since(active.started_at),
        })
    }

    pub(in crate::backend::wayland::state) fn cancel(&mut self, reason: OverlaySuppression) {
        if self.active.is_some_and(|active| active.reason == reason) {
            if let Some(active) = self.active {
                log::info!(
                    "capture.preflight id={} component=barrier reason={reason:?} phase=cancel elapsed_ms={}",
                    active.generation,
                    active.started_at.elapsed().as_millis()
                );
            }
            self.active = None;
        }
    }

    fn reason_waiting_for_gtk(&self) -> Option<OverlaySuppression> {
        self.active
            .filter(|active| active.gtk_paint_generation.is_some())
            .map(|active| active.reason)
    }
}

impl WaylandState {
    pub(in crate::backend::wayland) fn overlay_capture_barrier_timeout(
        &self,
        now: Instant,
    ) -> Option<Duration> {
        self.data.overlay_capture_barrier.frame_timeout(now)
    }

    pub(in crate::backend::wayland) fn poll_overlay_capture_barrier_timeout(
        &mut self,
        now: Instant,
    ) {
        let Some(timeout) = self.data.overlay_capture_barrier.take_frame_timeout(now) else {
            return;
        };
        log::warn!(
            "capture.preflight id={} component=main-surface reason={:?} phase=frame-timeout elapsed_ms={}",
            timeout.generation,
            timeout.reason,
            timeout.elapsed.as_millis()
        );

        // The missing callback leaves the render throttle armed. Release it
        // before restoring suppression state so the recovery frame is allowed
        // to commit even if the original callback never arrives.
        self.surface.clear_frame_callback_pending();
        self.cancel_overlay_capture_preflight(timeout.reason);
        self.input_state.push_toast(ToastPriority::Critical, "capture", Toast::warning("Screen capture cancelled because the compositor did not confirm the hidden overlay frame."));
    }

    /// Records the frame callback for the fresh hidden main-surface commit.
    pub(in crate::backend::wayland) fn mark_overlay_capture_frame_ready(
        &mut self,
        generation: u64,
        qh: &QueueHandle<Self>,
    ) {
        self.data
            .overlay_capture_barrier
            .mark_main_surface_frame_ready(generation);
        self.begin_ready_overlay_capture(qh);
    }

    /// Records that every normally mapped GTK toolbar painted transparently
    /// and GTK completed a compositor roundtrip. Capture still requires a
    /// fresh hidden commit from the main Wayland surface.
    pub(in crate::backend::wayland) fn acknowledge_gtk_capture_suppression(
        &mut self,
        generation: u64,
    ) {
        if self
            .data
            .overlay_capture_barrier
            .acknowledge_gtk_paint(generation)
        {
            self.buffer_damage
                .mark_all_full(FullDamageReason::OverlaySuppression);
            self.input_state.needs_redraw = true;
        }
    }

    /// Cancels a capture whose GTK opacity-zero commit could not be confirmed.
    /// A presentation timeout is capture-local and does not disable an
    /// otherwise healthy GTK frontend.
    pub(in crate::backend::wayland) fn reject_gtk_capture_suppression(
        &mut self,
        generation: u64,
        error: &str,
    ) {
        let Some(reason) = self
            .data
            .overlay_capture_barrier
            .reason_waiting_for_gtk_generation(generation)
        else {
            log::info!(
                "capture.preflight id={generation} component=gtk phase=failure-ignored cause=stale-or-complete error={error}"
            );
            return;
        };
        log::warn!(
            "capture.preflight id={generation} component=gtk reason={reason:?} phase=capture-cancelled error={error}"
        );
        self.cancel_overlay_capture_preflight(reason);
        self.input_state.push_toast(
            ToastPriority::Critical,
            "capture",
            Toast::warning(
                "Screen capture cancelled because GTK toolbar transparency was not confirmed.",
            ),
        );
    }

    /// A failed GTK connection cannot prove that its mapped surfaces painted
    /// transparent. Cancel only captures still waiting for that proof.
    pub(in crate::backend::wayland) fn cancel_overlay_capture_waiting_for_gtk(&mut self) {
        let Some(reason) = self.data.overlay_capture_barrier.reason_waiting_for_gtk() else {
            return;
        };
        log::warn!(
            "Cancelling {reason:?} because the GTK toolbars could not confirm capture suppression"
        );
        self.cancel_overlay_capture_preflight(reason);
        self.input_state.push_toast(ToastPriority::Critical, "capture", Toast::warning("Screen capture cancelled because the GTK toolbar could not become transparent safely."));
    }

    fn begin_ready_overlay_capture(&mut self, qh: &QueueHandle<Self>) {
        let Some(reason) = self.data.overlay_capture_barrier.take_ready() else {
            return;
        };
        if self.data.overlay_suppression != reason {
            log::warn!(
                "Capture barrier completed for {reason:?} while suppression is {:?}; cancelling",
                self.data.overlay_suppression
            );
            self.cancel_overlay_capture_preflight(reason);
            return;
        }

        match reason {
            OverlaySuppression::Frozen => {
                let Some(use_fallback) = self.frozen.take_preflight_pending() else {
                    log::warn!("Frozen capture barrier completed without a pending preflight");
                    self.cancel_overlay_capture_preflight(reason);
                    return;
                };
                if let Err(err) = self.frozen.begin_preflight_capture(
                    use_fallback,
                    &self.shm,
                    qh,
                    &self.tokio_handle,
                ) {
                    log::warn!("Frozen preflight capture failed: {err}");
                    self.frozen.cancel(&mut self.input_state);
                }
            }
            OverlaySuppression::Zoom => {
                let Some(use_fallback) = self.zoom.take_preflight_pending() else {
                    log::warn!("Zoom capture barrier completed without a pending preflight");
                    self.cancel_overlay_capture_preflight(reason);
                    return;
                };
                if let Err(err) = self.zoom.begin_preflight_capture(
                    use_fallback,
                    &self.shm,
                    qh,
                    &self.tokio_handle,
                ) {
                    log::warn!("Zoom preflight capture failed: {err}");
                    self.zoom.cancel(&mut self.input_state, false);
                }
            }
            OverlaySuppression::Capture | OverlaySuppression::DesktopBackdrop => {
                let Some(request) = self.capture.take_preflight_request() else {
                    log::warn!("Capture barrier completed without a pending request");
                    self.cancel_overlay_capture_preflight(reason);
                    return;
                };
                if !self.capture_suppressed() {
                    log::warn!(
                        "Capture preflight completed without capture suppression; cancelling"
                    );
                    self.cancel_overlay_capture_preflight(reason);
                } else {
                    self.begin_pending_capture(request);
                }
            }
            OverlaySuppression::None | OverlaySuppression::ExternalDialog => {
                log::warn!("Unexpected capture barrier reason {reason:?}");
            }
        }
    }

    fn cancel_overlay_capture_preflight(&mut self, reason: OverlaySuppression) {
        match reason {
            OverlaySuppression::Capture | OverlaySuppression::DesktopBackdrop => {
                self.capture.clear_preflight();
                self.capture.clear_in_progress();
                self.capture.clear_exit_on_success();
                self.capture.clear_pending_pdf_export();
                self.show_overlay();
            }
            OverlaySuppression::Frozen => {
                self.frozen.cancel(&mut self.input_state);
                self.exit_overlay_suppression(reason);
            }
            OverlaySuppression::Zoom => {
                self.zoom.cancel(&mut self.input_state, false);
                self.exit_overlay_suppression(reason);
            }
            OverlaySuppression::None | OverlaySuppression::ExternalDialog => {
                self.data.overlay_capture_barrier.cancel(reason);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn main_surface_frame_deadline_starts_only_after_submission() {
        let mut barrier = OverlayCaptureBarrier::default();
        assert_eq!(barrier.begin(OverlaySuppression::Zoom, false), None);
        let submitted_at = Instant::now();

        assert_eq!(barrier.frame_timeout(submitted_at), None);
        assert!(
            barrier
                .begin_main_surface_submission_at(submitted_at)
                .is_some()
        );
        assert_eq!(
            barrier.frame_timeout(submitted_at),
            Some(MAIN_SURFACE_FRAME_TIMEOUT)
        );
        assert_eq!(
            barrier.frame_timeout(submitted_at + MAIN_SURFACE_FRAME_TIMEOUT / 2),
            Some(MAIN_SURFACE_FRAME_TIMEOUT / 2)
        );
        assert_eq!(
            barrier
                .frame_timeout(submitted_at + MAIN_SURFACE_FRAME_TIMEOUT - Duration::from_nanos(1)),
            Some(Duration::from_nanos(1))
        );
    }

    #[test]
    fn expired_main_surface_frame_wait_is_terminal() {
        let mut barrier = OverlayCaptureBarrier::default();
        assert_eq!(barrier.begin(OverlaySuppression::Frozen, false), None);
        let submitted_at = Instant::now();
        assert!(
            barrier
                .begin_main_surface_submission_at(submitted_at)
                .is_some()
        );
        let deadline = submitted_at + MAIN_SURFACE_FRAME_TIMEOUT;

        assert_eq!(
            barrier.take_frame_timeout(deadline - Duration::from_nanos(1)),
            None
        );
        let timeout = barrier
            .take_frame_timeout(deadline)
            .expect("deadline must terminate the active barrier");
        assert_eq!(timeout.reason, OverlaySuppression::Frozen);
        assert_eq!(barrier.frame_timeout(deadline), None);
        assert_eq!(barrier.take_frame_timeout(deadline), None);

        barrier.mark_main_surface_frame_ready(timeout.generation);
        assert_eq!(barrier.take_ready(), None);
    }

    #[test]
    fn timed_out_callback_cannot_complete_a_retry() {
        let mut barrier = OverlayCaptureBarrier::default();
        assert_eq!(barrier.begin(OverlaySuppression::Frozen, false), None);
        let submitted_at = Instant::now();
        let timed_out_generation = barrier
            .begin_main_surface_submission_at(submitted_at)
            .expect("first capture generation");
        barrier
            .take_frame_timeout(submitted_at + MAIN_SURFACE_FRAME_TIMEOUT)
            .expect("first capture times out");

        assert_eq!(barrier.begin(OverlaySuppression::Frozen, false), None);
        let retry_submitted_at = submitted_at + MAIN_SURFACE_FRAME_TIMEOUT;
        let retry_generation = barrier
            .begin_main_surface_submission_at(retry_submitted_at)
            .expect("retry generation");

        barrier.mark_main_surface_frame_ready(timed_out_generation);
        assert_eq!(barrier.take_ready(), None);
        barrier.mark_main_surface_frame_ready(retry_generation);
        assert_eq!(barrier.take_ready(), Some(OverlaySuppression::Frozen));
    }

    #[test]
    fn requires_a_fresh_submission_after_gtk_ack() {
        let mut barrier = OverlayCaptureBarrier::default();
        let generation = barrier
            .begin(OverlaySuppression::Zoom, true)
            .expect("GTK paint generation");

        assert_eq!(barrier.begin_main_surface_submission(), None);
        barrier.mark_main_surface_frame_ready(generation);
        assert_eq!(barrier.take_ready(), None);

        assert!(barrier.acknowledge_gtk_paint(generation));
        assert_eq!(barrier.take_ready(), None);
        assert_eq!(barrier.begin_main_surface_submission(), Some(generation));
        barrier.mark_main_surface_frame_ready(generation);
        assert_eq!(barrier.take_ready(), Some(OverlaySuppression::Zoom));
    }

    #[test]
    fn accepts_gtk_ack_before_main_surface_submission() {
        let mut barrier = OverlayCaptureBarrier::default();
        let generation = barrier
            .begin(OverlaySuppression::Frozen, true)
            .expect("GTK paint generation");

        assert!(barrier.acknowledge_gtk_paint(generation));
        assert_eq!(barrier.begin_main_surface_submission(), Some(generation));
        barrier.mark_main_surface_frame_ready(generation);
        assert_eq!(barrier.take_ready(), Some(OverlaySuppression::Frozen));
    }

    #[test]
    fn ignores_stale_gtk_acknowledgements() {
        let mut barrier = OverlayCaptureBarrier::default();
        let generation = barrier
            .begin(OverlaySuppression::Capture, true)
            .expect("GTK paint generation");

        assert!(!barrier.acknowledge_gtk_paint(generation.wrapping_add(1)));
        assert_eq!(barrier.begin_main_surface_submission(), None);
        assert!(barrier.acknowledge_gtk_paint(generation));
        assert_eq!(barrier.begin_main_surface_submission(), Some(generation));
        barrier.mark_main_surface_frame_ready(generation);
        assert_eq!(barrier.take_ready(), Some(OverlaySuppression::Capture));
    }

    #[test]
    fn without_gtk_needs_only_the_main_surface_frame() {
        let mut barrier = OverlayCaptureBarrier::default();
        assert_eq!(barrier.begin(OverlaySuppression::Zoom, false), None);

        let generation = barrier
            .begin_main_surface_submission()
            .expect("main-surface generation");
        barrier.mark_main_surface_frame_ready(generation);
        assert_eq!(barrier.take_ready(), Some(OverlaySuppression::Zoom));
    }

    #[test]
    fn ignores_a_frame_that_precedes_the_suppression_render() {
        let mut barrier = OverlayCaptureBarrier::default();
        assert_eq!(barrier.begin(OverlaySuppression::Frozen, false), None);

        barrier.mark_main_surface_frame_ready(barrier.next_generation);
        assert_eq!(barrier.take_ready(), None);
        let generation = barrier
            .begin_main_surface_submission()
            .expect("main-surface generation");
        barrier.mark_main_surface_frame_ready(generation);
        assert_eq!(barrier.take_ready(), Some(OverlaySuppression::Frozen));
    }

    #[test]
    fn cancellation_rejects_its_late_gtk_ack() {
        let mut barrier = OverlayCaptureBarrier::default();
        let generation = barrier
            .begin(OverlaySuppression::DesktopBackdrop, true)
            .expect("GTK paint generation");
        assert_eq!(
            barrier.reason_waiting_for_gtk(),
            Some(OverlaySuppression::DesktopBackdrop)
        );

        barrier.cancel(OverlaySuppression::DesktopBackdrop);

        assert_eq!(barrier.reason_waiting_for_gtk(), None);
        assert!(!barrier.acknowledge_gtk_paint(generation));
    }

    #[test]
    fn a_presentation_failure_matches_only_its_active_generation() {
        let mut barrier = OverlayCaptureBarrier::default();
        let generation = barrier
            .begin(OverlaySuppression::Zoom, true)
            .expect("GTK paint generation");

        assert_eq!(
            barrier.reason_waiting_for_gtk_generation(generation.wrapping_add(1)),
            None
        );
        assert_eq!(
            barrier.reason_waiting_for_gtk_generation(generation),
            Some(OverlaySuppression::Zoom)
        );
    }
}
