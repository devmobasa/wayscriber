use super::*;

#[test]
fn item_drag_commit_accepts_one_revision_and_cancel_accepts_none() {
    let temp = crate::test_temp::tempdir().unwrap();
    let runtime_path = temp.path().join("runtime-ui.toml");
    let config = Config::default();
    let mut input = input_from_config(&config);
    let mut positions = ToolbarPositionSnapshot { top: (0.0, 0.0) };
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
        .resolved_toolbar_items()
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
            .resolved_toolbar_items()
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
        .resolved_toolbar_items()
        .order
        .ordered_ids(ToolbarItemOrderGroup::TopTools)
        .to_vec();
    let mut positions = ToolbarPositionSnapshot { top: (0.0, 0.0) };

    assert!(previews.begin_item_drag(ToolbarItemOrderGroup::TopTools, &input));
    assert!(input.start_toolbar_item_drag(ToolbarItemOrderGroup::TopTools, ids::TOP_TOOL_PEN,));
    assert!(input.drag_toolbar_item_over(ToolbarItemOrderGroup::TopTools, 5));
    assert_ne!(
        input
            .resolved_toolbar_items()
            .order
            .ordered_ids(ToolbarItemOrderGroup::TopTools),
        original
    );

    let finish = previews.finish_item_drag(false);
    input.clear_toolbar_item_drag();
    apply_finish(&mut input, &mut positions, finish);

    assert_eq!(
        input
            .resolved_toolbar_items()
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
    };
    let mut positions = original;

    assert!(previews.begin_position_drag(MoveDragKind::Top, positions));
    positions.top = (42.0, 43.0);

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
        .resolved_toolbar_items()
        .order
        .ordered_ids(ToolbarItemOrderGroup::TopTools)
        .to_vec();
    let original_positions = ToolbarPositionSnapshot {
        top: (config.ui.toolbar.top_offset, config.ui.toolbar.top_offset_y),
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
            .resolved_toolbar_items()
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
            .resolved_toolbar_items()
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
fn unrelated_position_reload_preserves_preview_and_cancel_only_restores_its_scope() {
    let temp = crate::test_temp::tempdir().unwrap();
    let runtime_path = temp.path().join("runtime-ui.toml");
    let config_a = Config::default();
    let original_top = (
        config_a.ui.toolbar.top_offset,
        config_a.ui.toolbar.top_offset_y,
    );
    let mut positions = ToolbarPositionSnapshot { top: original_top };
    let mut input = input_from_config(&config_a);
    let mut runtime = test_runtime(&config_a, &runtime_path);

    assert!(runtime.begin_position_drag(MoveDragKind::Top, positions));
    positions.top = (42.0, 43.0);
    let mut config_b = config_a;
    config_b.ui.toolbar.top_minimized = !config_b.ui.toolbar.top_minimized;

    let refresh = runtime.refresh_config_seeds(&config_b, &mut input, &mut positions);
    assert!(refresh.applied);
    assert!(!refresh.position_drag_aborted);
    assert_eq!(
        positions.top,
        (42.0, 43.0),
        "unrelated reload keeps preview"
    );

    let finish = runtime.finish_position_drag(false, positions);
    apply_finish(&mut input, &mut positions, finish);
    assert_eq!(positions.top, original_top);
    runtime.shutdown_blocking();
}

#[test]
fn release_during_barrier_is_consumed_once_and_never_replayed() {
    let temp = crate::test_temp::tempdir().unwrap();
    let runtime_path = temp.path().join("runtime-ui.toml");
    let config = Config::default();
    let mut input = input_from_config(&config);
    let original = input
        .resolved_toolbar_items()
        .order
        .ordered_ids(ToolbarItemOrderGroup::TopTools)
        .to_vec();
    let mut positions = ToolbarPositionSnapshot { top: (0.0, 0.0) };
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
            .resolved_toolbar_items()
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
    let mut positions = ToolbarPositionSnapshot { top: (0.0, 0.0) };
    let mut runtime = controller_only_runtime(&config, &runtime_path);
    let original_order = input
        .resolved_toolbar_items()
        .order
        .ordered_ids(ToolbarItemOrderGroup::TopTools)
        .to_vec();
    assert!(runtime.begin_item_drag(ToolbarItemOrderGroup::TopTools, &input));
    assert!(input.start_toolbar_item_drag(ToolbarItemOrderGroup::TopTools, ids::TOP_TOOL_PEN,));
    assert!(input.drag_toolbar_item_over(ToolbarItemOrderGroup::TopTools, 5));
    assert_ne!(
        input
            .resolved_toolbar_items()
            .order
            .ordered_ids(ToolbarItemOrderGroup::TopTools),
        original_order
    );
    let target = ToolbarRuntimeUiPersistenceTarget::TopPinned;
    let prepared = runtime
        .begin_toolbar_mutation(target, &input)
        .expect("top-pin permit");
    input.set_toolbar_top_pinned(false);
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
    assert!(input.toolbar_top_pinned());
    assert_eq!(
        input
            .resolved_toolbar_items()
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
            .resolved_toolbar_items()
            .order
            .ordered_ids(ToolbarItemOrderGroup::TopTools),
        original_order,
        "late release after authority replacement cannot restore the old preview"
    );
    assert_eq!(fs::read(&runtime_path).unwrap(), b"version = 1\n");
}
