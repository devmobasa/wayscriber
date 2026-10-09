//! Re-admission on the retained backend after a bounded layout settling period.
use super::super::core::overlay::OverlaySuppressionState;
use super::super::*;
use crate::backend::wayland::capture_preflight::CapturePreflightError;
use std::time::Instant;

impl WaylandState {
    pub(in crate::backend::wayland) fn poll_layout_retries(&mut self, now: Instant) {
        if !self.frozen.has_layout_retry() && !self.zoom.has_layout_retry() {
            return;
        }

        self.refresh_freeze_zoom_geometry();
        if advance_layout_retries(
            &mut self.frozen,
            &mut self.zoom,
            &mut self.suppression,
            &mut self.input_state,
            self.gtk_toolbar.is_some(),
            now,
        ) {
            self.buffer_damage
                .mark_all_full(FullDamageReason::OverlaySuppression);
            self.toolbar.mark_dirty();
        }
    }
}

/// Keep eligibility, domain terminals and barrier admission together. The
/// runtime publishes redraw damage and GTK updates before it renders again.
fn advance_layout_retries(
    frozen: &mut FrozenState,
    zoom: &mut ZoomState,
    suppression: &mut OverlaySuppressionState,
    input: &mut InputState,
    wait_for_gtk: bool,
    now: Instant,
) -> bool {
    let mut restarted = false;

    if frozen.has_layout_retry() {
        match restart_suppressed_retry(
            suppression,
            OverlaySuppression::Frozen,
            wait_for_gtk,
            || frozen.restart_preflight(now),
        ) {
            Ok(ready) => restarted |= ready,
            Err(error) => {
                log::warn!("Freeze retry preflight failed: {error}");
                frozen.finish_preflight_failure(error, input);
            }
        }
    }

    if zoom.has_layout_retry() {
        match restart_suppressed_retry(suppression, OverlaySuppression::Zoom, wait_for_gtk, || {
            zoom.restart_preflight(now)
        }) {
            Ok(ready) => restarted |= ready,
            Err(error) => {
                log::warn!("Zoom retry preflight failed: {error}");
                zoom.finish_preflight_failure(input, error);
            }
        }
    }

    input.needs_redraw |= restarted;
    restarted
}

fn restart_suppressed_retry(
    suppression: &mut OverlaySuppressionState,
    reason: OverlaySuppression,
    wait_for_gtk: bool,
    restart: impl FnOnce() -> Result<bool, CapturePreflightError>,
) -> Result<bool, CapturePreflightError> {
    if suppression.reason() != reason {
        return Err(CapturePreflightError::LostSuppression);
    }
    if !restart()? {
        return Ok(false);
    }

    suppression.barrier.begin(reason, wait_for_gtk);
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::wayland::acquisition::{
        ScreenAcquisitionId, ScreenAcquisitionOutcome, ScreenAcquisitionOwner,
    };
    use crate::backend::wayland::frozen_geometry::OutputGeometry;
    use crate::backend::wayland::state::acquisition::AcquisitionRuntime;
    use crate::backend::wayland::zoom::{ZoomCaptureBackend, ZoomCaptureId, ZoomSourceOutcome};
    use std::time::Duration;

    #[derive(Clone, Copy)]
    enum RetainedRequest {
        Freeze {
            id: ScreenAcquisitionId,
            owner: ScreenAcquisitionOwner,
        },
        Zoom(ZoomCaptureId),
    }

    impl RetainedRequest {
        fn is_freeze(self) -> bool {
            matches!(self, Self::Freeze { .. })
        }

        fn assert_terminal(self, frozen: &mut FrozenState, zoom: &mut ZoomState, stale: bool) {
            match self {
                Self::Freeze { id, owner } => {
                    let terminal = frozen.take_acquisition_completion().unwrap();
                    assert_eq!((terminal.id, terminal.owner), (id, owner));
                    assert_eq!(
                        terminal.outcome == ScreenAcquisitionOutcome::StaleLayout,
                        stale
                    );
                    if !stale {
                        assert!(matches!(
                            terminal.outcome,
                            ScreenAcquisitionOutcome::Failed(_)
                        ));
                    }

                    assert!(frozen.take_acquisition_completion().is_none());
                    assert!(frozen.take_capture_done());
                    assert!(!frozen.is_in_progress());
                }
                Self::Zoom(id) => {
                    let terminal = zoom.take_source_terminal().unwrap();
                    assert_eq!(terminal.id, id);
                    assert_eq!(terminal.outcome == ZoomSourceOutcome::StaleLayout, stale);
                    if !stale {
                        assert!(matches!(terminal.outcome, ZoomSourceOutcome::Failed(_)));
                    }

                    assert!(zoom.take_source_terminal().is_none());
                    assert!(zoom.take_capture_done());
                    assert!(!zoom.is_in_progress());
                }
            }
        }
    }

    fn start_request(
        reason: OverlaySuppression,
        frozen: &mut FrozenState,
        zoom: &mut ZoomState,
        registry: &mut AcquisitionRuntime,
        owner: ScreenAcquisitionOwner,
    ) -> RetainedRequest {
        match reason {
            OverlaySuppression::Frozen => {
                let id = registry.request(owner).unwrap();
                frozen.start_capture_for(id, owner).unwrap();
                registry.mark_started(id, owner);

                RetainedRequest::Freeze { id, owner }
            }
            OverlaySuppression::Zoom => {
                zoom.start_capture(ZoomCaptureBackend::Portal).unwrap();
                zoom.request_activation();

                RetainedRequest::Zoom(zoom.current_capture_id().unwrap())
            }
            _ => panic!("unexpected capture suppression"),
        }
    }

    #[test]
    fn output_discovery_during_preflight_retains_the_request_and_repeats_suppression() {
        use crate::backend::wayland::handlers::test_support::{
            CaptureFixtureBackend, HandlerFixture,
        };
        use smithay_client_toolkit::compositor::CompositorHandler;

        for reason in [OverlaySuppression::Frozen, OverlaySuppression::Zoom] {
            let mut fixture = HandlerFixture::with_capture_output(
                crate::config::Config::default(),
                CaptureFixtureBackend::Portal,
            );
            let output = fixture.complete_output_metadata("DP-3");
            let state = &mut fixture.state;
            assert!(state.surface.current_output().is_none());

            let request = start_request(
                reason,
                &mut state.frozen,
                &mut state.zoom,
                &mut state.acquisition,
                ScreenAcquisitionOwner::UserFreeze,
            );
            state
                .suppression
                .enter(reason, OverlaySuppressionKeyboardPolicy::Release, true)
                .unwrap();
            let first_gtk = state.suppression.barrier.gtk_paint_generation().unwrap();
            let qh = fixture.queue.handle();
            let surface = state.surface.wl_surface().unwrap().clone();

            state.surface_enter(&fixture.conn, &qh, &surface, &output);
            assert_preflight_request_retained(state, request);

            state.acknowledge_gtk_capture_suppression(first_gtk);
            assert_eq!(
                state.suppression.barrier.begin_main_surface_submission(),
                Some(first_gtk)
            );
            state.mark_overlay_capture_frame_ready(first_gtk, &qh);

            assert_eq!(state.frozen.has_layout_retry(), request.is_freeze());
            assert_eq!(state.zoom.has_layout_retry(), !request.is_freeze());
            assert_preflight_request_retained(state, request);

            state.surface_enter(&fixture.conn, &qh, &surface, &output);
            assert_preflight_request_retained(state, request);

            let now = Instant::now();
            assert!(!advance_layout_retries(
                &mut state.frozen,
                &mut state.zoom,
                &mut state.suppression,
                &mut state.input_state,
                true,
                now,
            ));
            assert!(advance_layout_retries(
                &mut state.frozen,
                &mut state.zoom,
                &mut state.suppression,
                &mut state.input_state,
                true,
                now + Duration::from_millis(150),
            ));

            assert_eq!(state.suppression.reason(), reason);
            let retry_gtk = state.suppression.barrier.gtk_paint_generation().unwrap();
            assert_ne!(retry_gtk, first_gtk);

            state.acknowledge_gtk_capture_suppression(first_gtk);
            assert_eq!(
                state.suppression.barrier.begin_main_surface_submission(),
                None
            );

            state.acknowledge_gtk_capture_suppression(retry_gtk);
            assert_eq!(
                state.suppression.barrier.begin_main_surface_submission(),
                Some(retry_gtk)
            );
            state.mark_overlay_capture_frame_ready(retry_gtk, &qh);
            assert_preflight_request_retained(state, request);
            assert!(!state.frozen.preflight_pending());
            assert!(!state.zoom.preflight_pending());

            state.frozen.cancel(&mut state.input_state);
            state.zoom.abort_capture();
        }
    }

    fn assert_preflight_request_retained(state: &WaylandState, request: RetainedRequest) {
        let freezing = request.is_freeze();
        assert_eq!(state.frozen.has_acquisition_attempt(), freezing);
        assert_eq!(state.frozen.is_in_progress(), freezing);
        assert_eq!(state.acquisition.slot().is_some(), freezing);
        assert_eq!(state.zoom.is_engaged(), !freezing);
        assert_eq!(state.zoom.is_in_progress(), !freezing);

        match request {
            RetainedRequest::Freeze { id, owner } => {
                assert_eq!(
                    state.acquisition.slot().map(|slot| (slot.id, slot.owner)),
                    Some((id, owner))
                );
                assert!(state.zoom.current_capture_id().is_none());
            }
            RetainedRequest::Zoom(id) => assert_eq!(state.zoom.current_capture_id(), Some(id)),
        }
    }

    #[tokio::test]
    async fn retry_coordinator_preserves_suppression_and_finishes_failure_once() {
        for reason in [OverlaySuppression::Frozen, OverlaySuppression::Zoom] {
            // Successful admission, lost suppression, switched output, and
            // metadata that never becomes complete exercise the real owners.
            for failure in [None, Some("suppression"), Some("output"), Some("metadata")] {
                let wake = crate::backend::wayland::RuntimeWakeSource::new().unwrap();
                let mut frozen = FrozenState::new_with_runtime_wake(None, wake.handle());
                let mut zoom = ZoomState::new_with_runtime_wake(None, wake.handle());
                let mut input = crate::input::state::test_support::make_test_input_state();
                let geometry = OutputGeometry::update_from(
                    Some((0, 0)),
                    Some((2, 1)),
                    (2, 1),
                    1,
                    wl_output::Transform::Normal,
                    Some((2, 1)),
                )
                .unwrap()
                .with_known_output_count(Some(1));
                frozen.set_active_output(None, Some(1));
                frozen.set_active_geometry(Some(geometry.clone()));
                zoom.set_active_output(None, Some(1));
                zoom.set_active_geometry(Some(geometry.clone()));

                let mut registry = AcquisitionRuntime::default();
                let request = start_request(
                    reason,
                    &mut frozen,
                    &mut zoom,
                    &mut registry,
                    ScreenAcquisitionOwner::Ocr,
                );
                frozen.take_preflight_pending();
                zoom.take_preflight_pending();

                let mut changed = geometry;
                changed.logical_x = 8;
                frozen.set_active_geometry(Some(changed.clone()));
                zoom.set_active_geometry(Some(changed));

                assert_eq!(frozen.retry_stale_preflight(), request.is_freeze());
                assert_eq!(zoom.retry_stale_preflight(), !request.is_freeze());

                let mut suppression = OverlaySuppressionState::default();
                let active_reason = if failure == Some("suppression") {
                    OverlaySuppression::ExternalDialog
                } else {
                    reason
                };
                suppression
                    .enter(
                        active_reason,
                        OverlaySuppressionKeyboardPolicy::Retain,
                        true,
                    )
                    .unwrap();
                let first_gtk = suppression.barrier.gtk_paint_generation();

                if failure == Some("output") {
                    frozen.set_active_output(None, Some(2));
                    zoom.set_active_output(None, Some(2));
                }
                if failure == Some("metadata") {
                    frozen.set_active_geometry(None);
                    zoom.set_active_geometry(None);
                }

                let now = Instant::now();

                if failure.is_none() {
                    assert!(!advance_layout_retries(
                        &mut frozen,
                        &mut zoom,
                        &mut suppression,
                        &mut input,
                        true,
                        now
                    ));
                    assert_eq!(suppression.barrier.gtk_paint_generation(), first_gtk);
                }

                let admitted = advance_layout_retries(
                    &mut frozen,
                    &mut zoom,
                    &mut suppression,
                    &mut input,
                    true,
                    now + if failure == Some("metadata") {
                        Duration::from_secs(2)
                    } else {
                        Duration::from_millis(150)
                    },
                );

                assert_eq!(suppression.reason(), active_reason);
                assert!(!frozen.has_layout_retry());
                assert!(!zoom.has_layout_retry());

                if failure.is_none() {
                    assert!(admitted);
                    assert!(!suppression.keyboard_passthrough_requested(false));
                    assert_ne!(suppression.barrier.gtk_paint_generation(), first_gtk);
                    assert!(input.needs_redraw);
                    assert!(frozen.take_acquisition_completion().is_none());
                    assert!(zoom.take_source_terminal().is_none());
                } else {
                    assert!(!admitted);
                    assert_eq!(suppression.barrier.gtk_paint_generation(), first_gtk);
                    request.assert_terminal(
                        &mut frozen,
                        &mut zoom,
                        matches!(failure, Some("output" | "metadata")),
                    );
                }
            }
        }
    }
}
