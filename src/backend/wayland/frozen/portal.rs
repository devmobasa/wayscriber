use anyhow::Result;
use log::warn;
use std::sync::mpsc;

use crate::backend::wayland::frozen::FrozenImage;
use crate::backend::wayland::portal_capture::{
    capture_via_portal_fullscreen_bytes, crop_argb, portal_output_matches,
};
use crate::capture::sources::frozen::decode_image_to_argb;
use crate::input::InputState;

use super::PortalCaptureResult;
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

        self.portal_in_progress = true;
        self.portal_started_at = Some(std::time::Instant::now());

        let (tx, rx) = mpsc::channel::<PortalCaptureResult>();
        self.portal_rx = Some(rx);
        self.portal_target_output_id = self.active_output_id;

        let geo = self.active_geometry.clone();
        let target_output_id = self.active_output_id;
        // Notify user that portal fallback is in progress
        crate::notification::send_notification_async(
            tokio_handle,
            "Freezing screen".to_string(),
            "Taking screenshot via portal…".to_string(),
            Some("camera-photo".to_string()),
        );
        tokio_handle.spawn(async move {
            let result = async {
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
                        && let Some(cropped) = crop_argb(
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
                        width = phys_w;
                        height = phys_h;
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
            .await;

            let _ = tx.send(result);
        });

        Ok(())
    }

    /// Check for completed portal capture and apply result if present.
    pub fn poll_portal_capture(&mut self, input_state: &mut InputState) {
        if !self.portal_in_progress {
            return;
        }

        // Timeout safeguard to avoid overlay staying hidden forever
        if let Some(start) = self.portal_started_at
            && start.elapsed() > std::time::Duration::from_secs(10)
        {
            warn!("Portal frozen capture timed out; restoring overlay");
            input_state.set_frozen_active(false);
            self.portal_in_progress = false;
            self.portal_rx = None;
            self.portal_target_output_id = None;
            self.portal_started_at = None;
            self.capture_done = true;
            return;
        }

        if let Some(rx) = self.portal_rx.as_ref() {
            match rx.try_recv() {
                Ok(Ok((target_output, image))) => {
                    let output_matches =
                        portal_output_matches(target_output, self.active_output_id);

                    if output_matches {
                        self.image = Some(image);
                        input_state.set_frozen_active(true);
                        input_state.dirty_tracker.mark_full();
                        input_state.needs_redraw = true;
                    } else {
                        warn!("Portal capture for inactive output discarded");
                        input_state.set_frozen_active(false);
                    }

                    self.portal_in_progress = false;
                    self.portal_rx = None;
                    self.portal_target_output_id = None;
                    self.portal_started_at = None;
                    self.capture_done = true;
                }
                Ok(Err(err)) => {
                    warn!("Portal frozen capture failed: {}", err);
                    input_state.set_frozen_active(false);
                    self.portal_in_progress = false;
                    self.portal_rx = None;
                    self.portal_target_output_id = None;
                    self.portal_started_at = None;
                    self.capture_done = true;
                }
                Err(mpsc::TryRecvError::Empty) => {}
                Err(mpsc::TryRecvError::Disconnected) => {
                    warn!("Portal frozen capture channel disconnected");
                    input_state.set_frozen_active(false);
                    self.portal_in_progress = false;
                    self.portal_rx = None;
                    self.portal_target_output_id = None;
                    self.portal_started_at = None;
                    self.capture_done = true;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::wayland::frozen::FrozenState;

    #[test]
    fn poll_portal_applies_image() {
        let mut frozen = FrozenState::new(None);
        let mut input = InputState::with_defaults(
            crate::draw::color::RED,
            1.0,
            12.0,
            crate::input::EraserMode::Brush,
            0.32,
            false,
            12.0,
            crate::draw::FontDescriptor::default(),
            false,
            10.0,
            10.0,
            false,
            false,
            crate::config::BoardsConfig::default(),
            std::collections::HashMap::new(),
            usize::MAX,
            crate::input::ClickHighlightSettings::disabled(),
            0,
            0,
            true,
            0,
            0,
            5,
            5,
            crate::config::PresenterModeConfig::default(),
        );

        // Simulate an in-flight portal capture
        let (tx, rx) = mpsc::channel();
        frozen.portal_rx = Some(rx);
        frozen.portal_in_progress = true;
        tx.send(Ok((
            None,
            FrozenImage {
                width: 2,
                height: 1,
                stride: 8,
                data: vec![0, 0, 0, 0, 0, 0, 0, 0],
            },
        )))
        .unwrap();

        frozen.poll_portal_capture(&mut input);

        assert!(input.frozen_active());
        assert!(!frozen.portal_in_progress);
        assert!(frozen.portal_rx.is_none());
        assert!(frozen.image.is_some());
        assert!(frozen.take_capture_done());
    }
}
