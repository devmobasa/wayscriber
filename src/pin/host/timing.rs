//! Debug-level observability for host-side create phases.

use std::time::Instant;

use super::PinHost;
use crate::pin::{PinId, PinRequestId};

pub(super) struct PinTiming {
    request_id: PinRequestId,
    received: Instant,
    decoded: Instant,
    configured: Option<Instant>,
    first_commit: Option<Instant>,
}

impl PinHost {
    pub(super) fn begin_timing(
        &mut self,
        pin_id: PinId,
        request_id: PinRequestId,
        received: Instant,
        decoded: Instant,
    ) {
        log::debug!(
            "Pin {pin_id} request {request_id} decoded: decode_ms={}",
            decoded.duration_since(received).as_millis()
        );
        self.timings.insert(
            pin_id,
            PinTiming {
                request_id,
                received,
                decoded,
                configured: None,
                first_commit: None,
            },
        );
    }

    pub(super) fn note_configured(&mut self, pin_id: PinId) {
        let Some(timing) = self.timings.get_mut(&pin_id) else {
            return;
        };
        let now = Instant::now();
        timing.configured = Some(now);
        log::debug!(
            "Pin {pin_id} request {} configured: since_decode_ms={} total_ms={}",
            timing.request_id,
            now.duration_since(timing.decoded).as_millis(),
            now.duration_since(timing.received).as_millis()
        );
    }

    pub(super) fn note_first_commit(&mut self, pin_id: PinId) {
        let Some(timing) = self.timings.get_mut(&pin_id) else {
            return;
        };
        let now = Instant::now();
        timing.first_commit = Some(now);
        log::debug!(
            "Pin {pin_id} request {} first commit queued: since_configure_ms={} total_ms={}",
            timing.request_id,
            timing
                .configured
                .map_or(0, |configured| now.duration_since(configured).as_millis()),
            now.duration_since(timing.received).as_millis()
        );
    }

    pub(super) fn finish_timing(&mut self, pin_id: PinId) {
        let Some(timing) = self.timings.remove(&pin_id) else {
            return;
        };
        let now = Instant::now();
        log::debug!(
            "Pin {pin_id} request {} Ready after Wayland flush: since_commit_ms={} total_ms={}",
            timing.request_id,
            timing
                .first_commit
                .map_or(0, |commit| now.duration_since(commit).as_millis()),
            now.duration_since(timing.received).as_millis()
        );
    }

    pub(super) fn discard_timing(&mut self, pin_id: PinId) {
        self.timings.remove(&pin_id);
    }
}
