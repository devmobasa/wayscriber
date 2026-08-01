use super::*;

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
