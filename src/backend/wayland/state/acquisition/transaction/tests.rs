use super::*;
use crate::backend::wayland::acquisition::ScreenAcquisitionRegistry;
use crate::input::state::test_support::make_test_input_state;

struct TransactionRuntime {
    registry: ScreenAcquisitionRegistry,
    waiter_matches: bool,
    input_state: crate::input::InputState,
    finished_eyedropper: Vec<u64>,
    finished_ocr: Vec<u64>,
    finished_region_capture: Vec<u64>,
    region_legacy_handoffs: usize,
    ready_activation_succeeds: bool,
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
            finished_region_capture: Vec::new(),
            region_legacy_handoffs: 0,
            ready_activation_succeeds: true,
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

    fn finish_eyedropper_ready(&mut self, installed_generation: u64) -> bool {
        if self.ready_activation_succeeds {
            self.finished_eyedropper.push(installed_generation);
        }
        self.ready_activation_succeeds
    }

    fn finish_ocr_ready(&mut self, installed_generation: u64) -> bool {
        if self.ready_activation_succeeds {
            self.finished_ocr.push(installed_generation);
        }
        self.ready_activation_succeeds
    }

    fn finish_region_capture_ready(&mut self, installed_generation: u64) -> bool {
        if self.ready_activation_succeeds {
            self.finished_region_capture.push(installed_generation);
        }
        self.ready_activation_succeeds
    }

    fn handoff_region_capture_to_legacy(&mut self) {
        self.region_legacy_handoffs += 1;
    }

    fn cancel_owner_ui(&mut self, owner: ScreenAcquisitionOwner) {
        self.cancelled_owners.push(owner);
        match owner {
            ScreenAcquisitionOwner::Eyedropper => {
                let _ = self.input_state.cancel_eyedropper();
            }
            ScreenAcquisitionOwner::Ocr => self.input_state.cancel_region_ui_only(),
            ScreenAcquisitionOwner::RegionCapture => self.input_state.cancel_region_ui_only(),
            ScreenAcquisitionOwner::UserFreeze => {}
        }
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
        let reports = matches!(outcome, ZoomSourceOutcome::Failed(_));
        assert_eq!(
            zoom_terminal_report(
                Some(ZoomWaiterOwner::RegionCapture),
                &ZoomSourceTerminal::for_test(outcome.clone(), None),
            )
            .is_some(),
            reports,
            "owner=RegionCapture outcome={outcome:?}"
        );
    }
}

#[test]
fn region_capture_zoom_failure_has_a_typed_fallback() {
    assert_eq!(
        zoom_terminal_report(
            Some(ZoomWaiterOwner::RegionCapture),
            &ZoomSourceTerminal::for_test(
                ZoomSourceOutcome::Failed("backend failed".to_string()),
                None,
            ),
        ),
        Some(("capture", "Screen capture for region selection failed."))
    );
}

#[test]
fn region_capture_zoom_abort_and_deactivation_cancel_quietly() {
    for outcome in [ZoomSourceOutcome::Aborted, ZoomSourceOutcome::Deactivated] {
        assert_eq!(
            zoom_terminal_report(
                Some(ZoomWaiterOwner::RegionCapture),
                &ZoomSourceTerminal::for_test(outcome, None),
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

    for owner in [
        Some(ZoomWaiterOwner::Ocr),
        Some(ZoomWaiterOwner::RegionCapture),
        None,
    ] {
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
            ScreenAcquisitionOwner::Eyedropper => Some((key, "Screen eyedropper capture failed.")),
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
                (owner != ScreenAcquisitionOwner::RegionCapture)
                    .then_some((key, "specific backend failure")),
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
        ScreenAcquisitionOwner::RegionCapture,
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
        assert_eq!(
            runtime.finished_region_capture,
            if owner == ScreenAcquisitionOwner::RegionCapture {
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
fn ready_activation_rejection_releases_the_modal_owned_freeze_once() {
    for owner in [
        ScreenAcquisitionOwner::Eyedropper,
        ScreenAcquisitionOwner::Ocr,
        ScreenAcquisitionOwner::RegionCapture,
    ] {
        let mut runtime = TransactionRuntime::started(owner);
        runtime.ready_activation_succeeds = false;
        match owner {
            ScreenAcquisitionOwner::Eyedropper => runtime
                .input_state
                .set_eyedropper_pending_capture(EyedropperCaptureSource::Frozen),
            ScreenAcquisitionOwner::Ocr => runtime.input_state.set_region_pending_capture(
                crate::input::state::RegionPurposeTag::Ocr,
                1,
                ScreenCaptureSource::Frozen,
            ),
            ScreenAcquisitionOwner::RegionCapture => {
                runtime.input_state.set_region_pending_capture(
                    crate::input::state::RegionPurposeTag::CaptureDeliver,
                    1,
                    ScreenCaptureSource::Frozen,
                )
            }
            ScreenAcquisitionOwner::UserFreeze => unreachable!(),
        }
        assert!(runtime.input_state.screen_modal_is_engaged());
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
        assert!(!runtime.frozen_active, "owner={owner:?}");
        assert_eq!(runtime.restore_count, 1, "owner={owner:?}");
        assert_eq!(runtime.unfreeze_count, 1, "owner={owner:?}");
        assert_eq!(runtime.cancelled_owners, vec![owner], "owner={owner:?}");
        assert!(
            !runtime.input_state.screen_modal_is_engaged(),
            "owner={owner:?}"
        );
        assert_eq!(runtime.input_state.test_toast_count(), 1, "owner={owner:?}");
        assert_eq!(
            runtime.input_state.test_active_toast_message(),
            Some(SOURCE_ACTIVATION_REJECTED_MESSAGE),
            "owner={owner:?}"
        );
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
fn region_capture_nonready_completion_falls_back_only_for_acquisition_failure() {
    for (outcome, expected_handoffs, expected_cancellations, expected_toasts) in [
        (ScreenAcquisitionOutcome::Cancelled, 0, 1, 0),
        (ScreenAcquisitionOutcome::Unavailable, 1, 0, 0),
        (
            ScreenAcquisitionOutcome::Failed("activation rejected".to_string()),
            1,
            0,
            0,
        ),
        (ScreenAcquisitionOutcome::StaleLayout, 0, 1, 1),
    ] {
        let mut runtime = TransactionRuntime::started(ScreenAcquisitionOwner::RegionCapture);
        let id = runtime.record().expect("started record").id;

        route_acquisition_transaction(
            &mut runtime,
            ScreenAcquisitionCompletion {
                id,
                owner: ScreenAcquisitionOwner::RegionCapture,
                outcome: outcome.clone(),
            },
        );

        assert_eq!(runtime.record(), None, "outcome={outcome:?}");
        assert_eq!(
            runtime.region_legacy_handoffs, expected_handoffs,
            "outcome={outcome:?}"
        );
        assert_eq!(
            runtime.cancelled_owners.len(),
            expected_cancellations,
            "outcome={outcome:?}"
        );
        assert_eq!(
            runtime.input_state.test_toast_count(),
            expected_toasts,
            "outcome={outcome:?}"
        );
        assert_eq!(runtime.restore_count, 0, "outcome={outcome:?}");
        assert_eq!(runtime.unfreeze_count, 0, "outcome={outcome:?}");
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
    for owner in [
        ScreenAcquisitionOwner::Ocr,
        ScreenAcquisitionOwner::RegionCapture,
    ] {
        let mut runtime = TransactionRuntime::started(owner);
        let id = runtime.record().expect("started record").id;
        runtime.frozen_completion = Some(ScreenAcquisitionCompletion {
            id,
            owner,
            outcome: ScreenAcquisitionOutcome::Ready {
                installed_generation: 7,
            },
        });
        runtime.capture_done = true;

        assert!(cancel_acquisition_transaction(&mut runtime, id, owner));

        assert_eq!(runtime.record(), None, "owner={owner:?}");
        assert_eq!(runtime.frozen_completion, None, "owner={owner:?}");
        assert!(!runtime.capture_done, "owner={owner:?}");
        assert!(!runtime.frozen_active, "owner={owner:?}");
        assert_eq!(runtime.restore_count, 1, "owner={owner:?}");
        assert_eq!(runtime.unfreeze_count, 1, "owner={owner:?}");
        assert!(!runtime.frozen_suppressed, "owner={owner:?}");
        assert_eq!(runtime.suppression_end_count, 1, "owner={owner:?}");
        assert_eq!(runtime.input_state.test_toast_count(), 0, "owner={owner:?}");

        assert!(!cancel_acquisition_transaction(&mut runtime, id, owner));
        assert_eq!(runtime.restore_count, 1, "owner={owner:?}");
        assert_eq!(runtime.unfreeze_count, 1, "owner={owner:?}");
        assert_eq!(runtime.suppression_end_count, 1, "owner={owner:?}");
    }
}

#[test]
fn started_and_queued_cancellation_have_exact_resource_postconditions() {
    for owner in [
        ScreenAcquisitionOwner::Eyedropper,
        ScreenAcquisitionOwner::Ocr,
        ScreenAcquisitionOwner::RegionCapture,
    ] {
        let mut started = TransactionRuntime::started(owner);
        let started_id = started.record().expect("started record").id;
        started.capture_done = true;

        assert!(cancel_acquisition_transaction(
            &mut started,
            started_id,
            owner,
        ));
        assert_eq!(started.record(), None, "owner={owner:?}");
        assert_eq!(started.restore_count, 1, "owner={owner:?}");
        assert_eq!(started.abandon_count, 1, "owner={owner:?}");
        assert_eq!(started.unfreeze_count, 0, "owner={owner:?}");
        assert!(!started.capture_done, "owner={owner:?}");
        assert!(!started.frozen_suppressed, "owner={owner:?}");
        assert_eq!(started.suppression_end_count, 1, "owner={owner:?}");
        assert_eq!(started.input_state.test_toast_count(), 0, "owner={owner:?}");

        let mut queued = TransactionRuntime::started(owner);
        let queued_record = queued.registry.take().expect("started record");
        let queued_id = queued.registry.request(owner).expect("queued replacement");
        assert_ne!(queued_record.id, queued_id);
        queued.frozen_suppressed = false;

        assert!(cancel_acquisition_transaction(
            &mut queued,
            queued_id,
            owner,
        ));
        assert_eq!(queued.record(), None, "owner={owner:?}");
        assert_eq!(queued.restore_count, 0, "owner={owner:?}");
        assert_eq!(queued.abandon_count, 0, "owner={owner:?}");
        assert_eq!(queued.unfreeze_count, 0, "owner={owner:?}");
        assert_eq!(queued.suppression_end_count, 0, "owner={owner:?}");
    }
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
        (
            ScreenAcquisitionOwner::RegionCapture,
            ZoomWaiterOwner::RegionCapture,
        ),
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
