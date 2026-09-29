//! Identified Zoom requests, waiters, terminal reports and cancellation cleanup.
use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(in crate::backend::wayland) struct ZoomCaptureId(u64);

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::backend::wayland) enum ZoomSourceOutcome {
    Ready { installed_generation: u64 },
    Aborted,
    Cancelled,
    Deactivated,
    StaleLayout,
    Failed(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::backend::wayland) struct ZoomSourceTerminal {
    pub id: ZoomCaptureId,
    pub outcome: ZoomSourceOutcome,
    pub report: Option<ZoomTerminalReport>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::backend::wayland) struct ZoomTerminalReport {
    pub source: &'static str,
    pub message: String,
}

#[cfg(test)]
impl ZoomSourceTerminal {
    pub fn for_test(outcome: ZoomSourceOutcome, report: Option<ZoomTerminalReport>) -> Self {
        Self {
            id: ZoomCaptureId(1),
            outcome,
            report,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(in crate::backend::wayland) enum ZoomWaiterOwner {
    Eyedropper,
    Ocr,
    RegionCapture,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::backend::wayland) struct ZoomWaiter {
    pub id: ZoomCaptureId,
    pub owner: ZoomWaiterOwner,
}

#[derive(Debug, Default)]
pub(in crate::backend::wayland) struct ZoomWaiterRegistry {
    waiter: Option<ZoomWaiter>,
}

impl ZoomWaiterRegistry {
    pub fn register(&mut self, waiter: ZoomWaiter) -> bool {
        if self.waiter.is_some() {
            return false;
        }

        self.waiter = Some(waiter);

        true
    }

    #[cfg(test)]
    pub fn waiter(&self) -> Option<ZoomWaiter> {
        self.waiter
    }

    pub fn take_for_terminal(
        &mut self,
        terminal: &ZoomSourceTerminal,
    ) -> Option<(ZoomWaiter, bool)> {
        let waiter = self.waiter.take()?;
        let matches = waiter.id == terminal.id;
        Some((waiter, matches))
    }

    pub fn clear_owner(&mut self, owner: ZoomWaiterOwner) -> bool {
        if !self.waiter.is_some_and(|waiter| waiter.owner == owner) {
            return false;
        }

        self.waiter.take();

        true
    }
}

impl ZoomState {
    pub(in crate::backend::wayland) fn current_capture_id(&self) -> Option<ZoomCaptureId> {
        self.current_capture_id
    }

    pub(in crate::backend::wayland) fn take_source_terminal(
        &mut self,
    ) -> Option<ZoomSourceTerminal> {
        self.source_terminal.take()
    }

    pub(in crate::backend::wayland::zoom) fn begin_identified_capture(&mut self) -> ZoomCaptureId {
        let id = ZoomCaptureId(self.next_capture_id);
        self.next_capture_id = self
            .next_capture_id
            .checked_add(1)
            .expect("zoom capture id space exhausted");
        self.current_capture_id = Some(id);
        self.layout_retry = PortalLayoutRetry::default();

        id
    }

    pub(in crate::backend::wayland::zoom) fn finish_source_capture(
        &mut self,
        outcome: ZoomSourceOutcome,
    ) {
        self.finish_source_capture_with_report(outcome, None);
    }

    fn finish_source_capture_with_report(
        &mut self,
        outcome: ZoomSourceOutcome,
        report: Option<ZoomTerminalReport>,
    ) {
        self.layout_retry = PortalLayoutRetry::default();
        let Some(id) = self.current_capture_id.take() else {
            return;
        };

        let report = report.or_else(|| {
            matches!(outcome, ZoomSourceOutcome::StaleLayout).then(|| ZoomTerminalReport {
                source: "zoom",
                message: CapturePreflightError::StaleLayout.message("Zoom"),
            })
        });
        debug_assert!(self.source_terminal.is_none());
        if self.source_terminal.is_none() {
            self.source_terminal = Some(ZoomSourceTerminal {
                id,
                outcome,
                report,
            });
        }
    }

    pub(in crate::backend::wayland) fn finish_preflight_failure(
        &mut self,
        input_state: &mut InputState,
        error: CapturePreflightError,
    ) {
        let message = error.message("Zoom");
        let report = ZoomTerminalReport {
            source: "zoom",
            message: message.clone(),
        };
        let outcome = if error.is_stale_layout() {
            ZoomSourceOutcome::StaleLayout
        } else {
            ZoomSourceOutcome::Failed(message)
        };

        self.cancel_with_outcome_and_report(input_state, false, outcome, Some(report));
    }

    pub(in crate::backend::wayland::zoom) fn cancel_with_outcome(
        &mut self,
        input_state: &mut InputState,
        force_reset: bool,
        outcome: ZoomSourceOutcome,
    ) {
        self.cancel_with_outcome_and_report(input_state, force_reset, outcome, None);
    }

    fn cancel_with_outcome_and_report(
        &mut self,
        input_state: &mut InputState,
        force_reset: bool,
        outcome: ZoomSourceOutcome,
        report: Option<ZoomTerminalReport>,
    ) {
        if let Some(capture) = self.capture.take() {
            capture.frame.destroy();
        }
        self.preflight = CapturePreflight::Idle;
        self.layout_retry = PortalLayoutRetry::default();
        self.capture_done = true;
        self.portal.finish();
        self.pending_activation = false;
        self.finish_source_capture_with_report(outcome, report);

        if force_reset || self.image.is_none() {
            self.active = false;
            self.locked = false;
            self.reset_view();
            self.clear_image();
        }

        input_state.set_zoom_status(self.active, self.locked, self.scale, self.view_offset);
        input_state.dirty_tracker.mark_full();
        input_state.needs_redraw = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mismatched_terminal_takes_the_waiter_for_fail_closed_owner_cancellation() {
        let stale_id = ZoomCaptureId(3);
        let newer = ZoomWaiter {
            id: ZoomCaptureId(4),
            owner: ZoomWaiterOwner::Ocr,
        };
        let mut registry = ZoomWaiterRegistry::default();
        assert!(registry.register(newer));

        let terminal = ZoomSourceTerminal {
            id: stale_id,
            outcome: ZoomSourceOutcome::Failed("old failure".to_string()),
            report: None,
        };

        assert_eq!(registry.take_for_terminal(&terminal), Some((newer, false)));
        assert_eq!(registry.waiter(), None);
    }
}
