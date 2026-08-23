//! Capture controller for managing screenshot capture state.
//!
//! Keeps the capture manager and in-progress flag together so the main
//! Wayland loop can coordinate capture requests and results.

use crate::{
    capture::{
        CaptureManager, CaptureRequest, CaptureRequestId, DesktopBackdropCaptureRequest,
        ImageOperationKind, file::FileSaveConfig,
    },
    config::Action,
    input::state::BoardPasteTarget,
    pin::{PinOutputHint, PinPlacementHint, PinRequestId},
};

use super::state::RegionCaptureIntent;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CapturePreflight {
    None,
    AwaitingRender,
    AwaitingFrame,
}

#[derive(Clone)]
pub(in crate::backend::wayland) enum CapturePreflightRequest {
    Screenshot(CaptureRequest),
    DesktopBackdrop(DesktopBackdropCaptureRequest),
}

impl std::fmt::Debug for CapturePreflightRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Screenshot(request) => f.debug_tuple("Screenshot").field(request).finish(),
            Self::DesktopBackdrop(request) => {
                f.debug_tuple("DesktopBackdrop").field(request).finish()
            }
        }
    }
}

#[derive(Clone, Debug)]
pub(in crate::backend::wayland) struct PendingPdfExport {
    pub action: Action,
    pub operation: ImageOperationKind,
    pub save_config: FileSaveConfig,
    pub layout_context: CaptureLayoutContext,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::backend::wayland) struct CaptureLayoutContext {
    target_output_id: u32,
    layout_generation: u64,
}

/// Backend-owned lifecycle for a native region capture.
#[derive(Clone, Debug)]
pub(in crate::backend::wayland) enum RegionCapturePhase {
    Idle,
    Reserved(RegionCaptureIntent),
    Submitting(RegionCaptureIntent),
    Accepted,
}

#[derive(Clone, Debug)]
pub(in crate::backend::wayland) struct PendingBoardPaste {
    pub accepted_id: CaptureRequestId,
    pub target: BoardPasteTarget,
}

#[derive(Clone, Debug)]
pub(in crate::backend::wayland) struct PendingPinRender {
    pub accepted_id: CaptureRequestId,
    pub pin_request_id: PinRequestId,
    pub output: PinOutputHint,
    pub placement: PinPlacementHint,
    pub picker_generation: u64,
}

impl CaptureLayoutContext {
    pub(in crate::backend::wayland) fn new(target_output_id: u32, layout_generation: u64) -> Self {
        Self {
            target_output_id,
            layout_generation,
        }
    }

    pub(in crate::backend::wayland) fn target_output_id(self) -> u32 {
        self.target_output_id
    }

    pub(in crate::backend::wayland) fn matches(
        self,
        active_output_id: Option<u32>,
        layout_generation: u64,
    ) -> bool {
        active_output_id == Some(self.target_output_id)
            && layout_generation == self.layout_generation
    }
}

/// Tracks capture manager state and in-progress flag.
pub struct CaptureState {
    manager: CaptureManager,
    in_progress: bool,
    accepted_id: Option<CaptureRequestId>,
    exit_on_success: bool,
    preflight: CapturePreflight,
    pending_request: Option<CapturePreflightRequest>,
    pending_pdf_export: Option<PendingPdfExport>,
    region: RegionCapturePhase,
    pending_board_paste: Option<PendingBoardPaste>,
    pending_pin_render: Option<PendingPinRender>,
}

impl CaptureState {
    /// Creates a new capture state wrapper.
    pub fn new(manager: CaptureManager) -> Self {
        Self {
            manager,
            in_progress: false,
            accepted_id: None,
            exit_on_success: false,
            preflight: CapturePreflight::None,
            pending_request: None,
            pending_pdf_export: None,
            region: RegionCapturePhase::Idle,
            pending_board_paste: None,
            pending_pin_render: None,
        }
    }

    /// Returns a mutable reference to the underlying capture manager.
    pub fn manager_mut(&mut self) -> &mut CaptureManager {
        &mut self.manager
    }

    /// Returns `true` if a capture request is currently active.
    pub fn is_in_progress(&self) -> bool {
        self.in_progress
    }

    /// Queue a capture request that should wait for a suppression render + frame callback.
    pub fn queue_preflight(&mut self, request: CapturePreflightRequest) {
        self.pending_request = Some(request);
        self.preflight = CapturePreflight::AwaitingRender;
    }

    /// Returns true if capture is waiting on suppression render/callback.
    #[cfg(test)]
    pub fn preflight_pending(&self) -> bool {
        !matches!(self.preflight, CapturePreflight::None)
    }

    /// Mark that the suppression frame has been rendered and committed.
    pub fn mark_preflight_rendered(&mut self) {
        if matches!(self.preflight, CapturePreflight::AwaitingRender) {
            self.preflight = CapturePreflight::AwaitingFrame;
        }
    }

    /// Take the queued request once the suppression frame callback fires.
    pub fn take_preflight_request(&mut self) -> Option<CapturePreflightRequest> {
        if matches!(self.preflight, CapturePreflight::AwaitingFrame) {
            self.preflight = CapturePreflight::None;
            return self.pending_request.take();
        }
        None
    }

    /// Clear any pending preflight capture request.
    fn clear_preflight(&mut self) {
        self.preflight = CapturePreflight::None;
        self.pending_request = None;
    }

    pub fn set_pending_pdf_export(&mut self, request: PendingPdfExport) {
        self.pending_pdf_export = Some(request);
    }

    pub fn take_pending_pdf_export(&mut self) -> Option<PendingPdfExport> {
        self.pending_pdf_export.take()
    }

    pub fn clear_pending_pdf_export(&mut self) {
        self.pending_pdf_export = None;
    }

    /// Marks capture as started.
    pub fn mark_in_progress(&mut self) {
        self.in_progress = true;
    }

    /// Reserve the shared capture lifecycle for a native region picker.
    ///
    /// The intent is accepted only from a fully idle lifecycle. Once reserved,
    /// every other capture path observes `is_in_progress() == true`.
    pub(in crate::backend::wayland) fn reserve_region(
        &mut self,
        intent: RegionCaptureIntent,
    ) -> bool {
        if self.in_progress || !matches!(&self.region, RegionCapturePhase::Idle) {
            return false;
        }

        self.in_progress = true;
        self.accepted_id = None;
        self.region = RegionCapturePhase::Reserved(intent);
        true
    }

    /// Transfer a reserved native picker to the existing slurp lifecycle.
    ///
    /// The returned intent is the original immutable snapshot. Region-specific
    /// ownership is cleared, while the generic in-progress reservation stays
    /// held for suppression, preflight, and manager submission.
    pub(in crate::backend::wayland) fn handoff_region_to_legacy(
        &mut self,
    ) -> Option<RegionCaptureIntent> {
        let RegionCapturePhase::Reserved(_) = &self.region else {
            return None;
        };
        let RegionCapturePhase::Reserved(intent) =
            std::mem::replace(&mut self.region, RegionCapturePhase::Idle)
        else {
            unreachable!("region phase changed after reservation check");
        };
        debug_assert!(self.in_progress);
        debug_assert!(self.accepted_id.is_none());
        Some(intent)
    }

    /// Start the synchronous crop-to-manager handoff for a reserved picker.
    ///
    /// A clone is returned for building the submission request; the lifecycle
    /// retains its own copy until the manager accepts or terminal cleanup runs.
    pub(in crate::backend::wayland) fn begin_region_submission(
        &mut self,
    ) -> Option<RegionCaptureIntent> {
        let RegionCapturePhase::Reserved(intent) = &self.region else {
            return None;
        };
        let submission_intent = intent.clone();
        let RegionCapturePhase::Reserved(intent) =
            std::mem::replace(&mut self.region, RegionCapturePhase::Idle)
        else {
            unreachable!("region phase changed after reservation check");
        };
        debug_assert!(self.in_progress);
        debug_assert!(self.accepted_id.is_none());
        self.region = RegionCapturePhase::Submitting(intent);
        Some(submission_intent)
    }

    pub(in crate::backend::wayland) fn region_phase(&self) -> &RegionCapturePhase {
        &self.region
    }

    pub(in crate::backend::wayland) fn active_region_action(&self) -> Option<Action> {
        match &self.region {
            RegionCapturePhase::Reserved(intent) | RegionCapturePhase::Submitting(intent) => {
                Some(intent.action())
            }
            RegionCapturePhase::Idle | RegionCapturePhase::Accepted => None,
        }
    }

    /// Finishes the current capture lifecycle and clears all reusable state.
    ///
    pub fn finish_capture_lifecycle(&mut self) {
        self.in_progress = false;
        self.accepted_id = None;
        self.exit_on_success = false;
        self.clear_preflight();
        self.region = RegionCapturePhase::Idle;
        self.pending_board_paste = None;
        self.pending_pin_render = None;
    }

    /// Records the manager identity accepted for the current lifecycle.
    pub fn record_accepted(&mut self, id: CaptureRequestId) -> bool {
        if !self.in_progress || self.accepted_id.is_some() {
            return false;
        }

        match &self.region {
            RegionCapturePhase::Reserved(_) | RegionCapturePhase::Accepted => return false,
            RegionCapturePhase::Idle => {}
            RegionCapturePhase::Submitting(_) => {
                self.region = RegionCapturePhase::Accepted;
            }
        }
        self.accepted_id = Some(id);
        true
    }

    /// Consumes the accepted identity only when the completion matches it.
    pub fn consume_accepted(&mut self, id: CaptureRequestId) -> bool {
        if self.accepted_id != Some(id)
            || matches!(
                &self.region,
                RegionCapturePhase::Reserved(_) | RegionCapturePhase::Submitting(_)
            )
        {
            return false;
        }
        self.accepted_id = None;
        true
    }

    pub fn accepted_id(&self) -> Option<CaptureRequestId> {
        self.accepted_id
    }

    pub(in crate::backend::wayland) fn set_pending_board_paste(
        &mut self,
        accepted_id: CaptureRequestId,
        target: BoardPasteTarget,
    ) -> bool {
        if self.accepted_id != Some(accepted_id)
            || !matches!(self.region, RegionCapturePhase::Accepted)
            || self.pending_board_paste.is_some()
            || self.pending_pin_render.is_some()
        {
            return false;
        }
        self.pending_board_paste = Some(PendingBoardPaste {
            accepted_id,
            target,
        });
        true
    }

    pub(in crate::backend::wayland) fn set_pending_pin_render(
        &mut self,
        pending: PendingPinRender,
    ) -> bool {
        if self.accepted_id != Some(pending.accepted_id)
            || !matches!(self.region, RegionCapturePhase::Accepted)
            || self.pending_board_paste.is_some()
            || self.pending_pin_render.is_some()
        {
            return false;
        }
        self.pending_pin_render = Some(pending);
        true
    }

    pub(in crate::backend::wayland) fn take_pending_pin_render_for(
        &mut self,
        accepted_id: CaptureRequestId,
    ) -> Option<PendingPinRender> {
        if self
            .pending_pin_render
            .as_ref()
            .is_some_and(|pending| pending.accepted_id == accepted_id)
        {
            self.pending_pin_render.take()
        } else {
            None
        }
    }

    pub(in crate::backend::wayland) fn take_pending_board_paste_for(
        &mut self,
        accepted_id: CaptureRequestId,
    ) -> Option<PendingBoardPaste> {
        if self
            .pending_board_paste
            .as_ref()
            .is_some_and(|pending| pending.accepted_id == accepted_id)
        {
            self.pending_board_paste.take()
        } else {
            None
        }
    }

    /// Marks whether the current capture should exit the overlay on success.
    pub fn set_exit_on_success(&mut self, value: bool) {
        self.exit_on_success = value;
    }

    /// Returns whether the current capture should exit on success.
    pub fn exit_on_success(&self) -> bool {
        self.exit_on_success
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        backend::ExitAfterCaptureMode,
        backend::wayland::state::RegionPickerOptions,
        capture::types::{CaptureDestination, CaptureType},
        input::state::RegionPurposeTag,
    };

    fn region_intent(action: Action) -> RegionCaptureIntent {
        RegionCaptureIntent::new(
            action,
            RegionPurposeTag::CaptureDeliver,
            CaptureDestination::ClipboardOnly,
            None,
            ExitAfterCaptureMode::Auto,
            RegionPickerOptions::new(true, false, true),
            true,
        )
    }

    fn screenshot_request() -> CapturePreflightRequest {
        CapturePreflightRequest::Screenshot(CaptureRequest {
            capture_type: CaptureType::FullScreen,
            destination: CaptureDestination::ClipboardOnly,
            save_config: None,
        })
    }

    fn board_target() -> BoardPasteTarget {
        BoardPasteTarget {
            board_id: "transparent".to_string(),
            page_index: 0,
            page_generation: 1,
            world_bounds: crate::util::Rect::new(10, 20, 30, 40).unwrap(),
        }
    }

    fn pending_pin(id: CaptureRequestId) -> PendingPinRender {
        PendingPinRender {
            accepted_id: id,
            pin_request_id: crate::pin::PinRequestId::new(7).unwrap(),
            output: crate::pin::PinOutputHint::new(
                "DP-1".to_string(),
                1920,
                1080,
                1,
                crate::pin::PinOutputTransform::Normal,
            )
            .unwrap(),
            placement: crate::pin::PinPlacementHint::new(10.0, 20.0, 300.0, 200.0).unwrap(),
            picker_generation: 11,
        }
    }

    #[test]
    fn preflight_waits_for_render_before_request() {
        let manager = CaptureManager::with_closed_channel_for_test();
        let mut state = CaptureState::new(manager);
        let request = CaptureRequest {
            capture_type: CaptureType::FullScreen,
            destination: CaptureDestination::ClipboardOnly,
            save_config: None,
        };

        state.queue_preflight(CapturePreflightRequest::Screenshot(request));
        assert!(state.preflight_pending());
        assert!(state.take_preflight_request().is_none());

        state.mark_preflight_rendered();
        assert!(matches!(
            state.take_preflight_request(),
            Some(CapturePreflightRequest::Screenshot(_))
        ));
        assert!(!state.preflight_pending());
    }

    #[test]
    fn accepted_identity_is_owned_by_exactly_one_active_lifecycle() {
        let manager = CaptureManager::with_closed_channel_for_test();
        let mut state = CaptureState::new(manager);
        let first = CaptureRequestId::for_test(7);
        let other = CaptureRequestId::for_test(8);

        assert!(!state.record_accepted(first));
        state.mark_in_progress();
        assert!(state.record_accepted(first));
        assert!(!state.record_accepted(other));
        assert!(!state.consume_accepted(other));
        assert_eq!(state.accepted_id(), Some(first));
        assert!(state.consume_accepted(first));
        assert_eq!(state.accepted_id(), None);

        assert!(state.record_accepted(other));
        state.finish_capture_lifecycle();
        assert_eq!(state.accepted_id(), None);
        assert!(!state.is_in_progress());
    }

    #[test]
    fn pending_board_paste_is_correlated_and_cleared_by_terminal_cleanup() {
        let manager = CaptureManager::with_closed_channel_for_test();
        let mut state = CaptureState::new(manager);
        let id = CaptureRequestId::for_test(21);
        assert!(state.reserve_region(region_intent(Action::CaptureRegionInteractive)));
        assert!(state.begin_region_submission().is_some());
        assert!(state.record_accepted(id));
        assert!(state.set_pending_board_paste(id, board_target()));
        assert!(
            state
                .take_pending_board_paste_for(CaptureRequestId::for_test(22))
                .is_none()
        );

        state.finish_capture_lifecycle();

        assert!(state.take_pending_board_paste_for(id).is_none());
        assert!(!state.is_in_progress());
    }

    #[test]
    fn pin_render_is_correlated_mutually_exclusive_and_cleared_once() {
        let manager = CaptureManager::with_closed_channel_for_test();
        let mut state = CaptureState::new(manager);
        let id = CaptureRequestId::for_test(31);
        assert!(state.reserve_region(region_intent(Action::CaptureRegionInteractive)));
        assert!(state.begin_region_submission().is_some());
        assert!(state.record_accepted(id));
        assert!(state.set_pending_pin_render(pending_pin(id)));
        assert!(!state.set_pending_board_paste(id, board_target()));
        assert!(
            state
                .take_pending_pin_render_for(CaptureRequestId::for_test(32))
                .is_none()
        );

        let pending = state
            .take_pending_pin_render_for(id)
            .expect("matching pin render");
        assert_eq!(pending.pin_request_id.get(), 7);
        assert!(state.take_pending_pin_render_for(id).is_none());

        assert!(state.set_pending_board_paste(id, board_target()));
        assert!(!state.set_pending_pin_render(pending_pin(id)));
        state.finish_capture_lifecycle();
        assert!(state.take_pending_board_paste_for(id).is_none());
        assert!(state.take_pending_pin_render_for(id).is_none());
    }

    #[test]
    fn stale_pin_completion_cleanup_is_idempotent() {
        let manager = CaptureManager::with_closed_channel_for_test();
        let mut state = CaptureState::new(manager);
        let accepted = CaptureRequestId::for_test(41);
        let stale = CaptureRequestId::for_test(42);
        assert!(state.reserve_region(region_intent(Action::CaptureRegionInteractive)));
        assert!(state.begin_region_submission().is_some());
        assert!(state.record_accepted(accepted));
        assert!(state.set_pending_pin_render(pending_pin(accepted)));

        assert!(!state.consume_accepted(stale));
        assert!(state.take_pending_pin_render_for(stale).is_none());
        state.finish_capture_lifecycle();
        state.finish_capture_lifecycle();

        assert!(state.take_pending_pin_render_for(accepted).is_none());
        assert!(!state.is_in_progress());
        assert_eq!(state.accepted_id(), None);
        assert!(matches!(state.region_phase(), RegionCapturePhase::Idle));
    }

    #[test]
    fn worker_failure_consumes_the_matching_pin_owner_and_finishes_once() {
        let manager = CaptureManager::with_closed_channel_for_test();
        let mut state = CaptureState::new(manager);
        let accepted = CaptureRequestId::for_test(51);
        assert!(state.reserve_region(region_intent(Action::CaptureRegionInteractive)));
        assert!(state.begin_region_submission().is_some());
        assert!(state.record_accepted(accepted));
        assert!(state.set_pending_pin_render(pending_pin(accepted)));

        let failed = state
            .take_pending_pin_render_for(accepted)
            .expect("worker failure takes its exact pin owner");
        assert_eq!(failed.accepted_id, accepted);
        assert!(state.consume_accepted(accepted));
        state.finish_capture_lifecycle();
        state.finish_capture_lifecycle();

        assert!(state.take_pending_pin_render_for(accepted).is_none());
        assert!(!state.is_in_progress());
        assert_eq!(state.accepted_id(), None);
    }

    #[test]
    fn lifecycle_finish_cancels_awaiting_render_and_clears_the_pending_request() {
        let manager = CaptureManager::with_closed_channel_for_test();
        let mut state = CaptureState::new(manager);
        let accepted = CaptureRequestId::for_test(9);

        state.mark_in_progress();
        state.set_exit_on_success(true);
        assert!(state.record_accepted(accepted));
        state.queue_preflight(screenshot_request());

        state.finish_capture_lifecycle();

        assert!(!state.is_in_progress());
        assert_eq!(state.accepted_id(), None);
        assert!(!state.exit_on_success());
        assert!(!state.preflight_pending());
        assert!(state.take_preflight_request().is_none());
    }

    #[test]
    fn lifecycle_finish_cancels_awaiting_frame_and_clears_the_pending_request() {
        let manager = CaptureManager::with_closed_channel_for_test();
        let mut state = CaptureState::new(manager);
        let accepted = CaptureRequestId::for_test(10);

        state.mark_in_progress();
        state.set_exit_on_success(true);
        assert!(state.record_accepted(accepted));
        state.queue_preflight(screenshot_request());
        state.mark_preflight_rendered();

        state.finish_capture_lifecycle();

        assert!(!state.is_in_progress());
        assert_eq!(state.accepted_id(), None);
        assert!(!state.exit_on_success());
        assert!(!state.preflight_pending());
        assert!(state.take_preflight_request().is_none());
    }

    #[test]
    fn region_reservation_is_exclusive_and_marks_the_generic_lifecycle_busy() {
        let manager = CaptureManager::with_closed_channel_for_test();
        let mut state = CaptureState::new(manager);

        assert!(state.reserve_region(region_intent(Action::CaptureSelection)));
        assert!(state.is_in_progress());
        assert!(matches!(
            state.region_phase(),
            RegionCapturePhase::Reserved(intent)
                if intent.action() == Action::CaptureSelection
        ));
        assert!(!state.reserve_region(region_intent(Action::CaptureClipboardSelection)));

        state.finish_capture_lifecycle();
        state.mark_in_progress();
        assert!(!state.reserve_region(region_intent(Action::CaptureClipboardSelection)));
        assert!(matches!(state.region_phase(), RegionCapturePhase::Idle));
    }

    #[test]
    fn legacy_handoff_returns_the_snapshot_and_keeps_generic_ownership() {
        let manager = CaptureManager::with_closed_channel_for_test();
        let mut state = CaptureState::new(manager);
        assert!(state.reserve_region(region_intent(Action::CaptureFileRegion)));

        let handed_off = state
            .handoff_region_to_legacy()
            .expect("reserved intent should hand off");

        assert_eq!(handed_off.action(), Action::CaptureFileRegion);
        assert_eq!(handed_off.purpose(), RegionPurposeTag::CaptureDeliver);
        assert!(state.is_in_progress());
        assert_eq!(state.accepted_id(), None);
        assert!(matches!(state.region_phase(), RegionCapturePhase::Idle));
        assert!(state.handoff_region_to_legacy().is_none());

        let legacy_id = CaptureRequestId::for_test(20);
        assert!(state.record_accepted(legacy_id));
        assert!(state.consume_accepted(legacy_id));
    }

    #[test]
    fn submission_can_begin_only_once_and_preserves_the_intent_until_acceptance() {
        let manager = CaptureManager::with_closed_channel_for_test();
        let mut state = CaptureState::new(manager);
        assert!(state.reserve_region(region_intent(Action::CaptureClipboardRegion)));

        let submission = state
            .begin_region_submission()
            .expect("reserved region should begin submission");

        assert_eq!(submission.action(), Action::CaptureClipboardRegion);
        assert!(matches!(
            state.region_phase(),
            RegionCapturePhase::Submitting(intent)
                if intent.action() == Action::CaptureClipboardRegion
        ));
        assert!(state.begin_region_submission().is_none());
        assert!(state.handoff_region_to_legacy().is_none());
    }

    #[test]
    fn region_accepted_identity_requires_the_submitting_phase() {
        let manager = CaptureManager::with_closed_channel_for_test();
        let mut state = CaptureState::new(manager);
        let accepted = CaptureRequestId::for_test(21);
        let other = CaptureRequestId::for_test(22);
        assert!(state.reserve_region(region_intent(Action::CaptureSelection)));

        assert!(!state.record_accepted(accepted));
        assert!(matches!(
            state.region_phase(),
            RegionCapturePhase::Reserved(_)
        ));
        assert!(state.begin_region_submission().is_some());
        assert!(state.record_accepted(accepted));
        assert!(matches!(state.region_phase(), RegionCapturePhase::Accepted));
        assert_eq!(state.accepted_id(), Some(accepted));
        assert!(!state.record_accepted(other));
        assert!(!state.consume_accepted(other));
        assert_eq!(state.accepted_id(), Some(accepted));
        assert!(state.consume_accepted(accepted));
        assert_eq!(state.accepted_id(), None);
        assert!(!state.record_accepted(other));
    }

    #[test]
    fn terminal_finish_resets_every_region_phase() {
        let manager = CaptureManager::with_closed_channel_for_test();
        let mut state = CaptureState::new(manager);

        assert!(state.reserve_region(region_intent(Action::CaptureSelection)));
        state.set_exit_on_success(true);
        state.finish_capture_lifecycle();
        assert_region_lifecycle_is_idle(&state);

        assert!(state.reserve_region(region_intent(Action::CaptureFileSelection)));
        assert!(state.begin_region_submission().is_some());
        state.set_exit_on_success(true);
        state.finish_capture_lifecycle();
        assert_region_lifecycle_is_idle(&state);

        assert!(state.reserve_region(region_intent(Action::CaptureClipboardSelection)));
        assert!(state.begin_region_submission().is_some());
        assert!(state.record_accepted(CaptureRequestId::for_test(23)));
        state.set_exit_on_success(true);
        state.finish_capture_lifecycle();
        assert_region_lifecycle_is_idle(&state);

        // Terminal cleanup remains safe when called again by teardown.
        state.finish_capture_lifecycle();
        assert_region_lifecycle_is_idle(&state);
    }

    fn assert_region_lifecycle_is_idle(state: &CaptureState) {
        assert!(!state.is_in_progress());
        assert_eq!(state.accepted_id(), None);
        assert!(!state.exit_on_success());
        assert!(matches!(state.region_phase(), RegionCapturePhase::Idle));
    }

    #[test]
    fn capture_layout_context_rejects_output_or_geometry_generation_changes() {
        let context = CaptureLayoutContext::new(7, 3);

        assert!(context.matches(Some(7), 3));
        assert!(!context.matches(Some(8), 3));
        assert!(!context.matches(Some(7), 4));
        assert!(!context.matches(None, 3));
    }
}
