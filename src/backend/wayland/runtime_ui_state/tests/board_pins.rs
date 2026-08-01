use super::*;

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
