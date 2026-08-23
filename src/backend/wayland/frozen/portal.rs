use crate::input::state::{Toast, ToastPriority};
use anyhow::Result;
use log::warn;
use std::time::{Duration, Instant};

use crate::backend::wayland::acquisition::ScreenAcquisitionOutcome;
use crate::backend::wayland::frozen::FrozenImage;
use crate::backend::wayland::frozen_geometry::require_verified_capture_source;
use crate::backend::wayland::portal_capture::{
    capture_via_portal_fullscreen_bytes, portal_output_matches,
};
use crate::backend::wayland::portal_task::{PortalPoll, PortalTask};
use crate::capture::sources::frozen::decode_image_to_argb;
use crate::capture::types::CaptureError;
use crate::input::InputState;

use super::state::FrozenState;

impl FrozenState {
    pub(in crate::backend::wayland) fn capture_via_portal(
        &mut self,
        tokio_handle: &tokio::runtime::Handle,
    ) -> Result<()> {
        if self.portal_in_progress {
            warn!("Portal capture already running; ignoring new request");
            return Ok(());
        }

        let runtime_wake = self
            .runtime_wake
            .clone()
            .ok_or_else(|| anyhow::anyhow!("portal capture runtime wake is unavailable"))?;
        let (source_geometry, target_output_id) = require_verified_capture_source(
            self.active_geometry.clone(),
            self.active_output_id,
            "portal freeze capture",
        )
        .map_err(anyhow::Error::msg)?;
        self.portal_in_progress = true;
        self.portal_target_output_id = Some(target_output_id);

        let layout_generation = self.output_layout_generation;
        // Notify user that portal fallback is in progress
        crate::notification::send_notification_async(
            tokio_handle,
            "Freezing screen".to_string(),
            "Requesting screen capture...".to_string(),
            Some("camera-photo".to_string()),
        );
        self.portal_task = Some(PortalTask::spawn(tokio_handle, runtime_wake, async move {
            async {
                let bytes = capture_via_portal_fullscreen_bytes().await?;

                let (data, width, height) = decode_image_to_argb(&bytes)
                    .map_err(|error| CaptureError::ImageError(format!("Decode failed: {error}")))?;

                Ok((
                    Some(target_output_id),
                    layout_generation,
                    Some(source_geometry),
                    FrozenImage {
                        width,
                        height,
                        stride: (width * 4) as i32,
                        data,
                    },
                ))
            }
            .await
        }));

        Ok(())
    }

    /// Check for completed portal capture and apply result if present.
    pub fn poll_portal_capture(&mut self, input_state: &mut InputState, now: Instant) {
        if !self.portal_in_progress {
            return;
        }

        // Timeout safeguard to avoid overlay staying hidden forever
        if self
            .portal_task
            .as_ref()
            .is_some_and(|task| task.timed_out(now))
        {
            warn!("Portal frozen capture timed out; restoring overlay");
            if self.has_acquisition_attempt() {
                self.finish_acquisition(
                    ScreenAcquisitionOutcome::Failed(
                        "Freeze timed out while waiting for screen capture.".to_string(),
                    ),
                    input_state,
                );
            } else {
                input_state.push_toast(
                    ToastPriority::Critical,
                    "freeze",
                    Toast::error("Freeze timed out while waiting for screen capture."),
                );
                input_state.set_frozen_active(false);
                self.finish_portal_task();
                self.capture_done = true;
            }
            return;
        }

        let poll = self
            .portal_task
            .as_mut()
            .map(PortalTask::poll)
            .unwrap_or(PortalPoll::Disconnected);
        match poll {
            PortalPoll::Ready(Ok((target_output, layout_generation, source_geometry, image))) => {
                let output_matches = portal_output_matches(target_output, self.active_output_id);
                let layout_matches = layout_generation == self.output_layout_generation;

                if output_matches && layout_matches {
                    self.set_pending_desktop_image(image, target_output, source_geometry);
                } else {
                    if !layout_matches {
                        warn!("Portal capture discarded after the output layout changed");
                    } else {
                        warn!("Portal capture for inactive output discarded");
                    }
                    if self.has_acquisition_attempt() {
                        self.finish_acquisition(ScreenAcquisitionOutcome::StaleLayout, input_state);
                    } else {
                        Self::push_stale_layout_toast(input_state);
                        self.capture_done = true;
                    }
                }

                self.finish_portal_task();
            }
            PortalPoll::Ready(Err(CaptureError::Cancelled(reason))) => {
                log::info!("Portal frozen capture cancelled: {reason}");
                if self.has_acquisition_attempt() {
                    self.finish_acquisition(ScreenAcquisitionOutcome::Cancelled, input_state);
                } else {
                    input_state.set_frozen_active(false);
                    input_state.needs_redraw = true;
                    self.finish_portal_task();
                    self.capture_done = true;
                }
            }
            PortalPoll::Ready(Err(err)) => {
                warn!("Portal frozen capture failed: {err}");
                if self.has_acquisition_attempt() {
                    self.finish_acquisition(
                        ScreenAcquisitionOutcome::Failed(
                            "Freeze could not capture the screen.".to_string(),
                        ),
                        input_state,
                    );
                } else {
                    input_state.push_toast(
                        ToastPriority::Critical,
                        "freeze",
                        Toast::error("Freeze could not capture the screen."),
                    );
                    input_state.set_frozen_active(false);
                    self.finish_portal_task();
                    self.capture_done = true;
                }
            }
            PortalPoll::Failed(err) => {
                warn!("Portal frozen capture task failed: {err}");
                if self.has_acquisition_attempt() {
                    self.finish_acquisition(
                        ScreenAcquisitionOutcome::Failed(
                            "Freeze could not capture the screen.".to_string(),
                        ),
                        input_state,
                    );
                } else {
                    input_state.push_toast(
                        ToastPriority::Critical,
                        "freeze",
                        Toast::error("Freeze could not capture the screen."),
                    );
                    input_state.set_frozen_active(false);
                    self.finish_portal_task();
                    self.capture_done = true;
                }
            }
            PortalPoll::Pending => {}
            PortalPoll::Disconnected => {
                warn!("Portal frozen capture channel disconnected");
                let message = "Freeze could not capture the screen because the system capture service stopped responding.";
                if self.has_acquisition_attempt() {
                    self.finish_acquisition(
                        ScreenAcquisitionOutcome::Failed(message.to_string()),
                        input_state,
                    );
                } else {
                    input_state.push_toast(
                        ToastPriority::Critical,
                        "freeze",
                        Toast::error(message),
                    );
                    input_state.set_frozen_active(false);
                    self.finish_portal_task();
                    self.capture_done = true;
                }
            }
        }
    }

    pub fn portal_timeout(&self, now: Instant) -> Option<Duration> {
        self.portal_task.as_ref().map(|task| task.timeout(now))
    }

    fn finish_portal_task(&mut self) {
        self.portal_in_progress = false;
        self.portal_task.take();
        self.portal_target_output_id = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::wayland::acquisition::{
        ScreenAcquisitionOutcome, ScreenAcquisitionOwner, ScreenAcquisitionRegistry,
    };
    use crate::backend::wayland::frozen::FrozenState;
    use crate::backend::wayland::portal_task::PORTAL_CAPTURE_TIMEOUT;
    use crate::input::state::test_support::make_test_input_state;

    fn image(byte: u8) -> FrozenImage {
        FrozenImage {
            width: 2,
            height: 1,
            stride: 8,
            data: vec![byte; 8],
        }
    }

    fn crop_geometry(
        origin: (u32, u32),
    ) -> crate::backend::wayland::frozen_geometry::OutputGeometry {
        crate::backend::wayland::frozen_geometry::OutputGeometry {
            logical_x: 0,
            logical_y: 0,
            logical_width: 2,
            logical_height: 1,
            scale: 1,
            transform: wayland_client::protocol::wl_output::Transform::Normal,
            overlay_buffer_size: (2, 1),
            pixel_size: Some((2, 1)),
            screenshot_origin: Some(origin),
            screenshot_size: None,
            known_output_count: None,
        }
    }

    async fn poll_until_finished(
        frozen: &mut FrozenState,
        input: &mut InputState,
    ) -> anyhow::Result<()> {
        for _ in 0..100 {
            frozen.poll_portal_capture(input, Instant::now());
            if !frozen.portal_in_progress {
                return Ok(());
            }
            tokio::task::yield_now().await;
        }
        anyhow::bail!("frozen portal task did not finish")
    }

    #[tokio::test]
    async fn portal_start_requires_verifiable_geometry_and_output_identity() -> anyhow::Result<()> {
        let wake = crate::backend::wayland::RuntimeWakeSource::new()?;
        let mut frozen = FrozenState::new_with_runtime_wake(None, wake.handle());

        let error = frozen
            .capture_via_portal(&tokio::runtime::Handle::current())
            .expect_err("missing geometry must fail closed");
        assert!(error.to_string().contains("geometry is unavailable"));
        assert!(!frozen.portal_in_progress);
        assert!(frozen.portal_task.is_none());

        frozen.set_active_geometry(Some(crop_geometry((0, 0))));
        let error = frozen
            .capture_via_portal(&tokio::runtime::Handle::current())
            .expect_err("missing output identity must fail closed");
        assert!(error.to_string().contains("identity is unavailable"));
        assert!(!frozen.portal_in_progress);
        assert!(frozen.portal_task.is_none());
        Ok(())
    }

    #[tokio::test]
    async fn poll_portal_applies_image() -> anyhow::Result<()> {
        let wake = crate::backend::wayland::RuntimeWakeSource::new()?;
        let mut frozen = FrozenState::new_with_runtime_wake(None, wake.handle());
        let mut input = make_test_input_state();
        frozen.set_active_output(None, Some(1));

        frozen.portal_task = Some(PortalTask::spawn(
            &tokio::runtime::Handle::current(),
            wake.handle(),
            async { Ok((Some(1), 0, Some(crop_geometry((0, 0))), image(0))) },
        ));
        frozen.portal_in_progress = true;
        poll_until_finished(&mut frozen, &mut input).await?;

        assert!(!input.frozen_active());
        assert!(frozen.has_pending_image());
        assert!(!frozen.portal_in_progress);
        assert!(frozen.portal_task.is_none());
        assert!(!frozen.take_capture_done());

        frozen
            .activate_pending_image(2, 1, &mut input)
            .expect("activate pending image");

        assert!(input.frozen_active());
        assert!(frozen.image.is_some());
        assert!(frozen.take_capture_done());
        Ok(())
    }

    #[tokio::test]
    async fn domain_error_and_task_panic_restore_the_frozen_lifecycle() -> anyhow::Result<()> {
        for panic_task in [false, true] {
            let wake = crate::backend::wayland::RuntimeWakeSource::new()?;
            let mut frozen = FrozenState::new_with_runtime_wake(None, wake.handle());
            let mut input = make_test_input_state();
            frozen.portal_task = Some(if panic_task {
                PortalTask::spawn(&tokio::runtime::Handle::current(), wake.handle(), async {
                    panic!("expected frozen portal panic")
                })
            } else {
                PortalTask::spawn(&tokio::runtime::Handle::current(), wake.handle(), async {
                    Err(CaptureError::PermissionDenied)
                })
            });
            frozen.portal_in_progress = true;

            poll_until_finished(&mut frozen, &mut input).await?;

            assert!(!frozen.is_in_progress());
            assert!(frozen.portal_task.is_none());
            assert!(frozen.take_capture_done());
            assert!(!input.frozen_active());
        }
        Ok(())
    }

    #[tokio::test]
    async fn disconnect_and_deadline_expiry_restore_without_a_producer_result() -> anyhow::Result<()>
    {
        let now = Instant::now();
        for timed_out in [false, true] {
            let wake = crate::backend::wayland::RuntimeWakeSource::new()?;
            let mut frozen = FrozenState::new_with_runtime_wake(None, wake.handle());
            let mut input = make_test_input_state();
            frozen.portal_task = Some(if timed_out {
                PortalTask::spawn_at_for_test(
                    &tokio::runtime::Handle::current(),
                    wake.handle(),
                    now.checked_sub(PORTAL_CAPTURE_TIMEOUT).ok_or_else(|| {
                        anyhow::anyhow!("monotonic clock cannot represent the test deadline")
                    })?,
                    std::future::pending(),
                )
            } else {
                PortalTask::disconnected_for_test(now)
            });
            frozen.portal_in_progress = true;

            frozen.poll_portal_capture(&mut input, now);

            assert!(!frozen.is_in_progress());
            assert!(frozen.portal_task.is_none());
            assert!(frozen.take_capture_done());
            assert!(!input.frozen_active());
        }
        Ok(())
    }

    #[tokio::test]
    async fn user_cancellation_restores_quietly_without_an_error_toast() -> anyhow::Result<()> {
        let wake = crate::backend::wayland::RuntimeWakeSource::new()?;
        let mut frozen = FrozenState::new_with_runtime_wake(None, wake.handle());
        let mut input = make_test_input_state();
        frozen.portal_task = Some(PortalTask::spawn(
            &tokio::runtime::Handle::current(),
            wake.handle(),
            async {
                Err(CaptureError::Cancelled(
                    "user closed the chooser".to_string(),
                ))
            },
        ));
        frozen.portal_in_progress = true;

        poll_until_finished(&mut frozen, &mut input).await?;

        assert!(!frozen.is_in_progress());
        assert!(frozen.take_capture_done());
        assert!(!input.frozen_active());
        assert!(input.ui_toast.is_none());
        Ok(())
    }

    #[tokio::test]
    async fn portal_cancellation_finishes_the_correlated_attempt() -> anyhow::Result<()> {
        let wake = crate::backend::wayland::RuntimeWakeSource::new()?;
        let mut frozen = FrozenState::new_with_runtime_wake(None, wake.handle());
        let mut input = make_test_input_state();
        let mut registry = ScreenAcquisitionRegistry::default();
        let id = registry
            .request(ScreenAcquisitionOwner::UserFreeze)
            .expect("id");
        frozen.start_capture_for(id, ScreenAcquisitionOwner::UserFreeze)?;
        frozen.portal_task = Some(PortalTask::spawn(
            &tokio::runtime::Handle::current(),
            wake.handle(),
            async {
                Err(CaptureError::Cancelled(
                    "user closed the chooser".to_string(),
                ))
            },
        ));
        frozen.portal_in_progress = true;

        poll_until_finished(&mut frozen, &mut input).await?;

        assert_eq!(
            frozen
                .take_acquisition_completion()
                .map(|completion| completion.outcome),
            Some(ScreenAcquisitionOutcome::Cancelled)
        );
        assert!(frozen.take_capture_done());
        assert!(input.ui_toast.is_none());
        Ok(())
    }

    #[tokio::test]
    async fn stale_output_is_discarded_without_mutating_current_frozen_state() -> anyhow::Result<()>
    {
        let wake = crate::backend::wayland::RuntimeWakeSource::new()?;
        let mut frozen = FrozenState::new_with_runtime_wake(None, wake.handle());
        let mut input = make_test_input_state();
        input.set_frozen_active(true);
        frozen.set_active_output(None, Some(2));
        frozen.portal_task = Some(PortalTask::spawn(
            &tokio::runtime::Handle::current(),
            wake.handle(),
            async { Ok((Some(1), 0, None, image(9))) },
        ));
        frozen.portal_in_progress = true;

        poll_until_finished(&mut frozen, &mut input).await?;

        assert!(input.frozen_active());
        assert!(!frozen.has_pending_image());
        assert!(frozen.take_capture_done());
        assert!(input.ui_toast.is_some());
        Ok(())
    }

    #[tokio::test]
    async fn stale_layout_is_discarded_without_a_pending_image() -> anyhow::Result<()> {
        let wake = crate::backend::wayland::RuntimeWakeSource::new()?;
        let mut frozen = FrozenState::new_with_runtime_wake(None, wake.handle());
        let mut input = make_test_input_state();
        frozen.set_active_geometry(Some(crop_geometry((0, 0))));
        let layout_generation = frozen.output_layout_generation;
        frozen.portal_task = Some(PortalTask::spawn(
            &tokio::runtime::Handle::current(),
            wake.handle(),
            async move { Ok((None, layout_generation, None, image(9))) },
        ));
        frozen.portal_in_progress = true;
        frozen.set_active_geometry(Some(crop_geometry((6, 0))));

        poll_until_finished(&mut frozen, &mut input).await?;

        assert!(!frozen.has_pending_image());
        assert!(frozen.take_capture_done());
        assert!(input.ui_toast.is_some());
        Ok(())
    }

    #[tokio::test]
    async fn matching_layout_still_applies_after_an_identical_geometry_refresh()
    -> anyhow::Result<()> {
        let wake = crate::backend::wayland::RuntimeWakeSource::new()?;
        let mut frozen = FrozenState::new_with_runtime_wake(None, wake.handle());
        let mut input = make_test_input_state();
        let geometry = crop_geometry((0, 0));
        frozen.set_active_geometry(Some(geometry.clone()));
        let layout_generation = frozen.output_layout_generation;
        frozen.portal_task = Some(PortalTask::spawn(
            &tokio::runtime::Handle::current(),
            wake.handle(),
            async move { Ok((None, layout_generation, None, image(0))) },
        ));
        frozen.portal_in_progress = true;
        frozen.set_active_geometry(Some(geometry));

        poll_until_finished(&mut frozen, &mut input).await?;

        assert!(frozen.has_pending_image());
        assert!(!frozen.take_capture_done());
        Ok(())
    }

    #[tokio::test]
    async fn supersession_is_ignored_and_explicit_cancel_owns_task_cancellation()
    -> anyhow::Result<()> {
        let wake = crate::backend::wayland::RuntimeWakeSource::new()?;
        let mut frozen = FrozenState::new_with_runtime_wake(None, wake.handle());
        let mut input = make_test_input_state();
        frozen.portal_task = Some(PortalTask::spawn(
            &tokio::runtime::Handle::current(),
            wake.handle(),
            std::future::pending(),
        ));
        frozen.portal_in_progress = true;

        frozen.capture_via_portal(&tokio::runtime::Handle::current())?;
        assert!(frozen.portal_task.is_some());
        frozen.cancel(&mut input);
        assert!(frozen.portal_task.is_none());
        assert!(!frozen.portal_in_progress);
        assert!(frozen.take_capture_done());
        Ok(())
    }
}
