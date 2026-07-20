use crate::input::state::{Toast, ToastPriority};
use anyhow::Result;
use log::warn;
use std::time::{Duration, Instant};

use crate::backend::wayland::frozen::FrozenImage;
use crate::backend::wayland::portal_capture::{
    capture_via_portal_fullscreen_bytes, portal_output_matches,
};
use crate::backend::wayland::portal_task::{PortalPoll, PortalTask};
use crate::capture::sources::frozen::decode_image_to_argb;
use crate::input::InputState;

use super::state::FrozenState;

impl FrozenState {
    pub(super) fn capture_via_portal(
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
        self.portal_in_progress = true;
        self.portal_target_output_id = self.active_output_id;

        let source_geometry = self.active_geometry.clone();
        let target_output_id = self.active_output_id;
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

                let (data, width, height) =
                    decode_image_to_argb(&bytes).map_err(|e| format!("Decode failed: {}", e))?;

                Ok((
                    target_output_id,
                    source_geometry,
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
            input_state.push_toast(
                ToastPriority::Critical,
                "freeze",
                Toast::error("Freeze timed out while waiting for screen capture."),
            );
            input_state.set_frozen_active(false);
            self.finish_portal_task();
            self.capture_done = true;
            return;
        }

        let poll = self
            .portal_task
            .as_mut()
            .map(PortalTask::poll)
            .unwrap_or(PortalPoll::Disconnected);
        match poll {
            PortalPoll::Ready(Ok((target_output, source_geometry, image))) => {
                let output_matches = portal_output_matches(target_output, self.active_output_id);

                if output_matches {
                    self.set_pending_desktop_image(image, source_geometry);
                } else {
                    warn!("Portal capture for inactive output discarded");
                    self.capture_done = true;
                }

                self.finish_portal_task();
            }
            PortalPoll::Ready(Err(err)) | PortalPoll::Failed(err) => {
                warn!("Portal frozen capture failed: {err}");
                input_state.push_toast(
                    ToastPriority::Critical,
                    "freeze",
                    Toast::error("Freeze could not capture the screen."),
                );
                input_state.set_frozen_active(false);
                self.finish_portal_task();
                self.capture_done = true;
            }
            PortalPoll::Pending => {}
            PortalPoll::Disconnected => {
                warn!("Portal frozen capture channel disconnected");
                input_state.push_toast(ToastPriority::Critical, "freeze", Toast::error("Freeze could not capture the screen because the system capture service stopped responding."));
                input_state.set_frozen_active(false);
                self.finish_portal_task();
                self.capture_done = true;
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

    async fn poll_until_finished(frozen: &mut FrozenState, input: &mut InputState) {
        for _ in 0..100 {
            frozen.poll_portal_capture(input, Instant::now());
            if !frozen.portal_in_progress {
                return;
            }
            tokio::task::yield_now().await;
        }
        panic!("frozen portal task did not finish");
    }

    #[tokio::test]
    async fn poll_portal_applies_image() {
        let wake = crate::backend::wayland::RuntimeWakeSource::new().unwrap();
        let mut frozen = FrozenState::new_with_runtime_wake(None, wake.handle());
        let mut input = make_test_input_state();

        frozen.portal_task = Some(PortalTask::spawn(
            &tokio::runtime::Handle::current(),
            wake.handle(),
            async { Ok((None, None, image(0))) },
        ));
        frozen.portal_in_progress = true;
        poll_until_finished(&mut frozen, &mut input).await;

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
    }

    #[tokio::test]
    async fn domain_error_and_task_panic_restore_the_frozen_lifecycle() {
        for panic_task in [false, true] {
            let wake = crate::backend::wayland::RuntimeWakeSource::new().unwrap();
            let mut frozen = FrozenState::new_with_runtime_wake(None, wake.handle());
            let mut input = make_test_input_state();
            frozen.portal_task = Some(if panic_task {
                PortalTask::spawn(&tokio::runtime::Handle::current(), wake.handle(), async {
                    panic!("expected frozen portal panic")
                })
            } else {
                PortalTask::spawn(&tokio::runtime::Handle::current(), wake.handle(), async {
                    Err("portal denied".to_string())
                })
            });
            frozen.portal_in_progress = true;

            poll_until_finished(&mut frozen, &mut input).await;

            assert!(!frozen.is_in_progress());
            assert!(frozen.portal_task.is_none());
            assert!(frozen.take_capture_done());
            assert!(!input.frozen_active());
        }
    }

    #[tokio::test]
    async fn disconnect_and_deadline_expiry_restore_without_a_producer_result() {
        let now = Instant::now();
        for timed_out in [false, true] {
            let wake = crate::backend::wayland::RuntimeWakeSource::new().unwrap();
            let mut frozen = FrozenState::new_with_runtime_wake(None, wake.handle());
            let mut input = make_test_input_state();
            frozen.portal_task = Some(if timed_out {
                PortalTask::spawn_at_for_test(
                    &tokio::runtime::Handle::current(),
                    wake.handle(),
                    now.checked_sub(PORTAL_CAPTURE_TIMEOUT).unwrap(),
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
    }

    #[tokio::test]
    async fn stale_output_is_discarded_without_mutating_current_frozen_state() {
        let wake = crate::backend::wayland::RuntimeWakeSource::new().unwrap();
        let mut frozen = FrozenState::new_with_runtime_wake(None, wake.handle());
        let mut input = make_test_input_state();
        input.set_frozen_active(true);
        frozen.set_active_output(None, Some(2));
        frozen.portal_task = Some(PortalTask::spawn(
            &tokio::runtime::Handle::current(),
            wake.handle(),
            async { Ok((Some(1), None, image(9))) },
        ));
        frozen.portal_in_progress = true;

        poll_until_finished(&mut frozen, &mut input).await;

        assert!(input.frozen_active());
        assert!(!frozen.has_pending_image());
        assert!(frozen.take_capture_done());
    }

    #[tokio::test]
    async fn supersession_is_ignored_and_explicit_cancel_owns_task_cancellation() {
        let wake = crate::backend::wayland::RuntimeWakeSource::new().unwrap();
        let mut frozen = FrozenState::new_with_runtime_wake(None, wake.handle());
        let mut input = make_test_input_state();
        frozen.portal_task = Some(PortalTask::spawn(
            &tokio::runtime::Handle::current(),
            wake.handle(),
            std::future::pending(),
        ));
        frozen.portal_in_progress = true;

        frozen
            .capture_via_portal(&tokio::runtime::Handle::current())
            .unwrap();
        assert!(frozen.portal_task.is_some());
        frozen.cancel(&mut input);
        assert!(frozen.portal_task.is_none());
        assert!(!frozen.portal_in_progress);
        assert!(frozen.take_capture_done());
    }
}
