use super::*;

#[test]
fn toolbar_seed_registry_covers_every_runtime_routed_target() {
    let config = Config::default();
    let board_pin_seeds = board_pin_seeds_from_input(&input_from_config(&config));
    let seeds = runtime_seeds_from_config(&config, &board_pin_seeds).expect("valid default seeds");

    for target in [
        InteractionSeedTarget::TopPinned,
        InteractionSeedTarget::TopMinimized,
        InteractionSeedTarget::TopPosition,
        InteractionSeedTarget::TopDisplayMode,
        InteractionSeedTarget::ToolbarLayoutMode,
        InteractionSeedTarget::ClickHighlight,
        InteractionSeedTarget::ClickHighlightToolRing,
        InteractionSeedTarget::FloatingBadge,
        InteractionSeedTarget::ZoomChip,
    ] {
        assert!(seeds.get(&target).is_some(), "missing seed for {target:?}");
    }
    for flag in crate::config::ToolbarSectionFlag::ALL {
        assert!(
            seeds
                .get(&InteractionSeedTarget::SectionVisibility(flag))
                .is_some(),
            "missing section seed for {flag:?}"
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
            "a section persists under its own target, not as an item override"
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
fn runtime_rebuild_reuses_minimize_transition_cleanup() {
    let temp = crate::test_temp::tempdir().unwrap();
    let runtime_path = temp.path().join("runtime-ui.toml");
    let config = Config::default();
    let mut source = input_from_config(&config);
    let mut runtime = test_runtime(&config, &runtime_path);

    let minimized = runtime
        .begin_toolbar_mutation(ToolbarRuntimeUiPersistenceTarget::TopMinimized, &source)
        .expect("top-minimized permit");
    source.test_set_toolbar_display_state(source.toolbar_top_display_mode(), true);
    assert!(matches!(
        runtime.finish_toolbar_mutation(minimized, true, &source),
        ToolbarRuntimeFinish::KeepPreview
    ));
    assert!(settle_runtime(&mut runtime).rollbacks.is_empty());

    for menu in [
        crate::input::state::TopMenuState::ShapePicker,
        crate::input::state::TopMenuState::TopOverflow,
        crate::input::state::TopMenuState::CanvasPopover,
        crate::input::state::TopMenuState::SessionPopover,
        crate::input::state::TopMenuState::SettingsPopover,
    ] {
        let mut rebuilt = input_from_config(&config);
        rebuilt.test_set_toolbar_menu_state(menu, rebuilt.toolbar_top_popover_scroll());
        rebuilt.test_set_toolbar_customization(
            true,
            Some(crate::ui::toolbar::ToolbarItemCustomizeGroup::TopTools),
            false,
        );
        let mut positions = ToolbarPositionSnapshot { top: (0.0, 0.0) };

        runtime.apply_live_state(
            &crate::ui_text::UiTextEngine::default(),
            &mut rebuilt,
            &mut positions,
        );

        assert!(rebuilt.toolbar_top_minimized());
        assert_eq!(
            rebuilt.toolbar_top_menu(),
            crate::input::state::TopMenuState::Closed
        );
        assert!(!rebuilt.toolbar_customize_items_open());
        assert!(rebuilt.toolbar_customize_items_group().is_none());
    }
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
    input.set_toolbar_top_pinned(false);
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

    let mut positions = ToolbarPositionSnapshot { top: (0.0, 0.0) };
    runtime.apply_live_state(
        &crate::ui_text::UiTextEngine::default(),
        &mut input,
        &mut positions,
    );
    assert_eq!(input.toolbar_top_pinned(), config.ui.toolbar.top_pinned);
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
    input.set_toolbar_top_pinned(false);
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
    };
    let mut runtime = test_runtime_allow_startup_incident(&config_a, &runtime_path);

    assert!(
        runtime.handle_persistence_lifecycle_event(
            &ToolbarEvent::RequestPreserveInvalidRuntimeUiReset
        )
    );
    let mut config_b = config_a;
    config_b.ui.toolbar.top_pinned = false;
    let refresh = runtime.refresh_config_seeds(
        &crate::ui_text::UiTextEngine::default(),
        &config_b,
        &mut input,
        &mut positions,
    );
    assert!(!refresh.applied, "the reload is staged behind recovery");
    assert!(
        input.toolbar_top_pinned(),
        "live input still has the old seed"
    );

    assert!(runtime.handle_persistence_lifecycle_event(&ToolbarEvent::CancelRuntimeUiRecovery));
    let drain = runtime.drain_writer_completions();
    assert!(
        drain.rebuild_live,
        "synchronous cancellation must publish the staged live authority"
    );
    runtime.apply_live_state(
        &crate::ui_text::UiTextEngine::default(),
        &mut input,
        &mut positions,
    );
    assert!(!input.toolbar_top_pinned());
    runtime.shutdown_blocking();
}

#[test]
fn runtime_toolbar_routes_leave_authored_config_bytes_exactly_unchanged() {
    const AUTHORED: &[u8] =
        b"# keep this formatting and comment\n[ui.toolbar]\ntop_pinned = true\n";
    let temp = crate::test_temp::tempdir().unwrap();
    let config_path = temp.path().join("config.toml");
    let runtime_path = temp.path().join("data/runtime-ui.toml");
    fs::write(&config_path, AUTHORED).unwrap();
    let config: Config = toml::from_str(std::str::from_utf8(AUTHORED).unwrap()).unwrap();
    let mut input = input_from_config(&config);
    let mut runtime = test_runtime(&config, &runtime_path);

    let mutations = [
        ToolbarRuntimeUiPersistenceTarget::TopPinned,
        ToolbarRuntimeUiPersistenceTarget::TopMinimized,
        ToolbarRuntimeUiPersistenceTarget::TopDisplayMode,
        ToolbarRuntimeUiPersistenceTarget::ToolbarVisibility,
    ];
    for target in mutations {
        let prepared = runtime
            .begin_toolbar_mutation(target, &input)
            .expect("runtime mutation permit");
        match target {
            ToolbarRuntimeUiPersistenceTarget::TopPinned
            | ToolbarRuntimeUiPersistenceTarget::ToolbarVisibility => {
                input.set_toolbar_top_pinned(false);
            }
            ToolbarRuntimeUiPersistenceTarget::TopMinimized => {
                input.test_set_toolbar_display_state(input.toolbar_top_display_mode(), true);
            }
            ToolbarRuntimeUiPersistenceTarget::TopDisplayMode => {
                input.set_top_display_mode_with_engine(
                    &crate::ui_text::UiTextEngine::default(),
                    crate::config::TopDisplayMode::Micro,
                );
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
