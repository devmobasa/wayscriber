use super::*;

fn add_line(input: &mut InputState, x2: i32) -> crate::draw::ShapeId {
    let id = super::add_line(input, x2);
    input.mark_session_dirty();
    id
}

fn output_command(
    session: &mut SessionState,
    target: &stored_session::SessionOptions,
) -> SessionCommand {
    session.stage_output_transition(target.clone(), Some("next-output".into()), Instant::now());
    SessionCommand::Output {
        transition: Box::new(session.pending_output_transition().unwrap().clone()),
        remembered: None,
        home: None,
    }
}

#[test]
fn output_switch_returns_with_save_and_load_held_then_commits_in_order() {
    let temp = crate::test_temp::tempdir().unwrap();
    let current = named_options(temp.path(), "current");
    let target = named_options(temp.path(), "destination");
    stored_session::save_snapshot(&sample_snapshot(), &target).unwrap();
    let mut input = test_input_state();
    add_line(&mut input, 51);
    let measurer = TextMeasurer::default();
    let mut session = SessionState::new(Some(current.clone()));
    session.commit_output_options(session.options().unwrap().clone(), true);
    let (persistence, worker) = PersistenceController::controlled_for_test();
    let mut runtime = CommandRuntime::new(&mut input, &measurer, &mut session, persistence);
    let command = output_command(runtime.session, &target);

    start_session_command(&mut runtime, command).unwrap();
    assert!(runtime.pending.is_some());
    assert!(!current.session_file_path().exists());
    assert_eq!(runtime.session.options().unwrap().target, current.target);
    worker.complete_next(); // source saved without replacing the canvas
    runtime.receive();
    assert_eq!(loaded_line_x2(&current), 51);
    assert_eq!(runtime.session.options().unwrap().target, current.target);
    assert!(runtime.reports.is_empty());
    worker.complete_next();
    runtime.receive();

    assert!(runtime.errors.is_empty(), "{:?}", runtime.errors);
    assert!(matches!(
        runtime.reports.as_slice(),
        [SessionCommandReport::Output { .. }]
    ));
    assert_eq!(runtime.session.options().unwrap().target, target.target);
    assert!(runtime.session.pending_output_transition().is_none());
    assert!(matches!(
        runtime.input.boards.active_frame().shapes[0].shape,
        crate::draw::Shape::Line { x2: 42, .. }
    ));
}

#[test]
fn output_load_cannot_replace_a_newer_destination_or_live_edits() {
    for change in ["destination", "return to source", "edit", "interaction"] {
        let temp = crate::test_temp::tempdir().unwrap();
        let current = named_options(temp.path(), "current");
        let target = named_options(temp.path(), "destination");
        stored_session::save_snapshot(&sample_snapshot(), &target).unwrap();
        let mut input = test_input_state();
        add_line(&mut input, 51);
        let measurer = TextMeasurer::default();
        let mut session = SessionState::new(Some(current.clone()));
        session.commit_output_options(session.options().unwrap().clone(), true);
        let (persistence, worker) = PersistenceController::controlled_for_test();
        let mut runtime = CommandRuntime::new(&mut input, &measurer, &mut session, persistence);
        let command = output_command(runtime.session, &target);
        start_session_command(&mut runtime, command).unwrap();
        worker.complete_next();
        runtime.receive();

        match change {
            "destination" => {
                let newer = named_options(temp.path(), "newer");
                runtime.session.stage_output_transition(
                    newer,
                    Some("third-output".into()),
                    Instant::now(),
                );
            }
            "return to source" => {
                runtime.session.cancel_pending_output_transition();
            }
            "edit" => {
                add_line(runtime.input, 77);
                runtime.input.mark_session_dirty();
            }
            "interaction" => {
                runtime.input.on_mouse_press_with_canvas_and_resources(
                    crate::input::state::InputTextResources {
                        measurer: &measurer,
                        ui_engine: &crate::ui_text::UiTextEngine::default(),
                    },
                    crate::input::MouseButton::Left,
                    500,
                    400,
                    500,
                    400,
                );
            }
            _ => unreachable!(),
        }
        worker.complete_next();
        runtime.receive();
        assert_eq!(runtime.session.options().unwrap().target, current.target);
        assert!(runtime.reports.is_empty());
        assert_eq!(runtime.errors.len(), 1, "{change}");
        if change != "return to source" {
            let remaining = runtime
                .session
                .output_transition_timeout(Instant::now())
                .unwrap();
            assert!(
                remaining <= Duration::from_millis(500),
                "{change}: {remaining:?}"
            );
        }
        assert!(matches!(
            runtime.input.boards.active_frame().shapes[0].shape,
            crate::draw::Shape::Line { x2: 51, .. }
        ));
        if change == "edit" {
            assert!(runtime.session.is_dirty());
        }
        if change == "interaction" {
            runtime.input.cancel_active_interaction_with(&measurer);
        }
        if let Some(transition) = runtime.session.pending_output_transition().cloned() {
            let expected = transition.staged_options.target.clone();
            start_session_command(
                &mut runtime,
                SessionCommand::Output {
                    transition: Box::new(transition),
                    remembered: None,
                    home: None,
                },
            )
            .unwrap();
            for _ in 0..10 {
                if runtime.pending.is_none() {
                    break;
                }
                worker.complete_next();
                runtime.receive();
            }
            assert!(runtime.pending.is_none(), "retry: {change}");
            assert_eq!(runtime.errors.len(), 1, "retry: {change}");
            assert_eq!(runtime.reports.len(), 1, "retry: {change}");
            assert_eq!(runtime.session.options().unwrap().target, expected);
        }
    }
}

#[test]
fn output_switch_does_not_recreate_missing_remembered_primary_or_restore_its_backup() {
    for home_enabled in [false, true] {
        let temp = crate::test_temp::tempdir().unwrap();
        let remembered = named_options(temp.path(), "remembered");
        let home = configured_home(temp.path());
        stored_session::save_snapshot(&sample_snapshot(), &remembered).unwrap();
        std::fs::rename(
            remembered.session_file_path(),
            remembered.backup_file_path(),
        )
        .unwrap();
        if home_enabled {
            stored_session::save_snapshot(&sample_snapshot(), &home).unwrap();
        }
        let mut input = test_input_state();
        add_line(&mut input, 51);
        input.clear_session_dirty(); // Existing fixture ink; no edit happened after the missing-file load.
        let measurer = TextMeasurer::default();
        let mut session = SessionState::new(Some(remembered.clone()));
        let (persistence, worker) = PersistenceController::controlled_for_test();
        let mut runtime = CommandRuntime::new(&mut input, &measurer, &mut session, persistence);
        let command = output_command(runtime.session, &remembered);
        let SessionCommand::Output { transition, .. } = command else {
            unreachable!()
        };
        start_session_command(
            &mut runtime,
            SessionCommand::Output {
                transition,
                remembered: Some(remembered.session_file_path()),
                home: home_enabled.then(|| Box::new(home.clone())),
            },
        )
        .unwrap();
        worker.complete_next(); // check primary before a save could recreate it
        runtime.receive();
        worker.complete_next(); // remembered load refuses backup fallback
        runtime.receive();
        if home_enabled {
            worker.complete_next();
            runtime.receive();
        }

        assert!(!remembered.session_file_path().exists());
        assert!(remembered.backup_file_path().exists());
        assert!(runtime.errors.is_empty(), "{:?}", runtime.errors);
        assert!(matches!(
            runtime.reports.as_slice(),
            [SessionCommandReport::Output {
                abandoned: Some(_),
                ..
            }]
        ));
        if home_enabled {
            assert_eq!(runtime.session.options().unwrap().target, home.target);
        } else {
            assert!(runtime.session.options().is_none());
            assert!(matches!(
                runtime.input.boards.active_frame().shapes[0].shape,
                crate::draw::Shape::Line { x2: 51, .. }
            ));
        }
    }
}

#[test]
fn clean_output_loads_the_destination_without_rotating_existing_source_bytes() {
    for protected in [false, true] {
        let temp = crate::test_temp::tempdir().unwrap();
        let current = named_options(temp.path(), "current");
        let target = named_options(temp.path(), "destination");
        stored_session::save_snapshot(&sample_snapshot(), &current).unwrap();
        stored_session::save_snapshot(&sample_snapshot(), &target).unwrap();
        let before = std::fs::read(current.session_file_path()).unwrap();
        let mut input = test_input_state();
        let measurer = TextMeasurer::default();
        let mut session = SessionState::new(Some(current.clone()));
        session.commit_output_options(session.options().unwrap().clone(), false);
        if protected {
            session.protect_session_path(current.session_file_path());
        }
        let (persistence, worker) = PersistenceController::controlled_for_test();
        let mut runtime = CommandRuntime::new(&mut input, &measurer, &mut session, persistence);
        let command = output_command(runtime.session, &target);
        start_session_command(&mut runtime, command).unwrap();
        assert!(runtime.pending.is_some());
        worker.complete_next();
        runtime.receive();
        assert!(runtime.errors.is_empty(), "{:?}", runtime.errors);
        assert_eq!(runtime.session.options().unwrap().target, target.target);
        assert_eq!(std::fs::read(current.session_file_path()).unwrap(), before);
        assert!(!current.backup_file_path().exists());
    }
}

#[test]
fn failed_output_save_keeps_the_source_canvas_and_pending_destination() {
    let temp = crate::test_temp::tempdir().unwrap();
    let current = named_options(temp.path(), "current");
    let target = named_options(temp.path(), "destination");
    stored_session::save_snapshot(&sample_snapshot(), &target).unwrap();
    // A forbidden primary is an actual save error, independent of worker control.
    std::fs::create_dir(current.session_file_path()).unwrap();
    let mut input = test_input_state();
    add_line(&mut input, 51);
    let measurer = TextMeasurer::default();
    let mut session = SessionState::new(Some(current.clone()));
    session.commit_output_options(session.options().unwrap().clone(), true);
    let (persistence, worker) = PersistenceController::controlled_for_test();
    let mut runtime = CommandRuntime::new(&mut input, &measurer, &mut session, persistence);
    let command = output_command(runtime.session, &target);
    start_session_command(&mut runtime, command).unwrap();
    worker.complete_next();
    runtime.receive();
    assert_eq!(runtime.errors.len(), 1);
    assert!(runtime.pending.is_none());
    assert!(runtime.reports.is_empty());
    assert!(runtime.session.pending_output_transition().is_some());
    let remaining = runtime
        .session
        .output_transition_timeout(Instant::now())
        .unwrap();
    assert!(
        remaining > Duration::from_secs(4),
        "I/O failure: {remaining:?}"
    );
    assert_eq!(runtime.session.options().unwrap().target, current.target);
    assert!(matches!(
        runtime.input.boards.active_frame().shapes[0].shape,
        crate::draw::Shape::Line { x2: 51, .. }
    ));
}

#[test]
fn queued_target_commands_refuse_a_changed_session_instead_of_using_its_canvas() {
    for action in ["open", "save as", "home"] {
        let temp = crate::test_temp::tempdir().unwrap();
        let current = named_options(temp.path(), "current");
        let target = named_options(temp.path(), "destination");
        let explicit = named_options(temp.path(), "explicit");
        stored_session::save_snapshot(&sample_snapshot(), &target).unwrap();
        if action == "open" || action == "home" {
            stored_session::save_snapshot(&sample_snapshot(), &explicit).unwrap();
        }
        let mut input = test_input_state();
        add_line(&mut input, 51);
        let measurer = TextMeasurer::default();
        let mut session = SessionState::new(Some(current.clone()));
        session.commit_output_options(session.options().unwrap().clone(), true);
        let (persistence, worker) = PersistenceController::controlled_for_test();
        let mut runtime = CommandRuntime::new(&mut input, &measurer, &mut session, persistence);
        let output = output_command(runtime.session, &target);
        start_session_command(&mut runtime, output).unwrap();
        let command = match action {
            "open" => SessionCommand::Open(explicit.session_file_path()),
            "save as" => SessionCommand::SaveAs(
                explicit.session_file_path(),
                stored_session::SaveAsOverwrite::Deny,
            ),
            "home" => SessionCommand::OpenHome(Some(Box::new(explicit.clone()))),
            _ => unreachable!(),
        };
        start_session_command(&mut runtime, command).unwrap();
        assert!(runtime.reports.is_empty());
        assert_eq!(runtime.session.options().unwrap().target, current.target);
        for _ in 0..10 {
            if runtime.pending.is_none() {
                break;
            }
            worker.complete_next();
            runtime.receive();
        }
        assert!(runtime.pending.is_none(), "{action}");
        assert_eq!(runtime.errors.len(), 1, "{action}: {:?}", runtime.errors);
        assert_eq!(runtime.reports.len(), 1, "{action}");
        assert!(matches!(
            runtime.reports[0],
            SessionCommandReport::Output { .. }
        ));
        assert_eq!(
            runtime.session.options().unwrap().session_file_path(),
            target.session_file_path()
        );
        assert_eq!(loaded_line_x2(&current), 51);
        if action == "save as" {
            assert!(!explicit.session_file_path().exists());
        } else {
            assert_eq!(loaded_line_x2(&explicit), 42);
        }
    }
}

#[test]
fn shutdown_finishes_remembered_home_fallback_before_final_persistence() {
    for home_enabled in [false, true] {
        let temp = crate::test_temp::tempdir().unwrap();
        let remembered = named_options(temp.path(), "remembered");
        let home = configured_home(temp.path());
        stored_session::save_snapshot(&sample_snapshot(), &remembered).unwrap();
        std::fs::rename(
            remembered.session_file_path(),
            remembered.backup_file_path(),
        )
        .unwrap();
        if home_enabled {
            stored_session::save_snapshot(&sample_snapshot(), &home).unwrap();
        }
        let mut input = test_input_state();
        add_line(&mut input, 51);
        input.clear_session_dirty(); // Existing fixture ink; no edit happened after the missing-file load.
        let measurer = TextMeasurer::default();
        let mut session = SessionState::new(Some(remembered.clone()));
        let persistence = PersistenceController::start_for_test().unwrap();
        let mut runtime = CommandRuntime::new(&mut input, &measurer, &mut session, persistence);
        let SessionCommand::Output { transition, .. } =
            output_command(runtime.session, &remembered)
        else {
            unreachable!()
        };
        start_session_command(
            &mut runtime,
            SessionCommand::Output {
                transition,
                remembered: Some(remembered.session_file_path()),
                home: home_enabled.then(|| Box::new(home.clone())),
            },
        )
        .unwrap();
        persist_after_pending_commands(&mut runtime, |runtime| {
            assert!(matches!(
                runtime.reports.as_slice(),
                [SessionCommandReport::Output {
                    abandoned: Some(_),
                    ..
                }]
            ));
            assert!(runtime.session.pending_output_transition().is_none());
            if let Some(options) = runtime.session.options() {
                assert_eq!(options.target, home.target);
                let snapshot = runtime
                    .input
                    .snapshot_for_persistence_with(&measurer, options)
                    .unwrap();
                stored_session::save_snapshot(&snapshot, options)?;
            } else {
                assert!(!home_enabled);
                assert!(matches!(
                    runtime.input.boards.active_frame().shapes[0].shape,
                    crate::draw::Shape::Line { x2: 51, .. }
                ));
            }
            Ok(())
        })
        .unwrap();
        assert!(runtime.errors.is_empty(), "{:?}", runtime.errors);
        assert!(!remembered.session_file_path().exists());
        assert_eq!(
            loaded_line_x2(&stored_session::SessionOptions {
                target: stored_session::SessionTarget::NamedFile(remembered.backup_file_path()),
                ..remembered.clone()
            }),
            42
        );
    }
}

#[test]
fn quarantine_failure_blocks_dirty_autosave_and_save_before_open() {
    let temp = crate::test_temp::tempdir().unwrap();
    let current = named_options(temp.path(), "current");
    let other = named_options(temp.path(), "other");
    stored_session::save_snapshot(&sample_snapshot(), &current).unwrap();
    std::fs::rename(current.session_file_path(), current.backup_file_path()).unwrap();
    let backup = std::fs::read(current.backup_file_path()).unwrap();
    std::fs::write(current.session_file_path(), b"broken primary").unwrap();
    std::fs::write(
        stored_session::append_path_suffix(
            &current.session_file_path(),
            ".corrupt-18446744073709551615",
        ),
        b"occupied",
    )
    .unwrap();
    stored_session::save_snapshot(&sample_snapshot(), &other).unwrap();
    let mut input = test_input_state();
    let measurer = TextMeasurer::default();
    let mut session = SessionState::new(Some(current.clone()));
    let (persistence, worker) = PersistenceController::controlled_for_test();
    let mut runtime = CommandRuntime::new(&mut input, &measurer, &mut session, persistence);
    let output = output_command(runtime.session, &current);
    start_session_command(&mut runtime, output).unwrap();
    for _ in 0..10 {
        if runtime.pending.is_none() {
            break;
        }
        worker.complete_next();
        runtime.receive();
    }
    assert!(
        runtime
            .session
            .refuses_source_write(&current.session_file_path())
    );
    add_line(runtime.input, 77);
    runtime.input.mark_session_dirty();
    observe_input_dirty(&mut runtime, Instant::now());
    assert!(
        !runtime
            .session
            .autosave_due(Instant::now() + Duration::from_secs(60), &current)
    );
    start_session_command(
        &mut runtime,
        SessionCommand::Open(other.session_file_path()),
    )
    .unwrap();
    worker.complete_next();
    runtime.receive(); // preflight; source save is then refused
    assert!(runtime.pending.is_none());
    assert_eq!(runtime.errors.len(), 2);
    assert_eq!(
        std::fs::read(current.session_file_path()).unwrap(),
        b"broken primary"
    );
    assert_eq!(std::fs::read(current.backup_file_path()).unwrap(), backup);
}

fn drain_output_worker(runtime: &mut CommandRuntime<'_>, mut complete: impl FnMut()) {
    for _ in 0..12 {
        if runtime.pending.is_none() {
            return;
        }
        complete();
        runtime.receive();
    }
    panic!("session command did not finish");
}

#[test]
fn unloaded_edits_cannot_overwrite_primary_on_retry_or_return_to_source() {
    for source in ["named", "remembered", "configured", "per-output"] {
        for cancel in [false, true] {
            let temp = crate::test_temp::tempdir().unwrap();
            let options = if source == "named" || source == "remembered" {
                named_options(temp.path(), "current")
            } else {
                let mut options =
                    stored_session::SessionOptions::new(temp.path().to_path_buf(), "configured");
                options.persist_transparent = true;
                options.per_output = source == "per-output";
                options
            };
            let mut target = options.clone();
            if source == "per-output" {
                target.set_output_identity(Some("new-output"));
            }
            stored_session::save_snapshot(&sample_snapshot(), &options).unwrap();
            let original = std::fs::read(options.session_file_path()).unwrap();
            let mut input = test_input_state();
            let measurer = TextMeasurer::default();
            let mut session = SessionState::new(Some(options.clone()));
            let (persistence, worker) = PersistenceController::controlled_for_test();
            let mut runtime = CommandRuntime::new(&mut input, &measurer, &mut session, persistence);
            let mut output = output_command(runtime.session, &target);
            if source == "remembered"
                && let SessionCommand::Output { remembered, .. } = &mut output
            {
                *remembered = Some(options.session_file_path());
            }
            start_session_command(&mut runtime, output).unwrap();
            add_line(runtime.input, 77);
            runtime.input.mark_session_dirty();
            drain_output_worker(&mut runtime, || worker.complete_next());
            assert!(!runtime.session.is_loaded());
            if cancel {
                runtime.session.cancel_output_transition_for_live_source();
                assert!(
                    !runtime.session.is_loaded(),
                    "cancel must not bless an unloaded canvas"
                );
            } else {
                let retry = output_command(runtime.session, &target);
                start_session_command(&mut runtime, retry).unwrap();
                drain_output_worker(&mut runtime, || worker.complete_next());
            }
            if source == "per-output" {
                assert!(!target.session_file_path().exists());
            }
            assert!(
                std::fs::read(options.session_file_path()).unwrap() == original,
                "the unloaded source must retain its saved ink"
            );
            assert!(!options.backup_file_path().exists());
            assert!(matches!(
                runtime.input.boards.active_frame().shapes[0].shape,
                crate::draw::Shape::Line { x2: 77, .. }
            ));
            assert!(
                !runtime
                    .session
                    .autosave_due(Instant::now() + Duration::from_secs(60), &options)
            );
        }
    }
}
#[test]
fn corrupt_remembered_backup_continues_and_recreates_primary_on_final_save() {
    let temp = crate::test_temp::tempdir().unwrap();
    let options = named_options(temp.path(), "remembered");
    stored_session::save_snapshot(&sample_snapshot(), &options).unwrap();
    std::fs::rename(options.session_file_path(), options.backup_file_path()).unwrap();
    let backup = std::fs::read(options.backup_file_path()).unwrap();
    std::fs::write(options.session_file_path(), b"corrupt remembered").unwrap();
    let mut input = test_input_state();
    let measurer = TextMeasurer::default();
    let mut session = SessionState::new(Some(options.clone()));
    let persistence = PersistenceController::start_for_test().unwrap();
    let mut runtime = CommandRuntime::new(&mut input, &measurer, &mut session, persistence);
    let SessionCommand::Output { transition, .. } = output_command(runtime.session, &options)
    else {
        unreachable!()
    };
    start_session_command(
        &mut runtime,
        SessionCommand::Output {
            transition,
            remembered: Some(options.session_file_path()),
            home: None,
        },
    )
    .unwrap();
    persist_after_pending_commands(&mut runtime, |runtime| {
        assert_eq!(runtime.session.options().unwrap().target, options.target);
        assert!(runtime.session.is_loaded());
        assert_eq!(
            loaded_line_x2(&options),
            42,
            "restoration must be durable before any final save"
        );
        let snapshot = runtime
            .input
            .snapshot_for_persistence_with(&measurer, &options)
            .unwrap();
        stored_session::save_snapshot(&snapshot, &options)?;
        Ok(())
    })
    .unwrap();
    assert_eq!(loaded_line_x2(&options), 42);
    assert_eq!(std::fs::read(options.backup_file_path()).unwrap(), backup);
    assert!(
        runtime
            .input
            .active_toast()
            .unwrap()
            .message
            .contains("Recent changes may be missing")
    );
}

#[test]
fn repaired_reload_unblocks_writes_and_dirty_blocked_sources_cannot_switch() {
    for repair in [false, true] {
        let temp = crate::test_temp::tempdir().unwrap();
        let current = named_options(temp.path(), "current");
        let destination = named_options(temp.path(), "destination");
        stored_session::save_snapshot(&sample_snapshot(), &current).unwrap();
        std::fs::rename(current.session_file_path(), current.backup_file_path()).unwrap();
        let backup = std::fs::read(current.backup_file_path()).unwrap();
        std::fs::write(current.session_file_path(), b"corrupt primary").unwrap();
        std::fs::write(
            stored_session::append_path_suffix(
                &current.session_file_path(),
                ".corrupt-18446744073709551615",
            ),
            b"occupied",
        )
        .unwrap();
        let mut input = test_input_state();
        let measurer = TextMeasurer::default();
        let mut session = SessionState::new(Some(current.clone()));
        let (persistence, worker) = PersistenceController::controlled_for_test();
        let mut runtime = CommandRuntime::new(&mut input, &measurer, &mut session, persistence);
        let output = output_command(runtime.session, &current);
        start_session_command(&mut runtime, output).unwrap();
        drain_output_worker(&mut runtime, || worker.complete_next());
        assert!(
            runtime
                .session
                .refuses_source_write(&current.session_file_path())
        );
        if repair {
            std::fs::copy(current.backup_file_path(), current.session_file_path()).unwrap();
            let retry = output_command(runtime.session, &current);
            start_session_command(&mut runtime, retry).unwrap();
            drain_output_worker(&mut runtime, || worker.complete_next());
            assert!(
                !runtime
                    .session
                    .refuses_source_write(&current.session_file_path())
            );
        }
        add_line(runtime.input, 77);
        runtime.input.mark_session_dirty();
        observe_input_dirty(&mut runtime, Instant::now());
        assert_eq!(
            runtime
                .session
                .autosave_due(Instant::now() + Duration::from_secs(60), &current),
            repair
        );
        let output = output_command(runtime.session, &destination);
        start_session_command(&mut runtime, output).unwrap();
        drain_output_worker(&mut runtime, || worker.complete_next());
        if repair {
            assert_eq!(
                runtime.session.options().unwrap().target,
                destination.target
            );
            let saved = stored_session::load_snapshot(&current).unwrap().unwrap();
            assert!(
                saved.boards[0].pages.pages[0]
                    .shapes
                    .iter()
                    .any(|shape| matches!(shape.shape, crate::draw::Shape::Line { x2: 77, .. }))
            );
        } else {
            assert_eq!(runtime.session.options().unwrap().target, current.target);
            assert!(
                runtime
                    .input
                    .boards
                    .active_frame()
                    .shapes
                    .iter()
                    .any(|shape| matches!(shape.shape, crate::draw::Shape::Line { x2: 77, .. }))
            );
            assert_eq!(
                std::fs::read(current.session_file_path()).unwrap(),
                b"corrupt primary"
            );
            assert_eq!(std::fs::read(current.backup_file_path()).unwrap(), backup);
            assert!(!destination.session_file_path().exists());
        }
    }
}

#[test]
fn queued_commands_remain_bound_to_the_visible_source_and_report_failures() {
    for fail_transport in [false, true] {
        let temp = crate::test_temp::tempdir().unwrap();
        let current = named_options(temp.path(), "current");
        let target = named_options(temp.path(), "destination");
        stored_session::save_snapshot(&sample_snapshot(), &target).unwrap();
        let mut input = test_input_state();
        add_line(&mut input, 51);
        let measurer = TextMeasurer::default();
        let mut session = SessionState::new(Some(current.clone()));
        session.commit_output_options(session.options().unwrap().clone(), true);
        let (persistence, worker) = PersistenceController::controlled_for_test();
        let mut runtime = CommandRuntime::new(&mut input, &measurer, &mut session, persistence);
        let output = output_command(runtime.session, &target);
        start_session_command(&mut runtime, output).unwrap();
        for command in [
            SessionCommand::Clear,
            SessionCommand::ClearTools(Box::new(stored_session::ToolStateSnapshot::from_config(
                &Config::default(),
            ))),
            SessionCommand::Inspect,
            SessionCommand::SaveAs(
                temp.path().join("queued-copy.wayscriber-session"),
                stored_session::SaveAsOverwrite::Deny,
            ),
        ] {
            start_session_command(&mut runtime, command).unwrap();
        }
        if fail_transport {
            drop(worker);
            persist_after_pending_commands(&mut runtime, |runtime| {
                assert_eq!(runtime.errors.len(), 5);
                assert_eq!(runtime.queued_failures, [(4, true)]);
                assert!(runtime.pending.is_none());
                assert_eq!(runtime.session.options().unwrap().target, current.target);
                assert!(!current.session_file_path().exists());
                Ok(())
            })
            .unwrap();
        } else {
            drain_output_worker(&mut runtime, || worker.complete_next());
            assert_eq!(runtime.errors.len(), 4, "{:?}", runtime.errors);
            assert_eq!(runtime.queued_failures, [(4, false)]);
            assert!(matches!(
                runtime.reports.as_slice(),
                [SessionCommandReport::Output { .. }]
            ));
            assert!(!runtime.input.boards.active_frame().shapes.is_empty());
            assert_eq!(loaded_line_x2(&current), 51);
            assert_eq!(loaded_line_x2(&target), 42);
            assert!(!temp.path().join("queued-copy.wayscriber-session").exists());
        }
    }
}

#[test]
fn output_queue_coalesces_duplicates_limits_growth_and_cancels_edits_at_shutdown() {
    let temp = crate::test_temp::tempdir().unwrap();
    let current = named_options(temp.path(), "current");
    let target = named_options(temp.path(), "destination");
    stored_session::save_snapshot(&sample_snapshot(), &target).unwrap();
    let mut input = test_input_state();
    add_line(&mut input, 51);
    let measurer = TextMeasurer::default();
    let mut session = SessionState::new(Some(current.clone()));
    session.commit_output_options(current.clone(), true);
    let persistence = PersistenceController::start_for_test().unwrap();
    let mut runtime = CommandRuntime::new(&mut input, &measurer, &mut session, persistence);
    let command = output_command(runtime.session, &target);
    start_session_command(&mut runtime, command).unwrap();

    for _ in 0..20 {
        start_session_command(&mut runtime, SessionCommand::Clear).unwrap();
    }
    for index in 0..7 {
        start_session_command(
            &mut runtime,
            SessionCommand::SaveAs(
                temp.path()
                    .join(format!("queued-{index}.wayscriber-session")),
                stored_session::SaveAsOverwrite::Deny,
            ),
        )
        .unwrap();
    }
    let error = start_session_command(&mut runtime, SessionCommand::Inspect).unwrap_err();
    assert!(error.to_string().contains("queue is full"));

    persist_after_pending_commands(&mut runtime, |runtime| {
        assert_eq!(runtime.errors.len(), 8);
        assert_eq!(runtime.queued_failures, [(8, true)]);
        assert!(
            runtime
                .errors
                .iter()
                .all(|error| error.to_string().contains("shutting down"))
        );
        assert_eq!(runtime.session.options().unwrap().target, target.target);
        assert!(matches!(
            runtime.reports.as_slice(),
            [SessionCommandReport::Output { .. }]
        ));
        assert!(!runtime.input.boards.active_frame().shapes.is_empty());
        Ok(())
    })
    .unwrap();
    assert_eq!(loaded_line_x2(&current), 51);
    assert_eq!(loaded_line_x2(&target), 42);
    for index in 0..7 {
        assert!(
            !temp
                .path()
                .join(format!("queued-{index}.wayscriber-session"))
                .exists()
        );
    }
}

#[test]
fn a_queued_clear_clears_what_it_was_requested_over_and_nothing_drawn_after() {
    for drawn_after_request in [false, true] {
        let temp = crate::test_temp::tempdir().unwrap();
        let current = named_options(temp.path(), "current");
        let target = named_options(temp.path(), "destination");
        stored_session::save_snapshot(&sample_snapshot(), &target).unwrap();
        let mut input = test_input_state();
        let measurer = TextMeasurer::default();
        let mut session = SessionState::new(Some(current.clone()));
        session.commit_output_options(current.clone(), true);
        let (persistence, worker) = PersistenceController::controlled_for_test();
        let mut runtime = CommandRuntime::new(&mut input, &measurer, &mut session, persistence);
        let output = output_command(runtime.session, &target);
        start_session_command(&mut runtime, output).unwrap();
        if !drawn_after_request {
            // Not yet seen by any tick when Clear is pressed.
            add_line(runtime.input, 51);
        }
        start_session_command(&mut runtime, SessionCommand::Clear).unwrap();
        if drawn_after_request {
            add_line(runtime.input, 77);
        }

        // The output load fails, so the Clear starts on the session it was
        // requested for.
        worker.respond_with(|_| Err(anyhow!("controlled output load failure")));
        runtime.receive();
        drain_output_worker(&mut runtime, || worker.complete_next());

        assert_eq!(runtime.session.options().unwrap().target, current.target);
        let refused = runtime
            .errors
            .iter()
            .any(|error| error.to_string().contains("nothing was cleared"));
        assert_eq!(refused, drawn_after_request, "{:?}", runtime.errors);
        let shapes = &runtime.input.boards.active_frame().shapes;
        if drawn_after_request {
            assert!(matches!(
                shapes[0].shape,
                crate::draw::Shape::Line { x2: 77, .. }
            ));
        } else {
            assert!(shapes.is_empty());
        }
    }
}

#[test]
fn repeated_failed_output_loads_do_not_rotate_a_clean_sources_backup() {
    let temp = crate::test_temp::tempdir().unwrap();
    let current = named_options(temp.path(), "current");
    let target = named_options(temp.path(), "destination");
    stored_session::save_snapshot(&sample_snapshot(), &current).unwrap();
    let mut input = test_input_state();
    add_line(&mut input, 51);
    let measurer = TextMeasurer::default();
    stored_session::save_snapshot(
        &input
            .snapshot_for_persistence_with(&measurer, &current)
            .unwrap(),
        &current,
    )
    .unwrap();
    input.clear_session_dirty();
    let primary = std::fs::read(current.session_file_path()).unwrap();
    let backup = std::fs::read(current.backup_file_path()).unwrap();
    std::fs::write(target.session_file_path(), b"corrupt destination").unwrap();
    std::fs::write(
        stored_session::append_path_suffix(
            &target.session_file_path(),
            ".corrupt-18446744073709551615",
        ),
        b"occupied",
    )
    .unwrap();
    let mut session = SessionState::new(Some(current.clone()));
    session.commit_output_options(current.clone(), true);
    let persistence = PersistenceController::start_for_test().unwrap();
    let mut runtime = CommandRuntime::new(&mut input, &measurer, &mut session, persistence);

    for _ in 0..3 {
        let command = output_command(runtime.session, &target);
        start_session_command(&mut runtime, command).unwrap();
        finish_pending_session_command(&mut runtime).unwrap();
        assert_eq!(runtime.session.options().unwrap().target, current.target);
        assert_eq!(std::fs::read(current.session_file_path()).unwrap(), primary);
        assert_eq!(std::fs::read(current.backup_file_path()).unwrap(), backup);
    }
    assert_eq!(runtime.errors.len(), 3);
    assert!(runtime.reports.is_empty());
}

#[test]
fn remembered_preservation_failure_goes_home_without_retrying_the_unreadable_source() {
    for home_enabled in [false, true] {
        let temp = crate::test_temp::tempdir().unwrap();
        let current = named_options(temp.path(), "remembered");
        let home = named_options(temp.path(), "home");
        stored_session::save_snapshot(&sample_snapshot(), &current).unwrap();
        std::fs::rename(current.session_file_path(), current.backup_file_path()).unwrap();
        let backup = std::fs::read(current.backup_file_path()).unwrap();
        std::fs::write(current.session_file_path(), b"corrupt remembered").unwrap();
        std::fs::write(
            stored_session::append_path_suffix(
                &current.session_file_path(),
                ".corrupt-18446744073709551615",
            ),
            b"occupied",
        )
        .unwrap();
        let mut input = test_input_state();
        let measurer = TextMeasurer::default();
        let mut session = SessionState::new(Some(current.clone()));
        let (persistence, worker) = PersistenceController::controlled_for_test();
        let mut runtime = CommandRuntime::new(&mut input, &measurer, &mut session, persistence);
        let SessionCommand::Output { transition, .. } = output_command(runtime.session, &current)
        else {
            unreachable!()
        };
        start_session_command(
            &mut runtime,
            SessionCommand::Output {
                transition,
                remembered: Some(current.session_file_path()),
                home: home_enabled.then(|| Box::new(home.clone())),
            },
        )
        .unwrap();
        drain_output_worker(&mut runtime, || worker.complete_next());
        assert!(runtime.errors.is_empty(), "{:?}", runtime.errors);
        assert!(runtime.session.is_loaded());
        assert!(runtime.session.pending_output_transition().is_none());
        assert_eq!(
            runtime.session.options().map(|options| &options.target),
            home_enabled.then_some(&home.target)
        );
        assert!(matches!(
            runtime.reports.as_slice(),
            [SessionCommandReport::Output {
                abandoned: Some(_),
                ..
            }]
        ));
        assert_eq!(
            std::fs::read(current.session_file_path()).unwrap(),
            b"corrupt remembered"
        );
        assert_eq!(std::fs::read(current.backup_file_path()).unwrap(), backup);
    }
}
