mod transaction;

use crate::backend::wayland::acquisition::{
    AcquisitionRecord, AcquisitionStage, ScreenAcquisitionBusy, ScreenAcquisitionCompletion,
    ScreenAcquisitionId, ScreenAcquisitionOutcome, ScreenAcquisitionOwner,
};
use crate::backend::wayland::zoom::{
    ZoomSourceOutcome, ZoomSourceTerminal, ZoomWaiter, ZoomWaiterOwner,
};
use crate::input::state::{EyedropperCaptureSource, ScreenCaptureSource};

use transaction::{
    AcquisitionTransactionRuntime, cancel_modal_owner_resources, release_owned_generation,
    report_inconsistent_capture_to, report_screen_terminal_to, route_acquisition_transaction,
};
pub(super) use transaction::{
    report_screen_source_activation_rejected_to, report_zoom_terminal_to,
};

use super::screen_image::displayed_screen_image;
use super::{OverlaySuppression, WaylandState};

impl AcquisitionTransactionRuntime for WaylandState {
    fn acquisition_slot(&self) -> Option<AcquisitionRecord> {
        self.data.screen_acquisition.slot().copied()
    }

    fn take_acquisition_record(&mut self) -> Option<AcquisitionRecord> {
        self.data.screen_acquisition.take()
    }

    fn take_matching_acquisition_record(
        &mut self,
        id: ScreenAcquisitionId,
        owner: ScreenAcquisitionOwner,
    ) -> Option<AcquisitionRecord> {
        self.data.screen_acquisition.take_matching(id, owner)
    }

    fn owner_waiter_matches(
        &self,
        record: &AcquisitionRecord,
        completion: &ScreenAcquisitionCompletion,
    ) -> bool {
        self.validate_owner_waiter(record, completion)
    }

    fn input_state(&mut self) -> &mut crate::input::InputState {
        &mut self.input_state
    }

    fn finish_eyedropper_ready(&mut self, installed_generation: u64) -> bool {
        self.finish_pending_eyedropper_capture(
            EyedropperCaptureSource::Frozen,
            Some(installed_generation),
        )
    }

    fn finish_ocr_ready(&mut self, installed_generation: u64) -> bool {
        self.finish_pending_ocr_capture(ScreenCaptureSource::Frozen, installed_generation)
    }

    fn finish_region_capture_ready(&mut self, installed_generation: u64) -> bool {
        self.finish_pending_region_capture(ScreenCaptureSource::Frozen, installed_generation)
    }

    fn handoff_region_capture_to_legacy(&mut self) {
        WaylandState::handoff_region_capture_to_legacy(self);
    }

    fn cancel_owner_ui(&mut self, owner: ScreenAcquisitionOwner) {
        WaylandState::cancel_owner_ui_only(self, owner);
    }

    fn clear_zoom_waiter_effect(&mut self, owner: ZoomWaiterOwner) {
        self.clear_zoom_waiter_for(owner);
    }

    fn frozen_generation(&self) -> u64 {
        self.frozen.image_generation()
    }

    fn frozen_active(&self) -> bool {
        self.input_state.frozen_active()
    }

    fn restore_xdg_after_frozen_effect(&mut self) {
        self.restore_xdg_after_frozen();
    }

    fn unfreeze_frozen_effect(&mut self) {
        self.frozen.unfreeze(&mut self.input_state);
    }

    fn abandon_frozen_effect(&mut self) {
        self.frozen.abandon_acquisition(&mut self.input_state);
    }

    fn frozen_completion(&self) -> Option<ScreenAcquisitionCompletion> {
        self.frozen.acquisition_completion().cloned()
    }

    fn take_matching_frozen_completion(
        &mut self,
        id: ScreenAcquisitionId,
        owner: ScreenAcquisitionOwner,
    ) -> Option<ScreenAcquisitionCompletion> {
        self.frozen.take_matching_acquisition_completion(id, owner)
    }

    fn take_frozen_capture_done(&mut self) -> bool {
        self.frozen.take_capture_done()
    }

    fn frozen_suppressed(&self) -> bool {
        self.data.overlay_suppression == OverlaySuppression::Frozen
    }

    fn end_frozen_suppression(&mut self) {
        self.exit_overlay_suppression(OverlaySuppression::Frozen);
    }
}

impl WaylandState {
    pub(in crate::backend::wayland) fn wait_for_current_zoom_capture(
        &mut self,
        owner: ZoomWaiterOwner,
    ) -> bool {
        let Some(id) = self.zoom.current_capture_id() else {
            return false;
        };
        self.data.zoom_waiter.register(ZoomWaiter { id, owner })
    }

    pub(in crate::backend::wayland) fn clear_zoom_waiter_for(
        &mut self,
        owner: ZoomWaiterOwner,
    ) -> bool {
        self.data.zoom_waiter.clear_owner(owner)
    }

    pub(in crate::backend::wayland) fn resolve_zoom_waiter(
        &mut self,
        terminal: ZoomSourceTerminal,
    ) {
        let Some((waiter, matches)) = self.data.zoom_waiter.take_for_terminal(&terminal) else {
            report_zoom_terminal_to(&mut self.input_state, None, &terminal);
            return;
        };
        if !matches {
            self.cancel_zoom_owner_ui_only(waiter.owner);
            report_inconsistent_capture_to(&mut self.input_state);
            return;
        }
        if !self.validate_zoom_waiter(waiter, &terminal) {
            self.cancel_zoom_owner_ui_only(waiter.owner);
            report_inconsistent_capture_to(&mut self.input_state);
            return;
        }

        self.report_zoom_terminal(waiter.owner, &terminal);
        match terminal.outcome {
            ZoomSourceOutcome::Ready {
                installed_generation,
            } => {
                let activated = match waiter.owner {
                    ZoomWaiterOwner::Eyedropper => {
                        self.finish_pending_eyedropper_capture(EyedropperCaptureSource::Zoom, None)
                    }
                    ZoomWaiterOwner::Ocr => self.finish_pending_ocr_capture(
                        ScreenCaptureSource::Zoom,
                        installed_generation,
                    ),
                    ZoomWaiterOwner::RegionCapture => self.finish_pending_region_capture(
                        ScreenCaptureSource::Zoom,
                        installed_generation,
                    ),
                };
                if !activated {
                    self.cancel_zoom_owner_ui_only(waiter.owner);
                    let owner = match waiter.owner {
                        ZoomWaiterOwner::Eyedropper => ScreenAcquisitionOwner::Eyedropper,
                        ZoomWaiterOwner::Ocr => ScreenAcquisitionOwner::Ocr,
                        ZoomWaiterOwner::RegionCapture => ScreenAcquisitionOwner::RegionCapture,
                    };
                    report_screen_source_activation_rejected_to(&mut self.input_state, owner);
                }
            }
            _ => self.cancel_zoom_owner_ui_only(waiter.owner),
        }
    }

    pub(in crate::backend::wayland) fn resolve_pending_zoom_terminal(&mut self) {
        if let Some(terminal) = self.zoom.take_source_terminal() {
            self.resolve_zoom_waiter(terminal);
        }
    }

    fn validate_zoom_waiter(&self, waiter: ZoomWaiter, terminal: &ZoomSourceTerminal) -> bool {
        let waiting = match waiter.owner {
            ZoomWaiterOwner::Eyedropper => {
                self.input_state.eyedropper_state().pending_source()
                    == Some(EyedropperCaptureSource::Zoom)
            }
            ZoomWaiterOwner::Ocr => self.data.active_screen_region.is_some_and(|region| {
                region.purpose() == crate::input::state::RegionPurposeTag::Ocr
                    && matches!(
                        region,
                        super::region_capture::ActiveScreenRegion::PendingZoom { .. }
                    )
            }),
            ZoomWaiterOwner::RegionCapture => {
                self.data.active_screen_region.is_some_and(|region| {
                    region.purpose().is_capture()
                        && matches!(
                            region,
                            super::region_capture::ActiveScreenRegion::PendingZoom { .. }
                        )
                        && matches!(
                            self.capture.region_phase(),
                            crate::backend::wayland::capture::RegionCapturePhase::Reserved(_)
                        )
                })
            }
        };
        if !waiting {
            return false;
        }
        match terminal.outcome {
            ZoomSourceOutcome::Ready {
                installed_generation,
            } => {
                self.zoom.image_generation() == installed_generation
                    && displayed_screen_image(
                        &self.zoom,
                        &self.frozen,
                        self.input_state.board_is_transparent(),
                    )
                    .is_some()
            }
            _ => true,
        }
    }

    fn report_zoom_terminal(&mut self, owner: ZoomWaiterOwner, terminal: &ZoomSourceTerminal) {
        report_zoom_terminal_to(&mut self.input_state, Some(owner), terminal);
    }

    fn cancel_zoom_owner_ui_only(&mut self, owner: ZoomWaiterOwner) {
        match owner {
            ZoomWaiterOwner::Eyedropper => self.cancel_eyedropper_ui_only(),
            ZoomWaiterOwner::Ocr => self.cancel_ocr_ui_only(),
            ZoomWaiterOwner::RegionCapture => self.cancel_region_capture_ui_and_lifecycle(),
        }
    }

    pub(in crate::backend::wayland) fn request_screen_acquisition(
        &mut self,
        owner: ScreenAcquisitionOwner,
    ) -> Result<ScreenAcquisitionId, ScreenAcquisitionBusy> {
        self.data.screen_acquisition.request(owner)
    }

    pub(in crate::backend::wayland) fn queued_screen_acquisition(
        &self,
    ) -> Option<AcquisitionRecord> {
        self.data
            .screen_acquisition
            .slot()
            .copied()
            .filter(|record| record.stage == AcquisitionStage::Queued)
    }

    pub(in crate::backend::wayland) fn screen_acquisition_slot(&self) -> Option<AcquisitionRecord> {
        self.data.screen_acquisition.slot().copied()
    }

    pub(in crate::backend::wayland) fn mark_screen_acquisition_started(
        &mut self,
        id: ScreenAcquisitionId,
        owner: ScreenAcquisitionOwner,
    ) {
        // This transition is a required side effect, not a debug-only check.
        // Keeping the mutation outside `debug_assert!` prevents release builds
        // from leaving the record queued and starting it again next pass.
        let transitioned = self.data.screen_acquisition.mark_started(id, owner);
        debug_assert!(transitioned, "the queued acquisition was just started");
    }

    pub(in crate::backend::wayland) fn complete_queued_acquisition(
        &mut self,
        id: ScreenAcquisitionId,
        owner: ScreenAcquisitionOwner,
        outcome: ScreenAcquisitionOutcome,
    ) {
        debug_assert!(!matches!(outcome, ScreenAcquisitionOutcome::Ready { .. }));
        self.route_acquisition_completion(ScreenAcquisitionCompletion { id, owner, outcome });
    }

    pub(in crate::backend::wayland) fn route_acquisition_completion(
        &mut self,
        completion: ScreenAcquisitionCompletion,
    ) {
        route_acquisition_transaction(self, completion);
    }

    fn validate_owner_waiter(
        &self,
        record: &AcquisitionRecord,
        completion: &ScreenAcquisitionCompletion,
    ) -> bool {
        let waiting = match completion.owner {
            ScreenAcquisitionOwner::UserFreeze => true,
            ScreenAcquisitionOwner::Eyedropper => {
                debug_assert_eq!(record.owner, ScreenAcquisitionOwner::Eyedropper);
                self.input_state.eyedropper_state().pending_source()
                    == Some(EyedropperCaptureSource::Frozen)
            }
            ScreenAcquisitionOwner::Ocr => self.data.active_screen_region.is_some_and(|region| {
                region.purpose() == crate::input::state::RegionPurposeTag::Ocr
                    && region.waits_for_acquisition(completion.id)
            }),
            ScreenAcquisitionOwner::RegionCapture => {
                self.data.active_screen_region.is_some_and(|region| {
                    region.purpose().is_capture()
                        && region.waits_for_acquisition(completion.id)
                        && matches!(
                            self.capture.region_phase(),
                            crate::backend::wayland::capture::RegionCapturePhase::Reserved(_)
                        )
                })
            }
        };
        if !waiting {
            return false;
        }
        let ScreenAcquisitionOutcome::Ready {
            installed_generation,
        } = completion.outcome
        else {
            return true;
        };
        match completion.owner {
            ScreenAcquisitionOwner::UserFreeze => true,
            ScreenAcquisitionOwner::Eyedropper => {
                self.frozen.image_generation() == installed_generation
                    && displayed_screen_image(
                        &self.zoom,
                        &self.frozen,
                        self.input_state.board_is_transparent(),
                    )
                    .is_some()
            }
            ScreenAcquisitionOwner::Ocr => {
                self.frozen.image_generation() == installed_generation
                    && displayed_screen_image(
                        &self.zoom,
                        &self.frozen,
                        self.input_state.board_is_transparent(),
                    )
                    .is_some()
            }
            ScreenAcquisitionOwner::RegionCapture => {
                self.frozen.image_generation() == installed_generation
                    && displayed_screen_image(
                        &self.zoom,
                        &self.frozen,
                        self.input_state.board_is_transparent(),
                    )
                    .is_some()
            }
        }
    }

    pub(in crate::backend::wayland) fn report_terminal(
        &mut self,
        owner: ScreenAcquisitionOwner,
        outcome: &ScreenAcquisitionOutcome,
    ) {
        report_screen_terminal_to(&mut self.input_state, owner, outcome);
    }

    pub(in crate::backend::wayland) fn cancel_eyedropper_ui_only(&mut self) {
        self.data.active_eyedropper_source = None;
        let _ = self.input_state.cancel_eyedropper();
    }

    pub(in crate::backend::wayland) fn cancel_ocr_ui_only(&mut self) {
        self.clear_screen_region_ui_only();
    }

    fn cancel_owner_ui_only(&mut self, owner: ScreenAcquisitionOwner) {
        match owner {
            ScreenAcquisitionOwner::UserFreeze => {}
            ScreenAcquisitionOwner::Eyedropper => self.cancel_eyedropper_ui_only(),
            ScreenAcquisitionOwner::Ocr => self.cancel_ocr_ui_only(),
            ScreenAcquisitionOwner::RegionCapture => self.cancel_region_capture_ui_and_lifecycle(),
        }
    }

    pub(super) fn cancel_modal_owner_resources(
        &mut self,
        owner: ScreenAcquisitionOwner,
        pending_acquisition: Option<ScreenAcquisitionId>,
        owned_generation: Option<u64>,
    ) {
        cancel_modal_owner_resources(self, owner, pending_acquisition, owned_generation);
    }

    pub(super) fn release_owned_frozen_generation(&mut self, generation: u64) -> bool {
        release_owned_generation(self, generation)
    }
}
