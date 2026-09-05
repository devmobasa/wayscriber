use super::*;

/// The recovery twin of the teardown race above: a transient write failure
/// leaves an unhealthy incident, the user starts a `RetryPending` recovery
/// — whose inspection command sits in the recovery outbox while the normal
/// pipeline reports no source mutation in flight — presses F9, and exits
/// before the recovery completes. Settling must count recovery work as
/// settleable: the inspection and the retried write both land, the
/// barrier closes, and the deferred toggle persists — restart shows the
/// exit-time screen.
#[test]
fn an_exit_during_retry_pending_recovery_still_lands_the_deferred_toggle() {
    use crate::domain::Action;
    use crate::input::state::PendingToolbarPersistence;
    use std::os::unix::fs::PermissionsExt;

    let temp = crate::test_temp::tempdir().unwrap();
    let store_dir = temp.path().join("store");
    fs::create_dir(&store_dir).unwrap();
    let runtime_path = store_dir.join("runtime-ui.toml");
    let config = Config::default();
    assert!(config.ui.toolbar.top_pinned);
    let mut input = input_from_config(&config);
    let mut runtime = test_runtime(&config, &runtime_path);

    // A transient failure: the store directory briefly refuses writes.
    fs::set_permissions(&store_dir, fs::Permissions::from_mode(0o555)).unwrap();
    commit_display_mode(&mut runtime, &mut input, TopDisplayMode::Micro);
    wait_for_runtime_mode(&mut runtime, RuntimeUiPersistenceMode::Unhealthy);
    assert!(runtime.mutation_barrier_active());
    fs::set_permissions(&store_dir, fs::Permissions::from_mode(0o755)).unwrap();

    assert!(
        runtime.handle_persistence_lifecycle_event(&ToolbarEvent::RetryRuntimeUiPersistence),
        "retry must start a recovery attempt"
    );
    assert!(runtime.mutation_barrier_active());
    input.handle_action(Action::ToggleToolbar);
    assert!(!input.toolbar_visible());
    assert!(input.has_pending_toolbar_persistence());

    // Exit: settling drives the inspection, the controller decision, and
    // the retried write through the real writer.
    runtime.settle_barrier_for_teardown();
    assert!(!runtime.mutation_barrier_active());
    let drain = runtime.drain_writer_completions();
    assert!(drain.rollbacks.is_empty());
    assert!(
        !drain.rebuild_live,
        "recovery under retained authority leaves live state alone"
    );
    assert_eq!(
        input.take_pending_toolbar_persistence(),
        vec![PendingToolbarPersistence::Visibility {
            previous_top_pinned: true,
        }],
        "the deferred entry must survive the settled recovery"
    );
    let prepared = runtime
        .begin_toolbar_mutation_with_rollback(
            ToolbarRuntimeUiPersistenceTarget::ToolbarVisibility,
            pins_rollback_values(true),
        )
        .expect("visibility permit after recovery settled");
    assert!(matches!(
        runtime.finish_toolbar_mutation(prepared, true, &input),
        ToolbarRuntimeFinish::KeepPreview
    ));
    runtime.shutdown_blocking();

    // Restart: the retried write and the deferred toggle both survived.
    let mut restarted_input = input_from_config(&config);
    let mut restarted = test_runtime(&config, &runtime_path);
    restarted.apply_startup_state(
        &crate::ui_text::UiTextEngine::default(),
        &mut restarted_input,
    );
    assert_eq!(
        stored_display_mode(&restarted),
        Some(PersistedTopDisplayMode::Micro),
        "the recovery must have landed the write that originally failed"
    );
    assert!(!restarted_input.toolbar_top_pinned());
    assert!(
        !restarted_input.toolbar_visible(),
        "the toggle pressed during the recovery barrier must survive the exit"
    );
    restarted.shutdown_blocking();
}

/// A rollback can resolve long after the toggle (the forced-barrier test
/// above proves the deferral). If presenter mode has meanwhile taken chrome
/// ownership, applying the rollback must not surface toolbars
/// mid-presentation: the derived visibility lands in presenter's restore
/// snapshot, so the live flags stay presenter's and exit hands back a
/// screen agreeing with the rolled-back pins.
#[test]
fn a_deferred_hide_rollback_lands_in_the_presenter_restore_snapshot() {
    let route_measurer = crate::draw::TextMeasurer::default();
    let route_ui_engine = crate::ui_text::UiTextEngine::default();
    let route_resources = crate::input::state::InputTextResources {
        measurer: &route_measurer,
        ui_engine: &route_ui_engine,
    };
    use crate::domain::Action;

    let config = Config::default();
    let mut input = input_from_config(&config);
    let mut positions = config_positions(&config);

    input.handle_action(Action::ToggleToolbar); // hide, pin → false
    assert!(!input.toolbar_visible());
    input.take_pending_toolbar_persistence(); // the write whose rollback arrives below

    input.presenter_mode_config_mut_for_test().hide_toolbars = true;
    input.toggle_presenter_mode_with_resources(route_resources);
    assert!(input.presenter_mode_active());

    apply_toolbar_runtime_rollback(
        &crate::ui_text::UiTextEngine::default(),
        &mut input,
        &mut positions,
        &pins_rollback(true),
    );

    assert!(input.toolbar_top_pinned());
    assert!(
        !input.toolbar_visible() && !input.toolbar_top_visible(),
        "the live presenter-hidden flags must not move under the owner"
    );

    input.toggle_presenter_mode_with_resources(route_resources);
    assert!(!input.presenter_mode_active());
    assert!(
        input.toolbar_visible() && input.toolbar_top_visible(),
        "presenter exit must restore visibility agreeing with the rolled-back pins"
    );
}

/// The same deferred-rollback contract for focus mode: the derived
/// visibility lands in the focus snapshot, and the second press restores a
/// screen agreeing with the rolled-back pins.
#[test]
fn a_deferred_hide_rollback_lands_in_the_focus_mode_snapshot() {
    use crate::config::{StatusBarStyle, StatusPosition};
    use crate::domain::Action;

    let config = Config::default();
    let mut input = input_from_config(&config);
    let mut positions = config_positions(&config);
    // A measured status HUD keeps chrome on screen after the toolbar hide,
    // so the focus press below snapshots-and-hides instead of taking the
    // rescue arm (which shows everything and stores no snapshot).
    input.update_status_hud_layout(
        StatusPosition::BottomLeft,
        &StatusBarStyle::default(),
        1280,
        720,
    );

    input.handle_action(Action::ToggleToolbar); // hide, pin → false
    assert!(!input.toolbar_visible());
    input.take_pending_toolbar_persistence(); // the write whose rollback arrives below

    input.handle_action(Action::ToggleFocusMode);
    assert!(input.focus_mode_active());

    apply_toolbar_runtime_rollback(
        &crate::ui_text::UiTextEngine::default(),
        &mut input,
        &mut positions,
        &pins_rollback(true),
    );

    assert!(input.toolbar_top_pinned());
    assert!(
        !input.toolbar_visible() && !input.toolbar_top_visible(),
        "the live focus-hidden flags must not move under the owner"
    );

    input.handle_action(Action::ToggleFocusMode); // restore
    assert!(!input.focus_mode_active());
    assert!(
        input.toolbar_visible() && input.toolbar_top_visible(),
        "focus exit must restore visibility agreeing with the rolled-back pins"
    );
}

/// And for light mode, which owns chrome the same way (its enter zeroes the
/// toolbar flags and its exit writes the snapshot back).
#[test]
fn a_deferred_hide_rollback_lands_in_the_light_mode_snapshot() {
    use crate::domain::Action;

    let config = Config::default();
    let mut input = input_from_config(&config);
    let mut positions = config_positions(&config);
    // Light mode refuses to start without layer-shell passthrough support.
    input.compositor_capabilities.layer_shell = true;

    input.handle_action(Action::ToggleToolbar); // hide, pin → false
    assert!(!input.toolbar_visible());
    input.take_pending_toolbar_persistence(); // the write whose rollback arrives below

    input.handle_action(Action::ToggleLightMode);
    assert!(input.light_mode_active());

    apply_toolbar_runtime_rollback(
        &crate::ui_text::UiTextEngine::default(),
        &mut input,
        &mut positions,
        &pins_rollback(true),
    );

    assert!(input.toolbar_top_pinned());
    assert!(
        !input.toolbar_visible() && !input.toolbar_top_visible(),
        "the live light-mode-hidden flags must not move under the owner"
    );

    input.handle_action(Action::ToggleLightMode); // exit restores the snapshot
    assert!(!input.light_mode_active());
    assert!(
        input.toolbar_visible() && input.toolbar_top_visible(),
        "light-mode exit must restore visibility agreeing with the rolled-back pins"
    );
}
