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
use wayland_client::{Dispatch, QueueHandle, WEnum, protocol::{wl_output, wl_shm}};
use wayland_protocols_wlr::screencopy::v1::client::zwlr_screencopy_manager_v1::ZwlrScreencopyManagerV1;
use wayland_protocols_wlr::screencopy::v1::client::zwlr_screencopy_frame_v1::{Event as FrameEvent, Flags, ZwlrScreencopyFrameV1};

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
            debug!("Requesting screencopy copy");
            self.frame.copy(buffer.wl_buffer());
            self.copy_requested = true;
        }
    }
}

/// End-to-end controller for frozen mode capture and image storage.
pub struct FrozenState {
    manager: Option<ZwlrScreencopyManagerV1>,
    active_output: Option<wl_output::WlOutput>,
    capture: Option<CaptureSession>,
    image: Option<FrozenImage>,
    overlay_hidden: bool,
}

impl FrozenState {
    pub fn new(manager: Option<ZwlrScreencopyManagerV1>) -> Self {
        Self {
            manager,
            active_output: None,
            capture: None,
            image: None,
            overlay_hidden: false,
        }
    }

    pub fn set_manager(&mut self, manager: Option<ZwlrScreencopyManagerV1>) {
        self.manager = manager;
    }

    pub fn set_active_output(&mut self, output: Option<wl_output::WlOutput>) {
        self.active_output = output;
    }

    pub fn image(&self) -> Option<&FrozenImage> {
        self.image.as_ref()
    }

    pub fn clear_image(&mut self) {
        self.image = None;
    }

    pub fn is_active(&self) -> bool {
        self.image.is_some()
    }

    pub fn is_in_progress(&self) -> bool {
        self.capture.is_some()
    }

    /// Drop frozen image if the surface size no longer matches.
    pub fn handle_resize(&mut self, width: u32, height: u32, input_state: &mut InputState) {
        if let Some(img) = &self.image {
            if img.width != width || img.height != height {
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
    ) -> Result<()>
    where
        State: Dispatch<ZwlrScreencopyFrameV1, ()> + Dispatch<ZwlrScreencopyManagerV1, ()> + 'static,
    {
        if self.capture.is_some() {
            warn!("Frozen-mode capture already in progress; ignoring toggle");
            return Ok(());
        }

        let manager = match self.manager.clone() {
            Some(mgr) => mgr,
            None => {
                anyhow::bail!("zwlr_screencopy_manager_v1 not available");
            }
        };

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

    fn hide_overlay(&mut self, surface: &mut SurfaceState) {
        if self.overlay_hidden {
            return;
        }

        if let Some(layer_surface) = surface.layer_surface_mut() {
            layer_surface.set_size(0, 0);
            let wl_surface = layer_surface.wl_surface();
            wl_surface.commit();
            self.overlay_hidden = true;
        }
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
}
