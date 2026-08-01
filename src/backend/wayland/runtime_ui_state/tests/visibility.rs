use super::*;

/// The keyboard visibility toggle (F9) persists through the same pin wire
/// keys the pin buttons write, batched so a restart cannot observe half a
/// toggle — and startup, which derives visibility from the pins, then shows
/// exactly what the toggle left on screen.
#[test]
fn keyboard_visibility_toggle_persists_both_pins_and_startup_hides_the_toolbar() {
    use crate::domain::Action;
    use crate::input::state::PendingToolbarPersistence;

    let temp = crate::test_temp::tempdir().unwrap();
    let runtime_path = temp.path().join("runtime-ui.toml");
    let config = Config::default();
    assert!(config.ui.toolbar.top_pinned && config.ui.toolbar.side_pinned);
    let mut input = input_from_config(&config);
    let mut runtime = test_runtime(&config, &runtime_path);
    let accepted_before = runtime.controller.pipeline().latest_accepted();

    // Driven through the real F9 arm and its queue: the toggle already
    // applied, so the drained entry carries the pre-toggle pins, which
    // supply the write's rollback.
    input.handle_action(Action::ToggleToolbar);
    assert!(!input.toolbar_top_pinned && !input.toolbar_side_pinned);
    assert_eq!(
        input.take_pending_toolbar_persistence(),
        vec![PendingToolbarPersistence::Visibility {
            previous_top_pinned: true,
            previous_side_pinned: true,
        }],
        "F9 must queue exactly one visibility entry carrying the pre-toggle pins"
    );
    let rollback = RuntimeUiMutationValues::batch([
        (
            InteractionSeedTarget::TopPinned,
            InteractionSeedValue::Bool(true),
        ),
        (
            InteractionSeedTarget::SidePinned,
            InteractionSeedValue::Bool(true),
        ),
    ])
    .unwrap();
    let prepared = runtime
        .begin_toolbar_mutation_with_rollback(
            ToolbarRuntimeUiPersistenceTarget::ToolbarVisibility,
            rollback,
        )
        .expect("visibility permit");
    assert!(matches!(
        runtime.finish_toolbar_mutation(prepared, true, &input),
        ToolbarRuntimeFinish::KeepPreview
    ));
    assert_eq!(
        runtime.controller.pipeline().latest_accepted().get(),
        accepted_before.get() + 1,
        "both pin overrides settle through one accepted revision"
    );
    assert!(settle_runtime(&mut runtime).rollbacks.is_empty());
    assert_eq!(
        stored_bool(&runtime, InteractionSeedTarget::TopPinned),
        Some(false)
    );
    assert_eq!(
        stored_bool(&runtime, InteractionSeedTarget::SidePinned),
        Some(false)
    );
    runtime.shutdown_blocking();

    let mut restarted_input = input_from_config(&config);
    assert!(restarted_input.toolbar_visible);
    let mut restarted = test_runtime(&config, &runtime_path);
    restarted.apply_startup_state(&mut restarted_input);
    assert!(!restarted_input.toolbar_top_pinned);
    assert!(!restarted_input.toolbar_side_pinned);
    assert!(
        !restarted_input.toolbar_visible
            && !restarted_input.toolbar_top_visible
            && !restarted_input.toolbar_side_visible,
        "both-pins-false overrides start the toolbar hidden"
    );
    restarted.shutdown_blocking();
}

/// A rolled-back hide (F9) must restore the pre-toggle screen: the snapshot
/// carries only the two pins, so the rollback path re-derives the live
/// visibility flags from the restored pins with the startup rule.
#[test]
fn a_rolled_back_hide_toggle_restores_live_visibility_from_the_pins() {
    let config = Config::default();
    let mut input = input_from_config(&config);
    let mut positions = config_positions(&config);
    // Post-toggle live state: everything hidden, both pins driven false.
    assert!(input.set_toolbar_visible(false));
    input.toolbar_top_pinned = false;
    input.toolbar_side_pinned = false;

    apply_toolbar_runtime_rollback(&mut input, &mut positions, &pins_rollback(true, true));

    assert!(input.toolbar_top_pinned && input.toolbar_side_pinned);
    assert!(
        input.toolbar_visible && input.toolbar_top_visible && input.toolbar_side_visible,
        "restored pins must bring the live toolbar back"
    );
}

/// The reverse direction: rolling back a show must hide the toolbar again,
/// not leave it visible over both-pins-false.
#[test]
fn a_rolled_back_show_toggle_re_hides_the_toolbar() {
    let mut config = Config::default();
    config.ui.toolbar.top_pinned = false;
    config.ui.toolbar.side_pinned = false;
    let mut input = input_from_config(&config);
    let mut positions = config_positions(&config);
    // Post-toggle live state: everything shown, both pins driven true.
    assert!(input.set_toolbar_visible(true));
    input.toolbar_top_pinned = true;
    input.toolbar_side_pinned = true;

    apply_toolbar_runtime_rollback(&mut input, &mut positions, &pins_rollback(false, false));

    assert!(!input.toolbar_top_pinned && !input.toolbar_side_pinned);
    assert!(
        !input.toolbar_visible && !input.toolbar_top_visible && !input.toolbar_side_visible,
        "restored pins must hide the live toolbar again"
    );
}

/// A hide pressed while the top strip sat on the cycle's hidden rung: the
/// rollback re-derives the visibility flags but must not unfold the still
/// live `Hidden` mode — that combination *is* the pre-toggle screen (side
/// palette back, top strip cycle-hidden), and any future show unfolds it.
///
/// Driven through the real actions: only the deprecated Panel side layout
/// reaches this state with a persisted toggle, because its still-visible
/// side palette makes F9 a genuine HIDE (pins true→false). Under the
/// shipping pill layout the same press is a show whose pins do not change,
/// and the toggle arm queues no persistence at all
/// (`cycle_hidden_show_with_unchanged_pins_queues_no_persistence` in the
/// input-state tests).
#[test]
fn a_rolled_back_hide_toggle_keeps_a_cycle_hidden_strip_folded() {
    use crate::domain::Action;
    use crate::input::state::PendingToolbarPersistence;

    let config = Config::default();
    let mut input = input_from_config(&config);
    let mut positions = config_positions(&config);
    input.init_toolbar_side_layout_from_config(crate::config::ToolbarSideLayout::Panel);
    input.handle_action(Action::CycleToolbarDisplay); // micro
    input.handle_action(Action::CycleToolbarDisplay); // hidden
    assert_eq!(input.toolbar_top_display_mode, TopDisplayMode::Hidden);
    input.take_pending_toolbar_persistence(); // drain the cycle's display-mode write
    assert!(
        input.toolbar_visible(),
        "the Panel side palette keeps a surface visible under a cycle-hidden strip"
    );

    input.handle_action(Action::ToggleToolbar); // a real HIDE
    assert!(!input.toolbar_visible());
    assert!(!input.toolbar_top_pinned && !input.toolbar_side_pinned);
    assert_eq!(
        input.take_pending_toolbar_persistence(),
        vec![PendingToolbarPersistence::Visibility {
            previous_top_pinned: true,
            previous_side_pinned: true,
        }],
        "a pin-changing hide queues its persistence"
    );

    apply_toolbar_runtime_rollback(&mut input, &mut positions, &pins_rollback(true, true));

    assert!(input.toolbar_top_pinned && input.toolbar_side_pinned);
    assert!(input.toolbar_visible && input.toolbar_top_visible && input.toolbar_side_visible);
    assert_eq!(
        input.toolbar_top_display_mode,
        TopDisplayMode::Hidden,
        "the rollback must not unfold the cycle-hidden strip"
    );
    // The pre-toggle screen: side palette back, top strip still cycle-hidden.
    assert!(input.toolbar_side_visible() && !input.toolbar_top_visible());
}

/// Control: the pin buttons persist single-pin scopes and are deliberately
/// decoupled from live visibility, so a single-pin rollback restores that
/// pin and leaves every visibility flag untouched.
#[test]
fn a_single_pin_rollback_never_touches_live_visibility() {
    let config = Config::default();
    let mut input = input_from_config(&config);
    let mut positions = config_positions(&config);
    // Pins and visibility legitimately disagree: an implicit hide (focus
    // mode, presenter mode) clears the flags without touching the pins.
    assert!(input.set_toolbar_visible(false));
    input.toolbar_side_pinned = false;

    let rollback = PreviewRollbackSnapshot {
        values: BTreeMap::from([(
            InteractionSeedTarget::SidePinned,
            InteractionSeedValue::Bool(true),
        )]),
    };
    apply_toolbar_runtime_rollback(&mut input, &mut positions, &rollback);

    assert!(input.toolbar_side_pinned);
    assert!(
        !input.toolbar_visible && !input.toolbar_top_visible && !input.toolbar_side_visible,
        "a pin-button rollback must not re-derive visibility"
    );
}

/// End-to-end through the store's forced-rollback path: a visibility toggle
/// abandoned behind a failed reset barrier resolves to a drained rollback,
/// and applying it restores the pre-toggle screen, not just the pins.
#[test]
fn visibility_toggle_rollback_through_a_failed_reset_restores_the_screen() {
    let temp = crate::test_temp::tempdir().unwrap();
    let runtime_path = temp.path().join("runtime-ui.toml");
    let config = Config::default();
    assert!(config.ui.toolbar.top_pinned && config.ui.toolbar.side_pinned);
    let mut input = input_from_config(&config);
    let mut positions = config_positions(&config);
    let mut runtime = controller_only_runtime(&config, &runtime_path);

    let rollback = RuntimeUiMutationValues::batch([
        (
            InteractionSeedTarget::TopPinned,
            InteractionSeedValue::Bool(input.toolbar_top_pinned),
        ),
        (
            InteractionSeedTarget::SidePinned,
            InteractionSeedValue::Bool(input.toolbar_side_pinned),
        ),
    ])
    .unwrap();
    let prepared = runtime
        .begin_toolbar_mutation_with_rollback(
            ToolbarRuntimeUiPersistenceTarget::ToolbarVisibility,
            rollback,
        )
        .expect("visibility permit");
    assert!(input.set_toolbar_visible(false));
    input.toolbar_top_pinned = false;
    input.toolbar_side_pinned = false;

    let reset = match runtime.controller.request_supported_reset() {
        RequestResetResult::Started { .. } => runtime
            .controller
            .take_source_mutation()
            .expect("reset command"),
        result => panic!("reset did not start: {result:?}"),
    };
    assert!(matches!(
        runtime.finish_toolbar_mutation(prepared, true, &input),
        ToolbarRuntimeFinish::DeferredBehindBarrier
    ));
    runtime.handle_source_mutation_result(SourceMutationResult::Failed {
        id: reset.id,
        error: RuntimeStateIoError::new("test reset failure"),
        active: Some(RuntimeStateSourceObservation::missing(
            reset.expected_source.clone(),
        )),
        recovery_artifacts: Vec::new(),
        path_effect: RuntimeStateFailurePathEffect::Known(
            RuntimeStateObservedPathEffect::Untouched,
        ),
    });
    let drain = runtime.drain_writer_completions();
    assert_eq!(drain.rollbacks.len(), 1);
    apply_toolbar_runtime_rollback(&mut input, &mut positions, &drain.rollbacks[0]);
    assert!(input.toolbar_top_pinned && input.toolbar_side_pinned);
    assert!(
        input.toolbar_visible && input.toolbar_top_visible && input.toolbar_side_visible,
        "the forced rollback must leave the screen matching the restored pins"
    );
}

/// A toggle pressed while a reset/recovery barrier is active must defer,
/// not vanish: `begin` refuses new mutations wholesale during the barrier,
/// so the backend drain leaves the queue untouched until it resolves. Here
/// the completed reset leaves live state exactly where it was (nothing was
/// ever persisted), so the deferred entry still describes a genuine pin
/// change and its write lands like any other — restart agrees with the
/// screen. A reset that *does* change live state rebuilds the pins
/// instead, and the take's no-op filter then drops the entry as moot.
#[test]
fn a_barrier_defers_queued_visibility_persistence_instead_of_dropping_it() {
    use crate::domain::Action;
    use crate::input::state::PendingToolbarPersistence;

    let temp = crate::test_temp::tempdir().unwrap();
    let runtime_path = temp.path().join("runtime-ui.toml");
    let config = Config::default();
    assert!(config.ui.toolbar.top_pinned && config.ui.toolbar.side_pinned);
    let mut input = input_from_config(&config);
    let mut runtime = controller_only_runtime(&config, &runtime_path);

    assert!(matches!(
        runtime.controller.request_supported_reset(),
        RequestResetResult::Started { .. }
    ));
    let reset = runtime
        .controller
        .take_source_mutation()
        .expect("reset command");
    assert!(runtime.mutation_barrier_active());

    // The press lands on screen and queues normally; only the write waits.
    input.handle_action(Action::ToggleToolbar);
    assert!(!input.toolbar_visible());
    assert!(input.has_pending_toolbar_persistence());
    assert!(
        runtime
            .begin_toolbar_mutation_with_rollback(
                ToolbarRuntimeUiPersistenceTarget::ToolbarVisibility,
                pins_rollback_values(true, true),
            )
            .is_none(),
        "the barrier refuses new mutations, which is why the drain defers"
    );

    // The reset completes through the writer-completion path the event
    // loop uses; the barrier closes.
    runtime.integrate_writer_completion(RuntimeStateWriterCompletion::SourceMutation(
        SourceMutationResult::Applied {
            id: reset.id,
            applied_through: reset.accepted_through,
            new_source: RuntimeStateSourceRevision::missing(
                reset.expected_source.path_identity().clone(),
            ),
            recovery_artifacts: Vec::new(),
        },
    ));
    assert!(!runtime.mutation_barrier_active());
    let drain = runtime.drain_writer_completions();
    assert!(drain.rollbacks.is_empty());
    assert!(
        !drain.rebuild_live,
        "resetting an empty store changes no live state, so nothing is stomped"
    );

    // The deferred drain: the entry survived the barrier with its rollback
    // baseline and its write lands normally.
    let accepted_after_reset = runtime.controller.pipeline().latest_accepted();
    assert_eq!(
        input.take_pending_toolbar_persistence(),
        vec![PendingToolbarPersistence::Visibility {
            previous_top_pinned: true,
            previous_side_pinned: true,
        }],
        "the deferred entry must survive the barrier untouched"
    );
    let prepared = runtime
        .begin_toolbar_mutation_with_rollback(
            ToolbarRuntimeUiPersistenceTarget::ToolbarVisibility,
            pins_rollback_values(true, true),
        )
        .expect("visibility permit after the barrier resolves");
    assert!(matches!(
        runtime.finish_toolbar_mutation(prepared, true, &input),
        ToolbarRuntimeFinish::KeepPreview
    ));
    assert_eq!(
        runtime.controller.pipeline().latest_accepted().get(),
        accepted_after_reset.get() + 1,
        "the deferred toggle still settles through one accepted revision"
    );
    assert_eq!(
        stored_bool(&runtime, InteractionSeedTarget::TopPinned),
        Some(false)
    );
    assert_eq!(
        stored_bool(&runtime, InteractionSeedTarget::SidePinned),
        Some(false)
    );
}

/// The teardown race: a reset is in flight, F9 changes the screen, and the
/// user exits before the reset completes. Teardown mirrors
/// `drain_toolbar_persistence_for_teardown`: settle the barrier by waiting
/// for the writer completion `shutdown_blocking` was going to consume
/// anyway, apply its resolution, then drain — so the deferred toggle's
/// write still reaches the file and a restart shows the exit-time screen,
/// not the pre-toggle pins.
#[test]
fn an_exit_during_an_active_reset_barrier_still_lands_the_deferred_toggle() {
    use crate::domain::Action;
    use crate::input::state::PendingToolbarPersistence;

    let temp = crate::test_temp::tempdir().unwrap();
    let runtime_path = temp.path().join("runtime-ui.toml");
    let config = Config::default();
    assert!(config.ui.toolbar.top_pinned && config.ui.toolbar.side_pinned);
    let mut input = input_from_config(&config);
    let mut positions = config_positions(&config);
    let mut runtime = test_runtime(&config, &runtime_path);

    // The reset's command stays undispatched until settling begins, so the
    // barrier is deterministically active while F9 lands.
    assert!(matches!(
        runtime.controller.request_supported_reset(),
        RequestResetResult::Started { .. }
    ));
    assert!(runtime.mutation_barrier_active());
    input.handle_action(Action::ToggleToolbar);
    assert!(!input.toolbar_visible());
    assert!(input.has_pending_toolbar_persistence());

    // Exit begins: teardown settles the barrier first...
    runtime.settle_barrier_for_teardown();
    assert!(!runtime.mutation_barrier_active());
    let drain = runtime.drain_writer_completions();
    assert!(drain.rollbacks.is_empty());
    if drain.rebuild_live {
        runtime.apply_live_state(&mut input, &mut positions);
    }
    // ...then drains the queue; resetting an empty store changed no live
    // state, so the entry still describes a genuine pin change.
    assert_eq!(
        input.take_pending_toolbar_persistence(),
        vec![PendingToolbarPersistence::Visibility {
            previous_top_pinned: true,
            previous_side_pinned: true,
        }],
        "the deferred entry must survive the settled barrier"
    );
    let prepared = runtime
        .begin_toolbar_mutation_with_rollback(
            ToolbarRuntimeUiPersistenceTarget::ToolbarVisibility,
            pins_rollback_values(true, true),
        )
        .expect("visibility permit after teardown settled the barrier");
    assert!(matches!(
        runtime.finish_toolbar_mutation(prepared, true, &input),
        ToolbarRuntimeFinish::KeepPreview
    ));
    runtime.shutdown_blocking();

    // Restart: the exit-time screen survived the mid-reset exit.
    let mut restarted_input = input_from_config(&config);
    let mut restarted = test_runtime(&config, &runtime_path);
    restarted.apply_startup_state(&mut restarted_input);
    assert!(!restarted_input.toolbar_top_pinned);
    assert!(!restarted_input.toolbar_side_pinned);
    assert!(
        !restarted_input.toolbar_visible,
        "the toggle pressed during the reset barrier must survive the exit"
    );
    restarted.shutdown_blocking();
}
