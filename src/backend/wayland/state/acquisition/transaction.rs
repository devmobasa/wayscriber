use crate::backend::wayland::acquisition::{
    AcquisitionRecord, AcquisitionStage, ScreenAcquisitionCompletion, ScreenAcquisitionId,
    ScreenAcquisitionOutcome, ScreenAcquisitionOwner, rejected_ready_generation,
};
use crate::backend::wayland::zoom::{ZoomSourceOutcome, ZoomSourceTerminal, ZoomWaiterOwner};
#[cfg(test)]
use crate::input::state::{EyedropperCaptureSource, ScreenCaptureSource};
use crate::input::state::{Toast, ToastPriority};

use super::super::region_capture::owned_generation_is_current;

const INCONSISTENT_CAPTURE_MESSAGE: &str = "Screen capture state was inconsistent; try again.";
const SOURCE_ACTIVATION_REJECTED_MESSAGE: &str =
    "Screen image changed before the selection could start; try again.";

pub(super) fn report_inconsistent_capture_to(input_state: &mut crate::input::InputState) {
    input_state.push_toast(
        ToastPriority::Critical,
        "capture",
        Toast::error(INCONSISTENT_CAPTURE_MESSAGE),
    );
}

pub(in crate::backend::wayland::state) fn report_screen_source_activation_rejected_to(
    input_state: &mut crate::input::InputState,
    owner: ScreenAcquisitionOwner,
) {
    let source = match owner {
        ScreenAcquisitionOwner::Eyedropper => "eyedropper",
        ScreenAcquisitionOwner::Ocr => "ocr",
        ScreenAcquisitionOwner::RegionCapture => "capture",
        ScreenAcquisitionOwner::UserFreeze => {
            debug_assert!(false, "user freeze has no modal selector activation");
            "capture"
        }
    };
    input_state.push_toast(
        ToastPriority::Info,
        source,
        Toast::warning(SOURCE_ACTIVATION_REJECTED_MESSAGE),
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

pub(in crate::backend::wayland::state) fn report_zoom_terminal_to(
    input_state: &mut crate::input::InputState,
    owner: Option<ZoomWaiterOwner>,
    terminal: &ZoomSourceTerminal,
) {
    let Some((source, message)) = zoom_terminal_report(owner, terminal) else {
        return;
    };
    input_state.push_toast(ToastPriority::Critical, source, Toast::error(message));
}

pub(super) fn report_screen_terminal_to(
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
pub(super) trait AcquisitionTransactionRuntime {
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
    fn finish_eyedropper_ready(&mut self, installed_generation: u64) -> bool;
    fn finish_ocr_ready(&mut self, installed_generation: u64) -> bool;
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

pub(super) fn route_acquisition_transaction<R: AcquisitionTransactionRuntime>(
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
        ) => {
            if !runtime.finish_eyedropper_ready(installed_generation) {
                cancel_modal_owner_resources(
                    runtime,
                    ScreenAcquisitionOwner::Eyedropper,
                    None,
                    Some(installed_generation),
                );
                report_screen_source_activation_rejected_to(
                    runtime.input_state(),
                    ScreenAcquisitionOwner::Eyedropper,
                );
            }
        }
        (
            ScreenAcquisitionOwner::Ocr,
            ScreenAcquisitionOutcome::Ready {
                installed_generation,
            },
        ) => {
            if !runtime.finish_ocr_ready(installed_generation) {
                cancel_modal_owner_resources(
                    runtime,
                    ScreenAcquisitionOwner::Ocr,
                    None,
                    Some(installed_generation),
                );
                report_screen_source_activation_rejected_to(
                    runtime.input_state(),
                    ScreenAcquisitionOwner::Ocr,
                );
            }
        }
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

pub(super) fn cancel_modal_owner_resources<R: AcquisitionTransactionRuntime>(
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

#[cfg(test)]
mod tests;
