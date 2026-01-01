use anyhow::Result;
use log::warn;
use std::sync::mpsc;

use crate::backend::wayland::frozen::FrozenImage;
use crate::capture::sources::frozen::decode_image_to_argb;
#[cfg(feature = "portal")]
use crate::capture::types::CaptureType;
use crate::input::InputState;

use super::PortalCaptureResult;
use super::state::ZoomState;

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

impl ZoomState {
    pub(super) fn capture_via_portal(
        &mut self,
        tokio_handle: &tokio::runtime::Handle,
    ) -> Result<()> {
        if self.portal_in_progress {
            warn!("Zoom portal capture already running; ignoring new request");
            return Ok(());
        }

        self.portal_in_progress = true;
        self.portal_started_at = Some(std::time::Instant::now());

        let (tx, rx) = mpsc::channel::<PortalCaptureResult>();
        self.portal_rx = Some(rx);
        self.portal_target_output_id = self.active_output_id;

        let geo = self.active_geometry.clone();
        let target_output_id = self.active_output_id;
        crate::notification::send_notification_async(
            tokio_handle,
            "Zoom capture".to_string(),
            "Taking screenshot via portal...".to_string(),
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

    pub fn poll_portal_capture(&mut self, input_state: &mut InputState) {
        if !self.portal_in_progress {
            return;
        }

        if let Some(start) = self.portal_started_at
            && start.elapsed() > std::time::Duration::from_secs(10)
        {
            warn!("Portal zoom capture timed out; restoring overlay");
            self.portal_in_progress = false;
            self.portal_rx = None;
            self.portal_target_output_id = None;
            self.portal_started_at = None;
            if self.image.is_none() {
                self.active = false;
            }
            self.pending_activation = false;
            input_state.set_zoom_status(self.active, self.locked, self.scale);
            input_state.needs_redraw = true;
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
                    } else {
                        warn!("Portal zoom capture for inactive output discarded");
                    }

                    self.portal_in_progress = false;
                    self.portal_rx = None;
                    self.portal_target_output_id = None;
                    self.portal_started_at = None;

                    if self.pending_activation && self.image.is_some() {
                        self.active = true;
                    }
                    if self.image.is_none() {
                        self.active = false;
                    }
                    self.pending_activation = false;
                    input_state.set_zoom_status(self.active, self.locked, self.scale);
                    input_state.dirty_tracker.mark_full();
                    input_state.needs_redraw = true;
                    self.capture_done = true;
                }
                Ok(Err(err)) => {
                    warn!("Portal zoom capture failed: {}", err);
                    self.portal_in_progress = false;
                    self.portal_rx = None;
                    self.portal_target_output_id = None;
                    self.portal_started_at = None;
                    if self.image.is_none() {
                        self.active = false;
                    }
                    self.pending_activation = false;
                    input_state.set_zoom_status(self.active, self.locked, self.scale);
                    input_state.needs_redraw = true;
                    self.capture_done = true;
                }
                Err(mpsc::TryRecvError::Empty) => {}
                Err(mpsc::TryRecvError::Disconnected) => {
                    warn!("Portal zoom capture channel disconnected");
                    self.portal_in_progress = false;
                    self.portal_rx = None;
                    self.portal_target_output_id = None;
                    self.portal_started_at = None;
                    if self.image.is_none() {
                        self.active = false;
                    }
                    self.pending_activation = false;
                    input_state.set_zoom_status(self.active, self.locked, self.scale);
                    input_state.needs_redraw = true;
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
