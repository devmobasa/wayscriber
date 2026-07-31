use super::*;

use std::fs;
use std::path::Path;
use std::thread;
use std::time::Duration;

use crate::config::{ToolbarItemsConfig, toolbar_item_ids as ids};
use crate::input::state::test_support::make_test_input_state;
use crate::ui::toolbar::{RuntimeUiPersistenceMode, RuntimeUiPersistenceSnapshot, ToolbarEvent};

fn input_from_config(config: &Config) -> InputState {
    let mut input = make_test_input_state();
    input.boards = crate::input::boards::BoardManager::from_config(config.resolved_boards());
    input.toolbar_items = config.ui.toolbar.items.clone();
    input.resolved_toolbar_items = input.toolbar_items.resolved();
    input.toolbar_top_pinned = config.ui.toolbar.top_pinned;
    input.toolbar_side_pinned = config.ui.toolbar.side_pinned;
    input.toolbar_top_minimized = config.ui.toolbar.top_minimized;
    input.toolbar_side_minimized = config.ui.toolbar.side_minimized;
    input.toolbar_top_visible = config.ui.toolbar.top_pinned;
    input.toolbar_side_visible = config.ui.toolbar.side_pinned;
    input.toolbar_visible = input.toolbar_top_visible || input.toolbar_side_visible;
    input.init_toolbar_side_panes_from_config(
        &config.ui.toolbar.side_active_pane,
        &config.ui.toolbar.collapsed_sections,
    );
    input
}

fn test_runtime(config: &Config, path: &Path) -> ToolbarRuntimeState {
    let runtime = test_runtime_allow_startup_incident(config, path);
    assert!(!matches!(
        runtime.persistence_snapshot().mode,
        RuntimeUiPersistenceMode::Unhealthy
    ));
    runtime
}

fn test_runtime_allow_startup_incident(config: &Config, path: &Path) -> ToolbarRuntimeState {
    fs::create_dir_all(path.parent().expect("runtime parent")).unwrap();
    let store = RuntimeUiStateStore::new(path);
    let mut board_pin_seeds = board_pin_seeds_from_input(&input_from_config(config));
    let inspection = store.inspect().unwrap();
    retain_stored_board_pin_seeds_for_session_restore(&mut board_pin_seeds, &inspection);
    let bootstrap = inspection
        .into_controller_bootstrap(runtime_seeds_from_config(config, &board_pin_seeds).unwrap());
    let mut runtime = ToolbarRuntimeState {
        controller: bootstrap.controller,
        runtime_path: path.to_path_buf(),
        lifecycle: RuntimeUiLifecycleState::startup(bootstrap.startup_incident),
        board_pin_seeds,
        deferred_board_pin_restores: BTreeMap::new(),
        writer: Some(RuntimeUiStateWriter::spawn(store).unwrap()),
        pending_writer_command: None,
        live_rebuild_pending: false,
        item_drag: None,
        position_drag: None,
    };
    runtime.dispatch_writer_command();
    runtime
}

fn controller_only_runtime(config: &Config, path: &Path) -> ToolbarRuntimeState {
    let mut board_pin_seeds = board_pin_seeds_from_input(&input_from_config(config));
    let inspection = RuntimeUiStateStore::new(path).inspect().unwrap();
    retain_stored_board_pin_seeds_for_session_restore(&mut board_pin_seeds, &inspection);
    let bootstrap = inspection
        .into_controller_bootstrap(runtime_seeds_from_config(config, &board_pin_seeds).unwrap());
    ToolbarRuntimeState {
        controller: bootstrap.controller,
        runtime_path: path.to_path_buf(),
        lifecycle: RuntimeUiLifecycleState::startup(bootstrap.startup_incident),
        board_pin_seeds,
        deferred_board_pin_restores: BTreeMap::new(),
        writer: None,
        pending_writer_command: None,
        live_rebuild_pending: false,
        item_drag: None,
        position_drag: None,
    }
}

fn settle_runtime(runtime: &mut ToolbarRuntimeState) -> ToolbarRuntimeDrain {
    let mut combined = ToolbarRuntimeDrain::default();
    for _ in 0..400 {
        let drain = runtime.drain_writer_completions();
        combined.rollbacks.extend(drain.rollbacks);
        combined.rebuild_live |= drain.rebuild_live;
        combined.lifecycle_changed |= drain.lifecycle_changed;
        let pipeline = runtime.controller.pipeline();
        if pipeline.settled_through() == pipeline.latest_accepted()
            && !pipeline.has_source_mutation_in_flight()
            && runtime.pending_writer_command.is_none()
        {
            return combined;
        }
        thread::sleep(Duration::from_millis(5));
    }
    panic!("runtime writer did not settle");
}

fn wait_for_runtime_mode(
    runtime: &mut ToolbarRuntimeState,
    expected: RuntimeUiPersistenceMode,
) -> RuntimeUiPersistenceSnapshot {
    for _ in 0..800 {
        runtime.drain_writer_completions();
        let snapshot = runtime.persistence_snapshot();
        if snapshot.mode == expected {
            return snapshot;
        }
        thread::sleep(Duration::from_millis(5));
    }
    panic!(
        "runtime UI lifecycle did not reach {expected:?}; last state: {:?}",
        runtime.persistence_snapshot()
    );
}

fn apply_finish(
    input: &mut InputState,
    positions: &mut ToolbarPositionSnapshot,
    finish: ToolbarRuntimeFinish,
) {
    if let ToolbarRuntimeFinish::Rollback(rollback) = finish {
        apply_toolbar_runtime_rollback(input, positions, &rollback);
    }
}

fn board_pinned(input: &InputState, board_id: &str) -> bool {
    input
        .boards
        .board_states()
        .iter()
        .find(|board| board.spec.id == board_id)
        .unwrap_or_else(|| panic!("missing test board {board_id}"))
        .spec
        .pinned
}

fn commit_board_pin_toggle(
    runtime: &mut ToolbarRuntimeState,
    config: &Config,
    input: &mut InputState,
    board_id: &str,
) -> ToolbarRuntimeFinish {
    let current = board_pinned(input, board_id);
    let seed = input.boards.pin_seed(board_id).expect("board pin seed");
    let prepared = runtime
        .begin_board_pin_toggle(config, board_id.to_string(), seed, current)
        .expect("board pin permit");
    assert!(input.apply_board_pinned_runtime(board_id, prepared.desired));
    runtime.finish_board_pin_toggle(prepared, true)
}

/// The seed inputs assembled the way `refresh_runtime_ui_config_seeds` does:
/// board pins are synced from the config first, then folded into the registry.
fn seeds_for_config(config: &Config) -> ValidatedInteractionSeeds {
    let mut input = input_from_config(config);
    input
        .boards
        .sync_pin_seeds_from_config(&config.resolved_boards());
    runtime_seeds_from_config(config, &board_pin_seeds_from_input(&input))
        .expect("probe config should produce valid seeds")
}

/// The same rule for the authored preferences that no longer travel through
/// the writer. `ToolbarPreference::affects_runtime_ui_seeds` declares the
/// layout mode and the section fields, because those are exactly the fields
/// `resolved_toolbar_item_seeds` reads through the legacy fold; every other
/// authored preference is declared seed-neutral and has to stay that way. The
/// declared pair is a deliberate superset — a redundant reseed is idempotent,
/// a missing one leaves overrides reconciling against a stale baseline — so
/// only the neutral half is asserted here.
#[test]
fn no_undeclared_authored_preference_moves_a_runtime_seed() {
    /// One authored preference field, changed away from the shipped default.
    type PreferenceProbe = (&'static str, fn(&mut Config));

    let baseline = Config::default();
    let baseline_seeds = seeds_for_config(&baseline);

    let seed_neutral: Vec<PreferenceProbe> = vec![
        ("zoom chip", |config| {
            config.ui.toolbar.show_zoom_chip = !config.ui.toolbar.show_zoom_chip;
        }),
        ("floating badge", |config| {
            config.ui.show_floating_badge = !config.ui.show_floating_badge;
        }),
        ("click highlight", |config| {
            config.ui.click_highlight.enabled = !config.ui.click_highlight.enabled;
            config.ui.click_highlight.show_on_highlight_tool =
                !config.ui.click_highlight.show_on_highlight_tool;
        }),
    ];
    for (label, change) in seed_neutral {
        let mut config = baseline.clone();
        change(&mut config);
        assert_eq!(
            seeds_for_config(&config),
            baseline_seeds,
            "{label} moves a runtime seed without being declared seed-moving"
        );
    }
}

#[test]
fn toolbar_seed_registry_covers_every_runtime_routed_target() {
    let config = Config::default();
    let board_pin_seeds = board_pin_seeds_from_input(&input_from_config(&config));
    let seeds = runtime_seeds_from_config(&config, &board_pin_seeds).expect("valid default seeds");

    for target in [
        InteractionSeedTarget::TopPinned,
        InteractionSeedTarget::SidePinned,
        InteractionSeedTarget::TopMinimized,
        InteractionSeedTarget::SideMinimized,
        InteractionSeedTarget::SidePane,
        InteractionSeedTarget::TopPosition,
        InteractionSeedTarget::SidePosition,
        InteractionSeedTarget::TopDisplayMode,
    ] {
        assert!(seeds.get(&target).is_some(), "missing seed for {target:?}");
    }
    for section in ToolbarSideSection::ALL {
        assert!(
            seeds
                .get(&InteractionSeedTarget::CollapsedSection(section))
                .is_some()
        );
    }
    for id in resettable_individual_toolbar_item_ids() {
        assert!(
            seeds
                .get(&InteractionSeedTarget::ItemVisibility(id))
                .is_some()
        );
    }
    for flag in crate::config::ToolbarSectionFlag::ALL {
        assert!(
            seeds
                .get(&InteractionSeedTarget::ItemVisibility(flag.item_id()))
                .is_none(),
            "authored section {flag:?} must not become a runtime seed"
        );
    }
    for group in ToolbarItemOrderGroup::ALL {
        assert!(
            seeds
                .get(&InteractionSeedTarget::ItemOrder(group))
                .is_some()
        );
    }
}

#[test]
fn toolbar_section_visibility_is_not_seeded_into_runtime_state() {
    let mut config = Config::default();
    config.ui.toolbar.layout_mode = crate::config::ToolbarLayoutMode::Regular;
    config.ui.toolbar.items = crate::config::ToolbarItemsConfig::default();
    config.ui.toolbar.show_zoom_actions = false;

    let board_pin_seeds = board_pin_seeds_from_input(&input_from_config(&config));
    let seeds = runtime_seeds_from_config(&config, &board_pin_seeds).expect("valid folded seeds");
    assert!(
        seeds
            .get(&InteractionSeedTarget::ItemVisibility(
                crate::config::ToolbarSectionFlag::ZoomActions.item_id(),
            ))
            .is_none()
    );
}

#[test]
fn runtime_rebuild_reuses_minimize_and_pane_transition_cleanup() {
    let temp = crate::test_temp::tempdir().unwrap();
    let runtime_path = temp.path().join("runtime-ui.toml");
    let config = Config::default();
    let mut source = input_from_config(&config);
    let mut runtime = test_runtime(&config, &runtime_path);

    let minimized = runtime
        .begin_toolbar_mutation(ToolbarRuntimeUiPersistenceTarget::TopMinimized, &source)
        .expect("top-minimized permit");
    source.toolbar_top_minimized = true;
    assert!(matches!(
        runtime.finish_toolbar_mutation(minimized, true, &source),
        ToolbarRuntimeFinish::KeepPreview
    ));
    assert!(settle_runtime(&mut runtime).rollbacks.is_empty());

    let pane = runtime
        .begin_toolbar_mutation(ToolbarRuntimeUiPersistenceTarget::SidePane, &source)
        .expect("side-pane permit");
    source.toolbar_side_pane = SidePane::Canvas;
    assert!(matches!(
        runtime.finish_toolbar_mutation(pane, true, &source),
        ToolbarRuntimeFinish::KeepPreview
    ));
    assert!(settle_runtime(&mut runtime).rollbacks.is_empty());

    let mut rebuilt = input_from_config(&config);
    rebuilt.toolbar_shapes_expanded = true;
    rebuilt.toolbar_top_overflow_open = true;
    rebuilt.toolbar_session_popover_open = true;
    rebuilt.toolbar_settings_popover_open = true;
    rebuilt.toolbar_canvas_popover_open = true;
    rebuilt.toolbar_side_pane = SidePane::Settings;
    rebuilt.toolbar_customize_items_open = true;
    rebuilt.toolbar_customize_items_group =
        Some(crate::ui::toolbar::ToolbarItemCustomizeGroup::SideSections);
    let mut positions = ToolbarPositionSnapshot {
        top: (0.0, 0.0),
        side: (0.0, 0.0),
    };

    runtime.apply_live_state(&mut rebuilt, &mut positions);

    assert!(rebuilt.toolbar_top_minimized);
    assert!(!rebuilt.toolbar_shapes_expanded);
    assert!(!rebuilt.toolbar_top_overflow_open);
    assert!(!rebuilt.toolbar_session_popover_open);
    assert!(!rebuilt.toolbar_settings_popover_open);
    assert!(!rebuilt.toolbar_canvas_popover_open);
    assert_eq!(rebuilt.toolbar_side_pane, SidePane::Canvas);
    assert!(!rebuilt.toolbar_customize_items_open);
    assert!(rebuilt.toolbar_customize_items_group.is_none());
    runtime.shutdown_blocking();
}

#[test]
fn supported_runtime_reset_returns_live_state_to_configured_defaults() {
    let temp = crate::test_temp::tempdir().unwrap();
    let runtime_path = temp.path().join("runtime-ui.toml");
    let config = Config::default();
    let mut input = input_from_config(&config);
    let mut runtime = test_runtime(&config, &runtime_path);

    let prepared = runtime
        .begin_toolbar_mutation(ToolbarRuntimeUiPersistenceTarget::TopPinned, &input)
        .expect("top-pin permit");
    assert!(prepared.is_persistent_preview());
    input.toolbar_top_pinned = false;
    assert!(matches!(
        runtime.finish_toolbar_mutation(prepared, true, &input),
        ToolbarRuntimeFinish::KeepPreview
    ));
    assert!(settle_runtime(&mut runtime).rollbacks.is_empty());
    assert!(runtime_path.exists());
    assert_eq!(
        runtime.persistence_snapshot().mode,
        RuntimeUiPersistenceMode::Supported
    );

    assert!(runtime.handle_persistence_lifecycle_event(&ToolbarEvent::RequestRuntimeUiReset));
    assert_eq!(
        runtime.persistence_snapshot().mode,
        RuntimeUiPersistenceMode::Resetting
    );
    let drain = settle_runtime(&mut runtime);
    assert!(drain.lifecycle_changed);
    assert!(drain.rebuild_live);
    assert_eq!(
        runtime.persistence_snapshot().mode,
        RuntimeUiPersistenceMode::Missing
    );
    assert!(!runtime_path.exists());

    let mut positions = ToolbarPositionSnapshot {
        top: (0.0, 0.0),
        side: (0.0, 0.0),
    };
    runtime.apply_live_state(&mut input, &mut positions);
    assert_eq!(input.toolbar_top_pinned, config.ui.toolbar.top_pinned);
}

#[test]
fn successful_writer_cleanup_artifacts_reach_toolbar_diagnostics() {
    let temp = crate::test_temp::tempdir().unwrap();
    let runtime_path = temp.path().join("runtime-ui.toml");
    let artifact_path = temp.path().join("runtime-ui.wayscriber-recovery-test.toml");
    let config = Config::default();
    let mut input = input_from_config(&config);
    let mut runtime = controller_only_runtime(&config, &runtime_path);

    let prepared = runtime
        .begin_toolbar_mutation(ToolbarRuntimeUiPersistenceTarget::TopPinned, &input)
        .expect("top-pin permit");
    input.toolbar_top_pinned = false;
    let desired = toolbar_values(ToolbarRuntimeUiPersistenceTarget::TopPinned, &input).unwrap();
    assert!(matches!(
        runtime.controller.finish_preview(
            prepared.session,
            RuntimePreviewFinishIntent::Commit(desired)
        ),
        PreviewFinishResult::AcceptedRuntime { .. }
    ));
    let request = runtime
        .controller
        .take_source_mutation()
        .expect("undispatched replacement");
    let new_source = RuntimeStateSourceRevision::present(
        request.expected_source.path_identity().clone(),
        b"version = 1\n".as_slice(),
    );
    let artifact = RuntimeStateRecoveryArtifact {
        path: artifact_path.clone(),
        observation: RuntimeStateSourceObservation {
            revision: new_source.clone(),
            envelope: RuntimeStateObservedEnvelope::Version(1),
        },
    };
    runtime.integrate_writer_completion(RuntimeStateWriterCompletion::SourceMutation(
        SourceMutationResult::Applied {
            id: request.id,
            applied_through: request.accepted_through,
            new_source,
            recovery_artifacts: vec![artifact],
        },
    ));

    assert_eq!(
        runtime.persistence_snapshot().recovery_artifacts,
        vec![artifact_path]
    );
}

#[test]
fn unsupported_runtime_reset_requires_confirmation_and_preserves_exact_source() {
    let temp = crate::test_temp::tempdir().unwrap();
    let runtime_path = temp.path().join("runtime-ui.toml");
    let unsupported = b"version = 73\nfuture = 'preserve exactly'\n";
    fs::write(&runtime_path, unsupported).unwrap();
    let config = Config::default();
    let mut runtime = test_runtime(&config, &runtime_path);
    assert_eq!(
        runtime.persistence_snapshot().mode,
        RuntimeUiPersistenceMode::UnsupportedReadOnly { version: Some(73) }
    );

    assert!(runtime.handle_persistence_lifecycle_event(&ToolbarEvent::RequestRuntimeUiReset));
    assert_eq!(
        runtime.persistence_snapshot().mode,
        RuntimeUiPersistenceMode::AwaitingUnsupportedResetConfirmation { version: Some(73) }
    );
    assert!(
        runtime.handle_persistence_lifecycle_event(&ToolbarEvent::CancelUnsupportedRuntimeUiReset)
    );
    assert_eq!(fs::read(&runtime_path).unwrap(), unsupported);
    assert_eq!(
        runtime.persistence_snapshot().mode,
        RuntimeUiPersistenceMode::UnsupportedReadOnly { version: Some(73) }
    );

    assert!(runtime.handle_persistence_lifecycle_event(&ToolbarEvent::RequestRuntimeUiReset));
    assert!(
        runtime.handle_persistence_lifecycle_event(&ToolbarEvent::ConfirmUnsupportedRuntimeUiReset)
    );
    let snapshot = wait_for_runtime_mode(&mut runtime, RuntimeUiPersistenceMode::Missing);
    assert!(!runtime_path.exists());
    assert_eq!(snapshot.recovery_artifacts.len(), 1);
    assert_eq!(
        fs::read(&snapshot.recovery_artifacts[0]).unwrap(),
        unsupported
    );
}

#[test]
fn invalid_runtime_reset_keeps_the_incident_handle_paired_with_confirmation() {
    let temp = crate::test_temp::tempdir().unwrap();
    let runtime_path = temp.path().join("runtime-ui.toml");
    let invalid = b"this is not = valid = toml\n";
    fs::write(&runtime_path, invalid).unwrap();
    let config = Config::default();
    let mut runtime = test_runtime_allow_startup_incident(&config, &runtime_path);
    assert_eq!(
        runtime.persistence_snapshot().mode,
        RuntimeUiPersistenceMode::Unhealthy
    );

    assert!(
        runtime.handle_persistence_lifecycle_event(
            &ToolbarEvent::RequestPreserveInvalidRuntimeUiReset
        )
    );
    assert!(
        runtime.has_retained_recovery_client(),
        "the adapter owns cancellation and completion until the exact attempt terminalizes"
    );
    wait_for_runtime_mode(
        &mut runtime,
        RuntimeUiPersistenceMode::AwaitingInvalidResetConfirmation,
    );
    assert!(
        runtime
            .handle_persistence_lifecycle_event(&ToolbarEvent::CancelPreserveInvalidRuntimeUiReset)
    );
    assert_eq!(fs::read(&runtime_path).unwrap(), invalid);
    assert_eq!(
        runtime.persistence_snapshot().mode,
        RuntimeUiPersistenceMode::Unhealthy
    );

    assert!(
        runtime.handle_persistence_lifecycle_event(
            &ToolbarEvent::RequestPreserveInvalidRuntimeUiReset
        )
    );
    wait_for_runtime_mode(
        &mut runtime,
        RuntimeUiPersistenceMode::AwaitingInvalidResetConfirmation,
    );
    assert!(
        runtime.handle_persistence_lifecycle_event(
            &ToolbarEvent::ConfirmPreserveInvalidRuntimeUiReset
        )
    );
    let snapshot = wait_for_runtime_mode(&mut runtime, RuntimeUiPersistenceMode::Missing);
    assert!(!runtime_path.exists());
    assert_eq!(snapshot.recovery_artifacts.len(), 1);
    assert_eq!(fs::read(&snapshot.recovery_artifacts[0]).unwrap(), invalid);
}

#[test]
fn cancelling_read_only_recovery_returns_the_same_incident_to_the_actor() {
    let temp = crate::test_temp::tempdir().unwrap();
    let runtime_path = temp.path().join("runtime-ui.toml");
    let invalid = b"not valid runtime state";
    fs::write(&runtime_path, invalid).unwrap();
    let config = Config::default();
    let mut runtime = test_runtime_allow_startup_incident(&config, &runtime_path);

    assert!(
        runtime.handle_persistence_lifecycle_event(
            &ToolbarEvent::RequestPreserveInvalidRuntimeUiReset
        )
    );
    assert!(runtime.has_retained_recovery_client());
    assert!(runtime.handle_persistence_lifecycle_event(&ToolbarEvent::CancelRuntimeUiRecovery));
    wait_for_runtime_mode(&mut runtime, RuntimeUiPersistenceMode::Unhealthy);
    assert!(!runtime.has_retained_recovery_client());
    assert_eq!(fs::read(&runtime_path).unwrap(), invalid);

    // The returned capability remains owned by this exact incident, so a
    // subsequent actor action can check it out again instead of stranding the
    // barrier behind an inert cancellation token.
    assert!(
        runtime.handle_persistence_lifecycle_event(
            &ToolbarEvent::RequestPreserveInvalidRuntimeUiReset
        )
    );
    wait_for_runtime_mode(
        &mut runtime,
        RuntimeUiPersistenceMode::AwaitingInvalidResetConfirmation,
    );
}

#[test]
fn cancelling_read_only_recovery_rebuilds_a_staged_seed_reload() {
    let temp = crate::test_temp::tempdir().unwrap();
    let runtime_path = temp.path().join("runtime-ui.toml");
    fs::write(&runtime_path, b"not valid runtime state").unwrap();
    let config_a = Config::default();
    let mut input = input_from_config(&config_a);
    let mut positions = ToolbarPositionSnapshot {
        top: (
            config_a.ui.toolbar.top_offset,
            config_a.ui.toolbar.top_offset_y,
        ),
        side: (
            config_a.ui.toolbar.side_offset_x,
            config_a.ui.toolbar.side_offset,
        ),
    };
    let mut runtime = test_runtime_allow_startup_incident(&config_a, &runtime_path);

    assert!(
        runtime.handle_persistence_lifecycle_event(
            &ToolbarEvent::RequestPreserveInvalidRuntimeUiReset
        )
    );
    let mut config_b = config_a;
    config_b.ui.toolbar.top_pinned = false;
    let refresh = runtime.refresh_config_seeds(&config_b, &mut input, &mut positions);
    assert!(!refresh.applied, "the reload is staged behind recovery");
    assert!(
        input.toolbar_top_pinned,
        "live input still has the old seed"
    );

    assert!(runtime.handle_persistence_lifecycle_event(&ToolbarEvent::CancelRuntimeUiRecovery));
    let drain = runtime.drain_writer_completions();
    assert!(
        drain.rebuild_live,
        "synchronous cancellation must publish the staged live authority"
    );
    runtime.apply_live_state(&mut input, &mut positions);
    assert!(!input.toolbar_top_pinned);
    runtime.shutdown_blocking();
}

#[test]
fn runtime_toolbar_routes_leave_authored_config_bytes_exactly_unchanged() {
    const AUTHORED: &[u8] = b"# keep this formatting and comment\n[ui.toolbar]\ntop_pinned = true\nside_pinned = true\n";
    let temp = crate::test_temp::tempdir().unwrap();
    let config_path = temp.path().join("config.toml");
    let runtime_path = temp.path().join("data/runtime-ui.toml");
    fs::write(&config_path, AUTHORED).unwrap();
    let config: Config = toml::from_str(std::str::from_utf8(AUTHORED).unwrap()).unwrap();
    let mut input = input_from_config(&config);
    let mut runtime = test_runtime(&config, &runtime_path);

    let mutations = [
        ToolbarRuntimeUiPersistenceTarget::TopPinned,
        ToolbarRuntimeUiPersistenceTarget::SidePinned,
        ToolbarRuntimeUiPersistenceTarget::TopMinimized,
        ToolbarRuntimeUiPersistenceTarget::SideMinimized,
        ToolbarRuntimeUiPersistenceTarget::SidePane,
        ToolbarRuntimeUiPersistenceTarget::CollapsedSection(ToolbarSideSection::Colors),
        ToolbarRuntimeUiPersistenceTarget::TopDisplayMode,
        ToolbarRuntimeUiPersistenceTarget::ToolbarVisibility,
    ];
    for target in mutations {
        let prepared = runtime
            .begin_toolbar_mutation(target, &input)
            .expect("runtime mutation permit");
        match target {
            ToolbarRuntimeUiPersistenceTarget::TopPinned => input.toolbar_top_pinned = false,
            ToolbarRuntimeUiPersistenceTarget::SidePinned => input.toolbar_side_pinned = false,
            ToolbarRuntimeUiPersistenceTarget::ToolbarVisibility => {
                input.toolbar_top_pinned = false;
                input.toolbar_side_pinned = false;
            }
            ToolbarRuntimeUiPersistenceTarget::TopMinimized => {
                input.toolbar_top_minimized = true;
            }
            ToolbarRuntimeUiPersistenceTarget::SideMinimized => {
                input.toolbar_side_minimized = true;
            }
            ToolbarRuntimeUiPersistenceTarget::SidePane => {
                input.toolbar_side_pane = SidePane::Settings;
            }
            ToolbarRuntimeUiPersistenceTarget::CollapsedSection(section) => {
                input.toolbar_collapsed_side_sections.insert(section);
            }
            ToolbarRuntimeUiPersistenceTarget::TopDisplayMode => {
                input.set_top_display_mode(crate::config::TopDisplayMode::Micro);
            }
            _ => unreachable!(),
        }
        assert!(matches!(
            runtime.finish_toolbar_mutation(prepared, true, &input),
            ToolbarRuntimeFinish::KeepPreview
        ));
    }
    let visibility = ToolbarRuntimeUiPersistenceTarget::ItemVisibility {
        id: ids::TOP_TOOL_PEN,
        setting: ItemVisibilitySetting::Hidden,
    };
    let prepared = runtime
        .begin_toolbar_mutation(visibility, &input)
        .expect("visibility permit");
    assert!(
        input.set_toolbar_item_visibility_setting(ids::TOP_TOOL_PEN, ItemVisibilitySetting::Hidden)
    );
    assert!(matches!(
        runtime.finish_toolbar_mutation(prepared, true, &input),
        ToolbarRuntimeFinish::KeepPreview
    ));
    assert!(settle_runtime(&mut runtime).rollbacks.is_empty());
    assert_eq!(fs::read(&config_path).unwrap(), AUTHORED);
    assert!(runtime_path.exists());
    runtime.shutdown_blocking();
}

#[test]
fn board_pin_is_runtime_owned_and_survives_restart_without_touching_config() {
    const AUTHORED: &[u8] = b"# authored config bytes stay exact\n";
    let temp = crate::test_temp::tempdir().unwrap();
    let config_path = temp.path().join("config.toml");
    let runtime_path = temp.path().join("data/runtime-ui.toml");
    fs::write(&config_path, AUTHORED).unwrap();
    let config = Config::default();
    let mut input = input_from_config(&config);
    let mut runtime = test_runtime(&config, &runtime_path);

    assert!(!board_pinned(&input, "whiteboard"));
    assert!(matches!(
        commit_board_pin_toggle(&mut runtime, &config, &mut input, "whiteboard"),
        ToolbarRuntimeFinish::KeepPreview
    ));
    assert!(settle_runtime(&mut runtime).rollbacks.is_empty());
    assert!(board_pinned(&input, "whiteboard"));
    assert_eq!(fs::read(&config_path).unwrap(), AUTHORED);
    assert!(
        !input
            .boards
            .to_config()
            .items
            .iter()
            .find(|item| item.id == "whiteboard")
            .expect("whiteboard config snapshot")
            .pinned
    );
    runtime.shutdown_blocking();

    let mut restarted_input = input_from_config(&config);
    let mut restarted = test_runtime(&config, &runtime_path);
    restarted.apply_startup_state(&mut restarted_input);
    assert!(board_pinned(&restarted_input, "whiteboard"));
    assert_eq!(fs::read(&config_path).unwrap(), AUTHORED);
    restarted.shutdown_blocking();
}

#[test]
fn session_only_board_pin_survives_startup_until_session_identity_is_known() {
    let temp = crate::test_temp::tempdir().unwrap();
    let runtime_path = temp.path().join("runtime-ui.toml");
    fs::write(
        &runtime_path,
        br#"version = 1

[boards.pinned.session-board]
seed = false
value = true
"#,
    )
    .unwrap();
    let config = Config::default();
    let mut input = input_from_config(&config);
    let mut runtime = test_runtime(&config, &runtime_path);
    assert!(!input.boards.has_board("session-board"));
    assert!(input.boards.ensure_board("session-board").is_some());
    input
        .boards
        .sync_pin_seeds_from_config(&config.resolved_boards());
    let mut positions = ToolbarPositionSnapshot {
        top: (0.0, 0.0),
        side: (0.0, 0.0),
    };

    let refresh = runtime.refresh_config_seeds(&config, &mut input, &mut positions);
    assert!(refresh.applied);
    assert!(board_pinned(&input, "session-board"));
    assert!(
        fs::read_to_string(&runtime_path)
            .unwrap()
            .contains("session-board")
    );
    runtime.shutdown_blocking();
}

#[test]
fn absent_provisional_board_pin_is_pruned_after_session_reconciliation() {
    let temp = crate::test_temp::tempdir().unwrap();
    let runtime_path = temp.path().join("runtime-ui.toml");
    fs::write(
        &runtime_path,
        br#"version = 1

[boards.pinned.stale-session-board]
seed = false
value = true
"#,
    )
    .unwrap();
    let config = Config::default();
    let mut input = input_from_config(&config);
    let mut runtime = test_runtime(&config, &runtime_path);
    let mut positions = ToolbarPositionSnapshot {
        top: (0.0, 0.0),
        side: (0.0, 0.0),
    };

    let refresh = runtime.refresh_config_seeds(&config, &mut input, &mut positions);
    assert!(refresh.applied);
    assert!(settle_runtime(&mut runtime).rollbacks.is_empty());
    assert!(
        !fs::read_to_string(&runtime_path)
            .unwrap()
            .contains("stale-session-board")
    );
    runtime.shutdown_blocking();
}

#[test]
fn newly_created_board_does_not_adopt_a_provisional_session_pin() {
    let temp = crate::test_temp::tempdir().unwrap();
    let runtime_path = temp.path().join("runtime-ui.toml");
    fs::write(
        &runtime_path,
        br#"version = 1

[boards.pinned.board-6]
seed = false
value = true
"#,
    )
    .unwrap();
    let config = Config::default();
    let mut input = input_from_config(&config);
    let mut runtime = test_runtime(&config, &runtime_path);
    assert!(input.create_board());
    let board_id = input.board_id().to_string();
    assert_eq!(board_id, "board-6");
    let pin_seed = input.boards.pin_seed(&board_id).unwrap();
    let pinned = board_pinned(&input, &board_id);

    let finish =
        runtime.restore_board_identity(&config, &mut input, board_id.clone(), pin_seed, pinned);
    assert!(finish.is_none());
    assert!(settle_runtime(&mut runtime).rollbacks.is_empty());
    runtime.apply_live_state(
        &mut input,
        &mut ToolbarPositionSnapshot {
            top: (0.0, 0.0),
            side: (0.0, 0.0),
        },
    );
    assert!(!board_pinned(&input, &board_id));
    runtime.shutdown_blocking();

    let mut restarted_input = input_from_config(&config);
    assert!(restarted_input.create_board());
    let mut restarted = test_runtime(&config, &runtime_path);
    restarted.apply_startup_state(&mut restarted_input);
    assert!(!board_pinned(&restarted_input, &board_id));
    restarted.shutdown_blocking();
}

#[test]
fn restored_board_pin_is_replayed_after_same_authority_recovery() {
    let temp = crate::test_temp::tempdir().unwrap();
    let runtime_path = temp.path().join("runtime-ui.toml");
    let config = Config::default();
    let mut input = input_from_config(&config);
    let mut runtime = test_runtime(&config, &runtime_path);

    let target = InteractionSeedTarget::TopPinned;
    let permit = runtime
        .controller
        .begin_mutation(RuntimeUiMutationScope::one(target.clone()))
        .unwrap();
    assert!(matches!(
        runtime.controller.commit(
            permit,
            RuntimeUiMutationValues::one(target, InteractionSeedValue::Bool(false)).unwrap(),
        ),
        CommitResult::Accepted { .. }
    ));
    let failed = runtime
        .controller
        .take_source_mutation()
        .expect("replacement to fail");
    let active = RuntimeStateSourceObservation::missing(failed.expected_source.clone());
    let incident = match runtime
        .controller
        .submit_source_mutation(SourceMutationResult::Failed {
            id: failed.id,
            error: RuntimeStateIoError::new("temporary board-pin test failure"),
            active: Some(active),
            recovery_artifacts: Vec::new(),
            path_effect: RuntimeStateFailurePathEffect::Known(
                RuntimeStateObservedPathEffect::Untouched,
            ),
        }) {
        SubmitSourceMutationResult::PersistenceUnhealthy { incident, .. } => incident,
        result => panic!("unexpected persistence result: {result:?}"),
    };

    assert!(input.create_board());
    let board_id = input.board_id().to_string();
    assert_eq!(board_id, "board-6");
    let pin_seed = input.boards.pin_seed(&board_id).unwrap();
    assert!(input.apply_board_pinned_runtime(&board_id, true));
    assert!(
        runtime
            .restore_board_identity(&config, &mut input, board_id.clone(), pin_seed, true)
            .is_none()
    );
    assert!(board_pinned(&input, &board_id));
    assert_eq!(runtime.deferred_board_pin_restores.len(), 1);

    let recovery = match runtime
        .controller
        .checkout_persistence_recovery_handle(incident)
    {
        CheckoutPersistenceRecoveryHandleResult::CheckedOut(handle) => handle,
        result => panic!("recovery checkout failed: {result:?}"),
    };
    let client = match runtime
        .controller
        .begin_persistence_recovery(PersistenceRecoveryRequest {
            recovery,
            action: PersistenceRecoveryAction::RetryPending,
        }) {
        BeginPersistenceRecoveryResult::Started { client, .. } => client,
        result => panic!("recovery start failed: {result:?}"),
    };
    runtime.dispatch_writer_command();
    let mut rebuild_live = false;
    for _ in 0..400 {
        let drain = runtime.drain_writer_completions();
        rebuild_live |= drain.rebuild_live;
        if runtime.controller.active_barrier().is_none() {
            break;
        }
        thread::sleep(Duration::from_millis(5));
    }
    assert!(runtime.controller.active_barrier().is_none());
    assert!(matches!(
        client.completion.try_recv(),
        Some(PersistenceRecoveryResult::Recovered { .. })
    ));
    assert!(rebuild_live);
    runtime.apply_live_state(
        &mut input,
        &mut ToolbarPositionSnapshot {
            top: (0.0, 0.0),
            side: (0.0, 0.0),
        },
    );
    assert!(!board_pinned(&input, &board_id));

    let finishes = runtime.finish_deferred_board_pin_restores(&mut input);
    assert_eq!(finishes.len(), 1);
    assert!(matches!(finishes[0], ToolbarRuntimeFinish::KeepPreview));
    assert!(board_pinned(&input, &board_id));
    assert!(runtime.deferred_board_pin_restores.is_empty());
    assert!(settle_runtime(&mut runtime).rollbacks.is_empty());
    assert!(
        fs::read_to_string(&runtime_path)
            .unwrap()
            .contains("board-6")
    );
    runtime.shutdown_blocking();
}

#[test]
fn deferred_board_pin_restore_is_discarded_when_reset_changes_authority() {
    let temp = crate::test_temp::tempdir().unwrap();
    let runtime_path = temp.path().join("runtime-ui.toml");
    let config = Config::default();
    let mut input = input_from_config(&config);
    let mut runtime = test_runtime(&config, &runtime_path);
    let original_epoch = runtime.controller.authority_epoch();
    assert!(matches!(
        runtime.controller.request_supported_reset(),
        RequestResetResult::Started { .. }
    ));

    assert!(input.create_board());
    let board_id = input.board_id().to_string();
    let pin_seed = input.boards.pin_seed(&board_id).unwrap();
    assert!(input.apply_board_pinned_runtime(&board_id, true));
    assert!(
        runtime
            .restore_board_identity(&config, &mut input, board_id.clone(), pin_seed, true)
            .is_none()
    );
    assert_eq!(runtime.deferred_board_pin_restores.len(), 1);

    runtime.dispatch_writer_command();
    let drain = settle_runtime(&mut runtime);
    assert!(drain.rebuild_live);
    assert!(runtime.controller.active_barrier().is_none());
    assert_ne!(runtime.controller.authority_epoch(), original_epoch);
    runtime.apply_live_state(
        &mut input,
        &mut ToolbarPositionSnapshot {
            top: (0.0, 0.0),
            side: (0.0, 0.0),
        },
    );
    assert!(!board_pinned(&input, &board_id));

    assert!(
        runtime
            .finish_deferred_board_pin_restores(&mut input)
            .is_empty()
    );
    assert!(runtime.deferred_board_pin_restores.is_empty());
    assert!(!board_pinned(&input, &board_id));
    assert!(!runtime_path.exists());
    runtime.shutdown_blocking();
}

#[test]
fn delayed_delete_and_same_id_reuse_cannot_resurrect_old_board_pin() {
    let temp = crate::test_temp::tempdir().unwrap();
    let runtime_path = temp.path().join("runtime-ui.toml");
    let config = Config::default();
    let mut input = input_from_config(&config);
    let mut runtime = test_runtime(&config, &runtime_path);

    assert!(matches!(
        commit_board_pin_toggle(&mut runtime, &config, &mut input, "whiteboard"),
        ToolbarRuntimeFinish::KeepPreview
    ));
    runtime.remove_board_identity(&config, "whiteboard");
    let finish =
        runtime.restore_board_identity(&config, &mut input, "whiteboard".to_string(), false, false);
    assert!(finish.is_none());
    assert!(settle_runtime(&mut runtime).rollbacks.is_empty());
    let mut positions = ToolbarPositionSnapshot {
        top: (0.0, 0.0),
        side: (0.0, 0.0),
    };
    runtime.apply_live_state(&mut input, &mut positions);
    assert!(!board_pinned(&input, "whiteboard"));
    runtime.shutdown_blocking();

    let mut restarted_input = input_from_config(&config);
    let mut restarted = test_runtime(&config, &runtime_path);
    restarted.apply_startup_state(&mut restarted_input);
    assert!(!board_pinned(&restarted_input, "whiteboard"));
    restarted.shutdown_blocking();
}

#[test]
fn stale_deferred_board_pin_is_rejected_after_authored_pin_reload() {
    let temp = crate::test_temp::tempdir().unwrap();
    let runtime_path = temp.path().join("runtime-ui.toml");
    let config_a = Config::default();
    let mut input = input_from_config(&config_a);
    let mut runtime = test_runtime(&config_a, &runtime_path);
    let captured_seed = input.boards.pin_seed("whiteboard").unwrap();
    let accepted_before = runtime.controller.pipeline().latest_accepted();

    let mut config_b = config_a;
    config_b
        .boards
        .as_mut()
        .expect("configured boards")
        .items
        .iter_mut()
        .find(|item| item.id == "whiteboard")
        .expect("whiteboard config")
        .pinned = true;
    input
        .boards
        .sync_pin_seeds_from_config(&config_b.resolved_boards());
    let mut positions = ToolbarPositionSnapshot {
        top: (0.0, 0.0),
        side: (0.0, 0.0),
    };
    let refresh = runtime.refresh_config_seeds(&config_b, &mut input, &mut positions);
    assert!(refresh.applied);
    assert!(board_pinned(&input, "whiteboard"));

    assert!(
        runtime
            .begin_board_pin_toggle(&config_b, "whiteboard".to_string(), captured_seed, true,)
            .is_none(),
        "deferred work captured under the old seed must be consumed"
    );
    assert_eq!(
        runtime.controller.pipeline().latest_accepted(),
        accepted_before
    );
    runtime.shutdown_blocking();
}

#[test]
fn unrelated_board_pin_write_preserves_supported_unknown_fields() {
    let temp = crate::test_temp::tempdir().unwrap();
    let runtime_path = temp.path().join("runtime-ui.toml");
    fs::write(
        &runtime_path,
        br#"version = 1
future_root = { answer = 42 }

[boards]
future_boards = "kept"

[boards.pinned.whiteboard]
seed = false
value = true
future_entry = [1, 2, 3]
"#,
    )
    .unwrap();
    let config = Config::default();
    let mut input = input_from_config(&config);
    let mut runtime = test_runtime(&config, &runtime_path);
    runtime.apply_startup_state(&mut input);
    assert!(board_pinned(&input, "whiteboard"));

    assert!(matches!(
        commit_board_pin_toggle(&mut runtime, &config, &mut input, "blackboard"),
        ToolbarRuntimeFinish::KeepPreview
    ));
    assert!(settle_runtime(&mut runtime).rollbacks.is_empty());
    let value: toml::Value = toml::from_str(&fs::read_to_string(&runtime_path).unwrap()).unwrap();
    assert_eq!(value["future_root"]["answer"].as_integer(), Some(42));
    assert_eq!(value["boards"]["future_boards"].as_str(), Some("kept"));
    assert_eq!(
        value["boards"]["pinned"]["whiteboard"]["future_entry"]
            .as_array()
            .map(Vec::len),
        Some(3)
    );
    runtime.shutdown_blocking();
}

#[test]
fn global_runtime_reset_clears_board_pin_override_and_live_value() {
    let temp = crate::test_temp::tempdir().unwrap();
    let runtime_path = temp.path().join("runtime-ui.toml");
    let config = Config::default();
    let mut input = input_from_config(&config);
    let mut runtime = test_runtime(&config, &runtime_path);
    assert!(matches!(
        commit_board_pin_toggle(&mut runtime, &config, &mut input, "whiteboard"),
        ToolbarRuntimeFinish::KeepPreview
    ));
    assert!(settle_runtime(&mut runtime).rollbacks.is_empty());
    assert!(board_pinned(&input, "whiteboard"));

    assert!(matches!(
        runtime.controller.request_supported_reset(),
        RequestResetResult::Started { .. }
    ));
    runtime.dispatch_writer_command();
    let drain = settle_runtime(&mut runtime);
    assert!(drain.rollbacks.is_empty());
    assert!(drain.rebuild_live);
    let mut positions = ToolbarPositionSnapshot {
        top: (0.0, 0.0),
        side: (0.0, 0.0),
    };
    runtime.apply_live_state(&mut input, &mut positions);
    assert!(!board_pinned(&input, "whiteboard"));
    assert!(!runtime_path.exists());
    runtime.shutdown_blocking();
}

#[test]
fn unsupported_runtime_file_keeps_toolbar_mutations_live_only_and_byte_exact() {
    const UNSUPPORTED: &[u8] = b"version = 22\nfuture = { keep = true }\n";
    let temp = crate::test_temp::tempdir().unwrap();
    let runtime_path = temp.path().join("runtime-ui.toml");
    fs::write(&runtime_path, UNSUPPORTED).unwrap();
    let config = Config::default();
    let mut input = input_from_config(&config);
    let mut runtime = test_runtime(&config, &runtime_path);
    let accepted_before = runtime.controller.pipeline().latest_accepted();

    let target = ToolbarRuntimeUiPersistenceTarget::TopMinimized;
    let prepared = runtime
        .begin_toolbar_mutation(target, &input)
        .expect("unsupported authority permits a live-only preview");
    assert!(!prepared.is_persistent_preview());
    input.toolbar_top_minimized = true;
    assert!(matches!(
        runtime.finish_toolbar_mutation(prepared, true, &input),
        ToolbarRuntimeFinish::KeepPreview
    ));
    assert!(input.toolbar_top_minimized);
    assert_eq!(
        runtime.controller.pipeline().latest_accepted(),
        accepted_before,
        "live-only changes never enter the persistence pipeline"
    );
    assert_eq!(fs::read(&runtime_path).unwrap(), UNSUPPORTED);
    runtime.shutdown_blocking();

    let mut restarted_input = input_from_config(&config);
    let mut restarted = test_runtime(&config, &runtime_path);
    restarted.apply_startup_state(&mut restarted_input);
    assert!(!restarted_input.toolbar_top_minimized);
    assert_eq!(fs::read(&runtime_path).unwrap(), UNSUPPORTED);
    restarted.shutdown_blocking();
}

#[test]
fn factory_visibility_reset_survives_restart_over_nondefault_authored_config() {
    const AUTHORED: &[u8] =
        b"# non-default authored toolbar seed\n[ui.toolbar.items]\nhidden = [\"top.tool.pen\"]\n";
    let temp = crate::test_temp::tempdir().unwrap();
    let config_path = temp.path().join("config.toml");
    let runtime_path = temp.path().join("data/runtime-ui.toml");
    fs::write(&config_path, AUTHORED).unwrap();
    let config: Config = toml::from_str(std::str::from_utf8(AUTHORED).unwrap()).unwrap();
    let mut input = input_from_config(&config);
    assert!(
        input
            .resolved_toolbar_items
            .hidden
            .contains(&ids::TOP_TOOL_PEN)
    );
    assert!(
        !input
            .resolved_toolbar_items
            .hidden
            .contains(&ids::TOP_UTILITY_SCREENSHOT)
    );
    let mut runtime = test_runtime(&config, &runtime_path);
    let accepted_before = runtime.controller.pipeline().latest_accepted();
    let prepared = runtime
        .begin_toolbar_mutation(
            ToolbarRuntimeUiPersistenceTarget::ResetItemVisibility,
            &input,
        )
        .expect("factory reset permit");
    assert!(input.reset_toolbar_item_hidden_overrides());
    assert!(matches!(
        runtime.finish_toolbar_mutation(prepared, true, &input),
        ToolbarRuntimeFinish::KeepPreview
    ));
    assert_eq!(
        runtime.controller.pipeline().latest_accepted().get(),
        accepted_before.get() + 1,
        "the all-item factory reset is one atomic accepted revision"
    );
    assert!(settle_runtime(&mut runtime).rollbacks.is_empty());
    assert_eq!(fs::read(&config_path).unwrap(), AUTHORED);
    runtime.shutdown_blocking();

    let mut restarted_input = input_from_config(&config);
    let mut restarted = test_runtime(&config, &runtime_path);
    restarted.apply_startup_state(&mut restarted_input);
    assert!(
        !restarted_input
            .resolved_toolbar_items
            .hidden
            .contains(&ids::TOP_TOOL_PEN)
    );
    assert!(
        restarted_input
            .resolved_toolbar_items
            .hidden
            .contains(&ids::TOP_UTILITY_SCREENSHOT)
    );
    assert_eq!(fs::read(&config_path).unwrap(), AUTHORED);
    restarted.shutdown_blocking();
}

#[test]
fn factory_order_reset_survives_restart_over_nondefault_authored_config() {
    const AUTHORED: &[u8] = b"# preserve authored order exactly\n[ui.toolbar.items.order]\ntop_tools = [\"top.tool.pen\", \"top.tool.select\"]\n";
    let temp = crate::test_temp::tempdir().unwrap();
    let config_path = temp.path().join("config.toml");
    let runtime_path = temp.path().join("data/runtime-ui.toml");
    fs::write(&config_path, AUTHORED).unwrap();
    let config: Config = toml::from_str(std::str::from_utf8(AUTHORED).unwrap()).unwrap();
    let mut input = input_from_config(&config);
    assert_eq!(
        input
            .resolved_toolbar_items
            .order
            .ordered_ids(ToolbarItemOrderGroup::TopTools)[0],
        ids::TOP_TOOL_PEN
    );
    let mut runtime = test_runtime(&config, &runtime_path);
    let target = ToolbarRuntimeUiPersistenceTarget::ItemOrder(ToolbarItemOrderGroup::TopTools);
    let prepared = runtime
        .begin_toolbar_mutation(target, &input)
        .expect("factory order permit");
    assert!(input.reset_toolbar_item_order(ToolbarItemOrderGroup::TopTools));
    assert!(matches!(
        runtime.finish_toolbar_mutation(prepared, true, &input),
        ToolbarRuntimeFinish::KeepPreview
    ));
    assert!(settle_runtime(&mut runtime).rollbacks.is_empty());
    assert_eq!(fs::read(&config_path).unwrap(), AUTHORED);
    runtime.shutdown_blocking();

    let mut restarted_input = input_from_config(&config);
    let mut restarted = test_runtime(&config, &runtime_path);
    restarted.apply_startup_state(&mut restarted_input);
    assert_eq!(
        restarted_input
            .resolved_toolbar_items
            .order
            .ordered_ids(ToolbarItemOrderGroup::TopTools),
        ToolbarItemsConfig::default()
            .resolved()
            .order
            .ordered_ids(ToolbarItemOrderGroup::TopTools)
    );
    assert_eq!(fs::read(&config_path).unwrap(), AUTHORED);
    restarted.shutdown_blocking();
}

#[test]
fn item_drag_commit_accepts_one_revision_and_cancel_accepts_none() {
    let temp = crate::test_temp::tempdir().unwrap();
    let runtime_path = temp.path().join("runtime-ui.toml");
    let config = Config::default();
    let mut input = input_from_config(&config);
    let mut positions = ToolbarPositionSnapshot {
        top: (0.0, 0.0),
        side: (0.0, 0.0),
    };
    let mut runtime = test_runtime(&config, &runtime_path);
    let before = runtime.controller.pipeline().latest_accepted();

    assert!(runtime.begin_item_drag(ToolbarItemOrderGroup::TopTools, &input));
    assert!(input.start_toolbar_item_drag(ToolbarItemOrderGroup::TopTools, ids::TOP_TOOL_PEN,));
    assert!(input.drag_toolbar_item_over(ToolbarItemOrderGroup::TopTools, 5));
    assert!(matches!(
        runtime.finish_item_drag(true, &input),
        ToolbarRuntimeFinish::KeepPreview
    ));
    input.clear_toolbar_item_drag();
    assert_eq!(
        runtime.controller.pipeline().latest_accepted().get(),
        before.get() + 1
    );
    assert!(settle_runtime(&mut runtime).rollbacks.is_empty());
    let persisted = fs::read(&runtime_path).unwrap();

    let accepted_after_drop = runtime.controller.pipeline().latest_accepted();
    let order_before_cancel = input
        .resolved_toolbar_items
        .order
        .ordered_ids(ToolbarItemOrderGroup::TopTools)
        .to_vec();
    assert!(runtime.begin_item_drag(ToolbarItemOrderGroup::TopTools, &input));
    assert!(input.start_toolbar_item_drag(ToolbarItemOrderGroup::TopTools, ids::TOP_TOOL_PEN,));
    assert!(input.drag_toolbar_item_over(ToolbarItemOrderGroup::TopTools, 0));
    let finish = runtime.finish_item_drag(false, &input);
    input.clear_toolbar_item_drag();
    apply_finish(&mut input, &mut positions, finish);
    assert_eq!(
        input
            .resolved_toolbar_items
            .order
            .ordered_ids(ToolbarItemOrderGroup::TopTools),
        order_before_cancel
    );
    assert_eq!(
        runtime.controller.pipeline().latest_accepted(),
        accepted_after_drop
    );
    assert_eq!(fs::read(&runtime_path).unwrap(), persisted);
    runtime.shutdown_blocking();
}

#[test]
fn unavailable_persistence_item_drag_cancel_restores_original_order() {
    let config = Config::default();
    let mut input = input_from_config(&config);
    let mut previews = UnavailablePersistencePreviews::default();
    let original = input
        .resolved_toolbar_items
        .order
        .ordered_ids(ToolbarItemOrderGroup::TopTools)
        .to_vec();
    let mut positions = ToolbarPositionSnapshot {
        top: (0.0, 0.0),
        side: (0.0, 0.0),
    };

    assert!(previews.begin_item_drag(ToolbarItemOrderGroup::TopTools, &input));
    assert!(input.start_toolbar_item_drag(ToolbarItemOrderGroup::TopTools, ids::TOP_TOOL_PEN,));
    assert!(input.drag_toolbar_item_over(ToolbarItemOrderGroup::TopTools, 5));
    assert_ne!(
        input
            .resolved_toolbar_items
            .order
            .ordered_ids(ToolbarItemOrderGroup::TopTools),
        original
    );

    let finish = previews.finish_item_drag(false);
    input.clear_toolbar_item_drag();
    apply_finish(&mut input, &mut positions, finish);

    assert_eq!(
        input
            .resolved_toolbar_items
            .order
            .ordered_ids(ToolbarItemOrderGroup::TopTools),
        original
    );
}

#[test]
fn unavailable_persistence_position_drag_cancel_restores_starting_offsets() {
    let config = Config::default();
    let mut input = input_from_config(&config);
    let mut previews = UnavailablePersistencePreviews::default();
    let original = ToolbarPositionSnapshot {
        top: (config.ui.toolbar.top_offset, config.ui.toolbar.top_offset_y),
        side: (
            config.ui.toolbar.side_offset_x,
            config.ui.toolbar.side_offset,
        ),
    };
    let mut positions = original;

    assert!(previews.begin_position_drag(MoveDragKind::Side, positions));
    positions.top.0 = 42.0;
    positions.side = (43.0, 44.0);

    let finish = previews.finish_position_drag(false);
    apply_finish(&mut input, &mut positions, finish);

    assert_eq!(positions, original);
}

#[test]
fn persistence_barrier_blocks_updates_without_consuming_untouched_drag_sessions() {
    let temp = crate::test_temp::tempdir().unwrap();
    let runtime_path = temp.path().join("runtime-ui.toml");
    let config = Config::default();
    let mut input = input_from_config(&config);
    let original_order = input
        .resolved_toolbar_items
        .order
        .ordered_ids(ToolbarItemOrderGroup::TopTools)
        .to_vec();
    let original_positions = ToolbarPositionSnapshot {
        top: (config.ui.toolbar.top_offset, config.ui.toolbar.top_offset_y),
        side: (
            config.ui.toolbar.side_offset_x,
            config.ui.toolbar.side_offset,
        ),
    };
    let positions = original_positions;
    let mut runtime = controller_only_runtime(&config, &runtime_path);
    assert!(runtime.begin_item_drag(ToolbarItemOrderGroup::TopTools, &input));
    assert!(input.start_toolbar_item_drag(ToolbarItemOrderGroup::TopTools, ids::TOP_TOOL_PEN,));
    assert!(runtime.begin_position_drag(MoveDragKind::Top, positions));

    assert!(matches!(
        runtime.controller.request_supported_reset(),
        RequestResetResult::Started { .. }
    ));
    assert!(!runtime.item_drag_update_allowed());
    assert!(!runtime.position_drag_update_allowed(MoveDragKind::Top));
    assert!(runtime.item_drag.is_some());
    assert!(runtime.position_drag.is_some());

    assert_eq!(
        input
            .resolved_toolbar_items
            .order
            .ordered_ids(ToolbarItemOrderGroup::TopTools),
        original_order
    );
    assert_eq!(positions, original_positions);
}

#[test]
fn relevant_reload_aborts_item_and_position_previews_without_restoring_old_seed() {
    let temp = crate::test_temp::tempdir().unwrap();
    let runtime_path = temp.path().join("runtime-ui.toml");
    let config_a = Config::default();
    let mut input = input_from_config(&config_a);
    let mut positions = ToolbarPositionSnapshot {
        top: (
            config_a.ui.toolbar.top_offset,
            config_a.ui.toolbar.top_offset_y,
        ),
        side: (
            config_a.ui.toolbar.side_offset_x,
            config_a.ui.toolbar.side_offset,
        ),
    };
    let mut runtime = test_runtime(&config_a, &runtime_path);
    let accepted_before = runtime.controller.pipeline().latest_accepted();
    assert!(runtime.begin_item_drag(ToolbarItemOrderGroup::TopTools, &input));
    assert!(input.start_toolbar_item_drag(ToolbarItemOrderGroup::TopTools, ids::TOP_TOOL_PEN,));
    assert!(input.drag_toolbar_item_over(ToolbarItemOrderGroup::TopTools, 5));

    let mut config_b = config_a.clone();
    assert!(config_b.ui.toolbar.items.move_item_to_index(
        ToolbarItemOrderGroup::TopTools,
        ids::TOP_TOOL_PEN,
        8,
    ));
    let expected_b = config_b
        .ui
        .toolbar
        .items
        .resolved()
        .order
        .ordered_ids(ToolbarItemOrderGroup::TopTools)
        .to_vec();
    let refresh = runtime.refresh_config_seeds(&config_b, &mut input, &mut positions);
    assert!(refresh.applied);
    assert!(refresh.item_drag_aborted);
    assert!(!refresh.position_drag_aborted);
    assert_eq!(
        input
            .resolved_toolbar_items
            .order
            .ordered_ids(ToolbarItemOrderGroup::TopTools),
        expected_b
    );
    assert!(matches!(
        runtime.finish_item_drag(true, &input),
        ToolbarRuntimeFinish::KeepPreview
    ));
    assert_eq!(
        runtime.controller.pipeline().latest_accepted(),
        accepted_before
    );

    assert!(runtime.begin_position_drag(MoveDragKind::Top, positions));
    positions.top = (42.0, 43.0);
    let mut config_c = config_b;
    config_c.ui.toolbar.top_offset = 100.0;
    config_c.ui.toolbar.top_offset_y = 101.0;
    let refresh = runtime.refresh_config_seeds(&config_c, &mut input, &mut positions);
    assert!(refresh.applied);
    assert!(!refresh.item_drag_aborted);
    assert!(refresh.position_drag_aborted);
    assert_eq!(positions.top, (100.0, 101.0));
    let finish = runtime.finish_position_drag(true, positions);
    assert!(matches!(finish, ToolbarRuntimeFinish::KeepPreview));
    runtime.shutdown_blocking();
}

#[test]
fn side_drag_top_seed_reload_restores_only_the_uncommitted_side_preview() {
    let temp = crate::test_temp::tempdir().unwrap();
    let runtime_path = temp.path().join("runtime-ui.toml");
    let config_a = Config::default();
    let original_side = (
        config_a.ui.toolbar.side_offset_x,
        config_a.ui.toolbar.side_offset,
    );
    let mut positions = ToolbarPositionSnapshot {
        top: (
            config_a.ui.toolbar.top_offset,
            config_a.ui.toolbar.top_offset_y,
        ),
        side: original_side,
    };
    let mut input = input_from_config(&config_a);
    let mut runtime = test_runtime(&config_a, &runtime_path);

    assert!(runtime.begin_position_drag(MoveDragKind::Side, positions));
    positions.side = (42.0, 43.0);

    let mut config_b = config_a;
    config_b.ui.toolbar.top_offset = 100.0;
    config_b.ui.toolbar.top_offset_y = 101.0;
    let refresh = runtime.refresh_config_seeds(&config_b, &mut input, &mut positions);

    assert!(refresh.applied);
    assert!(refresh.position_drag_aborted);
    assert_eq!(positions.top, (100.0, 101.0));
    assert_eq!(
        positions.side, original_side,
        "a top-seed reload must not leave the invalidated side preview live"
    );
    runtime.shutdown_blocking();
}

#[test]
fn unrelated_position_reload_preserves_preview_and_cancel_only_restores_its_scope() {
    let temp = crate::test_temp::tempdir().unwrap();
    let runtime_path = temp.path().join("runtime-ui.toml");
    let config_a = Config::default();
    let original_top = (
        config_a.ui.toolbar.top_offset,
        config_a.ui.toolbar.top_offset_y,
    );
    let mut positions = ToolbarPositionSnapshot {
        top: original_top,
        side: (
            config_a.ui.toolbar.side_offset_x,
            config_a.ui.toolbar.side_offset,
        ),
    };
    let mut input = input_from_config(&config_a);
    let mut runtime = test_runtime(&config_a, &runtime_path);

    assert!(runtime.begin_position_drag(MoveDragKind::Top, positions));
    positions.top = (42.0, 43.0);
    let mut config_b = config_a;
    config_b.ui.toolbar.side_offset_x = 120.0;
    config_b.ui.toolbar.side_offset = 121.0;

    let refresh = runtime.refresh_config_seeds(&config_b, &mut input, &mut positions);
    assert!(refresh.applied);
    assert!(!refresh.position_drag_aborted);
    assert_eq!(
        positions.top,
        (42.0, 43.0),
        "unrelated reload keeps preview"
    );
    assert_eq!(positions.side, (120.0, 121.0));

    let finish = runtime.finish_position_drag(false, positions);
    apply_finish(&mut input, &mut positions, finish);
    assert_eq!(positions.top, original_top);
    assert_eq!(
        positions.side,
        (120.0, 121.0),
        "top-drag rollback must not restore an unrelated side seed"
    );
    runtime.shutdown_blocking();
}

#[test]
fn release_during_barrier_is_consumed_once_and_never_replayed() {
    let temp = crate::test_temp::tempdir().unwrap();
    let runtime_path = temp.path().join("runtime-ui.toml");
    let config = Config::default();
    let mut input = input_from_config(&config);
    let original = input
        .resolved_toolbar_items
        .order
        .ordered_ids(ToolbarItemOrderGroup::TopTools)
        .to_vec();
    let mut positions = ToolbarPositionSnapshot {
        top: (0.0, 0.0),
        side: (0.0, 0.0),
    };
    let mut runtime = controller_only_runtime(&config, &runtime_path);
    assert!(runtime.begin_item_drag(ToolbarItemOrderGroup::TopTools, &input));
    assert!(input.start_toolbar_item_drag(ToolbarItemOrderGroup::TopTools, ids::TOP_TOOL_PEN,));
    assert!(input.drag_toolbar_item_over(ToolbarItemOrderGroup::TopTools, 5));
    let reset_through = match runtime.controller.request_supported_reset() {
        RequestResetResult::Started { through, .. } => through,
        result => panic!("reset did not start: {result:?}"),
    };
    let reset = runtime
        .controller
        .take_source_mutation()
        .expect("reset command");
    assert!(matches!(
        runtime.finish_item_drag(true, &input),
        ToolbarRuntimeFinish::DeferredBehindBarrier
    ));
    input.clear_toolbar_item_drag();
    assert_eq!(
        runtime.controller.pipeline().latest_accepted(),
        reset_through
    );

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
    assert_eq!(
        input
            .resolved_toolbar_items
            .order
            .ordered_ids(ToolbarItemOrderGroup::TopTools),
        original
    );
    assert!(matches!(
        runtime.finish_item_drag(true, &input),
        ToolbarRuntimeFinish::KeepPreview
    ));
    let second_drain = runtime.drain_writer_completions();
    assert!(second_drain.rollbacks.is_empty());
    assert!(!second_drain.rebuild_live);
    assert_eq!(
        runtime.controller.pipeline().latest_accepted(),
        reset_through
    );
    assert!(!runtime_path.exists());
}

#[test]
fn external_source_conflict_rebuilds_live_toolbar_from_external_authority() {
    let temp = crate::test_temp::tempdir().unwrap();
    let runtime_path = temp.path().join("runtime-ui.toml");
    let config = Config::default();
    assert!(config.ui.toolbar.top_pinned);
    let mut input = input_from_config(&config);
    let mut positions = ToolbarPositionSnapshot {
        top: (0.0, 0.0),
        side: (0.0, 0.0),
    };
    let mut runtime = controller_only_runtime(&config, &runtime_path);
    let original_order = input
        .resolved_toolbar_items
        .order
        .ordered_ids(ToolbarItemOrderGroup::TopTools)
        .to_vec();
    assert!(runtime.begin_item_drag(ToolbarItemOrderGroup::TopTools, &input));
    assert!(input.start_toolbar_item_drag(ToolbarItemOrderGroup::TopTools, ids::TOP_TOOL_PEN,));
    assert!(input.drag_toolbar_item_over(ToolbarItemOrderGroup::TopTools, 5));
    assert_ne!(
        input
            .resolved_toolbar_items
            .order
            .ordered_ids(ToolbarItemOrderGroup::TopTools),
        original_order
    );
    let target = ToolbarRuntimeUiPersistenceTarget::TopPinned;
    let prepared = runtime
        .begin_toolbar_mutation(target, &input)
        .expect("top-pin permit");
    input.toolbar_top_pinned = false;
    let desired = toolbar_values(target, &input).unwrap();
    assert!(matches!(
        runtime.controller.finish_preview(
            prepared.session,
            RuntimePreviewFinishIntent::Commit(desired)
        ),
        PreviewFinishResult::AcceptedRuntime { .. }
    ));
    let request = runtime
        .controller
        .take_source_mutation()
        .expect("local replacement");

    fs::write(&runtime_path, b"version = 1\n").unwrap();
    let external = RuntimeUiStateStore::new(&runtime_path)
        .inspect()
        .unwrap()
        .observation;
    runtime.integrate_writer_completion(RuntimeStateWriterCompletion::SourceMutation(
        SourceMutationResult::SourceChangedBeforeMutation {
            id: request.id,
            active: external,
        },
    ));
    let drain = runtime.drain_writer_completions();
    assert!(drain.rebuild_live);
    assert!(drain.rollbacks.is_empty());
    runtime.apply_live_state(&mut input, &mut positions);
    assert!(input.toolbar_top_pinned);
    assert_eq!(
        input
            .resolved_toolbar_items
            .order
            .ordered_ids(ToolbarItemOrderGroup::TopTools),
        original_order
    );
    assert!(matches!(
        runtime.finish_item_drag(true, &input),
        ToolbarRuntimeFinish::KeepPreview
    ));
    assert_eq!(
        input
            .resolved_toolbar_items
            .order
            .ordered_ids(ToolbarItemOrderGroup::TopTools),
        original_order,
        "late release after authority replacement cannot restore the old preview"
    );
    assert_eq!(fs::read(&runtime_path).unwrap(), b"version = 1\n");
}

fn config_positions(config: &Config) -> ToolbarPositionSnapshot {
    ToolbarPositionSnapshot {
        top: (config.ui.toolbar.top_offset, config.ui.toolbar.top_offset_y),
        side: (
            config.ui.toolbar.side_offset_x,
            config.ui.toolbar.side_offset,
        ),
    }
}

fn stored_position(
    runtime: &ToolbarRuntimeState,
    target: InteractionSeedTarget,
) -> Option<(f64, f64)> {
    match runtime
        .controller
        .model()
        .get(&target)
        .map(|entry| &entry.value)
    {
        Some(InteractionSeedValue::Position(position)) => {
            Some((position.x.get(), position.y.get()))
        }
        _ => None,
    }
}

fn stored_bool(runtime: &ToolbarRuntimeState, target: InteractionSeedTarget) -> Option<bool> {
    match runtime
        .controller
        .model()
        .get(&target)
        .map(|entry| &entry.value)
    {
        Some(InteractionSeedValue::Bool(value)) => Some(*value),
        _ => None,
    }
}

fn stored_display_mode(runtime: &ToolbarRuntimeState) -> Option<PersistedTopDisplayMode> {
    match runtime
        .controller
        .model()
        .get(&InteractionSeedTarget::TopDisplayMode)
        .map(|entry| &entry.value)
    {
        Some(InteractionSeedValue::TopDisplayMode(mode)) => Some(*mode),
        _ => None,
    }
}

fn commit_display_mode(
    runtime: &mut ToolbarRuntimeState,
    input: &mut InputState,
    mode: crate::config::TopDisplayMode,
) -> ToolbarRuntimeFinish {
    let target = ToolbarRuntimeUiPersistenceTarget::TopDisplayMode;
    let prepared = runtime
        .begin_toolbar_mutation(target, input)
        .expect("display mode permit");
    input.set_top_display_mode(mode);
    runtime.finish_toolbar_mutation(prepared, true, input)
}

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
    assert_eq!(
        stored_position(&runtime, InteractionSeedTarget::SidePosition),
        None,
        "a top drag never claims the side position"
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
    assert_eq!(restored.side, config_positions(&config).side);
    assert_eq!(fs::read(&config_path).unwrap(), AUTHORED);
}

#[test]
fn committed_side_drag_stores_both_position_overrides_in_one_write() {
    let temp = crate::test_temp::tempdir().unwrap();
    let runtime_path = temp.path().join("runtime-ui.toml");
    let config = Config::default();
    let mut runtime = test_runtime(&config, &runtime_path);
    let mut positions = config_positions(&config);
    let accepted_before = runtime.controller.pipeline().latest_accepted();

    assert!(runtime.begin_position_drag(MoveDragKind::Side, positions));
    // A side drag reconciles the top strip's X base before it commits.
    positions.top = (16.0, positions.top.1);
    positions.side = (-30.0, 12.0);
    assert!(matches!(
        runtime.finish_position_drag(true, positions),
        ToolbarRuntimeFinish::KeepPreview
    ));
    assert_eq!(
        runtime.controller.pipeline().latest_accepted().get(),
        accepted_before.get() + 1,
        "both overrides settle through one accepted revision"
    );
    assert!(settle_runtime(&mut runtime).rollbacks.is_empty());

    assert_eq!(
        stored_position(&runtime, InteractionSeedTarget::TopPosition),
        Some((16.0, config.ui.toolbar.top_offset_y))
    );
    assert_eq!(
        stored_position(&runtime, InteractionSeedTarget::SidePosition),
        Some((-30.0, 12.0))
    );
    runtime.shutdown_blocking();
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
    let refresh = runtime.refresh_config_seeds(&config_b, &mut input, &mut positions);
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

    assert!(previews.begin_position_drag(MoveDragKind::Side, positions));
    positions.top.0 = 42.0;
    positions.side = (43.0, 44.0);
    let finish = previews.finish_position_drag(true);
    assert!(matches!(finish, ToolbarRuntimeFinish::KeepPreview));
    apply_finish(&mut input, &mut positions, finish);

    assert_eq!(positions.top.0, 42.0, "the committed drag stays on screen");
    assert_eq!(positions.side, (43.0, 44.0));
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
    assert_eq!(input.toolbar_top_display_mode, TopDisplayMode::Full);

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
    assert_eq!(input.toolbar_top_display_mode, TopDisplayMode::Hidden);
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
    restarted_input.init_toolbar_display_mode_from_config(config.ui.toolbar.top_display_mode);
    assert_eq!(
        restarted_input.toolbar_top_display_mode,
        TopDisplayMode::Full
    );
    let restarted = test_runtime(&config, &runtime_path);
    restarted.apply_startup_state(&mut restarted_input);
    assert_eq!(
        restarted_input.toolbar_top_display_mode,
        TopDisplayMode::Micro
    );
}

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

/// The pre-toggle pins as the visibility toggle's write path batches them.
fn pins_rollback_values(top: bool, side: bool) -> RuntimeUiMutationValues {
    RuntimeUiMutationValues::batch([
        (
            InteractionSeedTarget::TopPinned,
            InteractionSeedValue::Bool(top),
        ),
        (
            InteractionSeedTarget::SidePinned,
            InteractionSeedValue::Bool(side),
        ),
    ])
    .expect("distinct pin targets batch")
}

/// The pre-toggle pins as the visibility toggle's rollback snapshot carries
/// them (visibility itself is never persisted, so it is not in there).
fn pins_rollback(top: bool, side: bool) -> PreviewRollbackSnapshot {
    PreviewRollbackSnapshot {
        values: BTreeMap::from([
            (
                InteractionSeedTarget::TopPinned,
                InteractionSeedValue::Bool(top),
            ),
            (
                InteractionSeedTarget::SidePinned,
                InteractionSeedValue::Bool(side),
            ),
        ]),
    }
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
    assert!(config.ui.toolbar.top_pinned && config.ui.toolbar.side_pinned);
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
            previous_side_pinned: true,
        }],
        "the deferred entry must survive the settled recovery"
    );
    let prepared = runtime
        .begin_toolbar_mutation_with_rollback(
            ToolbarRuntimeUiPersistenceTarget::ToolbarVisibility,
            pins_rollback_values(true, true),
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
    restarted.apply_startup_state(&mut restarted_input);
    assert_eq!(
        stored_display_mode(&restarted),
        Some(PersistedTopDisplayMode::Micro),
        "the recovery must have landed the write that originally failed"
    );
    assert!(!restarted_input.toolbar_top_pinned);
    assert!(!restarted_input.toolbar_side_pinned);
    assert!(
        !restarted_input.toolbar_visible,
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
    use crate::domain::Action;

    let config = Config::default();
    let mut input = input_from_config(&config);
    let mut positions = config_positions(&config);

    input.handle_action(Action::ToggleToolbar); // hide, pins → false/false
    assert!(!input.toolbar_visible());
    input.take_pending_toolbar_persistence(); // the write whose rollback arrives below

    input.presenter_mode_config.hide_toolbars = true;
    input.toggle_presenter_mode();
    assert!(input.presenter_mode);

    apply_toolbar_runtime_rollback(&mut input, &mut positions, &pins_rollback(true, true));

    assert!(input.toolbar_top_pinned && input.toolbar_side_pinned);
    assert!(
        !input.toolbar_visible && !input.toolbar_top_visible && !input.toolbar_side_visible,
        "the live presenter-hidden flags must not move under the owner"
    );

    input.toggle_presenter_mode();
    assert!(!input.presenter_mode);
    assert!(
        input.toolbar_visible && input.toolbar_top_visible && input.toolbar_side_visible,
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

    input.handle_action(Action::ToggleToolbar); // hide, pins → false/false
    assert!(!input.toolbar_visible());
    input.take_pending_toolbar_persistence(); // the write whose rollback arrives below

    input.handle_action(Action::ToggleFocusMode);
    assert!(input.focus_mode_active());

    apply_toolbar_runtime_rollback(&mut input, &mut positions, &pins_rollback(true, true));

    assert!(input.toolbar_top_pinned && input.toolbar_side_pinned);
    assert!(
        !input.toolbar_visible && !input.toolbar_top_visible && !input.toolbar_side_visible,
        "the live focus-hidden flags must not move under the owner"
    );

    input.handle_action(Action::ToggleFocusMode); // restore
    assert!(!input.focus_mode_active());
    assert!(
        input.toolbar_visible && input.toolbar_top_visible && input.toolbar_side_visible,
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

    input.handle_action(Action::ToggleToolbar); // hide, pins → false/false
    assert!(!input.toolbar_visible());
    input.take_pending_toolbar_persistence(); // the write whose rollback arrives below

    input.handle_action(Action::ToggleLightMode);
    assert!(input.light_mode);

    apply_toolbar_runtime_rollback(&mut input, &mut positions, &pins_rollback(true, true));

    assert!(input.toolbar_top_pinned && input.toolbar_side_pinned);
    assert!(
        !input.toolbar_visible && !input.toolbar_top_visible && !input.toolbar_side_visible,
        "the live light-mode-hidden flags must not move under the owner"
    );

    input.handle_action(Action::ToggleLightMode); // exit restores the snapshot
    assert!(!input.light_mode);
    assert!(
        input.toolbar_visible && input.toolbar_top_visible && input.toolbar_side_visible,
        "light-mode exit must restore visibility agreeing with the rolled-back pins"
    );
}

#[test]
fn a_display_mode_change_during_presenter_mode_stores_the_pre_presenter_value() {
    use crate::config::{PresenterToolbarMode, TopDisplayMode};

    let temp = crate::test_temp::tempdir().unwrap();
    let runtime_path = temp.path().join("runtime-ui.toml");
    let config = Config::default();
    let mut input = input_from_config(&config);
    input.presenter_mode_config.hide_toolbars = true;
    input.presenter_mode_config.toolbar_mode = PresenterToolbarMode::Micro;
    input.toolbar_top_display_mode = TopDisplayMode::Full;
    input.toggle_presenter_mode();
    assert_eq!(input.toolbar_top_display_mode, TopDisplayMode::Micro);

    // The live strip is presenter's; the persisted value stays the saved
    // pre-presenter mode, so committing it is a no-op against the seed.
    let values = top_display_mode_values(input.toolbar_top_display_mode, &input).unwrap();
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
    input.toggle_presenter_mode();
    assert!(input.presenter_restore.is_none());
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
    let refresh = runtime.refresh_config_seeds(&config_b, &mut input, &mut positions);
    assert!(refresh.applied);
    assert!(settle_runtime(&mut runtime).rollbacks.is_empty());
    assert_eq!(
        stored_display_mode(&runtime),
        None,
        "the authored default caught up with the runtime choice"
    );
    assert_eq!(input.toolbar_top_display_mode, TopDisplayMode::Micro);
    runtime.shutdown_blocking();
}

/// Status-bar content is chrome the user arranges from the overlay's settings
/// popover. It used to apply to the current run only, so every toggle came
/// back on the next launch; it now survives a restart the way the toolbars do,
/// as a runtime override layered over the configured value, with `config.toml`
/// still untouched.
#[test]
fn status_bar_content_survives_restart_without_touching_config() {
    use crate::config::StatusBarItem;

    const AUTHORED: &[u8] = b"# authored config bytes stay exact\n";
    let temp = crate::test_temp::tempdir().unwrap();
    let config_path = temp.path().join("config.toml");
    let runtime_path = temp.path().join("data/runtime-ui.toml");
    fs::write(&config_path, AUTHORED).unwrap();
    let config = Config::default();
    let mut input = input_from_config(&config);
    let mut runtime = test_runtime(&config, &runtime_path);

    assert!(input.status_bar_interactive, "shipped default");
    assert!(input.status_bar_item_visible(StatusBarItem::Size));

    let target = ToolbarRuntimeUiPersistenceTarget::StatusBarInteractive;
    let prepared = runtime
        .begin_toolbar_mutation(target, &input)
        .expect("interactivity permit");
    input.status_bar_interactive = false;
    let finish = runtime.finish_toolbar_mutation(prepared, true, &input);
    assert!(matches!(finish, ToolbarRuntimeFinish::KeepPreview));

    let target = ToolbarRuntimeUiPersistenceTarget::StatusBarItem(StatusBarItem::Size);
    let prepared = runtime
        .begin_toolbar_mutation(target, &input)
        .expect("item permit");
    input.set_status_bar_item_visible(StatusBarItem::Size, false);
    let finish = runtime.finish_toolbar_mutation(prepared, true, &input);
    assert!(matches!(finish, ToolbarRuntimeFinish::KeepPreview));

    assert!(settle_runtime(&mut runtime).rollbacks.is_empty());
    assert_eq!(fs::read(&config_path).unwrap(), AUTHORED);
    runtime.shutdown_blocking();

    let mut restarted_input = input_from_config(&config);
    let mut restarted = test_runtime(&config, &runtime_path);
    restarted.apply_startup_state(&mut restarted_input);

    assert!(
        !restarted_input.status_bar_interactive,
        "clickable-segments stays off across a restart"
    );
    assert!(
        !restarted_input.status_bar_item_visible(StatusBarItem::Size),
        "a hidden segment stays hidden across a restart"
    );
    assert!(
        restarted_input.status_bar_item_visible(StatusBarItem::Tool),
        "segments the user did not touch keep their configured value"
    );
    assert_eq!(fs::read(&config_path).unwrap(), AUTHORED);
    restarted.shutdown_blocking();
}

/// The toolbar and status-bar preference toggles the overlay exposes are
/// chrome the user arranges, and they used to reset on every launch. Each now
/// survives a restart as a runtime override, with `config.toml` untouched.
#[test]
fn toolbar_preference_toggles_survive_restart_without_touching_config() {
    const AUTHORED: &[u8] = b"# authored config bytes stay exact\n";
    let temp = crate::test_temp::tempdir().unwrap();
    let config_path = temp.path().join("config.toml");
    let runtime_path = temp.path().join("data/runtime-ui.toml");
    fs::write(&config_path, AUTHORED).unwrap();
    let config = Config::default();
    let mut input = input_from_config(&config);
    let mut runtime = test_runtime(&config, &runtime_path);

    // Flip each away from its configured value.
    type Flip = (ToolbarRuntimeUiPersistenceTarget, fn(&mut InputState));
    let flips: Vec<Flip> = vec![
        (ToolbarRuntimeUiPersistenceTarget::StatusBar, |input| {
            input.show_status_bar = !input.show_status_bar;
        }),
        (ToolbarRuntimeUiPersistenceTarget::ToolbarIcons, |input| {
            input.toolbar_use_icons = !input.toolbar_use_icons;
        }),
        (
            ToolbarRuntimeUiPersistenceTarget::ToolbarContextAwareUi,
            |input| input.context_aware_ui = !input.context_aware_ui,
        ),
        (
            ToolbarRuntimeUiPersistenceTarget::ToolbarDelaySliders,
            |input| input.show_delay_sliders = !input.show_delay_sliders,
        ),
        (
            ToolbarRuntimeUiPersistenceTarget::HistoryCustomSection,
            |input| input.custom_section_enabled = !input.custom_section_enabled,
        ),
        (
            ToolbarRuntimeUiPersistenceTarget::FloatingBadgeAlways,
            |input| input.show_floating_badge_always = !input.show_floating_badge_always,
        ),
    ];

    for (target, flip) in &flips {
        let prepared = runtime
            .begin_toolbar_mutation(*target, &input)
            .unwrap_or_else(|| panic!("{target:?} permit"));
        flip(&mut input);
        let finish = runtime.finish_toolbar_mutation(prepared, true, &input);
        assert!(
            matches!(finish, ToolbarRuntimeFinish::KeepPreview),
            "{target:?}"
        );
    }
    assert!(settle_runtime(&mut runtime).rollbacks.is_empty());
    assert_eq!(fs::read(&config_path).unwrap(), AUTHORED);

    let expected_status_bar = input.show_status_bar;
    let expected_icons = input.toolbar_use_icons;
    let expected_context = input.context_aware_ui;
    let expected_sliders = input.show_delay_sliders;
    let expected_custom = input.custom_section_enabled;
    let expected_badge = input.show_floating_badge_always;
    runtime.shutdown_blocking();

    let mut restarted_input = input_from_config(&config);
    let mut restarted = test_runtime(&config, &runtime_path);
    restarted.apply_startup_state(&mut restarted_input);

    assert_eq!(restarted_input.show_status_bar, expected_status_bar);
    assert_eq!(restarted_input.toolbar_use_icons, expected_icons);
    assert_eq!(restarted_input.context_aware_ui, expected_context);
    assert_eq!(restarted_input.show_delay_sliders, expected_sliders);
    assert_eq!(restarted_input.custom_section_enabled, expected_custom);
    assert_eq!(restarted_input.show_floating_badge_always, expected_badge);
    assert_eq!(fs::read(&config_path).unwrap(), AUTHORED);
    restarted.shutdown_blocking();
}
