//! Frozen-mode state and wlroots screencopy integration.
//!
//! This module owns the lifecycle for capturing a one-off framebuffer via
//! `zwlr_screencopy_manager_v1` and storing it as a CPU-side image that the
//! renderer can paint beneath existing drawings.

use anyhow::{Context, Result};
use log::{debug, info, warn};
use smithay_client_toolkit::{
    shell::WaylandSurface,
    shm::{Shm, slot::{Buffer, SlotPool}},
};
use std::sync::mpsc;
use wayland_client::{Dispatch, QueueHandle, WEnum, protocol::{wl_output, wl_shm}};
use wayland_protocols_wlr::screencopy::v1::client::zwlr_screencopy_manager_v1::ZwlrScreencopyManagerV1;
use wayland_protocols_wlr::screencopy::v1::client::zwlr_screencopy_frame_v1::{Event as FrameEvent, Flags, ZwlrScreencopyFrameV1};

use crate::backend::wayland::frozen_geometry::OutputGeometry;
use crate::capture::{
    sources::{frozen::decode_image_to_argb, portal::capture_via_portal_bytes},
    types::CaptureType,
};
use crate::input::InputState;

use super::surface::SurfaceState;

/// CPU-side frozen image ready for Cairo rendering.
pub struct FrozenImage {
    pub width: u32,
    pub height: u32,
    pub stride: i32,
    pub data: Vec<u8>,
}

/// Internal capture session tracking a single screencopy frame.
struct CaptureSession {
    frame: ZwlrScreencopyFrameV1,
    pool: Option<SlotPool>,
    buffer: Option<Buffer>,
    width: u32,
    height: u32,
    stride: i32,
    format: Option<wl_shm::Format>,
    y_invert: bool,
    copy_requested: bool,
}

impl CaptureSession {
    fn new(frame: ZwlrScreencopyFrameV1) -> Self {
        Self {
            frame,
            pool: None,
            buffer: None,
            width: 0,
            height: 0,
            stride: 0,
            format: None,
            y_invert: false,
            copy_requested: false,
        }
    }

    fn ready_for_copy(&self) -> bool {
        self.format.is_some()
            && self.width > 0
            && self.height > 0
            && self.stride > 0
            && self.buffer.is_some()
    }

    fn request_copy(&mut self) {
        if self.copy_requested {
            return;
        }
        if let Some(buffer) = self.buffer.as_ref() {
            if self.ready_for_copy() {
                debug!("Requesting screencopy copy");
                self.frame.copy(buffer.wl_buffer());
                self.copy_requested = true;
            } else {
                debug!("Screencopy copy requested before frame ready; skipping");
            }
        }
    }
}

/// End-to-end controller for frozen mode capture and image storage.
pub struct FrozenState {
    manager: Option<ZwlrScreencopyManagerV1>,
    active_output: Option<wl_output::WlOutput>,
    active_output_id: Option<u32>,
    active_geometry: Option<OutputGeometry>,
    capture: Option<CaptureSession>,
    image: Option<FrozenImage>,
    overlay_hidden: bool,
    portal_rx: Option<mpsc::Receiver<Result<(Option<u32>, FrozenImage), String>>>,
    portal_in_progress: bool,
    portal_target_output_id: Option<u32>,
    portal_started_at: Option<std::time::Instant>,
}

impl FrozenState {
    pub fn new(manager: Option<ZwlrScreencopyManagerV1>) -> Self {
        Self {
            manager,
            active_output: None,
            active_output_id: None,
            active_geometry: None,
            capture: None,
            image: None,
            overlay_hidden: false,
            portal_rx: None,
            portal_in_progress: false,
            portal_target_output_id: None,
            portal_started_at: None,
        }
    }

    #[allow(dead_code)]
    pub fn manager_available(&self) -> bool {
        self.manager.is_some()
    }

    pub fn set_active_output(&mut self, output: Option<wl_output::WlOutput>, id: Option<u32>) {
        self.active_output = output;
        self.active_output_id = id;
    }

    pub fn set_active_geometry(&mut self, geometry: Option<OutputGeometry>) {
        self.active_geometry = geometry;
    }

    pub fn active_output_matches(&self, info_id: u32) -> bool {
        self.active_output_id.map_or(false, |id| id == info_id)
    }

    pub fn image(&self) -> Option<&FrozenImage> {
        self.image.as_ref()
    }

    pub fn is_in_progress(&self) -> bool {
        self.capture.is_some() || self.portal_in_progress
    }

    /// Drop frozen image if the surface size no longer matches.
    pub fn handle_resize(&mut self, phys_width: u32, phys_height: u32, input_state: &mut InputState) {
        if let Some(img) = &self.image {
            if img.width != phys_width || img.height != phys_height {
                info!("Surface resized; clearing frozen image");
                self.image = None;
                input_state.set_frozen_active(false);
            }
        }
    }

    /// Toggle unfreeze: drop the image and mark redraw.
    pub fn unfreeze(&mut self, input_state: &mut InputState) {
        self.image = None;
        input_state.set_frozen_active(false);
        input_state.dirty_tracker.mark_full();
        input_state.needs_redraw = true;
    }

    /// Start a screencopy capture for the active output.
    pub fn start_capture<State>(
        &mut self,
        shm: &Shm,
        surface: &mut SurfaceState,
        qh: &QueueHandle<State>,
        use_fallback: bool,
        _input_state: &mut InputState,
        tokio_handle: &tokio::runtime::Handle,
    ) -> Result<()>
    where
        State: Dispatch<ZwlrScreencopyFrameV1, ()> + Dispatch<ZwlrScreencopyManagerV1, ()> + 'static,
    {
        if self.capture.is_some() {
            warn!("Frozen-mode capture already in progress; ignoring toggle");
            return Ok(());
        }

        if use_fallback || self.manager.is_none() {
            info!("Screencopy unavailable; using fallback portal capture for frozen mode");
            return self.capture_via_portal(surface, tokio_handle);
        }

        let manager = self
            .manager
            .clone()
            .context("zwlr_screencopy_manager_v1 not available")?;

        let output = match self.active_output.clone() {
            Some(out) => out,
            None => {
                anyhow::bail!("No active output available for frozen capture");
            }
        };

        self.hide_overlay(surface);

        debug!("Requesting screencopy frame for active output");
        let frame = manager.capture_output(0, &output, qh, ());
        self.capture = Some(CaptureSession::new(frame));

        // Pre-allocate a pool to avoid repeated allocations; size adjusted on buffer event
        // (SlotPool resize is cheap, so start with minimal size).
        if let Some(capture) = self.capture.as_mut() {
            capture.pool = Some(SlotPool::new(4, shm).context("Failed to create frozen capture pool")?);
        }

        Ok(())
    }

    /// Handle screencopy frame events.
    pub fn handle_frame_event(
        &mut self,
        event: FrameEvent,
        surface: &mut SurfaceState,
        input_state: &mut InputState,
    ) {
        match event {
            FrameEvent::Buffer {
                format,
                width,
                height,
                stride,
            } => {
                if let Err(err) = self.on_buffer(format, width, height, stride) {
                    warn!("Failed to prepare screencopy buffer: {}", err);
                    self.cancel(surface, input_state);
                }
            }
            FrameEvent::LinuxDmabuf { .. } => {
                // Not yet supported; rely on wl_shm path
                debug!("Ignoring linux-dmabuf event for frozen capture (SHM path only)");
            }
            FrameEvent::BufferDone => {
                if let Err(err) = self.on_buffer_done() {
                    warn!("Failed to issue screencopy copy: {}", err);
                    self.cancel(surface, input_state);
                }
            }
            FrameEvent::Flags { flags } => {
                if let Some(capture) = self.capture.as_mut() {
                    let raw_flags = match flags {
                        WEnum::Value(v) => v.bits(),
                        WEnum::Unknown(raw) => raw,
                    };
                    capture.y_invert =
                        Flags::from_bits(raw_flags).map(|f| f.contains(Flags::YInvert)).unwrap_or(false);
                }
            }
            FrameEvent::Ready { .. } => {
                if let Err(err) = self.on_ready(input_state) {
                    warn!("Frozen capture ready handling failed: {}", err);
                    self.cancel(surface, input_state);
                    return;
                }

                self.restore_overlay(surface);
                input_state.set_frozen_active(true);
                input_state.dirty_tracker.mark_full();
                input_state.needs_redraw = true;
            }
            FrameEvent::Failed => {
                warn!("Frozen capture failed");
                self.cancel(surface, input_state);
            }
            _ => {}
        }
    }

    fn on_buffer(
        &mut self,
        format: WEnum<wl_shm::Format>,
        width: u32,
        height: u32,
        stride: u32,
    ) -> Result<()> {
        let capture = self
            .capture
            .as_mut()
            .context("No capture session present for buffer event")?;

        let format = match format {
            WEnum::Value(fmt) => fmt,
            WEnum::Unknown(raw) => {
                anyhow::bail!("Unknown wl_shm format {}", raw);
            }
        };

        capture.width = width;
        capture.height = height;
        capture.stride = stride as i32;
        capture.format = Some(format);

        // Resize pool and create buffer
        let pool = capture
            .pool
            .as_mut()
            .context("Capture pool missing")?;
        let total_size = (capture.stride as usize) * (height as usize);
        if total_size > pool.len() {
            pool.resize(total_size)?;
        }
        let (buffer, _) = pool
            .create_buffer(
                width as i32,
                height as i32,
                capture.stride,
                format,
            )
            .context("Failed to create capture buffer")?;
        capture.buffer = Some(buffer);
        capture.request_copy();
        Ok(())
    }

    fn on_buffer_done(&mut self) -> Result<()> {
        let capture = self
            .capture
            .as_mut()
            .context("No capture session present for buffer_done")?;
        capture.request_copy();
        Ok(())
    }

    fn on_ready(&mut self, input_state: &mut InputState) -> Result<()> {
        let mut capture = self
            .capture
            .take()
            .context("No capture session present for ready event")?;

        let pool = capture
            .pool
            .as_mut()
            .context("Capture pool missing")?;
        let buffer = capture
            .buffer
            .as_ref()
            .context("Capture buffer missing")?;

        let canvas = buffer
            .canvas(pool)
            .context("Unable to map capture buffer")?;

        let pixel_width = (capture.width * 4) as usize;
        let stride = capture.stride as usize;
        if stride < pixel_width {
            anyhow::bail!("Capture stride smaller than expected pixel width");
        }

        let mut data = vec![0u8; (capture.width * capture.height * 4) as usize];

        for row in 0..capture.height as usize {
            let src_row = &canvas[(row * stride)..(row * stride + pixel_width)];
            let dest_row_index = if capture.y_invert {
                (capture.height as usize - 1 - row) * pixel_width
            } else {
                row * pixel_width
            };
            data[dest_row_index..dest_row_index + pixel_width].copy_from_slice(src_row);
        }

        if matches!(capture.format, Some(wl_shm::Format::Xrgb8888)) {
            for chunk in data.chunks_exact_mut(4) {
                // Ensure alpha channel is opaque
                chunk[3] = 0xFF;
            }
        }

        capture.frame.destroy();

        self.image = Some(FrozenImage {
            width: capture.width,
            height: capture.height,
            stride: (capture.width * 4) as i32,
            data,
        });

        input_state.set_frozen_active(true);

        Ok(())
    }

    pub fn cancel(&mut self, surface: &mut SurfaceState, input_state: &mut InputState) {
        if let Some(capture) = self.capture.take() {
            capture.frame.destroy();
        }
        self.restore_overlay(surface);
        input_state.set_frozen_active(false);
    }

    fn capture_via_portal(
        &mut self,
        surface: &mut SurfaceState,
        tokio_handle: &tokio::runtime::Handle,
    ) -> Result<()> {
        if self.portal_in_progress {
            warn!("Portal capture already running; ignoring new request");
            return Ok(());
        }

        self.hide_overlay(surface);
        self.portal_in_progress = true;
        self.portal_started_at = Some(std::time::Instant::now());

        let (tx, rx) = mpsc::channel();
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
                let bytes = capture_via_portal_bytes(CaptureType::FullScreen)
                    .await
                    .map_err(|e| format!("Portal capture failed: {}", e))?;

                let (mut data, mut width, mut height) =
                    decode_image_to_argb(&bytes).map_err(|e| format!("Decode failed: {}", e))?;

                if let Some(geo) = geo {
                    let (phys_w, phys_h) = geo.physical_size();
                    let (origin_x, origin_y) = geo.physical_origin();
                    if origin_x >= 0 && origin_y >= 0 && phys_w > 0 && phys_h > 0 {
                        if let Some(cropped) = crop_argb(
                            &data,
                            width,
                            height,
                            origin_x as u32,
                            origin_y as u32,
                            phys_w,
                            phys_h,
                        ) {
                            data = cropped;
                            width = phys_w;
                            height = phys_h;
                        }
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

    fn hide_overlay(&mut self, surface: &mut SurfaceState) {
        if self.overlay_hidden {
            return;
        }

        if let Some(layer_surface) = surface.layer_surface_mut() {
            layer_surface.set_size(0, 0);
            let wl_surface = layer_surface.wl_surface();
            wl_surface.commit();
        }
        self.overlay_hidden = true;
    }

    fn restore_overlay(&mut self, surface: &mut SurfaceState) {
        if !self.overlay_hidden {
            return;
        }

        let width = surface.width();
        let height = surface.height();

        if let Some(layer_surface) = surface.layer_surface_mut() {
            layer_surface.set_size(width, height);
            let wl_surface = layer_surface.wl_surface();
            wl_surface.commit();
        }

        self.overlay_hidden = false;
    }

    /// Check for completed portal capture and apply result if present.
    pub fn poll_portal_capture(
        &mut self,
        surface: &mut SurfaceState,
        input_state: &mut InputState,
    ) {
        if !self.portal_in_progress {
            return;
        }

        // Timeout safeguard to avoid overlay staying hidden forever
        if let Some(start) = self.portal_started_at {
            if start.elapsed() > std::time::Duration::from_secs(10) {
                warn!("Portal frozen capture timed out; restoring overlay");
                self.restore_overlay(surface);
                input_state.set_frozen_active(false);
                self.portal_in_progress = false;
                self.portal_rx = None;
                self.portal_target_output_id = None;
                self.portal_started_at = None;
                return;
            }
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

                    self.restore_overlay(surface);
                    self.portal_in_progress = false;
                    self.portal_rx = None;
                    self.portal_target_output_id = None;
                    self.portal_started_at = None;
                }
                Ok(Err(err)) => {
                    warn!("Portal frozen capture failed: {}", err);
                    self.restore_overlay(surface);
                    input_state.set_frozen_active(false);
                    self.portal_in_progress = false;
                    self.portal_rx = None;
                    self.portal_target_output_id = None;
                    self.portal_started_at = None;
                }
                Err(mpsc::TryRecvError::Empty) => {}
                Err(mpsc::TryRecvError::Disconnected) => {
                    warn!("Portal frozen capture channel disconnected");
                    self.restore_overlay(surface);
                    input_state.set_frozen_active(false);
                    self.portal_in_progress = false;
                    self.portal_rx = None;
                    self.portal_target_output_id = None;
                    self.portal_started_at = None;
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
        let mut surface = SurfaceState::new();
        let mut input = InputState::with_defaults(
            crate::draw::color::RED,
            1.0,
            12.0,
            crate::draw::FontDescriptor::default(),
            false,
            10.0,
            10.0,
            false,
            crate::config::BoardConfig::default(),
            std::collections::HashMap::new(),
            usize::MAX,
            crate::input::ClickHighlightSettings::disabled(),
        );

        // Simulate an in-flight portal capture
        let (tx, rx) = mpsc::channel();
        frozen.portal_rx = Some(rx);
        frozen.portal_in_progress = true;
        tx.send(Ok((None, FrozenImage {
            width: 2,
            height: 1,
            stride: 8,
            data: vec![0, 0, 0, 0, 0, 0, 0, 0],
        })))
        .unwrap();

        frozen.hide_overlay(&mut surface);
        assert!(frozen.overlay_hidden);

        frozen.poll_portal_capture(&mut surface, &mut input);

        assert!(input.frozen_active());
        assert!(!frozen.overlay_hidden);
        assert!(!frozen.portal_in_progress);
        assert!(frozen.portal_rx.is_none());
        assert!(frozen.image.is_some());
    }
}
