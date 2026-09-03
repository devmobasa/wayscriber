//! Off-dispatch loading for the process-wide system font catalog.

use std::time::{Duration, Instant};

use crate::backend::wayland::{
    RuntimeOperationController, RuntimeOperationIdSource, RuntimeOperationPoll,
    RuntimeOperationSubmitFailure, RuntimeWakeHandle,
};

use super::WaylandState;

/// One-shot system font-catalog prewarm and its retry latch.
pub(in crate::backend::wayland) struct FontCatalogPrewarm {
    controller: RuntimeOperationController<(), Duration>,
    started: bool,
}

impl FontCatalogPrewarm {
    pub(in crate::backend::wayland) fn new(
        ids: RuntimeOperationIdSource,
        wake: RuntimeWakeHandle,
    ) -> Self {
        Self {
            controller: RuntimeOperationController::new(ids, wake),
            started: false,
        }
    }

    fn start(
        &mut self,
        load_failed: bool,
        catalog_ready: bool,
    ) -> Result<(), RuntimeOperationSubmitFailure<()>> {
        if self.started || load_failed {
            return Ok(());
        }
        if catalog_ready {
            self.started = true;
            return Ok(());
        }

        self.controller
            .try_submit((), "wayscriber-font-catalog", || {
                let started = Instant::now();
                crate::draw::prewarm_system_font_catalog();
                started.elapsed()
            })
            .map(|_| self.started = true)
    }

    fn poll(&mut self) -> RuntimeOperationPoll<(), Duration> {
        let completion = self.controller.poll();
        if matches!(
            completion,
            RuntimeOperationPoll::ProducerFailed { .. } | RuntimeOperationPoll::Disconnected { .. }
        ) {
            self.started = false;
        }
        completion
    }
}

impl WaylandState {
    /// Start the one-time catalog walk after a frame has reached the compositor.
    pub(in crate::backend::wayland) fn start_font_catalog_prewarm(&mut self) {
        if let Err(failure) = self.font_catalog.start(
            self.input_state.font_picker_load_failed(),
            crate::draw::system_font_catalog_is_ready(),
        ) {
            let (error, ()) = failure.into_parts();
            log::warn!("Failed to start system font catalog prewarm: {error}");
            self.input_state.fail_font_picker_catalog_load();
        }
    }

    /// Apply a completed catalog to a picker that opened while it was loading.
    pub(in crate::backend::wayland) fn drain_font_catalog_prewarm(&mut self) {
        match self.font_catalog.poll() {
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
                self.input_state.fail_font_picker_catalog_load();
            }
            RuntimeOperationPoll::Disconnected { .. } => {
                log::warn!("System font catalog prewarm worker disconnected");
                self.input_state.fail_font_picker_catalog_load();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::wayland::RuntimeWakeSource;

    fn prewarm() -> FontCatalogPrewarm {
        let wake = RuntimeWakeSource::new().expect("runtime wake source");
        FontCatalogPrewarm::new(RuntimeOperationIdSource::new(), wake.handle())
    }

    #[test]
    fn failed_picker_load_does_not_start_prewarm() {
        let mut prewarm = prewarm();

        assert!(prewarm.start(true, false).is_ok());
        assert!(!prewarm.started);
    }

    #[test]
    fn ready_catalog_marks_prewarm_started_without_worker() {
        let mut prewarm = prewarm();

        assert!(prewarm.start(false, true).is_ok());
        assert!(prewarm.started);
        assert!(matches!(prewarm.poll(), RuntimeOperationPoll::Idle));
    }

    #[test]
    fn disconnected_worker_reopens_the_start_latch() {
        let mut prewarm = prewarm();
        prewarm
            .controller
            .try_submit_with_spawner_for_test((), || Duration::ZERO, |_job| Ok(()))
            .expect("test transport starts");
        prewarm.started = true;

        assert!(matches!(
            prewarm.poll(),
            RuntimeOperationPoll::Disconnected { .. }
        ));
        assert!(!prewarm.started);
    }
}
