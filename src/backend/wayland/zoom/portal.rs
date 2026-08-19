use anyhow::Result;
use log::warn;
use std::time::{Duration, Instant};

use crate::backend::wayland::frozen::FrozenImage;
use crate::backend::wayland::frozen_geometry::{OutputGeometry, require_verified_capture_source};
use crate::backend::wayland::portal_capture::{
    capture_via_portal_fullscreen_bytes, crop_argb, portal_output_matches,
};
use crate::backend::wayland::portal_task::{PortalPoll, PortalTask};
use crate::capture::sources::frozen::decode_image_to_argb;
use crate::capture::types::CaptureError;
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
        let (geo, target_output_id) = require_verified_capture_source(
            self.active_geometry.clone(),
            self.active_output_id,
            "portal zoom capture",
        )
        .map_err(anyhow::Error::msg)?;
        self.portal_in_progress = true;
        self.portal_target_output_id = Some(target_output_id);

        let layout_generation = self.output_layout_generation;
        crate::notification::send_notification_async(
            tokio_handle,
            "Zoom capture".to_string(),
            "Requesting screen capture...".to_string(),
            Some("camera-photo".to_string()),
        );
        self.portal_task = Some(PortalTask::spawn(tokio_handle, runtime_wake, async move {
            async {
                let bytes = capture_via_portal_fullscreen_bytes().await?;

                let (data, width, height) = decode_image_to_argb(&bytes)
                    .map_err(|error| CaptureError::ImageError(format!("Decode failed: {error}")))?;
                let image = crop_portal_image(data, width, height, &geo)?;

                Ok((Some(target_output_id), layout_generation, image))
            }
            .await
        }));

        Ok(())
    }

    pub fn poll_portal_capture(
        &mut self,
        input_state: &mut InputState,
        now: Instant,
        live_output_count: Option<u32>,
    ) {
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
            PortalPoll::Ready(Ok((target_output, layout_generation, image))) => {
                let output_matches = portal_output_matches(target_output, self.active_output_id);
                let layout_matches = layout_generation == self.output_layout_generation;

                if output_matches && layout_matches {
                    // Crop used the spawn-time geometry moved into the task.
                    // A processed topology change updates `known_output_count`,
                    // so `OutputGeometry`'s equality bumps
                    // `output_layout_generation` and `layout_matches` drops the
                    // result. This live-count check covers the SCTK window
                    // where a new `wl_output` is already in `OutputState`
                    // before `new_output` refreshes `active_geometry`. Freeze
                    // instead revalidates the pending snapshot, because it
                    // crops on the Wayland thread at activate.
                    if self.active_geometry.as_ref().is_some_and(|geometry| {
                        geometry.output_count_conflicts_with_live(live_output_count)
                    }) {
                        warn!("Portal zoom capture discarded after output topology changed");
                        Self::push_stale_layout_toast(input_state);
                        self.finish_failed_portal_task(input_state);
                        return;
                    }
                    self.set_image(image);
                } else {
                    if !layout_matches {
                        warn!("Portal zoom capture discarded after the output layout changed");
                    } else {
                        warn!("Portal zoom capture for inactive output discarded");
                    }
                    Self::push_stale_layout_toast(input_state);
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
            PortalPoll::Ready(Err(CaptureError::Cancelled(reason))) => {
                log::info!("Portal zoom capture cancelled: {reason}");
                self.finish_failed_portal_task(input_state);
            }
            PortalPoll::Ready(Err(err)) => {
                warn!("Portal zoom capture failed: {err}");
                self.finish_failed_portal_task(input_state);
            }
            PortalPoll::Failed(err) => {
                warn!("Portal zoom capture task failed: {err}");
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

fn crop_portal_image(
    data: Vec<u8>,
    width: u32,
    height: u32,
    geometry: &OutputGeometry,
) -> Result<FrozenImage, CaptureError> {
    let expected_len = u64::from(width)
        .checked_mul(u64::from(height))
        .and_then(|pixels| pixels.checked_mul(4))
        .and_then(|bytes| usize::try_from(bytes).ok())
        .ok_or_else(|| CaptureError::ImageError("Zoom capture is too large".to_string()))?;
    if data.len() != expected_len {
        return Err(CaptureError::ImageError(
            "Zoom capture buffer length does not match its dimensions".to_string(),
        ));
    }

    let (phys_w, phys_h) = geometry.verified_pixel_size().ok_or_else(|| {
        CaptureError::ImageError("Zoom capture output dimensions are invalid".to_string())
    })?;
    let buffer_size = geometry.buffer_size();
    if !OutputGeometry::dimensions_have_compatible_aspect((phys_w, phys_h), buffer_size) {
        return Err(CaptureError::ImageError(
            "Zoom capture aspect does not match the overlay surface".to_string(),
        ));
    }
    let (origin_x, origin_y) = geometry.portal_crop_origin(width, height).ok_or_else(|| {
        CaptureError::ImageError("Zoom capture does not match the active output layout".to_string())
    })?;
    let (cropped_w, cropped_h, cropped) =
        crop_argb(&data, width, height, origin_x, origin_y, phys_w, phys_h).ok_or_else(|| {
            CaptureError::ImageError("Zoom capture does not contain the active output".to_string())
        })?;
    if cropped_w != phys_w || cropped_h != phys_h {
        return Err(CaptureError::ImageError(
            "Zoom capture does not contain the active output".to_string(),
        ));
    }
    let stride = i32::try_from(
        cropped_w
            .checked_mul(4)
            .ok_or_else(|| CaptureError::ImageError("Zoom capture stride overflow".to_string()))?,
    )
    .map_err(|_| CaptureError::ImageError("Zoom capture stride is too large".to_string()))?;

    Ok(FrozenImage {
        width: cropped_w,
        height: cropped_h,
        stride,
        data: cropped,
    })
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

    #[tokio::test]
    async fn portal_start_requires_verifiable_geometry_and_output_identity() -> anyhow::Result<()> {
        let wake = crate::backend::wayland::RuntimeWakeSource::new()?;
        let mut zoom = ZoomState::new_with_runtime_wake(None, wake.handle());

        let error = zoom
            .capture_via_portal(&tokio::runtime::Handle::current())
            .expect_err("missing geometry must fail closed");
        assert!(error.to_string().contains("geometry is unavailable"));
        assert!(!zoom.portal_in_progress);
        assert!(zoom.portal_task.is_none());

        zoom.set_active_geometry(Some(crop_geometry((0, 0))));
        let error = zoom
            .capture_via_portal(&tokio::runtime::Handle::current())
            .expect_err("missing output identity must fail closed");
        assert!(error.to_string().contains("identity is unavailable"));
        assert!(!zoom.portal_in_progress);
        assert!(zoom.portal_task.is_none());
        Ok(())
    }

    #[test]
    fn portal_crop_uses_fractional_output_pixels_not_the_overlay_buffer_size() {
        let geometry = OutputGeometry::update_from(
            Some((0, 0)),
            Some((3, 2)),
            (3, 2),
            2,
            wayland_client::protocol::wl_output::Transform::Normal,
            Some((5, 3)),
        )
        .expect("fractional geometry")
        .with_desktop_backdrop_geometry(Some(crate::capture::DesktopBackdropGeometry {
            logical_x: 0,
            logical_y: 0,
            logical_width: 3,
            logical_height: 2,
            physical_width: Some(5),
            physical_height: Some(3),
            crop_x: Some(0),
            crop_y: Some(0),
            screenshot_width: Some(5),
            screenshot_height: Some(3),
        }));

        let image = crop_portal_image(vec![7; 5 * 3 * 4], 5, 3, &geometry)
            .expect("fractional output screenshot");

        assert_eq!((image.width, image.height), (5, 3));
        assert_eq!(image.stride, 20);
    }

    #[test]
    fn portal_crop_rejects_a_different_screenshot_layout() {
        let geometry = crop_geometry((0, 0)).with_desktop_backdrop_geometry(Some(
            crate::capture::DesktopBackdropGeometry {
                logical_x: 0,
                logical_y: 0,
                logical_width: 2,
                logical_height: 1,
                physical_width: Some(2),
                physical_height: Some(1),
                crop_x: Some(0),
                crop_y: Some(0),
                screenshot_width: Some(2),
                screenshot_height: Some(1),
            },
        ));

        assert!(crop_portal_image(vec![0; 3 * 4], 3, 1, &geometry).is_err());
    }

    async fn poll_until_finished(zoom: &mut ZoomState, input: &mut InputState) {
        poll_until_finished_with_live_outputs(zoom, input, None).await;
    }

    async fn poll_until_finished_with_live_outputs(
        zoom: &mut ZoomState,
        input: &mut InputState,
        live_output_count: Option<u32>,
    ) {
        for _ in 0..100 {
            zoom.poll_portal_capture(input, Instant::now(), live_output_count);
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
            async { Ok((None, 0, image(3))) },
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
                    Err(CaptureError::PermissionDenied)
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

            zoom.poll_portal_capture(&mut input, now, None);

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
            async { Ok((Some(1), 0, image(9))) },
        ));
        zoom.portal_in_progress = true;

        poll_until_finished(&mut zoom, &mut input).await;

        assert!(!zoom.active);
        assert!(!zoom.pending_activation);
        assert_eq!(zoom.image_generation(), generation);
        assert_eq!(zoom.image().unwrap().data, vec![4; 8]);
        assert!(zoom.take_capture_done());
        assert!(input.ui_toast.is_some());
    }

    #[tokio::test]
    async fn stale_layout_preserves_the_current_zoom_image_and_activation() {
        let wake = crate::backend::wayland::RuntimeWakeSource::new().unwrap();
        let mut zoom = ZoomState::new_with_runtime_wake(None, wake.handle());
        let mut input = make_test_input_state();
        zoom.set_image(image(4));
        let generation = zoom.image_generation();
        zoom.set_active_geometry(Some(crop_geometry((0, 0))));
        let layout_generation = zoom.output_layout_generation;
        zoom.request_activation();
        zoom.portal_task = Some(PortalTask::spawn(
            &tokio::runtime::Handle::current(),
            wake.handle(),
            async move { Ok((None, layout_generation, image(9))) },
        ));
        zoom.portal_in_progress = true;
        zoom.set_active_geometry(Some(crop_geometry((6, 0))));

        poll_until_finished(&mut zoom, &mut input).await;

        assert!(!zoom.active);
        assert!(!zoom.pending_activation);
        assert_eq!(zoom.image_generation(), generation);
        assert_eq!(zoom.image().unwrap().data, vec![4; 8]);
        assert!(zoom.take_capture_done());
        assert!(input.ui_toast.is_some());
    }

    #[tokio::test]
    async fn matching_layout_still_applies_after_an_identical_geometry_refresh() {
        let wake = crate::backend::wayland::RuntimeWakeSource::new().unwrap();
        let mut zoom = ZoomState::new_with_runtime_wake(None, wake.handle());
        let mut input = make_test_input_state();
        let geometry = crop_geometry((0, 0));
        zoom.set_active_geometry(Some(geometry.clone()));
        let layout_generation = zoom.output_layout_generation;
        zoom.request_activation();
        zoom.portal_task = Some(PortalTask::spawn(
            &tokio::runtime::Handle::current(),
            wake.handle(),
            async move { Ok((None, layout_generation, image(3))) },
        ));
        zoom.portal_in_progress = true;
        zoom.set_active_geometry(Some(geometry));

        poll_until_finished(&mut zoom, &mut input).await;

        assert!(zoom.active);
        assert!(!zoom.pending_activation);
        assert_eq!(zoom.image().unwrap().data, vec![3; 8]);
        assert!(zoom.take_capture_done());
    }

    #[tokio::test]
    async fn stale_live_output_count_discards_a_single_output_portal_image() {
        let wake = crate::backend::wayland::RuntimeWakeSource::new().unwrap();
        let mut zoom = ZoomState::new_with_runtime_wake(None, wake.handle());
        let mut input = make_test_input_state();
        zoom.set_image(image(4));
        let generation = zoom.image_generation();
        zoom.set_active_geometry(Some(crop_geometry((0, 0)).with_known_output_count(Some(1))));
        let layout_generation = zoom.output_layout_generation;
        zoom.request_activation();
        zoom.portal_task = Some(PortalTask::spawn(
            &tokio::runtime::Handle::current(),
            wake.handle(),
            async move { Ok((None, layout_generation, image(9))) },
        ));
        zoom.portal_in_progress = true;

        poll_until_finished_with_live_outputs(&mut zoom, &mut input, Some(2)).await;

        assert!(!zoom.active);
        assert!(!zoom.pending_activation);
        assert_eq!(zoom.image_generation(), generation);
        assert_eq!(zoom.image().unwrap().data, vec![4; 8]);
        assert!(zoom.take_capture_done());
        assert!(input.ui_toast.is_some());
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
