use crate::backend::wayland::acquisition::{
    AcquisitionRecord, AcquisitionStage, ScreenAcquisitionBusy, ScreenAcquisitionCompletion,
    ScreenAcquisitionId, ScreenAcquisitionOutcome, ScreenAcquisitionOwner,
    rejected_ready_generation,
};
use crate::backend::wayland::zoom::{
    ZoomSourceOutcome, ZoomSourceTerminal, ZoomWaiter, ZoomWaiterOwner,
};
use crate::input::state::{EyedropperCaptureSource, ScreenCaptureSource, Toast, ToastPriority};

use super::region_capture::owned_generation_is_current;
use super::screen_image::displayed_screen_image;
use super::{OverlaySuppression, WaylandState};

const INCONSISTENT_CAPTURE_MESSAGE: &str = "Screen capture state was inconsistent; try again.";

fn report_inconsistent_capture_to(input_state: &mut crate::input::InputState) {
    input_state.push_toast(
        ToastPriority::Critical,
        "capture",
        Toast::error(INCONSISTENT_CAPTURE_MESSAGE),
    );
}

fn zoom_terminal_report(
    owner: Option<ZoomWaiterOwner>,
    terminal: &ZoomSourceTerminal,
) -> Option<(&'static str, &str)> {
    if let Some(report) = terminal.report.as_ref() {
        return Some((report.source, report.message.as_str()));
    }
    let outcome = &terminal.outcome;
    if matches!(
        outcome,
        ZoomSourceOutcome::Ready { .. }
            | ZoomSourceOutcome::Cancelled
            | ZoomSourceOutcome::StaleLayout
    ) {
        return None;
    }
    match owner? {
        ZoomWaiterOwner::Eyedropper => Some(("eyedropper", "Screen eyedropper capture failed.")),
        ZoomWaiterOwner::Ocr => Some(("ocr", "Screen capture for text recognition failed.")),
        ZoomWaiterOwner::RegionCapture => None,
    }
}

pub(super) fn report_zoom_terminal_to(
    input_state: &mut crate::input::InputState,
    owner: Option<ZoomWaiterOwner>,
    terminal: &ZoomSourceTerminal,
) {
    let Some((source, message)) = zoom_terminal_report(owner, terminal) else {
        return;
    };
    input_state.push_toast(ToastPriority::Critical, source, Toast::error(message));
}

fn report_screen_terminal_to(
    input_state: &mut crate::input::InputState,
    owner: ScreenAcquisitionOwner,
    outcome: &ScreenAcquisitionOutcome,
) {
    if owner == ScreenAcquisitionOwner::UserFreeze
        && matches!(outcome, ScreenAcquisitionOutcome::Unavailable)
    {
        input_state.push_toast(
            ToastPriority::Info,
            "capture",
            Toast::warning("Freeze is already preparing another overlay operation."),
        );
        return;
    }
    let message = match outcome {
        ScreenAcquisitionOutcome::Ready { .. } | ScreenAcquisitionOutcome::Cancelled => return,
        ScreenAcquisitionOutcome::StaleLayout => "Freeze failed after the display layout changed",
        ScreenAcquisitionOutcome::Failed(message) => message,
        ScreenAcquisitionOutcome::Unavailable => match owner {
            ScreenAcquisitionOwner::UserFreeze => unreachable!("handled above"),
            ScreenAcquisitionOwner::Eyedropper => "Screen eyedropper capture failed.",
            ScreenAcquisitionOwner::Ocr => "Screen capture for text recognition failed.",
            ScreenAcquisitionOwner::RegionCapture => return,
        },
    };
    let source = match owner {
        ScreenAcquisitionOwner::UserFreeze => "freeze",
        ScreenAcquisitionOwner::Eyedropper => "eyedropper",
        ScreenAcquisitionOwner::Ocr => "ocr",
        ScreenAcquisitionOwner::RegionCapture => "capture",
    };
    input_state.push_toast(
        ToastPriority::Critical,
        source,
        Toast::error(message.to_string()),
    );
}

/// Effect boundary for the acquisition transaction. `WaylandState` supplies
/// the live protocol/UI effects; tests supply observable component effects.
/// The routing and fail-closed decisions below are shared production code.
trait AcquisitionTransactionRuntime {
    fn acquisition_slot(&self) -> Option<AcquisitionRecord>;
    fn take_acquisition_record(&mut self) -> Option<AcquisitionRecord>;
    fn take_matching_acquisition_record(
        &mut self,
        id: ScreenAcquisitionId,
        owner: ScreenAcquisitionOwner,
    ) -> Option<AcquisitionRecord>;
    fn owner_waiter_matches(
        &self,
        record: &AcquisitionRecord,
        completion: &ScreenAcquisitionCompletion,
    ) -> bool;
    fn input_state(&mut self) -> &mut crate::input::InputState;
    fn finish_eyedropper_ready(&mut self, installed_generation: u64);
    fn finish_ocr_ready(&mut self, installed_generation: u64);
    fn cancel_owner_ui(&mut self, owner: ScreenAcquisitionOwner);
    fn clear_zoom_waiter_effect(&mut self, owner: ZoomWaiterOwner);
    fn frozen_generation(&self) -> u64;
    fn frozen_active(&self) -> bool;
    fn restore_xdg_after_frozen_effect(&mut self);
    fn unfreeze_frozen_effect(&mut self);
    fn abandon_frozen_effect(&mut self);
    fn frozen_completion(&self) -> Option<ScreenAcquisitionCompletion>;
    fn take_matching_frozen_completion(
        &mut self,
        id: ScreenAcquisitionId,
        owner: ScreenAcquisitionOwner,
    ) -> Option<ScreenAcquisitionCompletion>;
    fn take_frozen_capture_done(&mut self) -> bool;
    fn frozen_suppressed(&self) -> bool;
    fn end_frozen_suppression(&mut self);
}

fn release_owned_generation<R: AcquisitionTransactionRuntime>(
    runtime: &mut R,
    generation: u64,
) -> bool {
    if !owned_generation_is_current(
        generation,
        runtime.frozen_generation(),
        runtime.frozen_active(),
    ) {
        return false;
    }
    runtime.restore_xdg_after_frozen_effect();
    runtime.unfreeze_frozen_effect();
    true
}

fn abandon_started_acquisition<R: AcquisitionTransactionRuntime>(runtime: &mut R) {
    runtime.restore_xdg_after_frozen_effect();
    runtime.abandon_frozen_effect();
}

fn cancel_owner_transition<R: AcquisitionTransactionRuntime>(
    runtime: &mut R,
    owner: ScreenAcquisitionOwner,
) {
    match owner {
        ScreenAcquisitionOwner::Eyedropper => {
            runtime.clear_zoom_waiter_effect(ZoomWaiterOwner::Eyedropper);
        }
        ScreenAcquisitionOwner::Ocr => {
            runtime.clear_zoom_waiter_effect(ZoomWaiterOwner::Ocr);
        }
        ScreenAcquisitionOwner::UserFreeze | ScreenAcquisitionOwner::RegionCapture => {}
    }
    runtime.cancel_owner_ui(owner);
}

fn fail_closed_acquisition_transaction<R: AcquisitionTransactionRuntime>(
    runtime: &mut R,
    record: Option<AcquisitionRecord>,
    completion: ScreenAcquisitionCompletion,
) {
    if record.is_some_and(|record| {
        record.stage == AcquisitionStage::Started
            && (record.id != completion.id || record.owner != completion.owner)
    }) {
        abandon_started_acquisition(runtime);
    }
    if runtime.frozen_suppressed() {
        runtime.end_frozen_suppression();
    }
    if let Some(generation) = rejected_ready_generation(
        completion.owner,
        &completion.outcome,
        runtime.frozen_generation(),
        runtime.frozen_active(),
    ) {
        release_owned_generation(runtime, generation);
    }
    cancel_owner_transition(runtime, completion.owner);
    if let Some(record) = record
        && record.owner != completion.owner
    {
        cancel_owner_transition(runtime, record.owner);
    }
    report_inconsistent_capture_to(runtime.input_state());
}

fn route_acquisition_transaction<R: AcquisitionTransactionRuntime>(
    runtime: &mut R,
    completion: ScreenAcquisitionCompletion,
) {
    let registry_matches = runtime
        .acquisition_slot()
        .is_some_and(|record| record.id == completion.id && record.owner == completion.owner);
    let record = runtime.take_acquisition_record();
    if !registry_matches {
        fail_closed_acquisition_transaction(runtime, record, completion);
        return;
    }
    let record = record.expect("a matched screen acquisition record exists");
    if !runtime.owner_waiter_matches(&record, &completion) {
        fail_closed_acquisition_transaction(runtime, Some(record), completion);
        return;
    }

    report_screen_terminal_to(runtime.input_state(), completion.owner, &completion.outcome);
    match (completion.owner, completion.outcome) {
        (ScreenAcquisitionOwner::UserFreeze, _) => {}
        (
            ScreenAcquisitionOwner::Eyedropper,
            ScreenAcquisitionOutcome::Ready {
                installed_generation,
            },
        ) => runtime.finish_eyedropper_ready(installed_generation),
        (
            ScreenAcquisitionOwner::Ocr,
            ScreenAcquisitionOutcome::Ready {
                installed_generation,
            },
        ) => runtime.finish_ocr_ready(installed_generation),
        (ScreenAcquisitionOwner::Eyedropper | ScreenAcquisitionOwner::Ocr, _) => {
            cancel_owner_transition(runtime, completion.owner);
        }
        (ScreenAcquisitionOwner::RegionCapture, _) => {
            // Phase 1 supplies the capture-purpose transition and legacy handoff.
        }
    }
}

fn cancel_acquisition_transaction<R: AcquisitionTransactionRuntime>(
    runtime: &mut R,
    id: ScreenAcquisitionId,
    owner: ScreenAcquisitionOwner,
) -> bool {
    let Some(record) = runtime
        .acquisition_slot()
        .filter(|record| record.id == id && record.owner == owner)
    else {
        log::debug!("Ignoring stale {owner:?} screen acquisition cancellation for {id:?}");
        return false;
    };

    if record.stage == AcquisitionStage::Started
        && runtime.frozen_completion().is_some()
        && !runtime
            .frozen_completion()
            .is_some_and(|completion| completion.id == id && completion.owner == owner)
    {
        log::debug!(
            "Retaining mismatched frozen completion for fail-closed routing while cancelling {owner:?} {id:?}"
        );
        return false;
    }

    let taken = runtime
        .take_matching_acquisition_record(id, owner)
        .expect("matching acquisition record was just validated");
    if taken.stage == AcquisitionStage::Queued {
        return true;
    }

    if let Some(completion) = runtime.take_matching_frozen_completion(id, owner) {
        report_screen_terminal_to(runtime.input_state(), owner, &completion.outcome);
        if owner != ScreenAcquisitionOwner::UserFreeze
            && let ScreenAcquisitionOutcome::Ready {
                installed_generation,
            } = completion.outcome
        {
            release_owned_generation(runtime, installed_generation);
        }
    } else if runtime.frozen_completion().is_none() {
        abandon_started_acquisition(runtime);
    }
    let _ = runtime.take_frozen_capture_done();
    if runtime.frozen_suppressed() {
        runtime.end_frozen_suppression();
    }
    true
}

fn cancel_modal_owner_resources<R: AcquisitionTransactionRuntime>(
    runtime: &mut R,
    owner: ScreenAcquisitionOwner,
    pending_acquisition: Option<ScreenAcquisitionId>,
    owned_generation: Option<u64>,
) {
    if let Some(id) = pending_acquisition {
        cancel_acquisition_transaction(runtime, id, owner);
    }
    if let Some(generation) = owned_generation {
        release_owned_generation(runtime, generation);
    }
    cancel_owner_transition(runtime, owner);
}

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

    fn finish_eyedropper_ready(&mut self, installed_generation: u64) {
        self.finish_pending_eyedropper_capture(
            EyedropperCaptureSource::Frozen,
            Some(installed_generation),
        );
    }

    fn finish_ocr_ready(&mut self, installed_generation: u64) {
        self.finish_pending_ocr_capture(ScreenCaptureSource::Frozen, installed_generation);
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
            } => match waiter.owner {
                ZoomWaiterOwner::Eyedropper => {
                    self.finish_pending_eyedropper_capture(EyedropperCaptureSource::Zoom, None)
                }
                ZoomWaiterOwner::Ocr => {
                    self.finish_pending_ocr_capture(ScreenCaptureSource::Zoom, installed_generation)
                }
                ZoomWaiterOwner::RegionCapture => {}
            },
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
            ZoomWaiterOwner::RegionCapture => false,
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
            ZoomWaiterOwner::RegionCapture => {}
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
    ) -> bool {
        self.data.screen_acquisition.mark_started(id, owner)
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
            ScreenAcquisitionOwner::RegionCapture => false,
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
            ScreenAcquisitionOwner::RegionCapture => false,
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
            ScreenAcquisitionOwner::RegionCapture => {
                // Phase 1 supplies the region UI-only teardown.
            }
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::wayland::acquisition::ScreenAcquisitionRegistry;
    use crate::input::state::test_support::make_test_input_state;

    struct TransactionRuntime {
        registry: ScreenAcquisitionRegistry,
        waiter_matches: bool,
        input_state: crate::input::InputState,
        finished_eyedropper: Vec<u64>,
        finished_ocr: Vec<u64>,
        cancelled_owners: Vec<ScreenAcquisitionOwner>,
        cleared_zoom_waiters: Vec<ZoomWaiterOwner>,
        frozen_generation: u64,
        frozen_active: bool,
        restore_count: usize,
        unfreeze_count: usize,
        abandon_count: usize,
        frozen_completion: Option<ScreenAcquisitionCompletion>,
        capture_done: bool,
        frozen_suppressed: bool,
        suppression_end_count: usize,
    }

    impl TransactionRuntime {
        fn started(owner: ScreenAcquisitionOwner) -> Self {
            let mut registry = ScreenAcquisitionRegistry::default();
            let id = registry.request(owner).expect("transaction slot");
            assert!(registry.mark_started(id, owner));
            Self {
                registry,
                waiter_matches: true,
                input_state: make_test_input_state(),
                finished_eyedropper: Vec::new(),
                finished_ocr: Vec::new(),
                cancelled_owners: Vec::new(),
                cleared_zoom_waiters: Vec::new(),
                frozen_generation: 7,
                frozen_active: true,
                restore_count: 0,
                unfreeze_count: 0,
                abandon_count: 0,
                frozen_completion: None,
                capture_done: false,
                frozen_suppressed: true,
                suppression_end_count: 0,
            }
        }

        fn record(&self) -> Option<AcquisitionRecord> {
            self.registry.slot().copied()
        }
    }

    impl AcquisitionTransactionRuntime for TransactionRuntime {
        fn acquisition_slot(&self) -> Option<AcquisitionRecord> {
            self.record()
        }

        fn take_acquisition_record(&mut self) -> Option<AcquisitionRecord> {
            self.registry.take()
        }

        fn take_matching_acquisition_record(
            &mut self,
            id: ScreenAcquisitionId,
            owner: ScreenAcquisitionOwner,
        ) -> Option<AcquisitionRecord> {
            self.registry.take_matching(id, owner)
        }

        fn owner_waiter_matches(
            &self,
            _record: &AcquisitionRecord,
            _completion: &ScreenAcquisitionCompletion,
        ) -> bool {
            self.waiter_matches
        }

        fn input_state(&mut self) -> &mut crate::input::InputState {
            &mut self.input_state
        }

        fn finish_eyedropper_ready(&mut self, installed_generation: u64) {
            self.finished_eyedropper.push(installed_generation);
        }

        fn finish_ocr_ready(&mut self, installed_generation: u64) {
            self.finished_ocr.push(installed_generation);
        }

        fn cancel_owner_ui(&mut self, owner: ScreenAcquisitionOwner) {
            self.cancelled_owners.push(owner);
        }

        fn clear_zoom_waiter_effect(&mut self, owner: ZoomWaiterOwner) {
            self.cleared_zoom_waiters.push(owner);
        }

        fn frozen_generation(&self) -> u64 {
            self.frozen_generation
        }

        fn frozen_active(&self) -> bool {
            self.frozen_active
        }

        fn restore_xdg_after_frozen_effect(&mut self) {
            self.restore_count += 1;
        }

        fn unfreeze_frozen_effect(&mut self) {
            self.unfreeze_count += 1;
            self.frozen_active = false;
        }

        fn abandon_frozen_effect(&mut self) {
            self.abandon_count += 1;
            self.frozen_active = false;
        }

        fn frozen_completion(&self) -> Option<ScreenAcquisitionCompletion> {
            self.frozen_completion.clone()
        }

        fn take_matching_frozen_completion(
            &mut self,
            id: ScreenAcquisitionId,
            owner: ScreenAcquisitionOwner,
        ) -> Option<ScreenAcquisitionCompletion> {
            if !self
                .frozen_completion
                .as_ref()
                .is_some_and(|completion| completion.id == id && completion.owner == owner)
            {
                return None;
            }
            self.frozen_completion.take()
        }

        fn take_frozen_capture_done(&mut self) -> bool {
            std::mem::take(&mut self.capture_done)
        }

        fn frozen_suppressed(&self) -> bool {
            self.frozen_suppressed
        }

        fn end_frozen_suppression(&mut self) {
            self.frozen_suppressed = false;
            self.suppression_end_count += 1;
        }
    }

    #[test]
    fn zoom_owner_outcome_reporting_matrix_is_typed() {
        let outcomes = [
            (
                ZoomSourceOutcome::Ready {
                    installed_generation: 1,
                },
                false,
            ),
            (ZoomSourceOutcome::Cancelled, false),
            (ZoomSourceOutcome::StaleLayout, false),
            (ZoomSourceOutcome::Aborted, true),
            (ZoomSourceOutcome::Deactivated, true),
            (ZoomSourceOutcome::Failed("detail".to_string()), true),
        ];

        for owner in [ZoomWaiterOwner::Eyedropper, ZoomWaiterOwner::Ocr] {
            for (outcome, reports) in &outcomes {
                assert_eq!(
                    zoom_terminal_report(
                        Some(owner),
                        &ZoomSourceTerminal::for_test(outcome.clone(), None),
                    )
                    .is_some(),
                    *reports,
                    "owner={owner:?} outcome={outcome:?}"
                );
            }
        }
        for (outcome, _) in &outcomes {
            assert_eq!(
                zoom_terminal_report(
                    Some(ZoomWaiterOwner::RegionCapture),
                    &ZoomSourceTerminal::for_test(outcome.clone(), None),
                ),
                None
            );
        }
    }

    #[test]
    fn specific_zoom_failure_report_replaces_the_owner_fallback() {
        let terminal = ZoomSourceTerminal::for_test(
            ZoomSourceOutcome::Failed("specific backend failure".to_string()),
            Some(crate::backend::wayland::zoom::ZoomTerminalReport {
                source: "zoom",
                message: "specific backend failure".to_string(),
            }),
        );

        for owner in [Some(ZoomWaiterOwner::Ocr), None] {
            let mut input_state = make_test_input_state();

            report_zoom_terminal_to(&mut input_state, owner, &terminal);

            assert_eq!(input_state.test_toast_count(), 1);
            assert_eq!(
                input_state.test_active_toast_message(),
                Some("specific backend failure")
            );
        }
    }

    #[test]
    fn zoom_correlation_failure_emits_exactly_one_inconsistency_toast() {
        let mut input_state = make_test_input_state();

        report_inconsistent_capture_to(&mut input_state);

        assert_eq!(input_state.test_toast_count(), 1);
        assert_eq!(
            input_state.test_active_toast_message(),
            Some(INCONSISTENT_CAPTURE_MESSAGE)
        );
    }

    #[test]
    fn screen_terminal_reporting_is_exactly_once_for_the_full_owner_outcome_matrix() {
        for owner in [
            ScreenAcquisitionOwner::UserFreeze,
            ScreenAcquisitionOwner::Eyedropper,
            ScreenAcquisitionOwner::Ocr,
            ScreenAcquisitionOwner::RegionCapture,
        ] {
            let key = match owner {
                ScreenAcquisitionOwner::UserFreeze => "freeze",
                ScreenAcquisitionOwner::Eyedropper => "eyedropper",
                ScreenAcquisitionOwner::Ocr => "ocr",
                ScreenAcquisitionOwner::RegionCapture => "capture",
            };
            let unavailable = match owner {
                ScreenAcquisitionOwner::UserFreeze => Some((
                    "capture",
                    "Freeze is already preparing another overlay operation.",
                )),
                ScreenAcquisitionOwner::Eyedropper => {
                    Some((key, "Screen eyedropper capture failed."))
                }
                ScreenAcquisitionOwner::Ocr => {
                    Some((key, "Screen capture for text recognition failed."))
                }
                ScreenAcquisitionOwner::RegionCapture => None,
            };
            let cases = [
                (
                    ScreenAcquisitionOutcome::Ready {
                        installed_generation: 7,
                    },
                    None,
                ),
                (ScreenAcquisitionOutcome::Cancelled, None),
                (ScreenAcquisitionOutcome::Unavailable, unavailable),
                (
                    ScreenAcquisitionOutcome::StaleLayout,
                    Some((key, "Freeze failed after the display layout changed")),
                ),
                (
                    ScreenAcquisitionOutcome::Failed("specific backend failure".to_string()),
                    Some((key, "specific backend failure")),
                ),
            ];

            for (outcome, expected) in cases {
                let mut input_state = make_test_input_state();

                report_screen_terminal_to(&mut input_state, owner, &outcome);

                assert_eq!(
                    input_state.test_toast_count(),
                    usize::from(expected.is_some()),
                    "owner={owner:?} outcome={outcome:?}"
                );
                assert_eq!(
                    input_state.test_active_toast_message(),
                    expected.map(|(_, message)| message),
                    "owner={owner:?} outcome={outcome:?}"
                );
                assert_eq!(
                    input_state.test_active_toast_key(),
                    expected.map(|(key, _)| key),
                    "owner={owner:?} outcome={outcome:?}"
                );
            }
        }
    }

    #[test]
    fn matched_ready_completions_run_the_owner_production_transactions() {
        for owner in [
            ScreenAcquisitionOwner::UserFreeze,
            ScreenAcquisitionOwner::Eyedropper,
            ScreenAcquisitionOwner::Ocr,
        ] {
            let mut runtime = TransactionRuntime::started(owner);
            if owner == ScreenAcquisitionOwner::Ocr {
                runtime.frozen_generation = 99;
            }
            let id = runtime.record().expect("started record").id;

            route_acquisition_transaction(
                &mut runtime,
                ScreenAcquisitionCompletion {
                    id,
                    owner,
                    outcome: ScreenAcquisitionOutcome::Ready {
                        installed_generation: 7,
                    },
                },
            );

            assert_eq!(runtime.record(), None, "owner={owner:?}");
            assert_eq!(
                runtime.finished_eyedropper,
                if owner == ScreenAcquisitionOwner::Eyedropper {
                    vec![7]
                } else {
                    Vec::new()
                },
                "owner={owner:?}"
            );
            assert_eq!(
                runtime.finished_ocr,
                if owner == ScreenAcquisitionOwner::Ocr {
                    vec![7]
                } else {
                    Vec::new()
                },
                "owner={owner:?}"
            );
            assert_eq!(runtime.input_state.test_toast_count(), 0);
            assert_eq!(runtime.restore_count, 0);
            assert_eq!(runtime.unfreeze_count, 0);
        }
    }

    #[test]
    fn matched_nonready_completions_report_then_cancel_the_owner_ui() {
        for owner in [
            ScreenAcquisitionOwner::Eyedropper,
            ScreenAcquisitionOwner::Ocr,
        ] {
            for (outcome, expected_toasts) in [
                (ScreenAcquisitionOutcome::Cancelled, 0),
                (ScreenAcquisitionOutcome::Unavailable, 1),
                (ScreenAcquisitionOutcome::StaleLayout, 1),
                (
                    ScreenAcquisitionOutcome::Failed("activation rejected".to_string()),
                    1,
                ),
            ] {
                let mut runtime = TransactionRuntime::started(owner);
                let id = runtime.record().expect("started record").id;

                route_acquisition_transaction(
                    &mut runtime,
                    ScreenAcquisitionCompletion {
                        id,
                        owner,
                        outcome: outcome.clone(),
                    },
                );

                assert_eq!(
                    runtime.record(),
                    None,
                    "owner={owner:?} outcome={outcome:?}"
                );
                assert_eq!(
                    runtime.cancelled_owners,
                    vec![owner],
                    "owner={owner:?} outcome={outcome:?}"
                );
                assert_eq!(
                    runtime.input_state.test_toast_count(),
                    expected_toasts,
                    "owner={owner:?} outcome={outcome:?}"
                );
                assert_eq!(runtime.restore_count, 0);
                assert_eq!(runtime.unfreeze_count, 0);
            }
        }
    }

    #[test]
    fn id_owner_waiter_and_empty_slot_mismatches_fail_closed_once() {
        let cases = ["id", "owner", "waiter", "empty"];
        for case in cases {
            let mut runtime = TransactionRuntime::started(ScreenAcquisitionOwner::Ocr);
            let record = runtime.record().expect("started record");
            let (id, owner) = match case {
                "id" => {
                    let mut other = ScreenAcquisitionRegistry::default();
                    other
                        .request(ScreenAcquisitionOwner::Ocr)
                        .expect("first other id");
                    other.take();
                    let different = other
                        .request(ScreenAcquisitionOwner::Ocr)
                        .expect("different id");
                    (different, ScreenAcquisitionOwner::Ocr)
                }
                "owner" => (record.id, ScreenAcquisitionOwner::Eyedropper),
                "waiter" => {
                    runtime.waiter_matches = false;
                    (record.id, record.owner)
                }
                "empty" => {
                    runtime.registry.take();
                    (record.id, record.owner)
                }
                _ => unreachable!(),
            };

            route_acquisition_transaction(
                &mut runtime,
                ScreenAcquisitionCompletion {
                    id,
                    owner,
                    outcome: ScreenAcquisitionOutcome::Failed("failure".to_string()),
                },
            );

            assert_eq!(runtime.record(), None, "case={case}");
            assert_eq!(runtime.input_state.test_toast_count(), 1, "case={case}");
            assert_eq!(
                runtime.input_state.test_active_toast_message(),
                Some(INCONSISTENT_CAPTURE_MESSAGE),
                "case={case}"
            );
            assert!(!runtime.frozen_suppressed, "case={case}");
            assert_eq!(runtime.suppression_end_count, 1, "case={case}");
            assert_eq!(
                runtime.abandon_count,
                usize::from(matches!(case, "id" | "owner")),
                "case={case}"
            );
        }
    }

    #[test]
    fn activation_terminal_then_same_batch_cancel_releases_once_and_cleans_up() {
        let mut runtime = TransactionRuntime::started(ScreenAcquisitionOwner::Ocr);
        let id = runtime.record().expect("started record").id;
        runtime.frozen_completion = Some(ScreenAcquisitionCompletion {
            id,
            owner: ScreenAcquisitionOwner::Ocr,
            outcome: ScreenAcquisitionOutcome::Ready {
                installed_generation: 7,
            },
        });
        runtime.capture_done = true;

        assert!(cancel_acquisition_transaction(
            &mut runtime,
            id,
            ScreenAcquisitionOwner::Ocr,
        ));

        assert_eq!(runtime.record(), None);
        assert_eq!(runtime.frozen_completion, None);
        assert!(!runtime.capture_done);
        assert!(!runtime.frozen_active);
        assert_eq!(runtime.restore_count, 1);
        assert_eq!(runtime.unfreeze_count, 1);
        assert!(!runtime.frozen_suppressed);
        assert_eq!(runtime.suppression_end_count, 1);
        assert_eq!(runtime.input_state.test_toast_count(), 0);

        assert!(!cancel_acquisition_transaction(
            &mut runtime,
            id,
            ScreenAcquisitionOwner::Ocr,
        ));
        assert_eq!(runtime.restore_count, 1);
        assert_eq!(runtime.unfreeze_count, 1);
        assert_eq!(runtime.suppression_end_count, 1);
    }

    #[test]
    fn started_and_queued_cancellation_have_exact_resource_postconditions() {
        let mut started = TransactionRuntime::started(ScreenAcquisitionOwner::Eyedropper);
        let started_id = started.record().expect("started record").id;
        started.capture_done = true;

        assert!(cancel_acquisition_transaction(
            &mut started,
            started_id,
            ScreenAcquisitionOwner::Eyedropper,
        ));
        assert_eq!(started.record(), None);
        assert_eq!(started.restore_count, 1);
        assert_eq!(started.abandon_count, 1);
        assert_eq!(started.unfreeze_count, 0);
        assert!(!started.capture_done);
        assert!(!started.frozen_suppressed);
        assert_eq!(started.suppression_end_count, 1);
        assert_eq!(started.input_state.test_toast_count(), 0);

        let mut queued = TransactionRuntime::started(ScreenAcquisitionOwner::Ocr);
        let queued_record = queued.registry.take().expect("started record");
        let queued_id = queued
            .registry
            .request(ScreenAcquisitionOwner::Ocr)
            .expect("queued replacement");
        assert_ne!(queued_record.id, queued_id);
        queued.frozen_suppressed = false;

        assert!(cancel_acquisition_transaction(
            &mut queued,
            queued_id,
            ScreenAcquisitionOwner::Ocr,
        ));
        assert_eq!(queued.record(), None);
        assert_eq!(queued.restore_count, 0);
        assert_eq!(queued.abandon_count, 0);
        assert_eq!(queued.unfreeze_count, 0);
        assert_eq!(queued.suppression_end_count, 0);
    }

    #[test]
    fn cancellation_reports_matching_nonready_terminals_without_releasing_pixels() {
        for outcome in [
            ScreenAcquisitionOutcome::Cancelled,
            ScreenAcquisitionOutcome::Unavailable,
            ScreenAcquisitionOutcome::StaleLayout,
            ScreenAcquisitionOutcome::Failed("activation rejected".to_string()),
        ] {
            let mut runtime = TransactionRuntime::started(ScreenAcquisitionOwner::Ocr);
            let id = runtime.record().expect("started record").id;
            runtime.frozen_completion = Some(ScreenAcquisitionCompletion {
                id,
                owner: ScreenAcquisitionOwner::Ocr,
                outcome: outcome.clone(),
            });

            assert!(cancel_acquisition_transaction(
                &mut runtime,
                id,
                ScreenAcquisitionOwner::Ocr,
            ));

            assert_eq!(runtime.frozen_completion, None, "outcome={outcome:?}");
            assert_eq!(runtime.restore_count, 0, "outcome={outcome:?}");
            assert_eq!(runtime.unfreeze_count, 0, "outcome={outcome:?}");
            assert_eq!(
                runtime.input_state.test_toast_count(),
                usize::from(!matches!(outcome, ScreenAcquisitionOutcome::Cancelled)),
                "outcome={outcome:?}"
            );
            assert!(!runtime.frozen_suppressed, "outcome={outcome:?}");
        }
    }

    #[test]
    fn mismatched_terminal_is_retained_for_fail_closed_routing() {
        let mut runtime = TransactionRuntime::started(ScreenAcquisitionOwner::Ocr);
        let id = runtime.record().expect("started record").id;
        let mut other = ScreenAcquisitionRegistry::default();
        other
            .request(ScreenAcquisitionOwner::Ocr)
            .expect("other first id");
        other.take();
        let other_id = other
            .request(ScreenAcquisitionOwner::Ocr)
            .expect("different completion id");
        runtime.frozen_completion = Some(ScreenAcquisitionCompletion {
            id: other_id,
            owner: ScreenAcquisitionOwner::Ocr,
            outcome: ScreenAcquisitionOutcome::Failed("other".to_string()),
        });

        assert!(!cancel_acquisition_transaction(
            &mut runtime,
            id,
            ScreenAcquisitionOwner::Ocr,
        ));

        assert!(runtime.record().is_some());
        assert!(runtime.frozen_completion.is_some());
        assert!(runtime.frozen_suppressed);
        assert_eq!(runtime.restore_count, 0);
        assert_eq!(runtime.abandon_count, 0);
        assert_eq!(runtime.input_state.test_toast_count(), 0);
    }

    #[test]
    fn replacement_generation_survives_ready_cancel_and_stale_release() {
        let mut runtime = TransactionRuntime::started(ScreenAcquisitionOwner::Eyedropper);
        let id = runtime.record().expect("started record").id;
        runtime.frozen_generation = 8;
        runtime.frozen_completion = Some(ScreenAcquisitionCompletion {
            id,
            owner: ScreenAcquisitionOwner::Eyedropper,
            outcome: ScreenAcquisitionOutcome::Ready {
                installed_generation: 7,
            },
        });

        assert!(cancel_acquisition_transaction(
            &mut runtime,
            id,
            ScreenAcquisitionOwner::Eyedropper,
        ));
        assert!(runtime.frozen_active);
        assert_eq!(runtime.frozen_generation, 8);
        assert_eq!(runtime.restore_count, 0);
        assert_eq!(runtime.unfreeze_count, 0);

        assert!(!release_owned_generation(&mut runtime, 7));
        assert!(runtime.frozen_active);
        assert_eq!(runtime.restore_count, 0);
        assert_eq!(runtime.unfreeze_count, 0);
    }

    #[test]
    fn armed_owner_cleanup_releases_generation_clears_waiter_and_cancels_ui_once() {
        for (owner, zoom_owner) in [
            (
                ScreenAcquisitionOwner::Eyedropper,
                ZoomWaiterOwner::Eyedropper,
            ),
            (ScreenAcquisitionOwner::Ocr, ZoomWaiterOwner::Ocr),
        ] {
            let mut runtime = TransactionRuntime::started(owner);
            runtime.registry.take();

            cancel_modal_owner_resources(&mut runtime, owner, None, Some(7));

            assert!(!runtime.frozen_active, "owner={owner:?}");
            assert_eq!(runtime.restore_count, 1, "owner={owner:?}");
            assert_eq!(runtime.unfreeze_count, 1, "owner={owner:?}");
            assert_eq!(runtime.cleared_zoom_waiters, vec![zoom_owner]);
            assert_eq!(runtime.cancelled_owners, vec![owner]);

            cancel_modal_owner_resources(&mut runtime, owner, None, None);
            assert_eq!(runtime.restore_count, 1, "owner={owner:?}");
            assert_eq!(runtime.unfreeze_count, 1, "owner={owner:?}");
            assert_eq!(runtime.cleared_zoom_waiters, vec![zoom_owner, zoom_owner]);
            assert_eq!(runtime.cancelled_owners, vec![owner, owner]);
        }
    }
}
