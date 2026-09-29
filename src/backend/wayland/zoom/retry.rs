use super::state::{ZoomCaptureBackend, ZoomState};
use crate::backend::wayland::frozen_geometry::require_verified_capture_source;

impl ZoomState {
    pub(super) fn queue_portal_layout_retry(
        &mut self,
        captured: Option<u32>,
        changed: bool,
    ) -> bool {
        if self.current_capture_id().is_none()
            || require_verified_capture_source(
                self.active_geometry.clone(),
                self.active_output_id,
                "portal zoom retry",
            )
            .is_err()
            || !self
                .layout_retry
                .schedule(captured, self.active_output_id, changed)
        {
            return false;
        }
        self.portal.finish();
        self.capture_done = false;
        log::info!(
            "portal.zoom phase=retry-queued output={captured:?} current_layout={} budget_remaining=0",
            self.portal_layout_generation
        );
        true
    }

    pub(in crate::backend::wayland) fn retry_stale_portal_preflight(
        &mut self,
        backend: ZoomCaptureBackend,
    ) -> bool {
        let changed = (backend == ZoomCaptureBackend::Portal)
            && self
                .preflight
                .changed_on_output(self.active_output_id, self.portal_layout_generation);
        self.queue_portal_layout_retry(self.active_output_id, changed)
    }

    pub(in crate::backend::wayland) fn has_portal_layout_retry(&self) -> bool {
        self.layout_retry.is_pending()
    }

    /// Retains the original capture ID, waiter, and requested Zoom activation.
    pub(in crate::backend::wayland) fn restart_portal_preflight(&mut self) -> Result<bool, String> {
        let Some(output_id) = self.layout_retry.take_pending() else {
            return Ok(false);
        };
        if self.active_output_id != Some(output_id) {
            return Err("Zoom failed after the display layout changed".to_string());
        }
        require_verified_capture_source(
            self.active_geometry.clone(),
            self.active_output_id,
            "portal zoom retry",
        )?;
        self.preflight.begin(
            ZoomCaptureBackend::Portal,
            Some(output_id),
            self.portal_layout_generation,
        );
        log::info!(
            "portal.zoom phase=retry-preflight output={output_id} layout={}",
            self.portal_layout_generation
        );
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::wayland::frozen::{FrozenImage, ScreenImageProvenance};
    use crate::backend::wayland::frozen_geometry::OutputGeometry;
    use crate::backend::wayland::portal_task::PortalTask;
    use crate::backend::wayland::zoom::{
        ZoomSourceOutcome, ZoomWaiter, ZoomWaiterOwner, ZoomWaiterRegistry,
    };
    use crate::capture::CaptureError;
    use crate::input::state::test_support::make_test_input_state;
    use std::time::Instant;
    use wayland_client::protocol::wl_output;

    fn geometry(x: i32) -> OutputGeometry {
        OutputGeometry::update_from(
            Some((x, 0)),
            Some((2, 1)),
            (2, 1),
            1,
            wl_output::Transform::Normal,
            Some((2, 1)),
        )
        .unwrap()
    }
    fn image(byte: u8) -> FrozenImage {
        FrozenImage {
            width: 2,
            height: 1,
            stride: 8,
            data: vec![byte; 8],
        }
    }
    async fn drain(zoom: &mut ZoomState, input: &mut crate::input::InputState) {
        for _ in 0..100 {
            zoom.poll_portal_capture(input, Instant::now(), Some(1));
            if !zoom.portal.is_running() {
                return;
            }
            tokio::task::yield_now().await;
        }
        panic!("portal task did not finish");
    }

    #[tokio::test]
    async fn stale_raster_retries_once_with_the_same_waiter_and_activation() {
        for second_change in [false, true] {
            let wake = crate::backend::wayland::RuntimeWakeSource::new().unwrap();
            let mut zoom = ZoomState::new_with_runtime_wake(None, wake.handle());
            let mut input = make_test_input_state();
            zoom.set_active_output(None, Some(1));
            zoom.set_active_geometry(Some(geometry(0)));
            zoom.start_capture(true, &tokio::runtime::Handle::current())
                .unwrap();
            assert_eq!(
                zoom.take_preflight_pending(),
                Some(ZoomCaptureBackend::Portal)
            );
            let id = zoom.current_capture_id().unwrap();
            let mut waiters = ZoomWaiterRegistry::default();
            assert!(waiters.register(ZoomWaiter {
                id,
                owner: ZoomWaiterOwner::Ocr
            }));
            zoom.request_activation();
            let old_generation = zoom.output_layout_generation;
            zoom.portal.start(PortalTask::spawn(
                &tokio::runtime::Handle::current(),
                wake.handle(),
                async move {
                    Ok((
                        Some(1),
                        old_generation,
                        ScreenImageProvenance::new(
                            1,
                            old_generation,
                            1,
                            wl_output::Transform::Normal,
                        )
                        .unwrap(),
                        Err(CaptureError::ImageError("old raster size".to_string())),
                    ))
                },
            ));
            zoom.set_active_geometry(Some(geometry(8)));
            drain(&mut zoom, &mut input).await;
            assert!(zoom.is_in_progress());
            assert!(zoom.has_portal_layout_retry());
            assert!(zoom.pending_activation);
            assert!(!zoom.take_capture_done());
            assert!(zoom.take_source_terminal().is_none());
            assert_eq!(zoom.current_capture_id(), Some(id));
            assert!(zoom.restart_portal_preflight().unwrap());
            assert_eq!(
                zoom.take_preflight_pending(),
                Some(ZoomCaptureBackend::Portal)
            );
            assert!(zoom.ensure_preflight_layout_current().is_ok());
            let fresh_generation = zoom.output_layout_generation;
            zoom.portal.start(PortalTask::spawn(
                &tokio::runtime::Handle::current(),
                wake.handle(),
                async move {
                    Ok((
                        Some(1),
                        fresh_generation,
                        ScreenImageProvenance::new(
                            1,
                            fresh_generation,
                            1,
                            wl_output::Transform::Normal,
                        )
                        .unwrap(),
                        Ok(image(9)),
                    ))
                },
            ));
            if second_change {
                zoom.set_active_geometry(Some(geometry(16)));
            }
            drain(&mut zoom, &mut input).await;
            let terminal = zoom.take_source_terminal().unwrap();
            assert_eq!(terminal.id, id);
            assert!(waiters.take_for_terminal(&terminal).unwrap().1);
            assert!(!zoom.has_portal_layout_retry());
            assert!(zoom.take_capture_done());
            assert!(zoom.take_source_terminal().is_none());
            if second_change {
                assert_eq!(terminal.outcome, ZoomSourceOutcome::StaleLayout);
                assert!(!zoom.active);
                assert!(zoom.image().is_none());
            } else {
                assert!(matches!(terminal.outcome, ZoomSourceOutcome::Ready { .. }));
                assert!(zoom.active);
                assert_eq!(zoom.image().unwrap().data, vec![9; 8]);
                assert_eq!(
                    zoom.image_provenance().unwrap().output_layout_generation,
                    fresh_generation
                );
            }
        }
    }

    #[tokio::test]
    async fn cancellation_and_stable_raster_failure_do_not_schedule_retry() {
        for cancelled in [false, true] {
            let wake = crate::backend::wayland::RuntimeWakeSource::new().unwrap();
            let mut zoom = ZoomState::new_with_runtime_wake(None, wake.handle());
            let mut input = make_test_input_state();
            zoom.set_active_output(None, Some(1));
            zoom.set_active_geometry(Some(geometry(0)));
            let id = zoom.begin_identified_capture();
            let generation = zoom.output_layout_generation;
            zoom.portal.start(PortalTask::spawn(
                &tokio::runtime::Handle::current(),
                wake.handle(),
                async move {
                    if cancelled {
                        Err(CaptureError::Cancelled("dismissed".to_string()))
                    } else {
                        Ok((
                            Some(1),
                            generation,
                            ScreenImageProvenance::new(
                                1,
                                generation,
                                1,
                                wl_output::Transform::Normal,
                            )
                            .unwrap(),
                            Err(CaptureError::ImageError("stable wrong size".to_string())),
                        ))
                    }
                },
            ));
            if cancelled {
                zoom.set_active_geometry(Some(geometry(8)));
            }
            drain(&mut zoom, &mut input).await;
            assert!(!zoom.has_portal_layout_retry());
            assert!(!zoom.is_in_progress());
            let terminal = zoom.take_source_terminal().unwrap();
            assert_eq!(terminal.id, id);
            assert!(if cancelled {
                terminal.outcome == ZoomSourceOutcome::Cancelled
            } else {
                matches!(terminal.outcome, ZoomSourceOutcome::Failed(_))
            });
        }
    }

    #[tokio::test]
    async fn abort_clears_a_queued_retry_and_output_switch_cannot_redirect_it() {
        for switch_output in [false, true] {
            let mut zoom = ZoomState::new(None);
            zoom.set_active_output(None, Some(1));
            zoom.set_active_geometry(Some(geometry(0)));
            let id = zoom.begin_identified_capture();
            zoom.request_activation();
            assert!(zoom.queue_portal_layout_retry(Some(1), true));
            if switch_output {
                zoom.set_active_output(None, Some(2));
                assert!(zoom.restart_portal_preflight().is_err());
            }
            assert!(zoom.abort_capture());
            assert!(!zoom.has_portal_layout_retry());
            assert!(!zoom.restart_portal_preflight().unwrap());
            assert_eq!(zoom.take_source_terminal().unwrap().id, id);
        }
    }
}
