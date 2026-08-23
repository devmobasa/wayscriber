//! Overlay-side correlation for publishing a rendered Review crop to the pin host.

use wayland_client::protocol::wl_output;

use super::WaylandState;
use crate::backend::wayland::RuntimeOperationPoll;
use crate::backend::wayland::runtime_operation::RuntimeOperationSubmitError;
use crate::input::state::{RegionPurposeTag, Toast, ToastPriority};
use crate::pin::{
    PinCreateAck, PinCreateRequest, PinOutputHint, PinOutputTransform, PinPlacementHint,
    PinRequestId,
};
use crate::screen_pixels::ImagePixelRect;

use super::region_capture::ActiveScreenRegion;
use super::screen_image::{ScreenSourceToken, screen_rect_for_image_rect};

const TOAST_SOURCE: &str = "capture.region.pin";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::backend::wayland) struct PendingPinPublish {
    pub pin_request_id: PinRequestId,
    pub picker_generation: u64,
}

#[derive(Debug, Clone)]
pub(in crate::backend::wayland::state) struct PreparedPinRender {
    pub pin_request_id: PinRequestId,
    pub output: PinOutputHint,
    pub placement: PinPlacementHint,
    pub picker_generation: u64,
}

impl WaylandState {
    /// Pin is a Review-only layer-shell action with an output connector and a
    /// private XDG runtime directory. This check has no host-start side effect.
    pub(in crate::backend::wayland) fn region_pin_eligible(&self) -> bool {
        let output_eligible = match self.data.active_screen_region {
            Some(ActiveScreenRegion::Ready {
                purpose: RegionPurposeTag::CaptureInteractive,
                source,
                ..
            }) => self.pin_output_hint(source).is_some(),
            _ => false,
        };
        pin_review_prerequisites(
            self.input_state.region_state().is_review(),
            self.layer_shell.is_some(),
            self.pin_runtime_available,
            output_eligible,
        )
    }

    pub(in crate::backend::wayland::state) fn prepare_pin_render(
        &mut self,
        source: ScreenSourceToken,
        rect: ImagePixelRect,
        picker_generation: u64,
    ) -> Result<PreparedPinRender, &'static str> {
        crate::pin::validate_source_dimensions(rect.width(), rect.height())
            .map_err(|_| "That region exceeds the pin safety limits.")?;
        let output = self
            .pin_output_hint(source)
            .ok_or("Pin is unavailable for the active output.")?;
        let display = screen_rect_for_image_rect(&source, rect);
        let placement = PinPlacementHint::new(
            f64::from(display.x),
            f64::from(display.y),
            f64::from(display.width),
            f64::from(display.height),
        )
        .ok_or("The selected region cannot be placed as a pin.")?;
        let pin_request_id = allocate_pin_request_id(&mut self.next_pin_request_id)?;
        Ok(PreparedPinRender {
            pin_request_id,
            output,
            placement,
            picker_generation,
        })
    }

    pub(in crate::backend::wayland) fn publish_rendered_pin(
        &mut self,
        image: crate::capture::RenderedImage,
        pending: crate::backend::wayland::capture::PendingPinRender,
    ) {
        let context = PendingPinPublish {
            pin_request_id: pending.pin_request_id,
            picker_generation: pending.picker_generation,
        };
        let request = PinCreateRequest {
            request_id: pending.pin_request_id,
            image,
            output: pending.output,
            placement: pending.placement,
        };
        if let Err(failure) =
            self.pin_publish
                .try_submit(context, "wayscriber-pin-publish", move || {
                    crate::pin::create_pin(request)
                })
        {
            let (error, context) = failure.into_parts();
            log::warn!(
                "Could not submit pin request {} from picker generation {}: {}",
                context.pin_request_id,
                context.picker_generation,
                error
            );
            let message = pin_submit_failure_message(&error);
            self.report_pin_failure(message);
        }
    }

    pub(in crate::backend::wayland) fn poll_pin_publish_completion(&mut self) {
        match self.pin_publish.poll() {
            RuntimeOperationPoll::Idle | RuntimeOperationPoll::Pending { .. } => {}
            RuntimeOperationPoll::Ready {
                context,
                outcome: Ok(ack),
                ..
            } if pin_ack_matches(context, &ack) => {
                log::info!(
                    "Pinned region {} from picker generation {} as pin {}",
                    context.pin_request_id,
                    context.picker_generation,
                    ack.pin_id
                );
                self.input_state.push_toast(
                    ToastPriority::Info,
                    TOAST_SOURCE,
                    Toast::info("Region pinned"),
                );
            }
            RuntimeOperationPoll::Ready {
                context,
                outcome: Ok(ack),
                ..
            } => {
                self.pin_publish.mark_unhealthy();
                log::error!(
                    "Pin acknowledgement {} did not match request {} from picker generation {}",
                    ack.request_id,
                    context.pin_request_id,
                    context.picker_generation
                );
                self.report_pin_failure(
                    "Region was not pinned because the pin response was stale.",
                );
            }
            RuntimeOperationPoll::Ready {
                context,
                outcome: Err(error),
                ..
            } => {
                log::warn!(
                    "Pin request {} from picker generation {} failed: {}",
                    context.pin_request_id,
                    context.picker_generation,
                    error
                );
                self.report_pin_failure(error.to_string());
            }
            RuntimeOperationPoll::ProducerFailed {
                context, reason, ..
            } => {
                log::error!(
                    "Pin worker for request {} from picker generation {} failed: {}",
                    context.pin_request_id,
                    context.picker_generation,
                    reason
                );
                self.report_pin_failure(
                    "Region was not pinned because the pin worker stopped unexpectedly.",
                );
            }
            RuntimeOperationPoll::Disconnected { context, .. } => {
                log::error!(
                    "Pin worker for request {} from picker generation {} disconnected",
                    context.pin_request_id,
                    context.picker_generation
                );
                self.report_pin_failure(
                    "Region was not pinned because the pin worker disconnected.",
                );
            }
        }
    }

    fn pin_output_hint(&self, source: ScreenSourceToken) -> Option<PinOutputHint> {
        let output = self.surface.current_output()?;
        let info = self.output_state.info(&output)?;
        if info.id != source.output_id {
            return None;
        }
        let connector_name = info.name.filter(|name| !name.is_empty())?;
        let (logical_width, logical_height) = info.logical_size?;
        let logical_width = u32::try_from(logical_width).ok()?;
        let logical_height = u32::try_from(logical_height).ok()?;
        if (logical_width, logical_height) != source.surface {
            return None;
        }
        let scale = u32::try_from(source.output_scale).ok()?;
        PinOutputHint::new(
            connector_name,
            logical_width,
            logical_height,
            scale,
            pin_output_transform(source.output_transform)?,
        )
        .ok()
    }

    fn report_pin_failure(&mut self, message: impl Into<String>) {
        self.input_state.push_toast(
            ToastPriority::Critical,
            TOAST_SOURCE,
            Toast::error(message.into()),
        );
    }
}

const fn pin_submit_failure_message(error: &RuntimeOperationSubmitError) -> &'static str {
    match error {
        RuntimeOperationSubmitError::Busy { .. } => {
            "Region was not pinned because another pin is still being published."
        }
        RuntimeOperationSubmitError::IdentityExhausted => {
            "Region was not pinned because pin operation identifiers are exhausted."
        }
        RuntimeOperationSubmitError::Unhealthy => {
            "Region was not pinned because the pin publisher is unavailable."
        }
        RuntimeOperationSubmitError::SpawnFailed { .. } => {
            "Region was not pinned because the pin worker could not start."
        }
    }
}

fn pin_ack_matches(context: PendingPinPublish, ack: &PinCreateAck) -> bool {
    ack.request_id == context.pin_request_id
}

fn allocate_pin_request_id(next: &mut Option<u64>) -> Result<PinRequestId, &'static str> {
    let value = next.ok_or("Pin request identifiers are exhausted.")?;
    let id = PinRequestId::new(value).ok_or("Pin request identifiers are exhausted.")?;
    *next = value.checked_add(1);
    Ok(id)
}

const fn pin_review_prerequisites(
    review: bool,
    layer_shell: bool,
    secure_runtime: bool,
    output_connector: bool,
) -> bool {
    review && layer_shell && secure_runtime && output_connector
}

const fn pin_output_transform(transform: wl_output::Transform) -> Option<PinOutputTransform> {
    Some(match transform {
        wl_output::Transform::Normal => PinOutputTransform::Normal,
        wl_output::Transform::_90 => PinOutputTransform::Rotate90,
        wl_output::Transform::_180 => PinOutputTransform::Rotate180,
        wl_output::Transform::_270 => PinOutputTransform::Rotate270,
        wl_output::Transform::Flipped => PinOutputTransform::Flipped,
        wl_output::Transform::Flipped90 => PinOutputTransform::Flipped90,
        wl_output::Transform::Flipped180 => PinOutputTransform::Flipped180,
        wl_output::Transform::Flipped270 => PinOutputTransform::Flipped270,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;
    use std::time::{Duration, Instant};

    use super::*;
    use crate::backend::wayland::{
        RuntimeOperationController, RuntimeOperationIdSource, RuntimeOperationPoll,
        RuntimeWakeSource,
    };

    fn pending(request_id: u64) -> PendingPinPublish {
        PendingPinPublish {
            pin_request_id: PinRequestId::new(request_id).unwrap(),
            picker_generation: request_id + 100,
        }
    }

    #[test]
    fn overlay_pin_request_ids_never_wrap_or_reuse() {
        let mut next = Some(u64::MAX - 1);
        assert_eq!(
            allocate_pin_request_id(&mut next).unwrap().get(),
            u64::MAX - 1
        );
        assert_eq!(allocate_pin_request_id(&mut next).unwrap().get(), u64::MAX);
        assert!(allocate_pin_request_id(&mut next).is_err());
    }

    #[test]
    fn every_current_wayland_transform_has_an_exact_pin_mapping() {
        for (wayland, pin) in [
            (wl_output::Transform::Normal, PinOutputTransform::Normal),
            (wl_output::Transform::_90, PinOutputTransform::Rotate90),
            (wl_output::Transform::_180, PinOutputTransform::Rotate180),
            (wl_output::Transform::_270, PinOutputTransform::Rotate270),
            (wl_output::Transform::Flipped, PinOutputTransform::Flipped),
            (
                wl_output::Transform::Flipped90,
                PinOutputTransform::Flipped90,
            ),
            (
                wl_output::Transform::Flipped180,
                PinOutputTransform::Flipped180,
            ),
            (
                wl_output::Transform::Flipped270,
                PinOutputTransform::Flipped270,
            ),
        ] {
            assert_eq!(pin_output_transform(wayland), Some(pin));
        }
    }

    #[test]
    fn pin_review_eligibility_requires_every_runtime_prerequisite() {
        assert!(pin_review_prerequisites(true, true, true, true));
        for unavailable in 0..4 {
            let mut gates = [true; 4];
            gates[unavailable] = false;
            assert!(!pin_review_prerequisites(
                gates[0], gates[1], gates[2], gates[3]
            ));
        }
    }

    #[test]
    fn pin_publish_busy_and_spawn_failure_have_exact_actionable_errors() {
        let wake = RuntimeWakeSource::new().unwrap();
        let mut controller =
            RuntimeOperationController::new(RuntimeOperationIdSource::new(), wake.handle());
        let (release_tx, release_rx) = mpsc::channel();
        controller
            .try_submit(pending(1), "test-pin-publish", move || {
                release_rx.recv().unwrap();
                1_u8
            })
            .unwrap();
        let failure = controller
            .try_submit(pending(2), "test-pin-publish-busy", || 2_u8)
            .unwrap_err();
        let (error, context) = failure.into_parts();
        assert_eq!(context, pending(2));
        assert!(matches!(error, RuntimeOperationSubmitError::Busy { .. }));
        assert_eq!(
            pin_submit_failure_message(&error),
            "Region was not pinned because another pin is still being published."
        );
        assert_eq!(
            pin_submit_failure_message(&RuntimeOperationSubmitError::SpawnFailed {
                reason: "injected".to_string(),
            }),
            "Region was not pinned because the pin worker could not start."
        );
        release_tx.send(()).unwrap();
    }

    #[test]
    fn pin_publish_worker_panic_is_terminal_and_retains_request_context() {
        let wake = RuntimeWakeSource::new().unwrap();
        let mut controller =
            RuntimeOperationController::new(RuntimeOperationIdSource::new(), wake.handle());
        controller
            .try_submit(pending(3), "test-pin-publish-panic", || -> u8 {
                panic!("injected pin publisher panic")
            })
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(1);
        loop {
            match controller.poll() {
                RuntimeOperationPoll::Pending { .. } => {
                    assert!(Instant::now() < deadline, "pin publisher did not terminate");
                    std::thread::yield_now();
                }
                RuntimeOperationPoll::ProducerFailed {
                    context, reason, ..
                } => {
                    assert_eq!(context, pending(3));
                    assert_eq!(reason, "injected pin publisher panic");
                    break;
                }
                terminal => panic!("unexpected pin publish terminal: {terminal:?}"),
            }
        }
    }

    #[test]
    fn stale_pin_ack_never_matches_a_newer_publish_context() {
        let ack = PinCreateAck {
            request_id: PinRequestId::new(4).unwrap(),
            pin_id: crate::pin::PinId::new(1).unwrap(),
        };
        assert!(pin_ack_matches(pending(4), &ack));
        assert!(!pin_ack_matches(pending(5), &ack));
    }
}
