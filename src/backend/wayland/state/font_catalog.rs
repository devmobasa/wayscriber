//! Off-dispatch loading for the process-wide system font catalog.

use std::time::Instant;

use crate::backend::wayland::RuntimeOperationPoll;

use super::WaylandState;

impl WaylandState {
    /// Start the one-time catalog walk after a frame has reached the compositor.
    pub(in crate::backend::wayland) fn start_font_catalog_prewarm(&mut self) {
        if self.font_catalog_prewarm_started {
            return;
        }
        if self.input_state.font_picker_load_failed() {
            return;
        }
        if crate::draw::system_font_catalog_is_ready() {
            self.font_catalog_prewarm_started = true;
            return;
        }

        match self
            .font_catalog_prewarm
            .try_submit((), "wayscriber-font-catalog", || {
                let started = Instant::now();
                crate::draw::prewarm_system_font_catalog();
                started.elapsed()
            }) {
            Ok(_) => self.font_catalog_prewarm_started = true,
            Err(failure) => {
                let (error, ()) = failure.into_parts();
                log::warn!("Failed to start system font catalog prewarm: {error}");
                self.input_state.fail_font_picker_catalog_load();
            }
        }
    }

    /// Apply a completed catalog to a picker that opened while it was loading.
    pub(in crate::backend::wayland) fn drain_font_catalog_prewarm(&mut self) {
        match self.font_catalog_prewarm.poll() {
            RuntimeOperationPoll::Idle | RuntimeOperationPoll::Pending { .. } => {}
            RuntimeOperationPoll::Ready {
                outcome: elapsed, ..
            } => {
                log::debug!(
                    "System font catalog prewarm completed in {:.1} ms",
                    elapsed.as_secs_f64() * 1000.0
                );
                self.input_state.finish_font_picker_catalog_load();
            }
            RuntimeOperationPoll::ProducerFailed { reason, .. } => {
                log::warn!("System font catalog prewarm worker failed: {reason}");
                self.font_catalog_prewarm_started = false;
                self.input_state.fail_font_picker_catalog_load();
            }
            RuntimeOperationPoll::Disconnected { .. } => {
                log::warn!("System font catalog prewarm worker disconnected");
                self.font_catalog_prewarm_started = false;
                self.input_state.fail_font_picker_catalog_load();
            }
        }
    }
}
