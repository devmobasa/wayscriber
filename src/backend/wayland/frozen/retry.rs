use super::state::{FrozenCaptureBackend, FrozenState};
use crate::backend::wayland::frozen_geometry::require_verified_capture_source;

impl FrozenState {
    pub(super) fn queue_portal_layout_retry(
        &mut self,
        captured: Option<u32>,
        changed: bool,
    ) -> bool {
        if require_verified_capture_source(
            self.active_geometry.clone(),
            self.active_output_id,
            "portal freeze retry",
        )
        .is_err()
            || !self
                .layout_retry
                .schedule(captured, self.active_output_id, changed)
        {
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

    /// Called only after the runtime refreshes all output geometry.
    pub(in crate::backend::wayland) fn restart_portal_preflight(&mut self) -> Result<bool, String> {
        let Some(output_id) = self.layout_retry.take_pending() else {
            return Ok(false);
        };
        if self.active_output_id != Some(output_id) {
            return Err("Freeze failed after the display layout changed".to_string());
        }
        require_verified_capture_source(
            self.active_geometry.clone(),
            self.active_output_id,
            "portal freeze retry",
        )?;
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
            let old_generation = frozen.output_layout_generation;
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
            assert!(frozen.restart_portal_preflight().unwrap());
            assert_eq!(
                frozen.take_preflight_pending(),
                Some(FrozenCaptureBackend::Portal)
            );
            let fresh_generation = frozen.output_layout_generation;
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

    #[tokio::test]
    async fn abandoning_a_retry_clears_its_admission_and_resources() {
        let mut frozen = FrozenState::new(None);
        let mut input = make_test_input_state();
        frozen.set_active_output(None, Some(1));
        frozen.set_active_geometry(Some(geometry(0)));
        assert!(frozen.queue_portal_layout_retry(Some(1), true));
        frozen.cancel(&mut input);
        assert!(!frozen.has_portal_layout_retry());
        assert!(!frozen.is_in_progress());
        assert!(!frozen.restart_portal_preflight().unwrap());
    }
}
