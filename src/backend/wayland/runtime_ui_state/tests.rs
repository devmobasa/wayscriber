use super::*;

use std::fs;
use std::path::Path;
use std::thread;
use std::time::Duration;

use crate::config::{ToolbarItemsConfig, toolbar_item_ids as ids};
use crate::input::state::test_support::make_test_input_state;

fn input_from_config(config: &Config) -> InputState {
    let mut input = make_test_input_state();
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
    fs::create_dir_all(path.parent().expect("runtime parent")).unwrap();
    let store = RuntimeUiStateStore::new(path);
    let bootstrap = store
        .inspect()
        .unwrap()
        .into_controller_bootstrap(toolbar_seeds_from_config(config).unwrap());
    assert!(bootstrap.startup_incident.is_none());
    let mut runtime = ToolbarRuntimeState {
        controller: bootstrap.controller,
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
    let bootstrap = RuntimeUiStateStore::new(path)
        .inspect()
        .unwrap()
        .into_controller_bootstrap(toolbar_seeds_from_config(config).unwrap());
    ToolbarRuntimeState {
        controller: bootstrap.controller,
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

fn apply_finish(
    input: &mut InputState,
    positions: &mut ToolbarPositionSnapshot,
    finish: ToolbarRuntimeFinish,
) {
    if let ToolbarRuntimeFinish::Rollback(rollback) = finish {
        apply_toolbar_runtime_rollback(input, positions, &rollback);
    }
}

#[test]
fn toolbar_seed_registry_covers_every_runtime_routed_target() {
    let config = Config::default();
    let seeds = toolbar_seeds_from_config(&config).expect("valid default seeds");

    for target in [
        InteractionSeedTarget::TopPinned,
        InteractionSeedTarget::SidePinned,
        InteractionSeedTarget::TopMinimized,
        InteractionSeedTarget::SideMinimized,
        InteractionSeedTarget::SidePane,
        InteractionSeedTarget::TopPosition,
        InteractionSeedTarget::SidePosition,
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

    let seeds = toolbar_seeds_from_config(&config).expect("valid folded seeds");
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
    ];
    for target in mutations {
        let prepared = runtime
            .begin_toolbar_mutation(target, &input)
            .expect("runtime mutation permit");
        match target {
            ToolbarRuntimeUiPersistenceTarget::TopPinned => input.toolbar_top_pinned = false,
            ToolbarRuntimeUiPersistenceTarget::SidePinned => input.toolbar_side_pinned = false,
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
    assert!(runtime.begin_position_drag(ConfigPositionTarget::Top, positions));

    assert!(matches!(
        runtime.controller.request_supported_reset(),
        RequestResetResult::Started { .. }
    ));
    assert!(!runtime.item_drag_update_allowed());
    assert!(!runtime.position_drag_update_allowed(ConfigPositionTarget::Top));
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

    assert!(runtime.begin_position_drag(ConfigPositionTarget::Top, positions));
    positions.top = (42.0, 43.0);
    let mut config_c = config_b;
    config_c.ui.toolbar.top_offset = 100.0;
    config_c.ui.toolbar.top_offset_y = 101.0;
    let refresh = runtime.refresh_config_seeds(&config_c, &mut input, &mut positions);
    assert!(refresh.applied);
    assert!(!refresh.item_drag_aborted);
    assert!(refresh.position_drag_aborted);
    assert_eq!(positions.top, (100.0, 101.0));
    let (finish, wrote_config) = runtime.finish_position_drag(true, positions, |_, _| {
        panic!("late position release must not write config")
    });
    assert!(matches!(finish, ToolbarRuntimeFinish::KeepPreview));
    assert!(!wrote_config);
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

    assert!(runtime.begin_position_drag(ConfigPositionTarget::Side, positions));
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

    assert!(runtime.begin_position_drag(ConfigPositionTarget::Top, positions));
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

    let (finish, wrote_config) = runtime.finish_position_drag(false, positions, |_, _| {
        panic!("cancelling a position preview must not write config")
    });
    assert!(!wrote_config);
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
            PreviewFinishRequest::RuntimeUi {
                session: prepared.session,
                intent: RuntimePreviewFinishIntent::Commit(desired),
            },
            |_, _| unreachable!(),
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
