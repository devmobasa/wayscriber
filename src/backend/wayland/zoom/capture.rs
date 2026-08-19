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

use crate::backend::wayland::capture::CaptureLayoutContext;
use crate::backend::wayland::frozen::{FrozenImage, copy_shm_argb, validate_shm_buffer_layout};
use crate::backend::wayland::frozen_geometry::{OutputGeometry, require_verified_capture_source};
use crate::input::InputState;

use super::state::ZoomState;

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
    context: CaptureContext,
}

impl CaptureSession {
    fn new(frame: ZwlrScreencopyFrameV1, context: CaptureContext) -> Self {
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
            context,
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

struct CaptureContext {
    layout: CaptureLayoutContext,
    source_geometry: OutputGeometry,
}

impl CaptureContext {
    fn new(target_output_id: u32, source_geometry: OutputGeometry, layout_generation: u64) -> Self {
        Self {
            layout: CaptureLayoutContext::new(target_output_id, layout_generation),
            source_geometry,
        }
    }
}

fn finalize_capture_image(image: FrozenImage, context: &CaptureContext) -> Result<FrozenImage> {
    let image = image.with_output_transform(context.source_geometry.transform)?;
    if !context
        .source_geometry
        .accepts_transformed_pixel_size(image.width, image.height)
    {
        anyhow::bail!("Zoom capture dimensions do not match the active output");
    }
    let buffer_size = context.source_geometry.buffer_size();
    if !OutputGeometry::dimensions_have_compatible_aspect((image.width, image.height), buffer_size)
    {
        anyhow::bail!("Zoom capture aspect does not match the overlay surface");
    }
    Ok(image)
}

impl ZoomState {
    /// Start a screencopy capture for the active output.
    pub fn start_capture(
        &mut self,
        use_fallback: bool,
        _tokio_handle: &tokio::runtime::Handle,
    ) -> Result<()> {
        if self.capture.is_some() || self.portal_in_progress || self.preflight_pending {
            warn!("Zoom capture already in progress; ignoring request");
            return Ok(());
        }

        self.capture_done = false;
        self.preflight_use_fallback = use_fallback || self.manager.is_none();
        self.snapshot_preflight_layout();
        self.preflight_pending = true;
        Ok(())
    }

    pub fn begin_preflight_capture<State>(
        &mut self,
        use_fallback: bool,
        shm: &Shm,
        qh: &QueueHandle<State>,
        tokio_handle: &tokio::runtime::Handle,
    ) -> Result<()>
    where
        State:
            Dispatch<ZwlrScreencopyFrameV1, ()> + Dispatch<ZwlrScreencopyManagerV1, ()> + 'static,
    {
        self.ensure_preflight_layout_current()
            .map_err(anyhow::Error::msg)?;
        if use_fallback || self.manager.is_none() {
            info!("capture.preflight component=zoom phase=portal-start suppression_ready=true");
            self.capture_via_portal(tokio_handle)
        } else {
            self.begin_screencopy(shm, qh)
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
                anyhow::bail!("No active output available for zoom capture");
            }
        };
        let (source_geometry, target_output_id) = require_verified_capture_source(
            self.active_geometry.clone(),
            self.active_output_id,
            "zoom capture",
        )
        .map_err(anyhow::Error::msg)?;
        let context = CaptureContext::new(
            target_output_id,
            source_geometry,
            self.output_layout_generation,
        );

        info!(
            "capture.preflight component=zoom phase=screencopy-request output_id={:?}",
            self.active_output_id
        );
        let frame = manager.capture_output(0, &output, qh, ());
        self.capture = Some(CaptureSession::new(frame, context));

        if let Some(capture) = self.capture.as_mut() {
            capture.pool = Some(SlotPool::new(4, shm).context("Failed to create zoom pool")?);
        }

        Ok(())
    }

    pub fn handle_frame_event(&mut self, event: FrameEvent, input_state: &mut InputState) {
        match event {
            FrameEvent::Buffer {
                format,
                width,
                height,
                stride,
            } => {
                info!(
                    "capture.preflight component=zoom phase=screencopy-buffer width={width} height={height} stride={stride} format={format:?}"
                );
                if let Err(err) = self.on_buffer(format, width, height, stride) {
                    warn!("Failed to prepare zoom buffer: {}", err);
                    self.cancel(input_state, false);
                }
            }
            FrameEvent::LinuxDmabuf { .. } => {
                debug!("Ignoring linux-dmabuf event for zoom capture (SHM path only)");
            }
            FrameEvent::BufferDone => {
                if let Err(err) = self.on_buffer_done() {
                    warn!("Failed to issue zoom copy: {}", err);
                    self.cancel(input_state, false);
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
                match self.on_ready() {
                    Ok(true) => {}
                    Ok(false) => {
                        warn!("Zoom capture discarded after the output layout changed");
                        self.finish_stale_direct_capture(input_state);
                        return;
                    }
                    Err(err) => {
                        warn!("Zoom capture ready handling failed: {}", err);
                        self.cancel(input_state, false);
                        return;
                    }
                }

                if self.pending_activation {
                    self.active = true;
                    self.pending_activation = false;
                }
                input_state.set_zoom_status(self.active, self.locked, self.scale, self.view_offset);
                input_state.dirty_tracker.mark_full();
                input_state.needs_redraw = true;
                self.capture_done = true;
                if let Some(image) = self.image() {
                    info!(
                        "capture.preflight component=zoom phase=screencopy-ready width={} height={} active={} image_generation={}",
                        image.width,
                        image.height,
                        self.active,
                        self.image_generation()
                    );
                }
            }
            FrameEvent::Failed => {
                warn!("Zoom capture failed");
                self.cancel(input_state, false);
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

        let layout = validate_shm_buffer_layout(width, height, stride)?;
        capture.width = width;
        capture.height = height;
        capture.stride = layout.stride;
        capture.format = Some(format);

        let pool = capture.pool.as_mut().context("Zoom pool missing")?;
        if layout.total_size > pool.len() {
            pool.resize(layout.total_size)?;
        }
        let (buffer, _) = pool
            .create_buffer(layout.width, layout.height, layout.stride, format)
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

    fn on_ready(&mut self) -> Result<bool> {
        let mut capture = self
            .capture
            .take()
            .context("No capture session present for ready event")?;

        let result = (|| {
            let pool = capture.pool.as_mut().context("Zoom pool missing")?;
            let buffer = capture.buffer.as_ref().context("Zoom buffer missing")?;
            let canvas = buffer.canvas(pool).context("Unable to map zoom buffer")?;
            let format = capture.format.context("Zoom format missing")?;
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
        if !capture
            .context
            .layout
            .matches(self.active_output_id, self.output_layout_generation)
        {
            return Ok(false);
        }

        self.set_image(finalize_capture_image(image, &capture.context)?);

        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wayland_client::protocol::wl_output;

    #[test]
    fn finalized_capture_rejects_known_pixel_size_mismatch() {
        let geometry = OutputGeometry::update_from(
            Some((0, 0)),
            Some((3, 2)),
            (3, 2),
            2,
            wl_output::Transform::Normal,
            Some((5, 3)),
        )
        .expect("known output geometry");
        let context = CaptureContext::new(7, geometry, 3);
        let image = crate::backend::wayland::frozen::FrozenImage {
            width: 4,
            height: 3,
            stride: 16,
            data: vec![0; 4 * 3 * 4],
        };

        assert!(finalize_capture_image(image, &context).is_err());
    }

    #[test]
    fn finalized_capture_applies_transform_before_size_validation() {
        let geometry = OutputGeometry::update_from(
            Some((0, 0)),
            Some((2, 3)),
            (2, 3),
            1,
            wl_output::Transform::_270,
            Some((2, 3)),
        )
        .expect("rotated output geometry");
        let context = CaptureContext::new(7, geometry, 3);
        let image = crate::backend::wayland::frozen::FrozenImage {
            width: 3,
            height: 2,
            stride: 12,
            data: vec![0; 3 * 2 * 4],
        };

        let image = finalize_capture_image(image, &context).expect("transformed image");
        assert_eq!((image.width, image.height), (2, 3));
    }

    #[test]
    fn finalized_capture_rejects_stretching_into_a_different_viewport() {
        let geometry = OutputGeometry::update_from(
            Some((0, 0)),
            Some((3200, 1800)),
            (3200, 1760),
            1,
            wl_output::Transform::Normal,
            Some((3200, 1800)),
        )
        .expect("known output geometry");
        let context = CaptureContext::new(7, geometry, 3);
        let image = FrozenImage {
            width: 3200,
            height: 1800,
            stride: 12_800,
            data: vec![0; 3200 * 1800 * 4],
        };

        let error = match finalize_capture_image(image, &context) {
            Err(error) => error,
            Ok(_) => panic!("a full output cannot be stretched into a shorter viewport"),
        };
        assert!(error.to_string().contains("aspect does not match"));
    }

    #[tokio::test]
    async fn portal_capture_waits_for_suppression_preflight() {
        let wake = crate::backend::wayland::RuntimeWakeSource::new().expect("runtime wake");
        let mut state = ZoomState::new_with_runtime_wake(None, wake.handle());

        state
            .start_capture(false, &tokio::runtime::Handle::current())
            .expect("queue portal zoom capture");

        assert!(state.preflight_pending());
        assert!(!state.portal_in_progress);
        assert_eq!(state.take_preflight_pending(), Some(true));
    }

    #[test]
    fn preflight_layout_snapshot_goes_stale_when_geometry_changes() {
        let wake = crate::backend::wayland::RuntimeWakeSource::new().expect("runtime wake");
        let mut state = ZoomState::new_with_runtime_wake(None, wake.handle());
        state.snapshot_preflight_layout();
        assert!(state.preflight_layout_is_current());
        state.set_active_geometry(Some(
            OutputGeometry::update_from(
                Some((0, 0)),
                Some((1, 1)),
                (1, 1),
                1,
                wl_output::Transform::Normal,
                Some((1, 1)),
            )
            .expect("geometry"),
        ));
        assert!(!state.preflight_layout_is_current());
    }
}
