use anyhow::{Context, Result};
use log::{debug, info, warn};
use smithay_client_toolkit::shm::{
    Shm,
    slot::{Buffer, SlotPool},
};
use wayland_client::{Dispatch, QueueHandle, WEnum, protocol::wl_shm};
use wayland_protocols_wlr::screencopy::v1::client::zwlr_screencopy_frame_v1::{
    Event as FrameEvent, Flags, ZwlrScreencopyFrameV1,
};
use wayland_protocols_wlr::screencopy::v1::client::zwlr_screencopy_manager_v1::ZwlrScreencopyManagerV1;

use crate::backend::wayland::frozen::FrozenImage;
use crate::input::InputState;

use super::state::FrozenState;

/// Internal capture session tracking a single screencopy frame.
pub(super) struct CaptureSession {
    pub(super) frame: ZwlrScreencopyFrameV1,
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

impl FrozenState {
    /// Start a screencopy capture for the active output.
    pub fn start_capture(
        &mut self,
        use_fallback: bool,
        tokio_handle: &tokio::runtime::Handle,
    ) -> Result<()> {
        if self.capture.is_some() || self.portal_in_progress || self.preflight_pending {
            warn!("Frozen-mode capture already in progress; ignoring toggle");
            return Ok(());
        }

        self.capture_done = false;

        if use_fallback || self.manager.is_none() {
            info!("Screencopy unavailable; using fallback portal capture for frozen mode");
            return self.capture_via_portal(tokio_handle);
        }

        self.preflight_pending = true;
        Ok(())
    }

    pub fn begin_screencopy<State>(&mut self, shm: &Shm, qh: &QueueHandle<State>) -> Result<()>
    where
        State:
            Dispatch<ZwlrScreencopyFrameV1, ()> + Dispatch<ZwlrScreencopyManagerV1, ()> + 'static,
    {
        let manager = self
            .manager
            .clone()
            .context("zwlr_screencopy_manager_v1 not available")?;

        self.capture_done = false;

        let output = match self.active_output.clone() {
            Some(out) => out,
            None => {
                anyhow::bail!("No active output available for frozen capture");
            }
        };

        debug!("Requesting screencopy frame for active output");
        let frame = manager.capture_output(0, &output, qh, ());
        self.capture = Some(CaptureSession::new(frame));

        // Pre-allocate a pool to avoid repeated allocations; size adjusted on buffer event
        // (SlotPool resize is cheap, so start with minimal size).
        if let Some(capture) = self.capture.as_mut() {
            capture.pool =
                Some(SlotPool::new(4, shm).context("Failed to create frozen capture pool")?);
        }

        Ok(())
    }

    /// Handle screencopy frame events.
    pub fn handle_frame_event(&mut self, event: FrameEvent, input_state: &mut InputState) {
        match event {
            FrameEvent::Buffer {
                format,
                width,
                height,
                stride,
            } => {
                if let Err(err) = self.on_buffer(format, width, height, stride) {
                    warn!("Failed to prepare screencopy buffer: {}", err);
                    self.cancel(input_state);
                }
            }
            FrameEvent::LinuxDmabuf { .. } => {
                // Not yet supported; rely on wl_shm path
                debug!("Ignoring linux-dmabuf event for frozen capture (SHM path only)");
            }
            FrameEvent::BufferDone => {
                if let Err(err) = self.on_buffer_done() {
                    warn!("Failed to issue screencopy copy: {}", err);
                    self.cancel(input_state);
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
                if let Err(err) = self.on_ready() {
                    warn!("Frozen capture ready handling failed: {}", err);
                    self.cancel(input_state);
                    return;
                }

                input_state.set_frozen_active(true);
                input_state.dirty_tracker.mark_full();
                input_state.needs_redraw = true;
                self.capture_done = true;
            }
            FrameEvent::Failed => {
                warn!("Frozen capture failed");
                self.cancel(input_state);
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
        let pool = capture.pool.as_mut().context("Capture pool missing")?;
        let total_size = (capture.stride as usize) * (height as usize);
        if total_size > pool.len() {
            pool.resize(total_size)?;
        }
        let (buffer, _) = pool
            .create_buffer(width as i32, height as i32, capture.stride, format)
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

    fn on_ready(&mut self) -> Result<()> {
        let mut capture = self
            .capture
            .take()
            .context("No capture session present for ready event")?;

        let pool = capture.pool.as_mut().context("Capture pool missing")?;
        let buffer = capture.buffer.as_ref().context("Capture buffer missing")?;

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

        Ok(())
    }
}
