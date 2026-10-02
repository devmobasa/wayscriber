use super::*;
use crate::backend::wayland::{runtime_ui_state::ToolbarPositionSnapshot, state::MoveDragKind};

#[test]
fn pending_destructive_commands_preserve_live_slider_edits_and_undo() {
    use crate::input::state::{PropertiesPanelHit, SelectionPropertyKind};

    for open in [false, true] {
        let temp = crate::test_temp::tempdir().unwrap();
        let options = named_options(temp.path(), "current");
        let target = named_options(temp.path(), "candidate");
        stored_session::save_snapshot(&sample_snapshot(), &options).unwrap();
        stored_session::save_snapshot(&sample_snapshot(), &target).unwrap();
        let mut input = test_input_state();
        let id = add_line(&mut input, 51);
        let original = input.boards.active_frame().shape(id).unwrap().shape.clone();
        let depth = input.boards.active_frame().undo_stack_len();
        let measurer = TextMeasurer::default();
        let mut session = SessionState::new(Some(options.clone()));
        let (persistence, worker) = PersistenceController::controlled_for_test();
        let mut runtime = CommandRuntime::new(&mut input, &measurer, &mut session, persistence);
        let command = if open {
            SessionCommand::Open(target.session_file_path())
        } else {
            SessionCommand::Clear
        };

        start_session_command(&mut runtime, command).unwrap();
        if open {
            worker.complete_next(); // preflight, followed by the held candidate load
            runtime.receive();
        }
        runtime.input.set_selection(vec![id]);
        assert!(runtime.input.show_properties_panel_with(&measurer));
        let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, 1, 1).unwrap();
        let ctx = cairo::Context::new(&surface).unwrap();
        runtime
            .input
            .update_properties_panel_layout(&ctx, 1280, 800);
        let panel = runtime.input.properties_panel().unwrap();
        let row = panel
            .entries
            .iter()
            .position(|entry| entry.kind == SelectionPropertyKind::Thickness)
            .unwrap();
        let track = runtime
            .input
            .properties_panel_layout()
            .unwrap()
            .hit_rect(panel, PropertiesPanelHit::Slider(row))
            .unwrap();
        assert!(runtime.input.begin_properties_slider_drag_with(
            &measurer,
            row,
            track.right() as i32 - 1
        ));
        assert!(
            !runtime.input.is_session_dirty(),
            "preview has not committed yet"
        );
        assert_eq!(runtime.input.boards.active_frame().undo_stack_len(), depth);

        worker.complete_next();
        runtime.receive();

        let retained = runtime
            .input
            .boards
            .active_frame()
            .shape(id)
            .expect("pending command must retain the edited shape");
        assert!(matches!(
            retained.shape,
            crate::draw::Shape::Line { thick: 50.0, .. }
        ));
        assert_eq!(
            runtime.session.options().unwrap().session_file_path(),
            options.session_file_path()
        );
        assert!(runtime.pending.is_none());
        assert!(runtime.reports.is_empty());
        assert!(
            runtime.errors[0]
                .to_string()
                .contains("interaction changed")
        );
        assert!(runtime.input.is_properties_slider_dragging());
        assert_eq!(runtime.input.boards.active_frame().undo_stack_len(), depth);

        runtime.input.finish_properties_slider_drag_with(&measurer);
        assert!(runtime.input.is_session_dirty());
        assert_eq!(
            runtime.input.boards.active_frame().undo_stack_len(),
            depth + 1
        );
        runtime.input.handle_action_with_resources(
            crate::input::state::InputTextResources {
                measurer: &measurer,
                ui_engine: &crate::ui_text::UiTextEngine::default(),
            },
            crate::config::Action::Undo,
        );
        assert_eq!(
            runtime.input.boards.active_frame().shape(id).unwrap().shape,
            original
        );
    }
}

#[test]
fn autosave_ownership_errors_do_not_publish_failures_or_delay_retry() {
    #[derive(Clone, Copy, Debug)]
    enum Receipt {
        MissingTicket,
        WrongTicket,
        WrongEpoch,
        WriteFailure,
    }

    for receipt in [
        Receipt::MissingTicket,
        Receipt::WrongTicket,
        Receipt::WrongEpoch,
        Receipt::WriteFailure,
    ] {
        let temp = crate::test_temp::tempdir().unwrap();
        let mut options = named_options(temp.path(), "current");
        options.autosave_enabled = true;
        options.autosave_idle = Duration::from_millis(1);
        options.autosave_interval = Duration::from_millis(1);
        options.autosave_failure_backoff = Duration::from_secs(60);

        let started = Instant::now();
        let mut input = test_input_state();
        let mut session = SessionState::new(Some(options.clone()));
        session.record_input_dirty(started, true);
        let measurer = TextMeasurer::default();
        let (persistence, worker) = PersistenceController::controlled_for_test();
        let mut runtime = CommandRuntime::new(&mut input, &measurer, &mut session, persistence);

        runtime.submit_autosave(sample_snapshot(), options.clone());
        if matches!(receipt, Receipt::MissingTicket) {
            assert!(runtime.session.restore_in_flight_autosave());
        }

        if matches!(receipt, Receipt::WriteFailure) {
            worker.respond_with(|_| Err(anyhow!("controlled write failure")));
        } else {
            worker.complete_next();
        }

        let mut completion = runtime.persistence.wait_for_completion().unwrap().unwrap();
        match receipt {
            Receipt::WrongTicket => completion.id.sequence += 1,
            // Defensive branch: real target changes also clear the in-flight ticket.
            // Keep receipt and ticket equal so only the session-epoch guard rejects it.
            Receipt::WrongEpoch => runtime.session.target_epoch += 1,
            _ => {}
        }

        assert!(runtime.apply_session_completion(completion).is_err());

        let owned_failure = matches!(receipt, Receipt::WriteFailure);
        assert_eq!(
            runtime.autosave_failures,
            usize::from(owned_failure),
            "{receipt:?}"
        );
        assert_eq!(runtime.autosaves, 0);
        assert!(runtime.session.is_dirty());
        assert!(runtime.persistence.is_healthy());
        assert_eq!(
            runtime
                .session
                .autosave_due(Instant::now() + Duration::from_millis(2), &options),
            !owned_failure,
            "{receipt:?}"
        );
    }
}

#[test]
fn shutdown_caller_retires_a_command_queued_behind_an_owned_failed_autosave() {
    let temp = crate::test_temp::tempdir().unwrap();
    let current = named_options(temp.path(), "current");
    let target = named_options(temp.path(), "target");
    stored_session::save_snapshot(&sample_snapshot(), &target).unwrap();

    let mut input = test_input_state();
    add_line(&mut input, 51);
    let mut session = SessionState::new(Some(current.clone()));
    session.record_input_dirty(Instant::now(), true);
    let measurer = TextMeasurer::default();
    let (persistence, worker) = PersistenceController::controlled_for_test();
    let mut runtime = CommandRuntime::new(&mut input, &measurer, &mut session, persistence);

    runtime.submit_autosave(sample_snapshot(), current.clone());

    start_session_command(
        &mut runtime,
        SessionCommand::Open(target.session_file_path()),
    )
    .unwrap();
    assert_eq!(runtime.pending.as_ref().unwrap().request_id, None);

    worker.respond_with(|_| Err(anyhow!("controlled autosave failure before open")));
    persist_after_pending_commands(&mut runtime, |runtime| {
        assert!(runtime.pending.is_none());
        assert!(runtime.reports.is_empty());
        assert_eq!(runtime.errors.len(), 1);
        assert!(
            runtime.errors[0]
                .to_string()
                .contains("controlled autosave failure before open")
        );
        assert!(runtime.persistence.is_healthy());
        assert!(runtime.session.is_dirty());
        assert_eq!(
            runtime.session.options().unwrap().session_file_path(),
            current.session_file_path()
        );

        runtime.input.mark_session_dirty();
        let snapshot = runtime
            .input
            .snapshot_for_persistence_with(&measurer, &current)
            .unwrap();
        stored_session::save_snapshot(&snapshot, runtime.session.options().unwrap())?;
        Ok(())
    })
    .unwrap();

    assert!(!worker.has_request());
    assert_eq!(loaded_line_x2(&current), 51);
    assert_eq!(loaded_line_x2(&target), 42);
}

#[test]
fn session_seed_refresh_aborts_changed_drags_and_publishes_live_chrome_positions() {
    #[derive(Clone, Copy, Debug)]
    enum Drag {
        Item,
        Builtin,
        Gtk,
    }

    for drag in [Drag::Item, Drag::Builtin, Drag::Gtk] {
        let temp = crate::test_temp::tempdir().unwrap();
        let options = named_options(temp.path(), "current");

        let mut input = test_input_state();
        add_line(&mut input, 51);

        let mut session = SessionState::new(Some(options));
        let measurer = TextMeasurer::default();
        let (persistence, _worker) = PersistenceController::controlled_for_test();
        let mut runtime = CommandRuntime::new(&mut input, &measurer, &mut session, persistence);

        runtime.enable_ui_runtime(&temp.path().join("runtime-ui.toml"));

        let group = crate::config::ToolbarItemOrderGroup::TopTools;
        let pen = crate::config::toolbar_item_ids::TOP_TOOL_PEN;

        match drag {
            Drag::Item => {
                assert!(
                    runtime
                        .ui
                        .as_mut()
                        .unwrap()
                        .begin_item_drag(group, runtime.input)
                );
                assert!(runtime.input.start_toolbar_item_drag(group, pen));
                runtime.drag.set_item_dragging(true);

                assert!(
                    runtime
                        .config
                        .ui
                        .toolbar
                        .items
                        .move_item_to_index(group, pen, 8)
                );
            }
            Drag::Builtin | Drag::Gtk => {
                assert!(runtime.ui.as_mut().unwrap().begin_position_drag(
                    MoveDragKind::Top,
                    ToolbarPositionSnapshot { top: (0.0, 0.0) }
                ));
                if matches!(drag, Drag::Builtin) {
                    runtime
                        .drag
                        .begin_move(MoveDragKind::Top, (0.0, 0.0), false, (0.0, 0.0));
                } else {
                    runtime
                        .drag
                        .begin_gtk_preview(crate::toolbar_gtk::GtkToolbarKind::Top, 0.0);
                }
                runtime.chrome.set_top_offset((80.0, 100.0));
            }
        }

        runtime.config.ui.toolbar.top_offset = 64.0;
        runtime.config.ui.toolbar.top_offset_y = 32.0;
        runtime.input.needs_redraw = false;

        runtime.refresh_session_ui_seeds();

        assert_eq!(runtime.chrome.top_offset(), (64.0, 32.0), "{drag:?}");
        assert!(!runtime.drag.item_dragging());
        assert!(!runtime.drag.is_moving());
        assert_eq!(runtime.drag.gtk_preview_kind(), None);
        assert!(runtime.input.needs_redraw);
        if matches!(drag, Drag::Item) {
            assert!(runtime.input.start_toolbar_item_drag(group, pen));
        }
    }
}
