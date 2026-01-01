use anyhow::Result;
use log::warn;
use std::sync::mpsc;

use crate::backend::wayland::frozen::FrozenImage;
use crate::capture::sources::frozen::decode_image_to_argb;
#[cfg(feature = "portal")]
use crate::capture::types::CaptureType;
use crate::input::InputState;

use super::PortalCaptureResult;
use super::state::FrozenState;

#[cfg(feature = "portal")]
async fn portal_capture_bytes() -> Result<Vec<u8>, String> {
    use crate::capture::sources::portal::capture_via_portal_bytes;
    capture_via_portal_bytes(CaptureType::FullScreen)
        .await
        .map_err(|e| format!("Portal capture failed: {}", e))
}

#[cfg(not(feature = "portal"))]
async fn portal_capture_bytes() -> Result<Vec<u8>, String> {
    Err("Portal capture is disabled (feature flag)".to_string())
}

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
                let bytes = portal_capture_bytes().await?;

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
                    let output_matches = match (target_output, self.active_output_id) {
                        (Some(t), Some(current)) => t == current,
                        (None, None) => true,
                        (None, Some(_)) => true,
                        (Some(_), None) => false,
                    };

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

fn crop_argb(
    data: &[u8],
    width: u32,
    height: u32,
    x: u32,
    y: u32,
    crop_w: u32,
    crop_h: u32,
) -> Option<Vec<u8>> {
    if x >= width || y >= height {
        return None;
    }
    let max_w = width.saturating_sub(x);
    let max_h = height.saturating_sub(y);
    let cw = crop_w.min(max_w);
    let ch = crop_h.min(max_h);

    let mut out = vec![0u8; (cw * ch * 4) as usize];
    let src_stride = (width * 4) as usize;
    let dst_stride = (cw * 4) as usize;
    for row in 0..ch as usize {
        let src_offset = ((y as usize + row) * src_stride) + (x as usize * 4);
        let dst_offset = row * dst_stride;
        let end = src_offset + dst_stride;
        if end > data.len() || dst_offset + dst_stride > out.len() {
            return None;
        }
        out[dst_offset..dst_offset + dst_stride]
            .copy_from_slice(&data[src_offset..src_offset + dst_stride]);
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::wayland::frozen::FrozenState;

    #[test]
    fn crop_argb_respects_bounds() {
        // 2x2 image with distinct pixels: row-major BGRA
        let data = vec![
            1, 2, 3, 4, 5, 6, 7, 8, //
            9, 10, 11, 12, 13, 14, 15, 16,
        ];
        let cropped = crop_argb(&data, 2, 2, 1, 0, 1, 2).expect("crop");
        assert_eq!(cropped, vec![5, 6, 7, 8, 13, 14, 15, 16]);
    }

    #[test]
    fn crop_argb_returns_none_when_out_of_bounds() {
        // x beyond width
        assert!(crop_argb(&[0u8; 4], 1, 1, 2, 0, 1, 1).is_none());
        // y beyond height
        assert!(crop_argb(&[0u8; 4], 1, 1, 0, 2, 1, 1).is_none());
    }

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
            crate::config::BoardConfig::default(),
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
