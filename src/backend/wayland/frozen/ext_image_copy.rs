use anyhow::{Context, Result};
use log::{debug, warn};
use smithay_client_toolkit::shm::{
    Shm,
    slot::{Buffer, SlotPool},
};
use wayland_client::{
    Dispatch, QueueHandle, WEnum,
    protocol::{wl_output, wl_shm},
};
use wayland_protocols::ext::{
    image_capture_source::v1::client::{
        ext_image_capture_source_v1::ExtImageCaptureSourceV1,
        ext_output_image_capture_source_manager_v1::ExtOutputImageCaptureSourceManagerV1,
    },
    image_copy_capture::v1::client::{
        ext_image_copy_capture_frame_v1::{
            Event as FrameEvent, ExtImageCopyCaptureFrameV1, FailureReason,
        },
        ext_image_copy_capture_manager_v1::{ExtImageCopyCaptureManagerV1, Options},
        ext_image_copy_capture_session_v1::{Event as SessionEvent, ExtImageCopyCaptureSessionV1},
    },
};

use crate::input::InputState;

use super::image::copy_shm_argb;
use super::state::{DirectCaptureBackend, DirectCaptureContext, FrozenState};

const MAX_CONSTRAINT_RETRIES: u8 = 2;

#[derive(Clone)]
pub(in crate::backend::wayland) struct ExtImageCopyManagers {
    capture: ExtImageCopyCaptureManagerV1,
    output_source: ExtOutputImageCaptureSourceManagerV1,
}

impl ExtImageCopyManagers {
    pub(in crate::backend::wayland) fn new(
        capture: ExtImageCopyCaptureManagerV1,
        output_source: ExtOutputImageCaptureSourceManagerV1,
    ) -> Self {
        Self {
            capture,
            output_source,
        }
    }
}

pub(super) struct ExtImageCopySession {
    source: ExtImageCaptureSourceV1,
    session: ExtImageCopyCaptureSessionV1,
    pool: SlotPool,
    constraints: ConstraintTracker,
    frame: Option<ExtImageCopyFrame>,
}

struct ExtImageCopyFrame {
    proxy: ExtImageCopyCaptureFrameV1,
    buffer: Buffer,
    width: u32,
    height: u32,
    stride: i32,
    format: wl_shm::Format,
    transform: Option<wl_output::Transform>,
}

impl ExtImageCopySession {
    fn new(
        source: ExtImageCaptureSourceV1,
        session: ExtImageCopyCaptureSessionV1,
        pool: SlotPool,
    ) -> Self {
        Self {
            source,
            session,
            pool,
            constraints: ConstraintTracker::default(),
            frame: None,
        }
    }

    pub(super) fn destroy(mut self) {
        if let Some(frame) = self.frame.take() {
            frame.proxy.destroy();
        }
        self.session.destroy();
        self.source.destroy();
    }
}

#[derive(Default)]
struct ConstraintTracker {
    pending: ExtBufferConstraints,
    replacement: Option<ExtBufferConstraints>,
    retries: u8,
}

impl ConstraintTracker {
    fn record_size(&mut self, width: u32, height: u32) {
        self.pending.size = Some((width, height));
    }

    fn record_format(&mut self, format: WEnum<wl_shm::Format>) {
        if let WEnum::Value(format) = format
            && !self.pending.formats.contains(&format)
        {
            self.pending.formats.push(format);
        }
    }

    fn finish_batch(&mut self, frame_pending: bool) -> Option<ExtBufferConstraints> {
        let batch = std::mem::take(&mut self.pending);
        if frame_pending {
            self.replacement = Some(batch);
            None
        } else {
            Some(batch)
        }
    }

    fn take_replacement_for_retry(&mut self) -> Result<ExtBufferConstraints> {
        if self.retries >= MAX_CONSTRAINT_RETRIES {
            anyhow::bail!(
                "ext-image-copy exceeded its {MAX_CONSTRAINT_RETRIES} buffer-constraint retries"
            );
        }
        self.retries += 1;
        self.replacement.take().context(
            "compositor reported a buffer-constraints failure without a complete replacement batch",
        )
    }
}

#[derive(Default)]
struct ExtBufferConstraints {
    size: Option<(u32, u32)>,
    formats: Vec<wl_shm::Format>,
}

impl FrozenState {
    pub(super) fn begin_ext_image_copy<State>(
        &mut self,
        shm: &Shm,
        qh: &QueueHandle<State>,
    ) -> Result<()>
    where
        State: Dispatch<ExtImageCaptureSourceV1, ()>
            + Dispatch<ExtImageCopyCaptureManagerV1, ()>
            + Dispatch<ExtImageCopyCaptureSessionV1, ()>
            + Dispatch<ExtImageCopyCaptureFrameV1, ()>
            + Dispatch<ExtOutputImageCaptureSourceManagerV1, ()>
            + 'static,
    {
        let managers = self
            .ext_managers
            .as_ref()
            .context("ext-image-copy-capture managers are unavailable")?;
        let output = self
            .active_output
            .as_ref()
            .context("No active output available for ext-image-copy capture")?;
        let target_output_id = self
            .active_output_id
            .context("Active output has no stable identity for ext-image-copy capture")?;
        let source_geometry = self.active_geometry.clone();

        // Allocate the only fallible local resource before creating protocol
        // objects so an allocation failure cannot leave a live source/session.
        let pool =
            SlotPool::new(4, shm).context("Failed to create ext-image-copy shared-memory pool")?;
        let source = managers.output_source.create_source(output, qh, ());
        let session = managers
            .capture
            .create_session(&source, Options::empty(), qh, ());
        self.ext_capture = Some(ExtImageCopySession::new(source, session, pool));
        self.direct_capture = Some(DirectCaptureContext::new(
            DirectCaptureBackend::ExtImageCopy,
            target_output_id,
            source_geometry,
        ));
        debug!("Requested ext-image-copy capture constraints for active output");
        Ok(())
    }

    pub(in crate::backend::wayland) fn handle_ext_session_event<State>(
        &mut self,
        event: SessionEvent,
        qh: &QueueHandle<State>,
    ) -> bool
    where
        State: Dispatch<ExtImageCopyCaptureFrameV1, ()> + 'static,
    {
        let result = match event {
            SessionEvent::BufferSize { width, height } => {
                let capture = self
                    .ext_capture
                    .as_mut()
                    .context("ext-image-copy capture session missing");
                capture.map(|capture| capture.constraints.record_size(width, height))
            }
            SessionEvent::ShmFormat { format } => {
                let capture = self
                    .ext_capture
                    .as_mut()
                    .context("ext-image-copy capture session missing");
                capture.map(|capture| capture.constraints.record_format(format))
            }
            SessionEvent::Done => self.finish_constraint_batch(qh),
            SessionEvent::Stopped => Err(anyhow::anyhow!(
                "ext-image-copy capture session was stopped by the compositor"
            )),
            SessionEvent::DmabufDevice { .. } | SessionEvent::DmabufFormat { .. } => Ok(()),
            _ => Ok(()),
        };

        if let Err(error) = result {
            warn!("Ext-image-copy session failed: {error:#}");
            return self.fail_ext_capture();
        }
        false
    }

    fn finish_constraint_batch<State>(&mut self, qh: &QueueHandle<State>) -> Result<()>
    where
        State: Dispatch<ExtImageCopyCaptureFrameV1, ()> + 'static,
    {
        let constraints = {
            let capture = self
                .ext_capture
                .as_mut()
                .context("ext-image-copy capture session missing")?;
            capture.constraints.finish_batch(capture.frame.is_some())
        };
        if let Some(constraints) = constraints {
            self.submit_ext_frame(qh, constraints)?;
        } else {
            debug!("Deferred replacement ext-image-copy constraints until the current frame ends");
        }
        Ok(())
    }

    fn submit_ext_frame<State>(
        &mut self,
        qh: &QueueHandle<State>,
        constraints: ExtBufferConstraints,
    ) -> Result<()>
    where
        State: Dispatch<ExtImageCopyCaptureFrameV1, ()> + 'static,
    {
        let capture = self
            .ext_capture
            .as_mut()
            .context("ext-image-copy capture session missing")?;
        if capture.frame.is_some() {
            anyhow::bail!("ext-image-copy frame submitted while another frame is pending");
        }
        let (width, height) = constraints
            .size
            .context("compositor omitted the ext-image-copy buffer size")?;
        if width == 0 || height == 0 {
            anyhow::bail!("compositor advertised an empty ext-image-copy buffer");
        }
        let format = select_shm_format(&constraints.formats)
            .context("compositor did not advertise an ARGB/XRGB shared-memory format")?;
        let stride = width
            .checked_mul(4)
            .and_then(|value| i32::try_from(value).ok())
            .context("ext-image-copy buffer stride overflow")?;
        let total_size = usize::try_from(stride)
            .ok()
            .and_then(|stride| {
                usize::try_from(height)
                    .ok()
                    .and_then(|height| stride.checked_mul(height))
            })
            .context("ext-image-copy buffer size overflow")?;
        let buffer_width = i32::try_from(width)
            .context("ext-image-copy buffer width exceeds the Wayland limit")?;
        let buffer_height = i32::try_from(height)
            .context("ext-image-copy buffer height exceeds the Wayland limit")?;
        if total_size > capture.pool.len() {
            capture.pool.resize(total_size)?;
        }
        let (buffer, _) = capture
            .pool
            .create_buffer(buffer_width, buffer_height, stride, format)
            .context("Failed to create ext-image-copy buffer")?;
        let frame = capture.session.create_frame(qh, ());
        frame.attach_buffer(buffer.wl_buffer());
        frame.damage_buffer(0, 0, buffer_width, buffer_height);
        frame.capture();
        capture.frame = Some(ExtImageCopyFrame {
            proxy: frame,
            buffer,
            width,
            height,
            stride,
            format,
            transform: None,
        });
        Ok(())
    }

    pub(in crate::backend::wayland) fn handle_ext_frame_event<State>(
        &mut self,
        event: FrameEvent,
        qh: &QueueHandle<State>,
        input_state: &mut InputState,
    ) -> bool
    where
        State: Dispatch<ExtImageCopyCaptureFrameV1, ()> + 'static,
    {
        match event {
            FrameEvent::Ready => match self.finish_ext_frame() {
                Ok(true) => input_state.needs_redraw = true,
                Ok(false) => {
                    warn!(
                        "Discarded ext-image-copy frozen capture because the active output changed"
                    );
                    self.finish_stale_direct_capture(input_state);
                }
                Err(error) => {
                    warn!("Ext-image-copy frame failed: {error:#}");
                    return self.fail_ext_capture();
                }
            },
            FrameEvent::Failed { reason } => {
                match reason {
                    WEnum::Value(FailureReason::BufferConstraints) => {
                        if let Err(error) = self.retry_ext_frame_after_constraints(qh) {
                            warn!(
                                "Failed to retry ext-image-copy with replacement constraints: {error:#}"
                            );
                            return self.fail_ext_capture();
                        }
                        return false;
                    }
                    WEnum::Value(FailureReason::Unknown) => {
                        warn!("Ext-image-copy frame failed: unknown compositor error");
                    }
                    WEnum::Value(FailureReason::Stopped) => {
                        warn!("Ext-image-copy frame failed: capture session stopped");
                    }
                    WEnum::Unknown(_) => {
                        warn!("Ext-image-copy frame failed: unknown failure reason");
                    }
                    _ => {
                        warn!("Ext-image-copy frame failed: unsupported failure reason");
                    }
                }
                return self.fail_ext_capture();
            }
            FrameEvent::Transform { transform } => {
                if let Some(capture) = self.ext_capture.as_mut()
                    && let Some(frame) = capture.frame.as_mut()
                    && let WEnum::Value(transform) = transform
                {
                    frame.transform = Some(transform);
                }
            }
            FrameEvent::Damage { .. } | FrameEvent::PresentationTime { .. } => {}
            _ => {}
        }
        false
    }

    fn retry_ext_frame_after_constraints<State>(&mut self, qh: &QueueHandle<State>) -> Result<()>
    where
        State: Dispatch<ExtImageCopyCaptureFrameV1, ()> + 'static,
    {
        let replacement = {
            let capture = self
                .ext_capture
                .as_mut()
                .context("ext-image-copy capture session missing")?;
            let frame = capture
                .frame
                .take()
                .context("failed ext-image-copy frame missing")?;
            frame.proxy.destroy();
            capture.constraints.take_replacement_for_retry()
        };

        let replacement = replacement?;
        debug!("Retrying ext-image-copy with replacement buffer constraints");
        self.submit_ext_frame(qh, replacement)
    }

    fn finish_ext_frame(&mut self) -> Result<bool> {
        let frame = self
            .ext_capture
            .as_mut()
            .context("ext-image-copy capture session missing")?;
        let frame = frame.frame.take().context("ext-image-copy frame missing")?;
        let result: Result<_> = (|| {
            let capture = self
                .ext_capture
                .as_mut()
                .context("ext-image-copy capture session missing")?;
            let canvas = frame
                .buffer
                .canvas(&mut capture.pool)
                .context("Unable to map ext-image-copy buffer")?;
            let image = copy_shm_argb(
                canvas,
                frame.width,
                frame.height,
                frame.stride,
                frame.format,
                false,
            )?;
            Ok((image, frame.transform))
        })();
        frame.proxy.destroy();
        let (image, output_transform) = result?;
        let capture = self
            .ext_capture
            .take()
            .context("ext-image-copy capture session missing after frame completion")?;
        capture.destroy();
        let context = self
            .direct_capture
            .take()
            .context("ext-image-copy direct capture context missing")?;
        if context.backend != DirectCaptureBackend::ExtImageCopy {
            anyhow::bail!("ext-image-copy completed with a mismatched direct backend context");
        }
        if !context.output_matches(self.active_output_id) {
            return Ok(false);
        }
        self.set_pending_output_image_with_transform(
            image,
            context.source_geometry,
            output_transform,
        );
        Ok(true)
    }

    fn fail_ext_capture(&mut self) -> bool {
        if let Some(capture) = self.ext_capture.take() {
            capture.destroy();
        }
        self.direct_capture = None;
        true
    }
}

fn select_shm_format(formats: &[wl_shm::Format]) -> Option<wl_shm::Format> {
    [wl_shm::Format::Argb8888, wl_shm::Format::Xrgb8888]
        .into_iter()
        .find(|preferred| formats.contains(preferred))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shm_format_selection_prefers_argb_and_accepts_xrgb() {
        let formats = [wl_shm::Format::Xrgb8888, wl_shm::Format::Argb8888];
        assert_eq!(select_shm_format(&formats), Some(wl_shm::Format::Argb8888));
        assert_eq!(
            select_shm_format(&[wl_shm::Format::Xrgb8888]),
            Some(wl_shm::Format::Xrgb8888)
        );
        assert_eq!(select_shm_format(&[wl_shm::Format::Rgb565]), None);
    }

    #[test]
    fn replacement_constraint_batch_does_not_mutate_the_in_flight_layout() {
        let mut tracker = ConstraintTracker::default();
        tracker.record_size(1920, 1080);
        tracker.record_format(WEnum::Value(wl_shm::Format::Argb8888));
        let initial = tracker
            .finish_batch(false)
            .expect("initial constraints should be submitted");

        tracker.record_size(1280, 720);
        tracker.record_format(WEnum::Value(wl_shm::Format::Xrgb8888));
        assert!(
            tracker.finish_batch(true).is_none(),
            "replacement constraints wait while the original frame is pending"
        );

        assert_eq!(initial.size, Some((1920, 1080)));
        assert_eq!(initial.formats, vec![wl_shm::Format::Argb8888]);
        let replacement = tracker
            .take_replacement_for_retry()
            .expect("replacement constraints retained for retry");
        assert_eq!(replacement.size, Some((1280, 720)));
        assert_eq!(replacement.formats, vec![wl_shm::Format::Xrgb8888]);
    }

    #[test]
    fn constraint_batches_do_not_accumulate_old_formats() {
        let mut tracker = ConstraintTracker::default();
        tracker.record_size(100, 100);
        tracker.record_format(WEnum::Value(wl_shm::Format::Argb8888));
        let _ = tracker.finish_batch(false);

        tracker.record_size(200, 200);
        tracker.record_format(WEnum::Value(wl_shm::Format::Xrgb8888));
        let replacement = tracker
            .finish_batch(false)
            .expect("second batch should be independent");

        assert_eq!(replacement.formats, vec![wl_shm::Format::Xrgb8888]);
    }

    #[test]
    fn constraint_retries_are_bounded_before_backend_fallback() {
        let mut tracker = ConstraintTracker::default();
        for attempt in 0..MAX_CONSTRAINT_RETRIES {
            tracker.record_size(100 + u32::from(attempt), 100);
            tracker.record_format(WEnum::Value(wl_shm::Format::Argb8888));
            assert!(tracker.finish_batch(true).is_none());
            tracker
                .take_replacement_for_retry()
                .expect("retry remains within the budget");
        }

        tracker.record_size(200, 100);
        tracker.record_format(WEnum::Value(wl_shm::Format::Argb8888));
        assert!(tracker.finish_batch(true).is_none());
        assert!(tracker.take_replacement_for_retry().is_err());
    }
}
