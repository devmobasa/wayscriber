use super::state::{FrozenCaptureBackend, FrozenState};
use crate::backend::wayland::capture_preflight::PortalRetryError;
use std::time::{Duration, Instant};

impl FrozenState {
    pub(super) fn queue_portal_layout_retry(
        &mut self,
        captured: Option<u32>,
        layout_changed: bool,
    ) -> bool {
        if !self.layout_retry.schedule(
            captured,
            self.active_output_id,
            layout_changed,
            self.portal_layout_generation,
            Instant::now(),
        ) {
            return false;
        }

        self.portal.finish();
        self.discard_pending_image_for_retry();
        self.capture_done = false;
        log::info!(
            "portal.freeze phase=retry-queued output={captured:?} current_layout={} budget_remaining=0",
            self.portal_layout_generation
        );

        true
    }

    pub(in crate::backend::wayland) fn retry_stale_portal_preflight(
        &mut self,
        backend: FrozenCaptureBackend,
    ) -> bool {
        let changed = backend == FrozenCaptureBackend::Portal
            && self
                .preflight
                .changed_on_output(self.active_output_id, self.portal_layout_generation);
        self.queue_portal_layout_retry(self.active_output_id, changed)
    }

    pub(in crate::backend::wayland) fn has_portal_layout_retry(&self) -> bool {
        self.layout_retry.is_pending()
    }

    pub(in crate::backend::wayland) fn portal_layout_retry_timeout(
        &self,
        now: Instant,
    ) -> Option<Duration> {
        self.layout_retry.timeout(now)
    }

    /// Called only after the runtime refreshes all output geometry.
    pub(in crate::backend::wayland) fn restart_portal_preflight(
        &mut self,
        now: Instant,
    ) -> Result<bool, String> {
        let Some(output_id) = self
            .layout_retry
            .take_ready(
                self.active_output_id,
                self.portal_layout_generation,
                self.active_geometry.as_ref(),
                now,
            )
            .map_err(|error| match error {
                PortalRetryError::OutputChanged => {
                    "Freeze failed after the display layout changed".to_string()
                }
                PortalRetryError::LayoutDidNotSettle => {
                    "Freeze failed because the display layout did not settle".to_string()
                }
            })?
        else {
            return Ok(false);
        };

        self.preflight.begin(
            FrozenCaptureBackend::Portal,
            Some(output_id),
            self.portal_layout_generation,
        );
        log::info!(
            "portal.freeze phase=retry-preflight output={output_id} layout={}",
            self.portal_layout_generation
        );

        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::wayland::acquisition::{
        ScreenAcquisitionOutcome, ScreenAcquisitionOwner, ScreenAcquisitionRegistry,
    };
    use crate::backend::wayland::frozen::FrozenImage;
    use crate::backend::wayland::frozen_geometry::OutputGeometry;
    use crate::backend::wayland::portal_task::PortalTask;
    use crate::capture::CaptureError;
    use crate::input::state::test_support::make_test_input_state;
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
        .with_known_output_count(Some(1))
    }

    async fn drain(frozen: &mut FrozenState, input: &mut crate::input::InputState) {
        for _ in 0..100 {
            frozen.poll_portal_capture(input, Instant::now(), None);
            if !frozen.portal.is_running() {
                return;
            }
            tokio::task::yield_now().await;
        }

        panic!("portal task did not finish");
    }

    #[tokio::test]
    async fn retry_retains_the_acquisition_and_completes_once() {
        for second_change in [false, true] {
            let wake = crate::backend::wayland::RuntimeWakeSource::new().unwrap();
            let mut frozen = FrozenState::new_with_runtime_wake(None, wake.handle());
            let mut input = make_test_input_state();
            let mut registry = ScreenAcquisitionRegistry::default();
            let owner = ScreenAcquisitionOwner::Ocr;
            let id = registry.request(owner).unwrap();
            frozen.set_active_output(None, Some(1));
            frozen.set_active_geometry(Some(geometry(0)));
            frozen.start_capture_for(id, owner).unwrap();
            assert_eq!(
                frozen.take_preflight_pending(),
                Some(FrozenCaptureBackend::Portal)
            );

            let old_generation = frozen.portal_layout_generation;
            frozen.portal.start(PortalTask::spawn(
                &tokio::runtime::Handle::current(),
                wake.handle(),
                async move {
                    Ok((
                        Some(1),
                        old_generation,
                        Some(geometry(0)),
                        Err(CaptureError::ImageError("stale size".to_string())),
                    ))
                },
            ));

            frozen.set_active_geometry(Some(geometry(8)));

            drain(&mut frozen, &mut input).await;

            assert!(frozen.has_portal_layout_retry());
            assert!(frozen.has_acquisition_attempt());
            assert!(!frozen.take_capture_done());
            assert!(frozen.take_acquisition_completion().is_none());
            assert!(
                frozen
                    .restart_portal_preflight(Instant::now() + Duration::from_millis(150))
                    .unwrap()
            );
            assert_eq!(
                frozen.take_preflight_pending(),
                Some(FrozenCaptureBackend::Portal)
            );

            let fresh_generation = frozen.portal_layout_generation;
            frozen.portal.start(PortalTask::spawn(
                &tokio::runtime::Handle::current(),
                wake.handle(),
                async move {
                    Ok((
                        Some(1),
                        fresh_generation,
                        Some(geometry(8)),
                        Ok(FrozenImage {
                            width: 2,
                            height: 1,
                            stride: 8,
                            data: vec![9; 8],
                        }),
                    ))
                },
            ));

            if second_change {
                frozen.set_active_geometry(Some(geometry(16)));
            }

            drain(&mut frozen, &mut input).await;
            if !second_change {
                assert!(frozen.activate_pending_image(2, 1, &mut input).unwrap());
                assert_eq!(frozen.image().unwrap().data, vec![9; 8]);
                assert!(input.frozen_active());
            }

            let terminal = frozen.take_acquisition_completion().unwrap();
            assert_eq!((terminal.id, terminal.owner), (id, owner));
            assert!(if second_change {
                terminal.outcome == ScreenAcquisitionOutcome::StaleLayout
            } else {
                matches!(terminal.outcome, ScreenAcquisitionOutcome::Ready { .. })
            });
            assert!(!frozen.has_portal_layout_retry());
            assert!(!frozen.has_acquisition_attempt());
            assert!(frozen.take_capture_done());
            assert!(frozen.take_acquisition_completion().is_none());
        }
    }

    #[test]
    fn activation_rechecks_layout_and_topology_before_installing_a_portal_crop() {
        for live_topology_changed in [false, true] {
            let wake = crate::backend::wayland::RuntimeWakeSource::new().unwrap();
            let mut frozen = FrozenState::new_with_runtime_wake(None, wake.handle());
            let mut input = make_test_input_state();
            let mut registry = ScreenAcquisitionRegistry::default();
            let owner = ScreenAcquisitionOwner::Ocr;
            let id = registry.request(owner).unwrap();
            frozen.set_active_output(None, Some(1));
            frozen.set_active_geometry(Some(geometry(0)));
            frozen.start_capture_for(id, owner).unwrap();
            frozen.take_preflight_pending();
            frozen.set_pending_portal_image(
                FrozenImage {
                    width: 2,
                    height: 1,
                    stride: 8,
                    data: vec![9; 8],
                },
                Some(1),
                Some(geometry(0)),
            );
            if !live_topology_changed {
                frozen.set_active_geometry(Some(geometry(8)));
            }

            assert!(
                !frozen
                    .activate_pending_image_with_live_outputs(
                        2,
                        1,
                        &mut input,
                        Some(if live_topology_changed { 2 } else { 1 })
                    )
                    .unwrap()
            );

            assert!(frozen.has_portal_layout_retry());
            assert!(frozen.has_acquisition_attempt());
            assert!(frozen.take_acquisition_completion().is_none());
            assert!(!frozen.take_capture_done());
            assert!(!frozen.has_pending_image());
            assert!(frozen.image().is_none());
            assert!(!input.frozen_active());
        }
    }

    #[tokio::test]
    async fn cancellation_and_stable_raster_failure_do_not_schedule_retry() {
        for cancelled in [false, true] {
            let wake = crate::backend::wayland::RuntimeWakeSource::new().unwrap();
            let mut frozen = FrozenState::new_with_runtime_wake(None, wake.handle());
            let mut input = make_test_input_state();
            let mut registry = ScreenAcquisitionRegistry::default();
            let owner = ScreenAcquisitionOwner::Ocr;
            let id = registry.request(owner).unwrap();
            frozen.set_active_output(None, Some(1));
            frozen.set_active_geometry(Some(geometry(0)));
            frozen.start_capture_for(id, owner).unwrap();
            frozen.take_preflight_pending();
            let generation = frozen.portal_layout_generation;
            frozen.portal.start(PortalTask::spawn(
                &tokio::runtime::Handle::current(),
                wake.handle(),
                async move {
                    if cancelled {
                        Err(CaptureError::Cancelled("dismissed".to_string()))
                    } else {
                        Ok((
                            Some(1),
                            generation,
                            Some(geometry(0)),
                            Err(CaptureError::ImageError("stable wrong size".to_string())),
                        ))
                    }
                },
            ));
            if cancelled {
                frozen.set_active_geometry(Some(geometry(8)));
            }

            drain(&mut frozen, &mut input).await;

            assert!(!frozen.has_portal_layout_retry());
            assert!(!frozen.is_in_progress());

            let terminal = frozen.take_acquisition_completion().unwrap();
            assert_eq!((terminal.id, terminal.owner), (id, owner));
            assert!(if cancelled {
                terminal.outcome == ScreenAcquisitionOutcome::Cancelled
            } else {
                matches!(terminal.outcome, ScreenAcquisitionOutcome::Failed(_))
            });
            assert!(frozen.take_acquisition_completion().is_none());
        }
    }

    #[test]
    fn cancel_clears_a_retry_and_an_output_switch_cannot_redirect_it() {
        for switch_output in [false, true] {
            let mut frozen = FrozenState::new(None);
            let mut input = make_test_input_state();
            frozen.set_active_output(None, Some(1));
            frozen.set_active_geometry(Some(geometry(0)));
            assert!(frozen.queue_portal_layout_retry(Some(1), true));
            if switch_output {
                frozen.set_active_output(None, Some(2));

                assert!(frozen.restart_portal_preflight(Instant::now()).is_err());
            }

            frozen.cancel(&mut input);

            assert!(!frozen.has_portal_layout_retry());
            assert!(!frozen.is_in_progress());
            assert!(!frozen.restart_portal_preflight(Instant::now()).unwrap());
        }
    }
}
