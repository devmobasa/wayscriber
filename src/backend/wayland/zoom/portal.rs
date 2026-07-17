use anyhow::Result;
use log::warn;
use std::time::{Duration, Instant};

use crate::backend::wayland::frozen::FrozenImage;
use crate::backend::wayland::portal_capture::{
    capture_via_portal_fullscreen_bytes, crop_argb, portal_output_matches,
};
use crate::backend::wayland::portal_task::{PortalPoll, PortalTask};
use crate::capture::sources::frozen::decode_image_to_argb;
use crate::input::InputState;

use super::state::ZoomState;

impl ZoomState {
    pub(super) fn capture_via_portal(
        &mut self,
        tokio_handle: &tokio::runtime::Handle,
    ) -> Result<()> {
        if self.portal_in_progress {
            warn!("Zoom portal capture already running; ignoring new request");
            return Ok(());
        }

        let runtime_wake = self
            .runtime_wake
            .clone()
            .ok_or_else(|| anyhow::anyhow!("portal capture runtime wake is unavailable"))?;
        self.portal_in_progress = true;
        self.portal_target_output_id = self.active_output_id;

        let geo = self.active_geometry.clone();
        let target_output_id = self.active_output_id;
        crate::notification::send_notification_async(
            tokio_handle,
            "Zoom capture".to_string(),
            "Requesting screen capture...".to_string(),
            Some("camera-photo".to_string()),
        );
        self.portal_task = Some(PortalTask::spawn(tokio_handle, runtime_wake, async move {
            async {
                let bytes = capture_via_portal_fullscreen_bytes().await?;

                let (mut data, mut width, mut height) =
                    decode_image_to_argb(&bytes).map_err(|e| format!("Decode failed: {}", e))?;

                if let Some(geo) = geo {
                    let (phys_w, phys_h) = geo.physical_size();
                    let (origin_x, origin_y) = geo.physical_origin();
                    if origin_x >= 0
                        && origin_y >= 0
                        && phys_w > 0
                        && phys_h > 0
                        && let Some((cropped_w, cropped_h, cropped)) = crop_argb(
                            &data,
                            width,
                            height,
                            origin_x as u32,
                            origin_y as u32,
                            phys_w,
                            phys_h,
                        )
                    {
                        data = cropped;
                        width = cropped_w;
                        height = cropped_h;
                    }
                }

                Ok((
                    target_output_id,
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

    pub fn poll_portal_capture(&mut self, input_state: &mut InputState, now: Instant) {
        if !self.portal_in_progress {
            return;
        }

        if self
            .portal_task
            .as_ref()
            .is_some_and(|task| task.timed_out(now))
        {
            warn!("Portal zoom capture timed out; restoring overlay");
            self.finish_portal_task();
            if self.image.is_none() {
                self.active = false;
            }
            self.pending_activation = false;
            input_state.set_zoom_status(self.active, self.locked, self.scale, self.view_offset);
            input_state.needs_redraw = true;
            self.capture_done = true;
            return;
        }

        let poll = self
            .portal_task
            .as_mut()
            .map(PortalTask::poll)
            .unwrap_or(PortalPoll::Disconnected);
        match poll {
            PortalPoll::Ready(Ok((target_output, image))) => {
                let output_matches = portal_output_matches(target_output, self.active_output_id);

                if output_matches {
                    self.set_image(image);
                } else {
                    warn!("Portal zoom capture for inactive output discarded");
                    self.finish_failed_portal_task(input_state);
                    return;
                }

                self.finish_portal_task();

                if self.pending_activation && self.image.is_some() {
                    self.active = true;
                }
                if self.image.is_none() {
                    self.active = false;
                }
                self.pending_activation = false;
                input_state.set_zoom_status(self.active, self.locked, self.scale, self.view_offset);
                input_state.dirty_tracker.mark_full();
                input_state.needs_redraw = true;
                self.capture_done = true;
            }
            PortalPoll::Ready(Err(err)) | PortalPoll::Failed(err) => {
                warn!("Portal zoom capture failed: {err}");
                self.finish_failed_portal_task(input_state);
            }
            PortalPoll::Pending => {}
            PortalPoll::Disconnected => {
                warn!("Portal zoom capture channel disconnected");
                self.finish_failed_portal_task(input_state);
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

    fn finish_failed_portal_task(&mut self, input_state: &mut InputState) {
        self.finish_portal_task();
        if self.image.is_none() {
            self.active = false;
        }
        self.pending_activation = false;
        input_state.set_zoom_status(self.active, self.locked, self.scale, self.view_offset);
        input_state.needs_redraw = true;
        self.capture_done = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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

    async fn poll_until_finished(zoom: &mut ZoomState, input: &mut InputState) {
        for _ in 0..100 {
            zoom.poll_portal_capture(input, Instant::now());
            if !zoom.portal_in_progress {
                return;
            }
            tokio::task::yield_now().await;
        }
        panic!("zoom portal task did not finish");
    }

    #[tokio::test]
    async fn success_activates_zoom_with_the_matching_image() {
        let wake = crate::backend::wayland::RuntimeWakeSource::new().unwrap();
        let mut zoom = ZoomState::new_with_runtime_wake(None, wake.handle());
        let mut input = make_test_input_state();
        zoom.request_activation();
        zoom.portal_task = Some(PortalTask::spawn(
            &tokio::runtime::Handle::current(),
            wake.handle(),
            async { Ok((None, image(3))) },
        ));
        zoom.portal_in_progress = true;

        poll_until_finished(&mut zoom, &mut input).await;

        assert!(zoom.active);
        assert!(!zoom.pending_activation);
        assert_eq!(zoom.image().unwrap().data, vec![3; 8]);
        assert!(zoom.take_capture_done());
    }

    #[tokio::test]
    async fn domain_error_and_task_panic_restore_the_zoom_lifecycle() {
        for panic_task in [false, true] {
            let wake = crate::backend::wayland::RuntimeWakeSource::new().unwrap();
            let mut zoom = ZoomState::new_with_runtime_wake(None, wake.handle());
            let mut input = make_test_input_state();
            zoom.request_activation();
            zoom.portal_task = Some(if panic_task {
                PortalTask::spawn(&tokio::runtime::Handle::current(), wake.handle(), async {
                    panic!("expected zoom portal panic")
                })
            } else {
                PortalTask::spawn(&tokio::runtime::Handle::current(), wake.handle(), async {
                    Err("portal denied".to_string())
                })
            });
            zoom.portal_in_progress = true;

            poll_until_finished(&mut zoom, &mut input).await;

            assert!(!zoom.is_in_progress());
            assert!(!zoom.active);
            assert!(!zoom.pending_activation);
            assert!(zoom.portal_task.is_none());
            assert!(zoom.take_capture_done());
        }
    }

    #[tokio::test]
    async fn disconnect_and_deadline_expiry_restore_without_a_producer_result() {
        let now = Instant::now();
        for timed_out in [false, true] {
            let wake = crate::backend::wayland::RuntimeWakeSource::new().unwrap();
            let mut zoom = ZoomState::new_with_runtime_wake(None, wake.handle());
            let mut input = make_test_input_state();
            zoom.request_activation();
            zoom.portal_task = Some(if timed_out {
                PortalTask::spawn_at_for_test(
                    &tokio::runtime::Handle::current(),
                    wake.handle(),
                    now.checked_sub(PORTAL_CAPTURE_TIMEOUT).unwrap(),
                    std::future::pending(),
                )
            } else {
                PortalTask::disconnected_for_test(now)
            });
            zoom.portal_in_progress = true;

            zoom.poll_portal_capture(&mut input, now);

            assert!(!zoom.is_in_progress());
            assert!(!zoom.active);
            assert!(!zoom.pending_activation);
            assert!(zoom.portal_task.is_none());
            assert!(zoom.take_capture_done());
        }
    }

    #[tokio::test]
    async fn stale_output_preserves_the_current_zoom_image_and_activation() {
        let wake = crate::backend::wayland::RuntimeWakeSource::new().unwrap();
        let mut zoom = ZoomState::new_with_runtime_wake(None, wake.handle());
        let mut input = make_test_input_state();
        zoom.set_image(image(4));
        let generation = zoom.image_generation();
        zoom.set_active_output(None, Some(2));
        zoom.request_activation();
        zoom.portal_task = Some(PortalTask::spawn(
            &tokio::runtime::Handle::current(),
            wake.handle(),
            async { Ok((Some(1), image(9))) },
        ));
        zoom.portal_in_progress = true;

        poll_until_finished(&mut zoom, &mut input).await;

        assert!(!zoom.active);
        assert!(!zoom.pending_activation);
        assert_eq!(zoom.image_generation(), generation);
        assert_eq!(zoom.image().unwrap().data, vec![4; 8]);
        assert!(zoom.take_capture_done());
    }

    #[tokio::test]
    async fn supersession_is_ignored_and_explicit_abort_owns_task_cancellation() {
        let wake = crate::backend::wayland::RuntimeWakeSource::new().unwrap();
        let mut zoom = ZoomState::new_with_runtime_wake(None, wake.handle());
        zoom.request_activation();
        zoom.portal_task = Some(PortalTask::spawn(
            &tokio::runtime::Handle::current(),
            wake.handle(),
            std::future::pending(),
        ));
        zoom.portal_in_progress = true;

        zoom.capture_via_portal(&tokio::runtime::Handle::current())
            .unwrap();
        assert!(zoom.portal_task.is_some());
        assert!(zoom.abort_capture());
        assert!(zoom.portal_task.is_none());
        assert!(!zoom.portal_in_progress);
    }
}
