use super::*;

#[test]
fn committed_top_drag_writes_runtime_state_and_leaves_config_untouched() {
    const AUTHORED: &[u8] = b"# authored config bytes stay exact\n";
    let temp = crate::test_temp::tempdir().unwrap();
    let config_path = temp.path().join("config.toml");
    let runtime_path = temp.path().join("data/runtime-ui.toml");
    fs::write(&config_path, AUTHORED).unwrap();
    let config = Config::default();
    let mut runtime = test_runtime(&config, &runtime_path);
    let mut positions = config_positions(&config);

    assert!(runtime.begin_position_drag(MoveDragKind::Top, positions));
    positions.top = (42.5, -7.25);
    assert!(matches!(
        runtime.finish_position_drag(true, positions),
        ToolbarRuntimeFinish::KeepPreview
    ));
    assert!(settle_runtime(&mut runtime).rollbacks.is_empty());

    assert_eq!(
        stored_position(&runtime, InteractionSeedTarget::TopPosition),
        Some((42.5, -7.25))
    );
    assert_eq!(fs::read(&config_path).unwrap(), AUTHORED);
    let stored = fs::read_to_string(&runtime_path).unwrap();
    assert!(stored.contains("top_position"), "{stored}");
    runtime.shutdown_blocking();

    // The override survives a restart on top of the unchanged config seeds.
    let restarted = test_runtime(&config, &runtime_path);
    let mut restored = config_positions(&config);
    restarted.apply_startup_positions(&mut restored);
    assert_eq!(restored.top, (42.5, -7.25));
    assert_eq!(fs::read(&config_path).unwrap(), AUTHORED);
}

#[test]
fn a_drag_back_to_the_authored_position_deletes_its_override() {
    let temp = crate::test_temp::tempdir().unwrap();
    let runtime_path = temp.path().join("runtime-ui.toml");
    let config = Config::default();
    let mut runtime = test_runtime(&config, &runtime_path);
    let seeded = config_positions(&config);
    let mut positions = seeded;

    assert!(runtime.begin_position_drag(MoveDragKind::Top, positions));
    positions.top = (10.0, 11.0);
    runtime.finish_position_drag(true, positions);
    assert!(settle_runtime(&mut runtime).rollbacks.is_empty());
    assert!(stored_position(&runtime, InteractionSeedTarget::TopPosition).is_some());

    assert!(runtime.begin_position_drag(MoveDragKind::Top, positions));
    positions.top = seeded.top;
    runtime.finish_position_drag(true, positions);
    assert!(settle_runtime(&mut runtime).rollbacks.is_empty());
    assert_eq!(
        stored_position(&runtime, InteractionSeedTarget::TopPosition),
        None,
        "an override equal to its seed is deleted, not stored"
    );
    runtime.shutdown_blocking();
}

#[test]
fn an_authored_position_edit_drops_the_stale_drag_override() {
    let temp = crate::test_temp::tempdir().unwrap();
    let runtime_path = temp.path().join("runtime-ui.toml");
    let config_a = Config::default();
    let mut input = input_from_config(&config_a);
    let mut runtime = test_runtime(&config_a, &runtime_path);
    let mut positions = config_positions(&config_a);

    assert!(runtime.begin_position_drag(MoveDragKind::Top, positions));
    positions.top = (55.0, 66.0);
    runtime.finish_position_drag(true, positions);
    assert!(settle_runtime(&mut runtime).rollbacks.is_empty());
    assert_eq!(
        stored_position(&runtime, InteractionSeedTarget::TopPosition),
        Some((55.0, 66.0))
    );

    let mut config_b = config_a;
    config_b.ui.toolbar.top_offset = 200.0;
    config_b.ui.toolbar.top_offset_y = 201.0;
    let refresh = runtime.refresh_config_seeds(
        &crate::ui_text::UiTextEngine::default(),
        &crate::draw::TextMeasurer::default(),
        &config_b,
        &mut input,
        &mut positions,
    );
    assert!(refresh.applied);
    assert!(settle_runtime(&mut runtime).rollbacks.is_empty());
    assert_eq!(
        stored_position(&runtime, InteractionSeedTarget::TopPosition),
        None,
        "an explicit config edit wins over an older drag"
    );
    assert_eq!(positions.top, (200.0, 201.0));
    runtime.shutdown_blocking();
}

#[test]
fn a_drag_without_a_runtime_store_is_process_only_and_never_touches_config() {
    const AUTHORED: &[u8] = b"# authored config bytes stay exact\n";
    let temp = crate::test_temp::tempdir().unwrap();
    let config_path = temp.path().join("config.toml");
    fs::write(&config_path, AUTHORED).unwrap();
    let config = Config::default();
    let mut input = input_from_config(&config);
    let mut previews = UnavailablePersistencePreviews::default();
    let seeded = config_positions(&config);
    let mut positions = seeded;

    assert!(previews.begin_position_drag(MoveDragKind::Top, positions));
    positions.top = (42.0, 43.0);
    let finish = previews.finish_position_drag(true);
    assert!(matches!(finish, ToolbarRuntimeFinish::KeepPreview));
    apply_finish(&mut input, &mut positions, finish);

    assert_eq!(
        positions.top,
        (42.0, 43.0),
        "the committed drag stays on screen"
    );
    assert_eq!(fs::read(&config_path).unwrap(), AUTHORED);
}

#[test]
fn display_mode_cycle_is_runtime_owned_and_hidden_persists_as_full() {
    use crate::config::TopDisplayMode;

    const AUTHORED: &[u8] = b"# authored config bytes stay exact\n";
    let temp = crate::test_temp::tempdir().unwrap();
    let config_path = temp.path().join("config.toml");
    let runtime_path = temp.path().join("data/runtime-ui.toml");
    fs::write(&config_path, AUTHORED).unwrap();
    let config = Config::default();
    let mut input = input_from_config(&config);
    let mut runtime = test_runtime(&config, &runtime_path);
    assert_eq!(input.toolbar_top_display_mode(), TopDisplayMode::Full);

    assert!(matches!(
        commit_display_mode(&mut runtime, &mut input, TopDisplayMode::Micro),
        ToolbarRuntimeFinish::KeepPreview
    ));
    assert!(settle_runtime(&mut runtime).rollbacks.is_empty());
    assert_eq!(
        stored_display_mode(&runtime),
        Some(PersistedTopDisplayMode::Micro)
    );

    // The hidden rung of the cycle is runtime-only: it stores `full` so the
    // strip comes back on the next start, exactly as before the move.
    assert!(matches!(
        commit_display_mode(&mut runtime, &mut input, TopDisplayMode::Hidden),
        ToolbarRuntimeFinish::KeepPreview
    ));
    assert!(settle_runtime(&mut runtime).rollbacks.is_empty());
    assert_eq!(input.toolbar_top_display_mode(), TopDisplayMode::Hidden);
    assert_eq!(
        stored_display_mode(&runtime),
        None,
        "folding hidden back to the authored full seed deletes the override"
    );
    assert_eq!(fs::read(&config_path).unwrap(), AUTHORED);
    let stored = fs::read_to_string(&runtime_path).unwrap();
    assert!(!stored.contains("hidden"), "{stored}");
    runtime.shutdown_blocking();
}

#[test]
fn a_stored_display_mode_is_restored_at_startup_over_the_config_seed() {
    use crate::config::TopDisplayMode;

    let temp = crate::test_temp::tempdir().unwrap();
    let runtime_path = temp.path().join("runtime-ui.toml");
    let config = Config::default();
    let mut input = input_from_config(&config);
    let mut runtime = test_runtime(&config, &runtime_path);

    commit_display_mode(&mut runtime, &mut input, TopDisplayMode::Micro);
    assert!(settle_runtime(&mut runtime).rollbacks.is_empty());
    runtime.shutdown_blocking();

    let mut restarted_input = input_from_config(&config);
    assert_eq!(
        restarted_input.toolbar_top_display_mode(),
        TopDisplayMode::Full
    );
    let restarted = test_runtime(&config, &runtime_path);
    restarted.apply_startup_state(
        &crate::ui_text::UiTextEngine::default(),
        &crate::draw::TextMeasurer::default(),
        &mut restarted_input,
    );
    assert_eq!(
        restarted_input.toolbar_top_display_mode(),
        TopDisplayMode::Micro
    );
}
#[test]
fn a_display_mode_change_during_presenter_mode_stores_the_pre_presenter_value() {
    let route_measurer = crate::draw::TextMeasurer::default();
    let route_ui_engine = crate::ui_text::UiTextEngine::default();
    let route_resources = crate::input::state::InputTextResources {
        measurer: &route_measurer,
        ui_engine: &route_ui_engine,
    };
    use crate::config::{PresenterToolbarMode, TopDisplayMode};

    let temp = crate::test_temp::tempdir().unwrap();
    let runtime_path = temp.path().join("runtime-ui.toml");
    let config = Config::default();
    let mut input = input_from_config(&config);
    input.presenter_mode_config_mut_for_test().hide_toolbars = true;
    input.presenter_mode_config_mut_for_test().toolbar_mode = PresenterToolbarMode::Micro;
    input.test_set_toolbar_display_state(TopDisplayMode::Full, input.toolbar_top_minimized());
    input.toggle_presenter_mode_with_resources(route_resources);
    assert_eq!(input.toolbar_top_display_mode(), TopDisplayMode::Micro);

    // The live strip is presenter's; the persisted value stays the saved
    // pre-presenter mode, so committing it is a no-op against the seed.
    let values = top_display_mode_values(input.toolbar_top_display_mode(), &input).unwrap();
    assert_eq!(
        values.values().get(&InteractionSeedTarget::TopDisplayMode),
        Some(&InteractionSeedValue::TopDisplayMode(
            PersistedTopDisplayMode::Full
        ))
    );

    let mut runtime = test_runtime(&config, &runtime_path);
    let target = ToolbarRuntimeUiPersistenceTarget::TopDisplayMode;
    let prepared = runtime
        .begin_toolbar_mutation(target, &input)
        .expect("display mode permit");
    assert!(matches!(
        runtime.finish_toolbar_mutation(prepared, true, &input),
        ToolbarRuntimeFinish::KeepPreview
    ));
    assert_eq!(stored_display_mode(&runtime), None);

    // Exiting presenter mode restores the live value; a change after that
    // persists the user's own choice again.
    input.toggle_presenter_mode_with_resources(route_resources);
    assert!(!input.presenter_restore_pending());
    assert!(matches!(
        commit_display_mode(&mut runtime, &mut input, TopDisplayMode::Micro),
        ToolbarRuntimeFinish::KeepPreview
    ));
    assert!(settle_runtime(&mut runtime).rollbacks.is_empty());
    assert_eq!(
        stored_display_mode(&runtime),
        Some(PersistedTopDisplayMode::Micro)
    );
    runtime.shutdown_blocking();
}

#[test]
fn an_authored_display_mode_edit_drops_the_stale_cycle_override() {
    use crate::config::TopDisplayMode;

    let temp = crate::test_temp::tempdir().unwrap();
    let runtime_path = temp.path().join("runtime-ui.toml");
    let config_a = Config::default();
    let mut input = input_from_config(&config_a);
    let mut runtime = test_runtime(&config_a, &runtime_path);
    let mut positions = config_positions(&config_a);

    commit_display_mode(&mut runtime, &mut input, TopDisplayMode::Micro);
    assert!(settle_runtime(&mut runtime).rollbacks.is_empty());
    assert_eq!(
        stored_display_mode(&runtime),
        Some(PersistedTopDisplayMode::Micro)
    );

    let mut config_b = config_a;
    config_b.ui.toolbar.top_display_mode = TopDisplayMode::Micro;
    let refresh = runtime.refresh_config_seeds(
        &crate::ui_text::UiTextEngine::default(),
        &crate::draw::TextMeasurer::default(),
        &config_b,
        &mut input,
        &mut positions,
    );
    assert!(refresh.applied);
    assert!(settle_runtime(&mut runtime).rollbacks.is_empty());
    assert_eq!(
        stored_display_mode(&runtime),
        None,
        "the authored default caught up with the runtime choice"
    );
    assert_eq!(input.toolbar_top_display_mode(), TopDisplayMode::Micro);
    runtime.shutdown_blocking();
}
