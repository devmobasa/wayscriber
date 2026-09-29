//! Re-admission of portal requests after a bounded layout settling period.
use super::super::core::overlay::OverlaySuppressionState;
use super::super::*;
use crate::input::state::{Toast, ToastPriority};
use std::time::Instant;

impl WaylandState {
    pub(in crate::backend::wayland) fn poll_portal_layout_retries(&mut self, now: Instant) {
        if !self.frozen.has_portal_layout_retry() && !self.zoom.has_portal_layout_retry() {
            return;
        }

        self.refresh_freeze_zoom_geometry();
        if advance_portal_layout_retries(
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
fn advance_portal_layout_retries(
    frozen: &mut FrozenState,
    zoom: &mut ZoomState,
    suppression: &mut OverlaySuppressionState,
    input: &mut InputState,
    wait_for_gtk: bool,
    now: Instant,
) -> bool {
    let mut restarted = false;

    if frozen.has_portal_layout_retry() {
        match restart_suppressed_portal_retry(
            suppression,
            OverlaySuppression::Frozen,
            wait_for_gtk,
            || frozen.restart_portal_preflight(now),
        ) {
            Ok(ready) => restarted |= ready,
            Err(error) => {
                log::warn!("Portal Freeze retry preflight failed: {error}");
                if !frozen.has_acquisition_attempt() {
                    input.push_toast(
                        ToastPriority::Critical,
                        "freeze",
                        Toast::error(error.clone()),
                    );
                }
                frozen.finish_preflight_failure(error, input);
            }
        }
    }

    if zoom.has_portal_layout_retry() {
        match restart_suppressed_portal_retry(
            suppression,
            OverlaySuppression::Zoom,
            wait_for_gtk,
            || zoom.restart_portal_preflight(now),
        ) {
            Ok(ready) => restarted |= ready,
            Err(error) => {
                log::warn!("Portal Zoom retry preflight failed: {error}");
                zoom.finish_preflight_failure(input, error);
            }
        }
    }

    input.needs_redraw |= restarted;
    restarted
}

fn restart_suppressed_portal_retry(
    suppression: &mut OverlaySuppressionState,
    reason: OverlaySuppression,
    wait_for_gtk: bool,
    restart: impl FnOnce() -> Result<bool, String>,
) -> Result<bool, String> {
    if suppression.reason() != reason {
        return Err("Capture retry lost overlay suppression".to_string());
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
    use crate::backend::wayland::acquisition::{ScreenAcquisitionOwner, ScreenAcquisitionRegistry};
    use crate::backend::wayland::frozen::FrozenCaptureBackend;
    use crate::backend::wayland::frozen_geometry::OutputGeometry;
    use crate::backend::wayland::zoom::{ZoomCaptureBackend, ZoomSourceOutcome};
    use std::time::Duration;

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

                let mut registry = ScreenAcquisitionRegistry::default();
                let owner = ScreenAcquisitionOwner::Ocr;
                let id = registry.request(owner).unwrap();
                let mut zoom_id = None;

                let mut changed = geometry;
                changed.logical_x = 8;
                if reason == OverlaySuppression::Frozen {
                    frozen.start_capture_for(id, owner).unwrap();
                    frozen.take_preflight_pending();
                    frozen.set_active_geometry(Some(changed));
                    assert!(frozen.retry_stale_portal_preflight(FrozenCaptureBackend::Portal));
                } else {
                    zoom.start_capture(true, &tokio::runtime::Handle::current())
                        .unwrap();
                    zoom_id = zoom.current_capture_id();
                    zoom.take_preflight_pending();
                    zoom.set_active_geometry(Some(changed));
                    assert!(zoom.retry_stale_portal_preflight(ZoomCaptureBackend::Portal));
                }

                let mut suppression = OverlaySuppressionState::default();
                let active_reason = if failure == Some("suppression") {
                    OverlaySuppression::ExternalDialog
                } else {
                    reason
                };
                suppression
                    .enter(
                        active_reason,
                        OverlaySuppressionKeyboardPolicy::Release,
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
                    assert!(!advance_portal_layout_retries(
                        &mut frozen,
                        &mut zoom,
                        &mut suppression,
                        &mut input,
                        true,
                        now
                    ));
                    assert_eq!(suppression.barrier.gtk_paint_generation(), first_gtk);
                }

                let admitted = advance_portal_layout_retries(
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
                assert!(!frozen.has_portal_layout_retry());
                assert!(!zoom.has_portal_layout_retry());
                if failure.is_none() {
                    assert!(admitted);
                    assert_ne!(suppression.barrier.gtk_paint_generation(), first_gtk);
                    assert!(input.needs_redraw);
                    assert!(frozen.take_acquisition_completion().is_none());
                    assert!(zoom.take_source_terminal().is_none());
                } else {
                    assert!(!admitted);
                    assert_eq!(suppression.barrier.gtk_paint_generation(), first_gtk);
                    if reason == OverlaySuppression::Frozen {
                        let terminal = frozen.take_acquisition_completion().unwrap();
                        assert_eq!((terminal.id, terminal.owner), (id, owner));
                        assert!(frozen.take_acquisition_completion().is_none());
                        assert!(frozen.take_capture_done());
                        assert!(!frozen.is_in_progress());
                    } else {
                        let terminal = zoom.take_source_terminal().unwrap();
                        assert_eq!(terminal.id, zoom_id.unwrap());
                        if failure == Some("output") {
                            assert_eq!(terminal.outcome, ZoomSourceOutcome::StaleLayout);
                        } else {
                            assert!(matches!(terminal.outcome, ZoomSourceOutcome::Failed(_)));
                        }
                        assert!(zoom.take_source_terminal().is_none());
                        assert!(zoom.take_capture_done());
                        assert!(!zoom.is_in_progress());
                    }
                }
            }
        }
    }
}
