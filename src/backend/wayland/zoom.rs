//! Zoom mode state and screencopy integration.
//!
//! Captures a one-off framebuffer (like frozen mode) and renders a magnified
//! view with pan/lock controls.

use anyhow::{Context, Result};
use log::{debug, info, warn};
use smithay_client_toolkit::{
    shell::WaylandSurface,
    shm::{
        Shm,
        slot::{Buffer, SlotPool},
    },
};
use std::sync::mpsc;
use wayland_client::{
    Dispatch, QueueHandle, WEnum,
    protocol::{wl_output, wl_shm},
};
use wayland_protocols_wlr::screencopy::v1::client::zwlr_screencopy_frame_v1::{
    Event as FrameEvent, Flags, ZwlrScreencopyFrameV1,
};
use wayland_protocols_wlr::screencopy::v1::client::zwlr_screencopy_manager_v1::ZwlrScreencopyManagerV1;

use crate::backend::wayland::frozen::FrozenImage;
use crate::backend::wayland::frozen_geometry::OutputGeometry;
use crate::capture::sources::frozen::decode_image_to_argb;
#[cfg(feature = "portal")]
use crate::capture::types::CaptureType;
use crate::input::InputState;

use super::surface::SurfaceState;

const MIN_ZOOM_SCALE: f64 = 1.0;
const MAX_ZOOM_SCALE: f64 = 8.0;

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
                debug!("Requesting zoom screencopy copy");
                self.frame.copy(buffer.wl_buffer());
                self.copy_requested = true;
            } else {
                debug!("Zoom screencopy copy requested before frame ready; skipping");
            }
        }
    }
}

type PortalCaptureResult = Result<(Option<u32>, FrozenImage), String>;
type PortalCaptureRx = mpsc::Receiver<PortalCaptureResult>;

/// Zoom state, capture logic, and pan/lock bookkeeping.
pub struct ZoomState {
    manager: Option<ZwlrScreencopyManagerV1>,
    active_output: Option<wl_output::WlOutput>,
    active_output_id: Option<u32>,
    active_geometry: Option<OutputGeometry>,
    capture: Option<CaptureSession>,
    image: Option<FrozenImage>,
    overlay_hidden: bool,
    portal_rx: Option<PortalCaptureRx>,
    portal_in_progress: bool,
    portal_target_output_id: Option<u32>,
    portal_started_at: Option<std::time::Instant>,
    pub active: bool,
    pub locked: bool,
    pub scale: f64,
    pub view_offset: (f64, f64),
    pub panning: bool,
    last_pan_pos: (f64, f64),
}

impl ZoomState {
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
            active: false,
            locked: false,
            scale: MIN_ZOOM_SCALE,
            view_offset: (0.0, 0.0),
            panning: false,
            last_pan_pos: (0.0, 0.0),
        }
    }

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
        self.active_output_id == Some(info_id)
    }

    pub fn image(&self) -> Option<&FrozenImage> {
        self.image.as_ref()
    }

    pub fn is_in_progress(&self) -> bool {
        self.capture.is_some() || self.portal_in_progress
    }

    pub fn activate(&mut self) {
        self.active = true;
    }

    pub fn deactivate(&mut self, surface: &mut SurfaceState, input_state: &mut InputState) {
        self.cancel(surface, input_state, true);
    }

    pub fn reset_view(&mut self) {
        self.scale = MIN_ZOOM_SCALE;
        self.view_offset = (0.0, 0.0);
        self.panning = false;
        self.last_pan_pos = (0.0, 0.0);
    }

    pub fn zoom_at_screen_point(
        &mut self,
        factor: f64,
        screen_x: f64,
        screen_y: f64,
        screen_width: u32,
        screen_height: u32,
    ) -> bool {
        let old_scale = self.scale;
        let mut new_scale = old_scale * factor;
        new_scale = new_scale.clamp(MIN_ZOOM_SCALE, MAX_ZOOM_SCALE);
        if (new_scale - old_scale).abs() < f64::EPSILON {
            return false;
        }
        let (world_x, world_y) = self.screen_to_world(screen_x, screen_y);
        self.scale = new_scale;
        self.view_offset.0 = world_x - (screen_x / new_scale);
        self.view_offset.1 = world_y - (screen_y / new_scale);
        self.clamp_offsets(screen_width, screen_height);
        true
    }

    pub fn screen_to_world(&self, screen_x: f64, screen_y: f64) -> (f64, f64) {
        (
            self.view_offset.0 + (screen_x / self.scale),
            self.view_offset.1 + (screen_y / self.scale),
        )
    }

    pub fn clamp_offsets(&mut self, screen_width: u32, screen_height: u32) {
        let width = screen_width as f64;
        let height = screen_height as f64;
        let visible_w = width / self.scale.max(MIN_ZOOM_SCALE);
        let visible_h = height / self.scale.max(MIN_ZOOM_SCALE);
        let max_x = (width - visible_w).max(0.0);
        let max_y = (height - visible_h).max(0.0);
        self.view_offset.0 = self.view_offset.0.clamp(0.0, max_x);
        self.view_offset.1 = self.view_offset.1.clamp(0.0, max_y);
    }

    pub fn start_pan(&mut self, screen_x: f64, screen_y: f64) {
        self.panning = true;
        self.last_pan_pos = (screen_x, screen_y);
    }

    pub fn stop_pan(&mut self) {
        self.panning = false;
    }

    pub fn pan_by_screen_delta(&mut self, dx: f64, dy: f64, screen_width: u32, screen_height: u32) {
        if self.scale <= MIN_ZOOM_SCALE {
            return;
        }
        self.view_offset.0 -= dx / self.scale;
        self.view_offset.1 -= dy / self.scale;
        self.clamp_offsets(screen_width, screen_height);
    }

    pub fn update_pan_position(&mut self, screen_x: f64, screen_y: f64) -> (f64, f64) {
        let (last_x, last_y) = self.last_pan_pos;
        self.last_pan_pos = (screen_x, screen_y);
        (screen_x - last_x, screen_y - last_y)
    }

    /// Drop zoom image if the surface size no longer matches.
    pub fn handle_resize(
        &mut self,
        phys_width: u32,
        phys_height: u32,
        surface: &mut SurfaceState,
        input_state: &mut InputState,
    ) {
        if let Some(img) = &self.image
            && (img.width != phys_width || img.height != phys_height)
        {
            info!("Surface resized; clearing zoom image");
            self.deactivate(surface, input_state);
        }
    }

    /// Start a screencopy capture for the active output.
    pub fn start_capture<State>(
        &mut self,
        shm: &Shm,
        surface: &mut SurfaceState,
        qh: &QueueHandle<State>,
        use_fallback: bool,
        tokio_handle: &tokio::runtime::Handle,
    ) -> Result<()>
    where
        State:
            Dispatch<ZwlrScreencopyFrameV1, ()> + Dispatch<ZwlrScreencopyManagerV1, ()> + 'static,
    {
        if self.capture.is_some() {
            warn!("Zoom capture already in progress; ignoring request");
            return Ok(());
        }

        if use_fallback || self.manager.is_none() {
            info!("Screencopy unavailable; using portal fallback for zoom capture");
            return self.capture_via_portal(surface, tokio_handle);
        }

        let manager = self
            .manager
            .clone()
            .context("zwlr_screencopy_manager_v1 not available")?;

        let output = match self.active_output.clone() {
            Some(out) => out,
            None => {
                anyhow::bail!("No active output available for zoom capture");
            }
        };

        self.hide_overlay(surface);

        debug!("Requesting screencopy frame for zoom");
        let frame = manager.capture_output(0, &output, qh, ());
        self.capture = Some(CaptureSession::new(frame));

        if let Some(capture) = self.capture.as_mut() {
            capture.pool = Some(SlotPool::new(4, shm).context("Failed to create zoom pool")?);
        }

        Ok(())
    }

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
                    warn!("Failed to prepare zoom buffer: {}", err);
                    self.cancel(surface, input_state, false);
                }
            }
            FrameEvent::LinuxDmabuf { .. } => {
                debug!("Ignoring linux-dmabuf event for zoom capture (SHM path only)");
            }
            FrameEvent::BufferDone => {
                if let Err(err) = self.on_buffer_done() {
                    warn!("Failed to issue zoom copy: {}", err);
                    self.cancel(surface, input_state, false);
                }
            }
            FrameEvent::Flags { flags } => {
                if let Some(capture) = self.capture.as_mut() {
                    let raw_flags = match flags {
                        WEnum::Value(v) => v.bits(),
                        WEnum::Unknown(raw) => raw,
                    };
                    capture.y_invert = Flags::from_bits(raw_flags)
                        .map(|f| f.contains(Flags::YInvert))
                        .unwrap_or(false);
                }
            }
            FrameEvent::Ready { .. } => {
                if let Err(err) = self.on_ready(input_state) {
                    warn!("Zoom capture ready handling failed: {}", err);
                    self.cancel(surface, input_state, false);
                    return;
                }

                self.restore_overlay(surface);
                input_state.set_zoom_status(self.active, self.locked, self.scale);
                input_state.dirty_tracker.mark_full();
                input_state.needs_redraw = true;
            }
            FrameEvent::Failed => {
                warn!("Zoom capture failed");
                self.cancel(surface, input_state, false);
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

        let pool = capture.pool.as_mut().context("Zoom pool missing")?;
        let total_size = (capture.stride as usize) * (height as usize);
        if total_size > pool.len() {
            pool.resize(total_size)?;
        }
        let (buffer, _) = pool
            .create_buffer(width as i32, height as i32, capture.stride, format)
            .context("Failed to create zoom buffer")?;
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

    fn on_ready(&mut self, _input_state: &mut InputState) -> Result<()> {
        let mut capture = self
            .capture
            .take()
            .context("No capture session present for ready event")?;

        let pool = capture.pool.as_mut().context("Zoom pool missing")?;
        let buffer = capture.buffer.as_ref().context("Zoom buffer missing")?;

        let canvas = buffer.canvas(pool).context("Unable to map zoom buffer")?;

        let pixel_width = (capture.width * 4) as usize;
        let stride = capture.stride as usize;
        if stride < pixel_width {
            anyhow::bail!("Zoom stride smaller than expected pixel width");
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

        Ok(())
    }

    pub fn cancel(
        &mut self,
        surface: &mut SurfaceState,
        input_state: &mut InputState,
        force_reset: bool,
    ) {
        if let Some(capture) = self.capture.take() {
            capture.frame.destroy();
        }
        self.restore_overlay(surface);
        self.portal_in_progress = false;
        self.portal_rx = None;
        self.portal_target_output_id = None;
        self.portal_started_at = None;

        if force_reset || self.image.is_none() {
            self.active = false;
            self.locked = false;
            self.reset_view();
            self.image = None;
        }

        input_state.set_zoom_status(self.active, self.locked, self.scale);
        input_state.dirty_tracker.mark_full();
        input_state.needs_redraw = true;
    }

    fn capture_via_portal(
        &mut self,
        surface: &mut SurfaceState,
        tokio_handle: &tokio::runtime::Handle,
    ) -> Result<()> {
        if self.portal_in_progress {
            warn!("Zoom portal capture already running; ignoring new request");
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

    pub fn poll_portal_capture(
        &mut self,
        surface: &mut SurfaceState,
        input_state: &mut InputState,
    ) {
        if !self.portal_in_progress {
            return;
        }

        if let Some(start) = self.portal_started_at
            && start.elapsed() > std::time::Duration::from_secs(10)
        {
            warn!("Portal zoom capture timed out; restoring overlay");
            self.restore_overlay(surface);
            self.portal_in_progress = false;
            self.portal_rx = None;
            self.portal_target_output_id = None;
            self.portal_started_at = None;
            if self.image.is_none() {
                self.active = false;
            }
            input_state.set_zoom_status(self.active, self.locked, self.scale);
            input_state.needs_redraw = true;
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

                    self.restore_overlay(surface);
                    self.portal_in_progress = false;
                    self.portal_rx = None;
                    self.portal_target_output_id = None;
                    self.portal_started_at = None;

                    if self.image.is_none() {
                        self.active = false;
                    }
                    input_state.set_zoom_status(self.active, self.locked, self.scale);
                    input_state.dirty_tracker.mark_full();
                    input_state.needs_redraw = true;
                }
                Ok(Err(err)) => {
                    warn!("Portal zoom capture failed: {}", err);
                    self.restore_overlay(surface);
                    self.portal_in_progress = false;
                    self.portal_rx = None;
                    self.portal_target_output_id = None;
                    self.portal_started_at = None;
                    if self.image.is_none() {
                        self.active = false;
                    }
                    input_state.set_zoom_status(self.active, self.locked, self.scale);
                    input_state.needs_redraw = true;
                }
                Err(mpsc::TryRecvError::Empty) => {}
                Err(mpsc::TryRecvError::Disconnected) => {
                    warn!("Portal zoom capture channel disconnected");
                    self.restore_overlay(surface);
                    self.portal_in_progress = false;
                    self.portal_rx = None;
                    self.portal_target_output_id = None;
                    self.portal_started_at = None;
                    if self.image.is_none() {
                        self.active = false;
                    }
                    input_state.set_zoom_status(self.active, self.locked, self.scale);
                    input_state.needs_redraw = true;
                }
            }
        }
    }

    fn hide_overlay(&mut self, surface: &mut SurfaceState) {
        if self.overlay_hidden {
            return;
        }

        if let Some(layer_surface) = surface.layer_surface_mut() {
            layer_surface.set_size(0, 0);
            let wl_surface = layer_surface.wl_surface();
            wl_surface.commit();
        } else if surface.is_xdg_window() {
            if let Some(wl_surface) = surface.wl_surface() {
                wl_surface.attach(None, 0, 0);
                wl_surface.commit();
            } else {
                warn!("xdg-shell surface missing wl_surface; cannot hide zoom overlay");
            }
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
        } else if surface.is_xdg_window() {
            debug!("xdg-shell zoom overlay will be restored on next render");
        }

        self.overlay_hidden = false;
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
