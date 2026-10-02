use super::*;
use crate::backend::wayland::session::{
    PersistenceOperation, SaveStrategy, SessionState,
    tests::{add_line, loaded_line_x2, named_options, sample_snapshot, test_input_state},
};
use crate::{config::Config, draw::TextMeasurer, input::InputState, session as stored_session};

pub(in crate::backend::wayland::session) struct CommandRuntime<'a> {
    pub input: &'a mut InputState,
    measurer: &'a TextMeasurer,
    pub session: &'a mut SessionState,
    pub persistence: PersistenceController,
    pending: Option<ExplicitSessionTransaction>,
    config: Config,
    config_failed: bool,
    reports: Vec<SessionCommandReport>,
    errors: Vec<anyhow::Error>,
    seed_refreshes: usize,
    autosaves: usize,
}

impl<'a> CommandRuntime<'a> {
    pub fn new(
        input: &'a mut InputState,
        measurer: &'a TextMeasurer,
        session: &'a mut SessionState,
        persistence: PersistenceController,
    ) -> Self {
        Self {
            input,
            measurer,
            session,
            persistence,
            pending: None,
            config: Config::default(),
            config_failed: false,
            reports: Vec::new(),
            errors: Vec::new(),
            seed_refreshes: 0,
            autosaves: 0,
        }
    }

    pub fn into_result(mut self) -> Result<SessionCommandReport> {
        if let Some(error) = self.errors.pop() {
            return Err(error);
        }
        self.reports
            .pop()
            .ok_or_else(|| anyhow!("command did not publish a terminal report"))
    }

    fn receive(&mut self) {
        let completion = self.persistence.wait_for_completion().unwrap().unwrap();
        self.apply_session_completion(completion).unwrap();
    }
}

impl SessionCommandRuntime for CommandRuntime<'_> {
    fn session_context(&mut self) -> SessionTransaction<'_> {
        SessionTransaction {
            input_state: self.input,
            measurer: self.measurer,
            session: self.session,
        }
    }
    fn pending_command(&mut self) -> &mut Option<ExplicitSessionTransaction> {
        &mut self.pending
    }
    fn persistence(&mut self) -> &mut PersistenceController {
        &mut self.persistence
    }
    fn session_config_failed(&self) -> bool {
        self.config_failed
    }
    fn refresh_session_ui_seeds(&mut self) {
        self.seed_refreshes += 1;
        self.input
            .boards
            .sync_pin_seeds_from_config(&self.config.resolved_boards());
    }
    fn finish_session_command(&mut self, report: SessionCommandReport) {
        self.reports.push(report);
    }
    fn fail_session_command(&mut self, _: &SessionCommand, error: &anyhow::Error) {
        self.errors.push(anyhow!("{error:#}"));
    }
    fn session_transport_failed(&mut self, _: &anyhow::Error) {
        self.session.restore_in_flight_autosave();
    }

    fn apply_session_completion(&mut self, completion: PersistenceCompletion) -> Result<()> {
        if route_session_completion(self, completion)?.is_some() {
            self.autosaves += 1;
        }
        Ok(())
    }
}

#[test]
fn admission_rejects_each_guard_without_submitting_or_retargeting() {
    for case in ["pending", "unhealthy", "clear-config", "tools-config"] {
        let temp = crate::test_temp::tempdir().unwrap();
        let options = named_options(temp.path(), "current");
        let mut input = test_input_state();
        let mut session = SessionState::new(Some(options.clone()));
        let measurer = TextMeasurer::default();
        let (persistence, worker) = PersistenceController::controlled_for_test();
        let mut runtime = CommandRuntime::new(&mut input, &measurer, &mut session, persistence);
        let mut worker = Some(worker);
        let command = match case {
            "pending" => {
                start_session_command(&mut runtime, SessionCommand::Inspect).unwrap();
                worker.as_ref().unwrap().complete_next();
                // Receipt is held before runtime delivery; the original identity must survive.
                SessionCommand::Clear
            }
            "unhealthy" => {
                drop(worker.take());
                assert!(
                    runtime
                        .persistence
                        .try_submit(
                            0,
                            PersistenceOperation::HasArtifacts {
                                options: options.clone()
                            }
                        )
                        .is_err()
                );
                SessionCommand::Inspect
            }
            "clear-config" => {
                runtime.config_failed = true;
                SessionCommand::Clear
            }
            "tools-config" => {
                runtime.config_failed = true;
                SessionCommand::ClearTools(Box::new(
                    stored_session::ToolStateSnapshot::from_config(&runtime.config),
                ))
            }
            _ => unreachable!(),
        };
        let before = runtime
            .pending
            .as_ref()
            .and_then(|pending| pending.request_id);
        let error = start_session_command(&mut runtime, command)
            .unwrap_err()
            .to_string();
        let expected = match case {
            "pending" => "already pending",
            "unhealthy" => "unhealthy",
            _ => "refusing to modify saved session data",
        };
        assert!(error.contains(expected), "{case}: {error}");
        assert_eq!(
            runtime
                .pending
                .as_ref()
                .and_then(|pending| pending.request_id),
            before
        );
        assert_eq!(
            runtime.session.options().unwrap().session_file_path(),
            options.session_file_path()
        );
        assert!(runtime.reports.is_empty());
        if let Some(worker) = worker {
            assert!(!worker.has_request());
        }
        if case == "pending" {
            runtime.receive();
            assert!(matches!(
                runtime.reports.as_slice(),
                [SessionCommandReport::Inspection(_)]
            ));
        }
    }
}

#[test]
fn explicit_command_waits_for_autosave_receipt_and_survives_dispatch_work() {
    let temp = crate::test_temp::tempdir().unwrap();
    let options = named_options(temp.path(), "current");
    let mut input = test_input_state();
    let mut session = SessionState::new(Some(options.clone()));
    session.record_input_dirty(Instant::now(), true);
    let measurer = TextMeasurer::default();
    let (persistence, worker) = PersistenceController::controlled_for_test();
    let mut runtime = CommandRuntime::new(&mut input, &measurer, &mut session, persistence);
    let window = runtime.session.prepare_autosave_submission().unwrap();
    let autosave_id = runtime
        .persistence
        .try_submit(
            0,
            PersistenceOperation::Save {
                snapshot: sample_snapshot(),
                options,
                strategy: SaveStrategy::Autosave,
                contentless_clear_boundary: false,
            },
        )
        .unwrap();
    runtime
        .session
        .commit_autosave_submission(autosave_id, window);
    start_session_command(&mut runtime, SessionCommand::Inspect).unwrap();
    poll_pending_session_command(&mut runtime);
    assert_eq!(runtime.pending.as_ref().unwrap().request_id, None);
    assert!(runtime.reports.is_empty());
    assert_eq!(runtime.session.edit_generation(), 1);
    // The worker has received no completion yet; unrelated live input still progresses.
    add_line(runtime.input, 77);
    runtime.input.mark_session_dirty();
    worker.complete_next();
    runtime.receive();
    assert_eq!(runtime.autosaves, 1);
    assert_eq!(runtime.pending.as_ref().unwrap().request_id, None);
    assert!(!worker.has_request());
    assert!(runtime.session.is_dirty());
    poll_pending_session_command(&mut runtime);
    let explicit_id = runtime.pending.as_ref().unwrap().request_id.unwrap();
    assert_ne!(explicit_id, autosave_id);
    worker.complete_next();
    runtime.receive();
    assert!(runtime.pending.is_none());
    assert!(matches!(
        runtime.reports.as_slice(),
        [SessionCommandReport::Inspection(_)]
    ));
}

#[test]
fn live_completion_gate_rejects_a_different_request_identity() {
    let mut input = test_input_state();
    let mut session = SessionState::new(None);
    let measurer = TextMeasurer::default();
    let (persistence, worker) = PersistenceController::controlled_for_test();
    let mut runtime = CommandRuntime::new(&mut input, &measurer, &mut session, persistence);
    // No active session makes Inspect complete inline; use a disk-bound command instead.
    let temp = crate::test_temp::tempdir().unwrap();
    let options = named_options(temp.path(), "current");
    *runtime.session = SessionState::new(Some(options));
    start_session_command(&mut runtime, SessionCommand::Inspect).unwrap();
    worker.complete_next();
    let mut completion = runtime.persistence.wait_for_completion().unwrap().unwrap();
    completion.id.sequence += 1;
    runtime.apply_session_completion(completion).unwrap();
    assert!(runtime.pending.is_none());
    assert!(runtime.reports.is_empty());
    assert!(runtime.errors[0].to_string().contains("identity mismatch"));
    assert!(!worker.has_request());
}

#[test]
fn advance_observes_live_edits_and_finalizes_stale_destructive_work() {
    let temp = crate::test_temp::tempdir().unwrap();
    let options = named_options(temp.path(), "clear");
    stored_session::save_snapshot(&sample_snapshot(), &options).unwrap();
    let mut input = test_input_state();
    add_line(&mut input, 51);
    let mut session = SessionState::new(Some(options));
    let measurer = TextMeasurer::default();
    let (persistence, worker) = PersistenceController::controlled_for_test();
    let mut runtime = CommandRuntime::new(&mut input, &measurer, &mut session, persistence);
    start_session_command(&mut runtime, SessionCommand::Clear).unwrap();
    add_line(runtime.input, 88);
    runtime.input.mark_session_dirty();
    worker.complete_next();
    let completion = runtime.persistence.wait_for_completion().unwrap().unwrap();
    // Exercise advance's own dirty observation, not the completion router's observation.
    complete_session_command(&mut runtime, completion);
    assert!(runtime.pending.is_none());
    assert_eq!(runtime.input.boards.active_frame().shapes.len(), 2);
    assert!(runtime.session.is_dirty());
    assert_eq!(runtime.session.edit_generation(), 1);
    assert!(runtime.errors[0].to_string().contains("edited while"));
    assert!(runtime.reports.is_empty());
}

#[test]
fn deferred_submission_rejection_is_terminal() {
    let temp = crate::test_temp::tempdir().unwrap();
    let options = named_options(temp.path(), "current");
    let mut input = test_input_state();
    let mut session = SessionState::new(Some(options.clone()));
    let measurer = TextMeasurer::default();
    let (persistence, worker) = PersistenceController::controlled_for_test();
    let mut runtime = CommandRuntime::new(&mut input, &measurer, &mut session, persistence);
    runtime
        .persistence
        .try_submit(0, PersistenceOperation::HasArtifacts { options })
        .unwrap();
    start_session_command(&mut runtime, SessionCommand::Inspect).unwrap();
    worker.complete_next();
    runtime.persistence.wait_for_completion().unwrap().unwrap();
    drop(worker);
    poll_pending_session_command(&mut runtime);
    assert!(runtime.pending.is_none());
    assert!(runtime.reports.is_empty());
    assert!(runtime.errors[0].to_string().contains("failed to submit"));
}

#[test]
fn advance_failure_publishes_error_and_retires_the_command() {
    let mut input = test_input_state();
    let mut session = SessionState::new(None);
    let measurer = TextMeasurer::default();
    let (persistence, worker) = PersistenceController::controlled_for_test();
    let mut runtime = CommandRuntime::new(&mut input, &measurer, &mut session, persistence);
    start_session_command(&mut runtime, SessionCommand::Inspect).unwrap();
    assert!(runtime.pending.is_none());
    assert!(runtime.reports.is_empty());
    assert!(
        runtime.errors[0]
            .to_string()
            .contains("no active persisted session target")
    );
    assert!(!worker.has_request());
}

#[test]
fn open_refreshes_consumer_seeds_before_catalog_work_and_at_completion() {
    for catalog in ["success", "failure", "rejected"] {
        let temp = crate::test_temp::tempdir().unwrap();
        let current = named_options(temp.path(), "current");
        let target = named_options(temp.path(), "target");
        stored_session::save_snapshot(&sample_snapshot(), &target).unwrap();
        let mut input = test_input_state();
        let mut session = SessionState::new(Some(current));
        let measurer = TextMeasurer::default();
        let (persistence, worker) = PersistenceController::controlled_for_test();
        let mut runtime = CommandRuntime::new(&mut input, &measurer, &mut session, persistence);
        runtime.config.boards = Some(runtime.input.boards.to_config());
        for item in &mut runtime.config.boards.as_mut().unwrap().items {
            item.pinned = true;
        }
        start_session_command(
            &mut runtime,
            SessionCommand::Open(target.session_file_path()),
        )
        .unwrap();
        worker.complete_next(); // open preflight
        runtime.receive();
        worker.complete_next(); // candidate load, no dirty current source
        let completion = runtime.persistence.wait_for_completion().unwrap().unwrap();
        let worker = if catalog == "rejected" {
            drop(worker);
            None
        } else {
            Some(worker)
        };
        runtime.apply_session_completion(completion).unwrap();
        assert_eq!(
            runtime.session.options().unwrap().session_file_path(),
            target.session_file_path()
        );
        assert_eq!(runtime.input.boards.active_frame().shapes.len(), 1);
        assert!(
            runtime
                .input
                .boards
                .to_config()
                .items
                .iter()
                .all(|item| item.pinned)
        );
        assert_eq!(runtime.seed_refreshes, 1);
        if let Some(worker) = worker {
            // Make a changed authored seed observable on the terminal refresh too.
            for item in &mut runtime.config.boards.as_mut().unwrap().items {
                item.pinned = false;
            }
            if catalog == "failure" {
                worker.respond_with(|_| Err(anyhow!("controlled catalog error")));
            } else {
                worker.complete_next();
            }
            runtime.receive();
            assert_eq!(runtime.seed_refreshes, 2);
            assert!(
                runtime
                    .input
                    .boards
                    .to_config()
                    .items
                    .iter()
                    .all(|item| !item.pinned)
            );
        }
        assert!(runtime.pending.is_none());
        assert!(runtime.errors.is_empty());
        assert!(matches!(
            runtime.reports.as_slice(),
            [SessionCommandReport::Open(_)]
        ));
    }
}

#[test]
fn clear_completion_refreshes_consumer_seeds() {
    let temp = crate::test_temp::tempdir().unwrap();
    let options = named_options(temp.path(), "clear");
    let mut input = test_input_state();
    add_line(&mut input, 51);
    let mut session = SessionState::new(Some(options));
    let measurer = TextMeasurer::default();
    let (persistence, worker) = PersistenceController::controlled_for_test();
    let mut runtime = CommandRuntime::new(&mut input, &measurer, &mut session, persistence);
    runtime.config.boards = Some(runtime.input.boards.to_config());
    for item in &mut runtime.config.boards.as_mut().unwrap().items {
        item.pinned = true;
    }
    start_session_command(&mut runtime, SessionCommand::Clear).unwrap();
    worker.complete_next();
    runtime.receive();
    assert!(runtime.input.boards.active_frame().shapes.is_empty());
    assert!(
        runtime
            .input
            .boards
            .to_config()
            .items
            .iter()
            .all(|item| item.pinned)
    );
    assert!(matches!(
        runtime.reports.as_slice(),
        [SessionCommandReport::Clear(_)]
    ));
}

#[test]
fn shutdown_drains_save_as_and_open_at_every_disk_phase_before_final_save() {
    for (open, phases) in [(false, 2), (true, 4)] {
        for completed in 0..phases {
            let temp = crate::test_temp::tempdir().unwrap();
            let current = named_options(temp.path(), "current");
            let target = named_options(temp.path(), "target");
            if open {
                stored_session::save_snapshot(&sample_snapshot(), &target).unwrap();
            }
            let mut input = test_input_state();
            add_line(&mut input, 51);
            input.mark_session_dirty();
            let mut session = SessionState::new(Some(current.clone()));
            let measurer = TextMeasurer::default();
            let (persistence, worker) = PersistenceController::controlled_for_test();
            let mut runtime = CommandRuntime::new(&mut input, &measurer, &mut session, persistence);
            let command = if open {
                SessionCommand::Open(target.session_file_path())
            } else {
                SessionCommand::SaveAs(
                    target.session_file_path(),
                    stored_session::SaveAsOverwrite::Deny,
                )
            };
            start_session_command(&mut runtime, command).unwrap();
            for _ in 0..completed {
                worker.complete_next();
                runtime.receive();
            }
            assert!(runtime.pending.is_some());
            std::thread::scope(|scope| {
                scope.spawn(move || {
                    for _ in completed..phases {
                        worker.complete_next();
                    }
                });
                persist_after_pending_commands(&mut runtime, |runtime| {
                    assert!(runtime.pending.is_none());
                    assert!(runtime.errors.is_empty());
                    assert_eq!(runtime.reports.len(), 1);
                    assert_eq!(
                        runtime.session.options().unwrap().session_file_path(),
                        target.session_file_path()
                    );
                    add_line(runtime.input, 99);
                    runtime.input.mark_session_dirty();
                    let snapshot = runtime
                        .input
                        .snapshot_for_persistence_with(&measurer, &target)
                        .unwrap();
                    stored_session::save_snapshot(&snapshot, runtime.session.options().unwrap())?;
                    Ok(())
                })
                .unwrap();
            });
            assert!(runtime.pending.is_none());
            assert!(runtime.errors.is_empty());
            assert_eq!(runtime.reports.len(), 1);
            assert_eq!(
                runtime.session.options().unwrap().session_file_path(),
                target.session_file_path()
            );
            assert_eq!(loaded_line_x2(&target), if open { 42 } else { 51 });
            let loaded = stored_session::load_snapshot(&target).unwrap().unwrap();
            assert_eq!(loaded.boards[0].pages.pages[0].shapes.len(), 2);
            if open {
                assert_eq!(loaded_line_x2(&current), 51);
            }
        }
    }
}

#[test]
fn shutdown_cleans_up_pending_work_after_worker_disconnect_and_when_ready_to_poll() {
    for disconnect in [true, false] {
        let temp = crate::test_temp::tempdir().unwrap();
        let options = named_options(temp.path(), "current");
        let mut input = test_input_state();
        let mut session = SessionState::new(Some(options.clone()));
        let measurer = TextMeasurer::default();
        let (persistence, worker) = PersistenceController::controlled_for_test();
        let mut runtime = CommandRuntime::new(&mut input, &measurer, &mut session, persistence);
        // Queue explicit work behind an active worker; delivering the unrelated receipt
        // leaves the command ready to poll rather than already submitted.
        runtime
            .persistence
            .try_submit(0, PersistenceOperation::HasArtifacts { options })
            .unwrap();
        start_session_command(&mut runtime, SessionCommand::Inspect).unwrap();
        if disconnect {
            drop(worker);
            persist_after_pending_commands(&mut runtime, |runtime| {
                assert!(runtime.pending.is_none());
                assert!(runtime.errors[0].to_string().contains("disconnected"));
                Ok(())
            })
            .unwrap();
        } else {
            worker.complete_next();
            runtime.persistence.wait_for_completion().unwrap().unwrap();
            std::thread::scope(|scope| {
                scope.spawn(move || worker.complete_next());
                persist_after_pending_commands(&mut runtime, |runtime| {
                    assert!(runtime.pending.is_none());
                    assert_eq!(runtime.reports.len(), 1);
                    Ok(())
                })
                .unwrap();
            });
        }
        assert!(runtime.pending.is_none());
    }
}
