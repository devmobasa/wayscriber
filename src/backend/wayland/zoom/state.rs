use wayland_client::protocol::wl_output;
use wayland_protocols_wlr::screencopy::v1::client::zwlr_screencopy_manager_v1::ZwlrScreencopyManagerV1;

use crate::backend::wayland::RuntimeWakeHandle;
use crate::backend::wayland::frozen::{FrozenImage, ScreenImageProvenance};
use crate::backend::wayland::frozen_geometry::OutputGeometry;
use crate::backend::wayland::portal_capture::layout_token_matches;
use crate::backend::wayland::portal_task::PortalTask;
use crate::input::InputState;

use super::capture::CaptureSession;
use super::{MIN_ZOOM_SCALE, PortalCaptureResult};

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
    #[allow(dead_code)] // Phase 1 connects the native region-capture owner.
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

/// Zoom state, capture logic, and pan/lock bookkeeping.
pub struct ZoomState {
    pub(super) manager: Option<ZwlrScreencopyManagerV1>,
    pub(super) active_output: Option<wl_output::WlOutput>,
    pub(super) active_output_id: Option<u32>,
    pub(super) active_geometry: Option<OutputGeometry>,
    /// Bumped when freeze/zoom crop geometry actually changes so in-flight
    /// portal captures can be rejected after a layout change.
    pub(super) output_layout_generation: u64,
    pub(super) capture: Option<CaptureSession>,
    pub(super) image: Option<FrozenImage>,
    image_provenance: Option<ScreenImageProvenance>,
    pub(super) image_target_dimensions: Option<(u32, u32)>,
    image_generation: u64,
    pub(super) portal_task: Option<PortalTask<PortalCaptureResult>>,
    pub(super) portal_in_progress: bool,
    pub(super) portal_target_output_id: Option<u32>,
    pub(super) runtime_wake: Option<RuntimeWakeHandle>,
    pub(super) preflight_pending: bool,
    pub(super) preflight_use_fallback: bool,
    preflight_output_id: Option<u32>,
    preflight_layout_generation: Option<u64>,
    pub(super) capture_done: bool,
    next_capture_id: u64,
    current_capture_id: Option<ZoomCaptureId>,
    pub(super) source_terminal: Option<ZoomSourceTerminal>,
    pub(super) pending_activation: bool,
    pub active: bool,
    pub locked: bool,
    pub scale: f64,
    pub view_offset: (f64, f64),
    pub panning: bool,
    pub(super) last_pan_pos: (f64, f64),
}

impl ZoomState {
    #[cfg(test)]
    pub fn new(manager: Option<ZwlrScreencopyManagerV1>) -> Self {
        Self::new_inner(manager, None)
    }

    pub(in crate::backend::wayland) fn new_with_runtime_wake(
        manager: Option<ZwlrScreencopyManagerV1>,
        runtime_wake: RuntimeWakeHandle,
    ) -> Self {
        Self::new_inner(manager, Some(runtime_wake))
    }

    fn new_inner(
        manager: Option<ZwlrScreencopyManagerV1>,
        runtime_wake: Option<RuntimeWakeHandle>,
    ) -> Self {
        Self {
            manager,
            active_output: None,
            active_output_id: None,
            active_geometry: None,
            output_layout_generation: 0,
            capture: None,
            image: None,
            image_provenance: None,
            image_target_dimensions: None,
            image_generation: 0,
            portal_task: None,
            portal_in_progress: false,
            portal_target_output_id: None,
            runtime_wake,
            preflight_pending: false,
            preflight_use_fallback: false,
            preflight_output_id: None,
            preflight_layout_generation: None,
            capture_done: false,
            next_capture_id: 1,
            current_capture_id: None,
            source_terminal: None,
            pending_activation: false,
            active: false,
            locked: false,
            scale: MIN_ZOOM_SCALE,
            view_offset: (0.0, 0.0),
            panning: false,
            last_pan_pos: (0.0, 0.0),
        }
    }

    pub fn manager_available(&self) -> bool {
        self.manager.is_some()
    }

    pub fn set_active_output(&mut self, output: Option<wl_output::WlOutput>, id: Option<u32>) {
        self.active_output = output;
        self.active_output_id = id;
    }

    pub fn set_active_geometry(&mut self, geometry: Option<OutputGeometry>) {
        if self.active_geometry != geometry {
            self.output_layout_generation = self.output_layout_generation.wrapping_add(1);
        }
        self.active_geometry = geometry;
    }

    pub(in crate::backend::wayland) fn source_context_matches(
        &self,
        provenance: ScreenImageProvenance,
    ) -> bool {
        self.active_output_id == Some(provenance.output_id)
            && self.output_layout_generation == provenance.output_layout_generation
    }

    pub(in crate::backend::wayland) fn image_provenance(&self) -> Option<ScreenImageProvenance> {
        self.image.as_ref()?;
        self.image_provenance
    }

    pub fn image(&self) -> Option<&FrozenImage> {
        self.image.as_ref()
    }

    pub fn image_generation(&self) -> u64 {
        self.image_generation
    }

    pub(in crate::backend::wayland) fn install_image(
        &mut self,
        image: FrozenImage,
        provenance: ScreenImageProvenance,
    ) {
        self.image_target_dimensions = self
            .active_geometry
            .as_ref()
            .map(OutputGeometry::buffer_size)
            .or(Some((image.width, image.height)));
        self.image = Some(image);
        self.image_provenance = Some(provenance);
        self.bump_image_generation();
    }

    #[cfg(test)]
    pub fn set_image(&mut self, image: FrozenImage) {
        self.image_target_dimensions = self
            .active_geometry
            .as_ref()
            .map(OutputGeometry::buffer_size)
            .or(Some((image.width, image.height)));
        self.image = Some(image);
        self.image_provenance = None;
        self.bump_image_generation();
    }

    #[cfg(test)]
    pub(in crate::backend::wayland) fn set_image_with_provenance_for_test(
        &mut self,
        image: FrozenImage,
        provenance: ScreenImageProvenance,
    ) {
        self.image_target_dimensions = self
            .active_geometry
            .as_ref()
            .map(OutputGeometry::buffer_size)
            .or(Some((image.width, image.height)));
        self.image = Some(image);
        self.image_provenance = Some(provenance);
        self.bump_image_generation();
    }

    pub fn clear_image(&mut self) -> bool {
        let had_image = self.image.take().is_some();
        self.image_provenance = None;
        self.image_target_dimensions = None;
        if had_image {
            self.bump_image_generation();
        }
        had_image
    }

    pub fn is_in_progress(&self) -> bool {
        self.capture.is_some() || self.portal_in_progress || self.preflight_pending
    }

    #[cfg(test)]
    pub fn preflight_pending(&self) -> bool {
        self.preflight_pending
    }

    pub fn take_preflight_pending(&mut self) -> Option<bool> {
        if !self.preflight_pending {
            return None;
        }
        let use_fallback = self.preflight_use_fallback;
        self.preflight_pending = false;
        self.preflight_use_fallback = false;
        Some(use_fallback)
    }

    pub(super) fn snapshot_preflight_layout(&mut self) {
        self.preflight_output_id = self.active_output_id;
        self.preflight_layout_generation = Some(self.output_layout_generation);
    }

    pub(super) fn ensure_preflight_layout_current(&self) -> Result<(), String> {
        let Some(generation) = self.preflight_layout_generation else {
            return Ok(());
        };
        if layout_token_matches(
            self.preflight_output_id,
            generation,
            self.active_output_id,
            self.output_layout_generation,
        ) {
            Ok(())
        } else {
            Err("Zoom failed after the display layout changed".to_string())
        }
    }

    #[cfg(test)]
    pub fn preflight_layout_is_current(&self) -> bool {
        self.ensure_preflight_layout_current().is_ok()
    }

    pub(super) fn finish_stale_direct_capture(&mut self, input_state: &mut InputState) {
        self.cancel_with_outcome(input_state, false, ZoomSourceOutcome::StaleLayout);
    }

    fn clear_preflight_layout_snapshot(&mut self) {
        self.preflight_output_id = None;
        self.preflight_layout_generation = None;
    }

    pub fn take_capture_done(&mut self) -> bool {
        let done = self.capture_done;
        self.capture_done = false;
        done
    }

    pub(in crate::backend::wayland) fn current_capture_id(&self) -> Option<ZoomCaptureId> {
        self.current_capture_id
    }

    pub(in crate::backend::wayland) fn take_source_terminal(
        &mut self,
    ) -> Option<ZoomSourceTerminal> {
        self.source_terminal.take()
    }

    pub(super) fn begin_identified_capture(&mut self) -> ZoomCaptureId {
        let id = ZoomCaptureId(self.next_capture_id);
        self.next_capture_id = self
            .next_capture_id
            .checked_add(1)
            .expect("zoom capture id space exhausted");
        self.current_capture_id = Some(id);
        id
    }

    pub(super) fn finish_source_capture(&mut self, outcome: ZoomSourceOutcome) {
        self.finish_source_capture_with_report(outcome, None);
    }

    fn finish_source_capture_with_report(
        &mut self,
        outcome: ZoomSourceOutcome,
        report: Option<ZoomTerminalReport>,
    ) {
        let Some(id) = self.current_capture_id.take() else {
            return;
        };
        let report = report.or_else(|| {
            matches!(outcome, ZoomSourceOutcome::StaleLayout).then(|| ZoomTerminalReport {
                source: "zoom",
                message: "Zoom failed after the display layout changed".to_string(),
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

    pub fn is_engaged(&self) -> bool {
        self.active || self.pending_activation
    }

    pub fn request_activation(&mut self) {
        if !self.active {
            self.pending_activation = true;
        }
    }

    pub fn activate_without_capture(&mut self) {
        self.active = true;
        self.pending_activation = false;
    }

    pub fn abort_capture(&mut self) -> bool {
        let mut changed = self.pending_activation;
        if let Some(capture) = self.capture.take() {
            capture.frame.destroy();
            changed = true;
        }
        if self.preflight_pending || self.portal_in_progress {
            changed = true;
        }
        self.preflight_pending = false;
        self.preflight_use_fallback = false;
        self.clear_preflight_layout_snapshot();
        self.portal_in_progress = false;
        if let Some(mut task) = self.portal_task.take() {
            task.cancel();
        }
        self.portal_target_output_id = None;
        self.pending_activation = false;
        if changed {
            self.finish_source_capture(ZoomSourceOutcome::Aborted);
            self.capture_done = true;
        }
        changed
    }

    pub fn deactivate(&mut self, input_state: &mut InputState) {
        self.cancel_with_outcome(input_state, true, ZoomSourceOutcome::Deactivated);
    }

    pub fn reset_view(&mut self) {
        self.scale = MIN_ZOOM_SCALE;
        self.view_offset = (0.0, 0.0);
        self.panning = false;
        self.last_pan_pos = (0.0, 0.0);
    }

    #[allow(dead_code)] // Kept as the explicit non-deactivation capture terminal.
    pub fn cancel(&mut self, input_state: &mut InputState, force_reset: bool) {
        self.cancel_with_outcome(input_state, force_reset, ZoomSourceOutcome::Cancelled);
    }

    pub(in crate::backend::wayland) fn fail_capture(
        &mut self,
        input_state: &mut InputState,
        force_reset: bool,
        message: impl Into<String>,
    ) {
        self.cancel_with_outcome(
            input_state,
            force_reset,
            ZoomSourceOutcome::Failed(message.into()),
        );
    }

    pub(in crate::backend::wayland) fn finish_preflight_failure(
        &mut self,
        input_state: &mut InputState,
        message: String,
    ) {
        let report = ZoomTerminalReport {
            source: "zoom",
            message: message.clone(),
        };
        let outcome = if message == "Zoom failed after the display layout changed" {
            ZoomSourceOutcome::StaleLayout
        } else {
            ZoomSourceOutcome::Failed(message)
        };
        self.cancel_with_outcome_and_report(input_state, false, outcome, Some(report));
    }

    pub(super) fn cancel_with_outcome(
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
        self.preflight_pending = false;
        self.preflight_use_fallback = false;
        self.clear_preflight_layout_snapshot();
        self.capture_done = true;
        self.portal_in_progress = false;
        if let Some(mut task) = self.portal_task.take() {
            task.cancel();
        }
        self.portal_target_output_id = None;
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

    fn bump_image_generation(&mut self) {
        self.image_generation = self.image_generation.wrapping_add(1).max(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::state::test_support::make_test_input_state;

    #[tokio::test]
    async fn capture_ids_are_monotonic_and_terminals_require_a_current_capture() {
        let mut state = ZoomState::new(None);

        assert_eq!(state.current_capture_id(), None);
        state.abort_capture();
        assert_eq!(state.take_source_terminal(), None);

        state
            .start_capture(false, &tokio::runtime::Handle::current())
            .expect("first capture starts");
        let first = state.current_capture_id().expect("first capture id");
        assert!(state.abort_capture());
        assert_eq!(
            state.take_source_terminal(),
            Some(ZoomSourceTerminal {
                id: first,
                outcome: ZoomSourceOutcome::Aborted,
                report: None,
            })
        );

        state
            .start_capture(false, &tokio::runtime::Handle::current())
            .expect("second capture starts");
        let second = state.current_capture_id().expect("second capture id");
        assert!(second > first);
    }

    #[tokio::test]
    async fn capture_refuses_to_start_until_the_previous_terminal_is_drained() {
        let mut state = ZoomState::new(None);

        state
            .start_capture(false, &tokio::runtime::Handle::current())
            .expect("first capture starts");
        let first = state.current_capture_id().expect("first capture id");
        assert!(state.abort_capture());

        let error = state
            .start_capture(false, &tokio::runtime::Handle::current())
            .expect_err("an undrained terminal blocks the next capture");

        assert_eq!(
            error.to_string(),
            "a zoom capture terminal is still pending"
        );
        assert_eq!(state.current_capture_id(), None);
        assert!(!state.preflight_pending());
        assert_eq!(
            state.take_source_terminal(),
            Some(ZoomSourceTerminal {
                id: first,
                outcome: ZoomSourceOutcome::Aborted,
                report: None,
            })
        );
    }

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

    #[test]
    fn typed_terminal_is_emitted_once_for_each_current_capture() {
        let outcomes = [
            ZoomSourceOutcome::Ready {
                installed_generation: 9,
            },
            ZoomSourceOutcome::Aborted,
            ZoomSourceOutcome::Cancelled,
            ZoomSourceOutcome::Deactivated,
            ZoomSourceOutcome::StaleLayout,
            ZoomSourceOutcome::Failed("failed".to_string()),
        ];

        for outcome in outcomes {
            let mut state = ZoomState::new(None);
            let id = state.begin_identified_capture();
            state.finish_source_capture(outcome.clone());
            state.finish_source_capture(ZoomSourceOutcome::Failed("duplicate".to_string()));
            let report =
                matches!(&outcome, ZoomSourceOutcome::StaleLayout).then(|| ZoomTerminalReport {
                    source: "zoom",
                    message: "Zoom failed after the display layout changed".to_string(),
                });

            assert_eq!(
                state.take_source_terminal(),
                Some(ZoomSourceTerminal {
                    id,
                    outcome,
                    report,
                })
            );
            assert_eq!(state.current_capture_id(), None);
            assert_eq!(state.take_source_terminal(), None);
        }
    }

    #[test]
    fn cancel_and_deactivate_publish_distinct_terminals() {
        let mut state = ZoomState::new(None);
        let mut input_state = make_test_input_state();
        let cancelled = state.begin_identified_capture();
        state.cancel(&mut input_state, false);
        assert_eq!(
            state.take_source_terminal(),
            Some(ZoomSourceTerminal {
                id: cancelled,
                outcome: ZoomSourceOutcome::Cancelled,
                report: None,
            })
        );

        let deactivated = state.begin_identified_capture();
        state.deactivate(&mut input_state);
        assert_eq!(
            state.take_source_terminal(),
            Some(ZoomSourceTerminal {
                id: deactivated,
                outcome: ZoomSourceOutcome::Deactivated,
                report: None,
            })
        );
    }

    #[test]
    fn preflight_failure_terminal_carries_the_specific_error_report() {
        let mut state = ZoomState::new(None);
        let mut input_state = make_test_input_state();
        let id = state.begin_identified_capture();

        state.finish_preflight_failure(&mut input_state, "specific backend failure".to_string());

        assert_eq!(
            state.take_source_terminal(),
            Some(ZoomSourceTerminal {
                id,
                outcome: ZoomSourceOutcome::Failed("specific backend failure".to_string()),
                report: Some(ZoomTerminalReport {
                    source: "zoom",
                    message: "specific backend failure".to_string(),
                }),
            })
        );
    }

    #[test]
    fn aborting_pending_activation_completes_the_capture_lifecycle() {
        let mut state = ZoomState::new(None);
        state.request_activation();
        assert!(state.is_engaged());
        assert!(!state.is_in_progress());

        assert!(state.abort_capture());

        assert!(!state.is_engaged());
        assert!(state.take_capture_done());
    }

    #[test]
    fn stale_direct_capture_publishes_one_report_and_preserves_the_current_image() {
        let mut state = ZoomState::new(None);
        let mut input_state = make_test_input_state();
        let id = state.begin_identified_capture();
        state.set_image(FrozenImage {
            width: 1,
            height: 1,
            stride: 4,
            data: vec![4; 4],
        });
        let generation = state.image_generation();

        state.finish_stale_direct_capture(&mut input_state);

        assert_eq!(input_state.test_toast_count(), 0);
        assert_eq!(
            state.take_source_terminal(),
            Some(ZoomSourceTerminal {
                id,
                outcome: ZoomSourceOutcome::StaleLayout,
                report: Some(ZoomTerminalReport {
                    source: "zoom",
                    message: "Zoom failed after the display layout changed".to_string(),
                }),
            })
        );
        assert_eq!(state.image_generation(), generation);
        assert_eq!(state.image().unwrap().data, vec![4; 4]);
        assert!(state.take_capture_done());
    }
}
