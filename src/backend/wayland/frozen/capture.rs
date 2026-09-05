use anyhow::{Context, Result};
use log::{debug, info, warn};
use smithay_client_toolkit::shm::{
    Shm,
    slot::{Buffer, SlotPool},
};
use wayland_client::{Dispatch, QueueHandle, WEnum, protocol::wl_shm};
use wayland_protocols::ext::{
    image_capture_source::v1::client::{
        ext_image_capture_source_v1::ExtImageCaptureSourceV1,
        ext_output_image_capture_source_manager_v1::ExtOutputImageCaptureSourceManagerV1,
    },
    image_copy_capture::v1::client::{
        ext_image_copy_capture_frame_v1::ExtImageCopyCaptureFrameV1,
        ext_image_copy_capture_manager_v1::ExtImageCopyCaptureManagerV1,
        ext_image_copy_capture_session_v1::ExtImageCopyCaptureSessionV1,
    },
};
use wayland_protocols_wlr::screencopy::v1::client::zwlr_screencopy_frame_v1::{
    Event as FrameEvent, Flags, ZwlrScreencopyFrameV1,
};
use wayland_protocols_wlr::screencopy::v1::client::zwlr_screencopy_manager_v1::ZwlrScreencopyManagerV1;

use crate::backend::wayland::acquisition::ScreenAcquisitionOutcome;
use crate::backend::wayland::capture::CaptureLayoutContext;
use crate::backend::wayland::frozen_geometry::require_verified_capture_source;
use crate::input::InputState;

use super::image::{copy_shm_argb, validate_shm_buffer_layout};
use super::state::{DirectCaptureAttempt, DirectCaptureContext, FrozenCaptureBackend, FrozenState};

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
    pub fn start_capture(&mut self) -> Result<()> {
        if self.direct_capture.is_some() || self.portal_in_progress || self.preflight.is_pending() {
            warn!("Frozen-mode capture already in progress; ignoring toggle");
            return Ok(());
        }

        self.capture_done = false;
        let backend = self
            .preferred_backend()
            .context("no frozen capture backend is available")?;
        self.preflight.begin(
            backend,
            self.active_output_id,
            self.output_layout_generation,
        );
        Ok(())
    }

    pub fn begin_preflight_capture<State>(
        &mut self,
        backend: FrozenCaptureBackend,
        shm: &Shm,
        qh: &QueueHandle<State>,
        tokio_handle: &tokio::runtime::Handle,
    ) -> Result<()>
    where
        State: Dispatch<ZwlrScreencopyFrameV1, ()>
            + Dispatch<ZwlrScreencopyManagerV1, ()>
            + Dispatch<ExtImageCaptureSourceV1, ()>
            + Dispatch<ExtImageCopyCaptureFrameV1, ()>
            + Dispatch<ExtImageCopyCaptureManagerV1, ()>
            + Dispatch<ExtImageCopyCaptureSessionV1, ()>
            + Dispatch<ExtOutputImageCaptureSourceManagerV1, ()>
            + 'static,
    {
        self.begin_capture_chain(backend, shm, qh, tokio_handle)
    }

    pub(in crate::backend::wayland) fn begin_fallback_capture<State>(
        &mut self,
        failed_backend: FrozenCaptureBackend,
        shm: &Shm,
        qh: &QueueHandle<State>,
        tokio_handle: &tokio::runtime::Handle,
    ) -> Result<()>
    where
        State: Dispatch<ZwlrScreencopyFrameV1, ()>
            + Dispatch<ZwlrScreencopyManagerV1, ()>
            + Dispatch<ExtImageCaptureSourceV1, ()>
            + Dispatch<ExtImageCopyCaptureFrameV1, ()>
            + Dispatch<ExtImageCopyCaptureManagerV1, ()>
            + Dispatch<ExtImageCopyCaptureSessionV1, ()>
            + Dispatch<ExtOutputImageCaptureSourceManagerV1, ()>
            + 'static,
    {
        let backend = self
            .next_backend_after(failed_backend)
            .context("no remaining frozen capture backend is available")?;
        self.begin_capture_chain(backend, shm, qh, tokio_handle)
    }

    fn begin_capture_chain<State>(
        &mut self,
        first_backend: FrozenCaptureBackend,
        shm: &Shm,
        qh: &QueueHandle<State>,
        tokio_handle: &tokio::runtime::Handle,
    ) -> Result<()>
    where
        State: Dispatch<ZwlrScreencopyFrameV1, ()>
            + Dispatch<ZwlrScreencopyManagerV1, ()>
            + Dispatch<ExtImageCaptureSourceV1, ()>
            + Dispatch<ExtImageCopyCaptureFrameV1, ()>
            + Dispatch<ExtImageCopyCaptureManagerV1, ()>
            + Dispatch<ExtImageCopyCaptureSessionV1, ()>
            + Dispatch<ExtOutputImageCaptureSourceManagerV1, ()>
            + 'static,
    {
        self.ensure_preflight_layout_current()
            .map_err(anyhow::Error::msg)?;
        let mut backend = Some(first_backend);
        let mut last_error = None;

        while let Some(current) = backend {
            match self.begin_capture_backend(current, shm, qh, tokio_handle) {
                Ok(()) => return Ok(()),
                Err(error) => {
                    warn!("Failed to start {current:?} frozen capture: {error:#}");
                    last_error = Some(error);
                    backend = self.next_backend_after(current);
                }
            }
        }

        Err(last_error
            .unwrap_or_else(|| anyhow::anyhow!("no frozen capture backend was attempted")))
    }

    fn begin_capture_backend<State>(
        &mut self,
        backend: FrozenCaptureBackend,
        shm: &Shm,
        qh: &QueueHandle<State>,
        tokio_handle: &tokio::runtime::Handle,
    ) -> Result<()>
    where
        State: Dispatch<ZwlrScreencopyFrameV1, ()>
            + Dispatch<ZwlrScreencopyManagerV1, ()>
            + Dispatch<ExtImageCaptureSourceV1, ()>
            + Dispatch<ExtImageCopyCaptureFrameV1, ()>
            + Dispatch<ExtImageCopyCaptureManagerV1, ()>
            + Dispatch<ExtImageCopyCaptureSessionV1, ()>
            + Dispatch<ExtOutputImageCaptureSourceManagerV1, ()>
            + 'static,
    {
        match backend {
            FrozenCaptureBackend::WlrScreencopy => {
                info!("Suppression frame committed; using wlr-screencopy for frozen mode");
                self.begin_screencopy(shm, qh)
            }
            FrozenCaptureBackend::ExtImageCopy => {
                info!("Suppression frame committed; using ext-image-copy for frozen mode");
                self.begin_ext_image_copy(shm, qh)
            }
            FrozenCaptureBackend::Portal => {
                info!("Suppression frame committed; using portal capture for frozen mode");
                self.capture_via_portal(tokio_handle)
            }
        }
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
        let (source_geometry, target_output_id) = require_verified_capture_source(
            self.active_geometry.clone(),
            self.active_output_id,
            "frozen capture",
        )
        .map_err(anyhow::Error::msg)?;

        let pool = SlotPool::new(4, shm).context("Failed to create frozen capture pool")?;
        debug!("Requesting screencopy frame for active output");
        let frame = manager.capture_output(0, &output, qh, ());
        let mut capture = CaptureSession::new(frame);
        capture.pool = Some(pool);
        self.direct_capture = Some(DirectCaptureAttempt::WlrScreencopy {
            session: Box::new(capture),
            context: DirectCaptureContext::new(
                CaptureLayoutContext::new(target_output_id, self.output_layout_generation),
                source_geometry,
            ),
        });

        Ok(())
    }

    /// Handle screencopy frame events.
    pub fn handle_frame_event(&mut self, event: FrameEvent, input_state: &mut InputState) -> bool {
        if !matches!(
            self.direct_capture.as_ref(),
            Some(DirectCaptureAttempt::WlrScreencopy { .. })
        ) {
            debug!("Ignoring screencopy frame event without an active WLR frozen capture");
            return false;
        }

        match event {
            FrameEvent::Buffer {
                format,
                width,
                height,
                stride,
            } => {
                if let Err(err) = self.on_buffer(format, width, height, stride) {
                    warn!("Failed to prepare screencopy buffer: {}", err);
                    return self.fail_wlr_capture();
                }
            }
            FrameEvent::LinuxDmabuf { .. } => {
                // Not yet supported; rely on wl_shm path
                debug!("Ignoring linux-dmabuf event for frozen capture (SHM path only)");
            }
            FrameEvent::BufferDone => {
                if let Err(err) = self.on_buffer_done() {
                    warn!("Failed to issue screencopy copy: {}", err);
                    return self.fail_wlr_capture();
                }
            }
            FrameEvent::Flags { flags } => {
                if let Some(DirectCaptureAttempt::WlrScreencopy {
                    session: capture, ..
                }) = self.direct_capture.as_mut()
                {
                    let raw_flags = match flags {
                        WEnum::Value(v) => v.bits(),
                        WEnum::Unknown(raw) => raw,
                    };
                    capture.y_invert = Flags::from_bits(raw_flags)
                        .map(|f| f.contains(Flags::YInvert))
                        .unwrap_or(false);
                }
            }
            FrameEvent::Ready { .. } => match self.on_ready() {
                Ok(true) => input_state.needs_redraw = true,
                Ok(false) => {
                    warn!("Discarded WLR frozen capture because the active output changed");
                    self.finish_stale_direct_capture(input_state);
                }
                Err(err) => {
                    warn!("Frozen capture ready handling failed: {}", err);
                    // `on_ready` owns and destroys the completed WLR attempt,
                    // including error paths, so only the fallback decision
                    // remains here.
                    return true;
                }
            },
            FrameEvent::Failed => {
                warn!("Frozen capture failed");
                return self.fail_wlr_capture();
            }
            _ => {}
        }
        false
    }

    fn fail_wlr_capture(&mut self) -> bool {
        let Some(capture) = self.direct_capture.take() else {
            return false;
        };
        match capture {
            DirectCaptureAttempt::WlrScreencopy { session, .. } => {
                session.frame.destroy();
                true
            }
            capture @ DirectCaptureAttempt::ExtImageCopy { .. } => {
                self.direct_capture = Some(capture);
                false
            }
        }
    }

    fn on_buffer(
        &mut self,
        format: WEnum<wl_shm::Format>,
        width: u32,
        height: u32,
        stride: u32,
    ) -> Result<()> {
        let capture = match self.direct_capture.as_mut() {
            Some(DirectCaptureAttempt::WlrScreencopy { session, .. }) => session,
            _ => anyhow::bail!("No WLR capture session present for buffer event"),
        };

        let format = match format {
            WEnum::Value(fmt) => fmt,
            WEnum::Unknown(raw) => {
                anyhow::bail!("Unknown wl_shm format {}", raw);
            }
        };

        let layout = validate_shm_buffer_layout(width, height, stride)?;
        capture.width = width;
        capture.height = height;
        capture.stride = layout.stride;
        capture.format = Some(format);

        // Resize pool and create buffer
        let pool = capture.pool.as_mut().context("Capture pool missing")?;
        if layout.total_size > pool.len() {
            pool.resize(layout.total_size)?;
        }
        let (buffer, _) = pool
            .create_buffer(layout.width, layout.height, layout.stride, format)
            .context("Failed to create capture buffer")?;
        capture.buffer = Some(buffer);
        capture.request_copy();
        Ok(())
    }

    fn on_buffer_done(&mut self) -> Result<()> {
        let capture = match self.direct_capture.as_mut() {
            Some(DirectCaptureAttempt::WlrScreencopy { session, .. }) => session,
            _ => anyhow::bail!("No WLR capture session present for buffer_done"),
        };
        capture.request_copy();
        Ok(())
    }

    fn on_ready(&mut self) -> Result<bool> {
        let attempt = self
            .direct_capture
            .take()
            .context("No WLR capture attempt present for ready event")?;
        let (mut capture, context) = match attempt {
            DirectCaptureAttempt::WlrScreencopy { session, context } => (session, context),
            attempt @ DirectCaptureAttempt::ExtImageCopy { .. } => {
                self.direct_capture = Some(attempt);
                anyhow::bail!("No WLR capture attempt present for ready event");
            }
        };
        let result = (|| {
            let pool = capture.pool.as_mut().context("Capture pool missing")?;
            let buffer = capture.buffer.as_ref().context("Capture buffer missing")?;
            let canvas = buffer
                .canvas(pool)
                .context("Unable to map capture buffer")?;
            let format = capture.format.context("Capture format missing")?;
            copy_shm_argb(
                canvas,
                capture.width,
                capture.height,
                capture.stride,
                format,
                capture.y_invert,
            )
        })();
        capture.frame.destroy();
        let image = result?;
        if !context
            .layout
            .matches(self.active_output_id, self.output_layout_generation)
        {
            return Ok(false);
        }

        self.set_pending_output_image(
            image,
            context.layout.target_output_id(),
            context.source_geometry,
        );

        Ok(true)
    }

    pub(super) fn finish_stale_direct_capture(&mut self, input_state: &mut InputState) {
        if self.has_acquisition_attempt() {
            self.finish_acquisition(ScreenAcquisitionOutcome::StaleLayout, input_state);
            return;
        }
        self.capture_done = true;
        input_state.set_frozen_active(false);
        input_state.needs_redraw = true;
        Self::push_stale_layout_toast(input_state);
    }
}
