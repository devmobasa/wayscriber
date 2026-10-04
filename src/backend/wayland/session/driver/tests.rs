use super::*;
use crate::backend::wayland::backend::runtime_wake::RuntimeWakeSource;

mod lifecycle_regressions;
use crate::backend::wayland::session::{
    PersistenceOperation, RequestId, SaveStrategy, SessionState,
    persistence::SubmitError,
    tests::{EnvGuard, add_line, loaded_line_x2, named_options, sample_snapshot, test_input_state},
};
use crate::backend::wayland::{
    backend::event_loop::session_save::record_autosave_failure,
    runtime_ui_state::{
        RuntimeUiSeedRefresh, SeedRefreshContext, ToolbarRuntimeState,
        refresh_runtime_ui_config_seeds,
    },
    state::{ToolbarChrome, ToolbarDrag},
    toolbar::ToolbarSurfaceManager,
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
    /// The target at each commit notice, in order, with the number of
    /// terminal reports published before it.
    pub committed_targets: Vec<(Option<stored_session::SessionTarget>, usize)>,
    ui: Option<ToolbarRuntimeState>,
    ui_engine: crate::ui_text::UiTextEngine,
    chrome: ToolbarChrome,
    drag: ToolbarDrag,
    toolbar: ToolbarSurfaceManager,
    autosaves: usize,
    autosave_failures: usize,
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
            committed_targets: Vec::new(),
            ui: None,
            ui_engine: crate::ui_text::UiTextEngine::default(),
            chrome: ToolbarChrome::new(true, (0.0, 0.0)),
            drag: ToolbarDrag::new(),
            toolbar: ToolbarSurfaceManager::new(),
            autosaves: 0,
            autosave_failures: 0,
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

    fn enable_ui_runtime(&mut self, path: &std::path::Path) {
        let wake = RuntimeWakeSource::new().unwrap();
        self.ui = Some(
            ToolbarRuntimeState::start(&self.config, self.input, path, wake.handle()).unwrap(),
        );
    }

    fn submit_autosave(
        &mut self,
        snapshot: stored_session::SessionSnapshot,
        options: stored_session::SessionOptions,
    ) -> RequestId {
        let window = self.session.prepare_autosave_submission().unwrap();
        let epoch = self.session.target_epoch();
        let id = self
            .persistence
            .try_submit(
                epoch,
                PersistenceOperation::Save {
                    snapshot,
                    options,
                    strategy: SaveStrategy::Autosave,
                    contentless_clear_boundary: false,
                },
            )
            .unwrap();

        self.session.commit_autosave_submission(id, window);

        id
    }

    fn apply_session_completion(&mut self, completion: PersistenceCompletion) -> Result<()> {
        apply_session_completion(self, completion)
    }

    fn receive(&mut self) {
        let completion = self.persistence.wait_for_completion().unwrap().unwrap();
        self.apply_session_completion(completion).unwrap();
    }
}

impl RuntimeUiSeedRefresh for CommandRuntime<'_> {
    fn seed_refresh_context(&mut self) -> SeedRefreshContext<'_> {
        SeedRefreshContext {
            config: &self.config,
            input: self.input,
            engine: &self.ui_engine,
            measurer: self.measurer,
            runtime: self.ui.as_mut(),
            drag: &mut self.drag,
            chrome: &mut self.chrome,
            toolbar: &mut self.toolbar,
        }
    }

    fn cancel_position_drags(&mut self) {
        self.drag.end_move();
        self.drag.set_preview_active(false);
        self.drag.cancel_gtk();
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
        refresh_runtime_ui_config_seeds(self);
    }

    fn session_target_committed(&mut self) {
        self.committed_targets.push((
            self.session.options().map(|options| options.target.clone()),
            self.reports.len(),
        ));
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

    fn autosave_succeeded(&mut self, _: SaveCompletion, _: Duration) {
        self.autosaves += 1;
    }

    fn autosave_failed(&mut self, _: &anyhow::Error) {
        let options = self.session.options().unwrap().clone();
        record_autosave_failure(self.session, Instant::now(), &options);
        self.autosave_failures += 1;
    }
}

#[test]
fn admission_rejects_each_guard_without_submitting_or_retargeting() {
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Guard {
        Pending,
        Unhealthy,
        ClearConfig,
        ToolsConfig,
    }

    for case in [
        Guard::Pending,
        Guard::Unhealthy,
        Guard::ClearConfig,
        Guard::ToolsConfig,
    ] {
        let temp = crate::test_temp::tempdir().unwrap();
        let options = named_options(temp.path(), "current");
        let mut input = test_input_state();
        let mut session = SessionState::new(Some(options.clone()));
        let measurer = TextMeasurer::default();
        let (persistence, worker) = PersistenceController::controlled_for_test();
        let mut runtime = CommandRuntime::new(&mut input, &measurer, &mut session, persistence);
        let mut worker = Some(worker);

        let command = match case {
            Guard::Pending => {
                start_session_command(&mut runtime, SessionCommand::Inspect).unwrap();
                worker.as_ref().unwrap().complete_next();
                // Receipt is held before runtime delivery; the original identity must survive.
                SessionCommand::Clear
            }
            Guard::Unhealthy => {
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
            Guard::ClearConfig => {
                runtime.config_failed = true;
                SessionCommand::Clear
            }
            Guard::ToolsConfig => {
                runtime.config_failed = true;
                SessionCommand::ClearTools(Box::new(
                    stored_session::ToolStateSnapshot::from_config(&runtime.config),
                ))
            }
        };

        let before = runtime
            .pending
            .as_ref()
            .and_then(|pending| pending.request_id);
        let error = start_session_command(&mut runtime, command)
            .unwrap_err()
            .to_string();
        let expected = match case {
            Guard::Pending => "already pending",
            Guard::Unhealthy => "unhealthy",
            _ => "refusing to modify saved session data",
        };

        assert!(error.contains(expected), "{case:?}: {error}");
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
        if case == Guard::Pending {
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

    let autosave_id = runtime.submit_autosave(sample_snapshot(), options);

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CatalogOutcome {
    Success,
    Failure,
    Rejected,
}

impl CatalogOutcome {
    fn assert_terminal_report(
        self,
        runtime: &CommandRuntime<'_>,
        target: &stored_session::SessionOptions,
        root: &std::path::Path,
    ) {
        assert!(runtime.pending.is_none());
        assert!(runtime.errors.is_empty());
        let [SessionCommandReport::Open(report)] = runtime.reports.as_slice() else {
            panic!("expected committed Open report");
        };
        assert_eq!(report.catalog_error.is_some(), self != Self::Success);
        assert_eq!(
            runtime.session.options().unwrap().session_file_path(),
            target.session_file_path()
        );
        assert_eq!(runtime.input.boards.active_frame().shapes.len(), 1);
        assert!(matches!(
            runtime.input.boards.active_frame().shapes[0].shape,
            crate::draw::Shape::Line { x2: 42, .. }
        ));
        assert_eq!(loaded_line_x2(target), 42);
        // Catalog I/O failure leaves the worker usable; rejected transport marks it unhealthy.
        assert_eq!(runtime.persistence.is_healthy(), self != Self::Rejected);

        match self {
            Self::Failure => {
                let error = report.catalog_error.as_ref().unwrap();
                assert!(
                    format!("{error:#}").contains("failed to create session catalog directory")
                );
                assert!(error.downcast_ref::<std::io::Error>().is_some());
                assert_eq!(
                    std::fs::read(root.join("wayscriber")).unwrap(),
                    b"catalog blocked"
                );
            }
            Self::Success => {
                let entries = stored_session::catalog::recent_sessions().unwrap();
                assert_eq!(entries.len(), 1);
                assert!(stored_session::catalog::session_paths_match(
                    std::path::Path::new(&entries[0].path),
                    &target.session_file_path()
                ));
                assert!(entries[0].last_opened_at_millis.is_some());
            }
            Self::Rejected => {
                assert!(matches!(
                    report
                        .catalog_error
                        .as_ref()
                        .unwrap()
                        .downcast_ref::<SubmitError>(),
                    Some(SubmitError::Disconnected)
                ));
            }
        }
    }
}

#[test]
fn open_refreshes_consumer_seeds_before_catalog_work_and_at_completion() {
    for catalog in [
        CatalogOutcome::Success,
        CatalogOutcome::Failure,
        CatalogOutcome::Rejected,
    ] {
        let temp = crate::test_temp::tempdir().unwrap();
        let current = named_options(temp.path(), "current");
        let target = named_options(temp.path(), "target");
        stored_session::save_snapshot(&sample_snapshot(), &target).unwrap();
        let _env = EnvGuard::set_xdg_data_home(temp.path());
        if catalog == CatalogOutcome::Failure {
            // A regular file blocks catalog directory creation, without relying on uid or modes.
            std::fs::write(temp.path().join("wayscriber"), b"catalog blocked").unwrap();
        }

        let mut input = test_input_state();
        let mut session = SessionState::new(Some(current));
        let measurer = TextMeasurer::default();
        let (persistence, worker) = PersistenceController::controlled_for_test();
        let mut runtime = CommandRuntime::new(&mut input, &measurer, &mut session, persistence);
        runtime.enable_ui_runtime(&temp.path().join("runtime-ui.toml"));
        runtime.config.ui.toolbar.top_offset = 64.0;
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
        let worker = if catalog == CatalogOutcome::Rejected {
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
        assert_eq!(runtime.chrome.top_offset(), (64.0, 0.0));

        if let Some(worker) = worker {
            // Make changed consumer-visible seeds observable on the terminal refresh too.
            runtime.config.ui.toolbar.top_offset = 96.0;
            for item in &mut runtime.config.boards.as_mut().unwrap().items {
                item.pinned = false;
            }
            worker.complete_next();
            runtime.receive();

            assert_eq!(runtime.chrome.top_offset(), (96.0, 0.0));
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

        catalog.assert_terminal_report(&runtime, &target, temp.path());
    }
}

#[test]
fn committed_open_announces_its_target_before_the_terminal_report() {
    for catalog in [
        CatalogOutcome::Success,
        CatalogOutcome::Failure,
        CatalogOutcome::Rejected,
    ] {
        let temp = crate::test_temp::tempdir().unwrap();
        let current = named_options(temp.path(), "current");
        let target = named_options(temp.path(), "target");
        stored_session::save_snapshot(&sample_snapshot(), &target).unwrap();
        let _env = EnvGuard::set_xdg_data_home(temp.path());
        if catalog == CatalogOutcome::Failure {
            std::fs::write(temp.path().join("wayscriber"), b"catalog blocked").unwrap();
        }
        let mut input = test_input_state();
        let mut session = SessionState::new(Some(current));
        let measurer = TextMeasurer::default();
        let (persistence, worker) = PersistenceController::controlled_for_test();
        let mut runtime = CommandRuntime::new(&mut input, &measurer, &mut session, persistence);
        let committed = (Some(target.target.clone()), 0);

        start_session_command(
            &mut runtime,
            SessionCommand::Open(target.session_file_path()),
        )
        .unwrap();
        worker.complete_next(); // open preflight
        runtime.receive();
        assert!(runtime.committed_targets.is_empty());

        worker.complete_next(); // candidate load
        let completion = runtime.persistence.wait_for_completion().unwrap().unwrap();
        let worker = (catalog != CatalogOutcome::Rejected).then_some(worker);
        runtime.apply_session_completion(completion).unwrap();
        // Announced at commit, before the catalog work.
        assert_eq!(runtime.committed_targets.first(), Some(&committed));

        if let Some(worker) = worker {
            worker.complete_next();
            runtime.receive();
        }
        catalog.assert_terminal_report(&runtime, &target, temp.path());
        assert!(
            runtime
                .committed_targets
                .iter()
                .all(|notice| *notice == committed),
            "{catalog:?}: {:?}",
            runtime.committed_targets
        );
    }
}

#[test]
fn committed_save_as_announces_its_target_before_the_terminal_report() {
    let temp = crate::test_temp::tempdir().unwrap();
    let current = named_options(temp.path(), "current");
    let target = named_options(temp.path(), "target");
    let mut input = test_input_state();
    add_line(&mut input, 51);
    let mut session = SessionState::new(Some(current));
    let measurer = TextMeasurer::default();
    let (persistence, worker) = PersistenceController::controlled_for_test();
    let mut runtime = CommandRuntime::new(&mut input, &measurer, &mut session, persistence);

    start_session_command(
        &mut runtime,
        SessionCommand::SaveAs(
            target.session_file_path(),
            stored_session::SaveAsOverwrite::Deny,
        ),
    )
    .unwrap();
    worker.complete_next(); // overwrite preflight
    runtime.receive();
    assert!(runtime.committed_targets.is_empty());
    worker.complete_next(); // save as
    runtime.receive();

    assert!(runtime.errors.is_empty());
    assert!(matches!(
        runtime.reports.as_slice(),
        [SessionCommandReport::SaveAs(_)]
    ));
    assert_eq!(
        runtime.committed_targets,
        [(Some(target.target.clone()), 0)]
    );
}

#[test]
fn failed_open_and_save_as_announce_nothing() {
    let temp = crate::test_temp::tempdir().unwrap();
    let current = named_options(temp.path(), "current");
    let existing = named_options(temp.path(), "existing");
    stored_session::save_snapshot(&sample_snapshot(), &existing).unwrap();
    let missing = temp.path().join("missing.wayscriber-session");
    for command in [
        SessionCommand::Open(missing),
        SessionCommand::SaveAs(
            existing.session_file_path(),
            stored_session::SaveAsOverwrite::Deny,
        ),
    ] {
        let mut input = test_input_state();
        add_line(&mut input, 51);
        let mut session = SessionState::new(Some(current.clone()));
        let measurer = TextMeasurer::default();
        let (persistence, worker) = PersistenceController::controlled_for_test();
        let mut runtime = CommandRuntime::new(&mut input, &measurer, &mut session, persistence);

        start_session_command(&mut runtime, command).unwrap();
        worker.complete_next(); // preflight
        runtime.receive();

        assert_eq!(runtime.errors.len(), 1);
        assert!(runtime.reports.is_empty());
        assert!(runtime.committed_targets.is_empty());
        assert_eq!(runtime.session.options().unwrap().target, current.target);
    }
}

fn configured_home(base: &std::path::Path) -> stored_session::SessionOptions {
    let mut options = stored_session::SessionOptions::new(base.join("configured"), "home");
    options.persist_transparent = true;
    options
}

#[test]
fn returning_home_saves_the_current_session_then_loads_home_like_a_launch() {
    let temp = crate::test_temp::tempdir().unwrap();
    let current = named_options(temp.path(), "current");
    let home = configured_home(temp.path());
    stored_session::save_snapshot(&sample_snapshot(), &home).unwrap();
    let mut input = test_input_state();
    add_line(&mut input, 51);
    input.mark_session_dirty();
    let mut session = SessionState::new(Some(current.clone()));
    let measurer = TextMeasurer::default();
    let (persistence, worker) = PersistenceController::controlled_for_test();
    let mut runtime = CommandRuntime::new(&mut input, &measurer, &mut session, persistence);
    let epoch = runtime.session.target_epoch();

    start_session_command(
        &mut runtime,
        SessionCommand::OpenHome(Some(Box::new(home.clone()))),
    )
    .unwrap();
    worker.complete_next(); // save the current session
    runtime.receive();
    assert_eq!(loaded_line_x2(&current), 51);
    assert!(runtime.reports.is_empty());
    worker.complete_next(); // load home
    runtime.receive();

    assert!(runtime.errors.is_empty(), "{:?}", runtime.errors);
    assert!(matches!(
        runtime.reports.as_slice(),
        [SessionCommandReport::Home]
    ));
    assert_eq!(runtime.session.options().unwrap().target, home.target);
    assert_ne!(runtime.session.target_epoch(), epoch);
    assert!(!runtime.session.is_dirty() && !runtime.input.is_session_dirty());
    let shapes = &runtime.input.boards.active_frame().shapes;
    assert_eq!(shapes.len(), 1);
    assert!(matches!(
        shapes[0].shape,
        crate::draw::Shape::Line { x2: 42, .. }
    ));
    // The commit was announced before the terminal report.
    assert_eq!(
        runtime.committed_targets.last(),
        Some(&(Some(home.target.clone()), 0))
    );
}

/// A home whose session file a save could not replace, of the kind `case`
/// names, and the error that names it.
fn unsaveable_home(
    base: &std::path::Path,
    case: &str,
) -> (stored_session::SessionOptions, &'static str) {
    let configured = configured_home(base);
    let saved = named_options(base, "saved");
    stored_session::save_snapshot(&sample_snapshot(), &saved).unwrap();
    match case {
        // A launch would start on an empty canvas here.
        "configured directory" => {
            std::fs::create_dir_all(configured.session_file_path()).unwrap();
            (configured, "is a directory")
        }
        // A launch would restore the recovery copy, then fail every save.
        "configured directory beside a recovery copy" => {
            std::fs::create_dir_all(configured.session_file_path()).unwrap();
            std::fs::copy(saved.session_file_path(), configured.recovery_file_path()).unwrap();
            (configured, "is a directory")
        }
        // A launch would follow the link, then fail every save.
        "configured symlink" => {
            std::fs::create_dir_all(&configured.base_dir).unwrap();
            std::os::unix::fs::symlink(saved.session_file_path(), configured.session_file_path())
                .unwrap();
            (configured, "is a symlink")
        }
        "named directory" => {
            let named = named_options(base, "named-home");
            std::fs::create_dir(named.session_file_path()).unwrap();
            (named, "directory")
        }
        _ => unreachable!(),
    }
}

#[test]
fn a_home_that_cannot_be_loaded_leaves_the_current_session() {
    for case in [
        "configured directory",
        "configured directory beside a recovery copy",
        "configured symlink",
        "named directory",
    ] {
        let temp = crate::test_temp::tempdir().unwrap();
        let current = named_options(temp.path(), "current");
        let (home, expected) = unsaveable_home(temp.path(), case);
        let mut input = test_input_state();
        add_line(&mut input, 51);
        input.mark_session_dirty();
        let mut session = SessionState::new(Some(current.clone()));
        let measurer = TextMeasurer::default();
        let (persistence, worker) = PersistenceController::controlled_for_test();
        let mut runtime = CommandRuntime::new(&mut input, &measurer, &mut session, persistence);

        start_session_command(&mut runtime, SessionCommand::OpenHome(Some(Box::new(home))))
            .unwrap();
        worker.complete_next(); // save the current session
        runtime.receive();
        worker.complete_next(); // load home
        runtime.receive();

        assert!(runtime.reports.is_empty());
        assert!(
            format!("{:#}", runtime.errors[0]).contains(expected),
            "{case}: {:?}",
            runtime.errors
        );
        assert_eq!(runtime.session.options().unwrap().target, current.target);
        let shapes = &runtime.input.boards.active_frame().shapes;
        assert_eq!(shapes.len(), 1, "{case}");
        assert!(matches!(
            shapes[0].shape,
            crate::draw::Shape::Line { x2: 51, .. }
        ));
        assert!(runtime.committed_targets.is_empty());
    }
}

#[test]
fn returning_home_is_refused_when_the_canvas_changes_while_home_loads() {
    let temp = crate::test_temp::tempdir().unwrap();
    let current = named_options(temp.path(), "current");
    let home = configured_home(temp.path());
    let mut input = test_input_state();
    let mut session = SessionState::new(Some(current.clone()));
    let measurer = TextMeasurer::default();
    let (persistence, worker) = PersistenceController::controlled_for_test();
    let mut runtime = CommandRuntime::new(&mut input, &measurer, &mut session, persistence);

    start_session_command(&mut runtime, SessionCommand::OpenHome(Some(Box::new(home)))).unwrap();
    add_line(runtime.input, 77);
    runtime.input.mark_session_dirty();
    worker.complete_next(); // load home
    runtime.receive();

    assert!(runtime.reports.is_empty());
    assert!(
        runtime.errors[0]
            .to_string()
            .contains("session was edited while the command was pending"),
        "{:?}",
        runtime.errors
    );
    assert_eq!(runtime.session.options().unwrap().target, current.target);
    assert_eq!(runtime.input.boards.active_frame().shapes.len(), 1);
}

#[test]
fn returning_to_a_home_without_persistence_leaves_an_unsaved_empty_canvas() {
    let temp = crate::test_temp::tempdir().unwrap();
    let current = named_options(temp.path(), "current");
    let mut input = test_input_state();
    add_line(&mut input, 51);
    input.mark_session_dirty();
    let mut session = SessionState::new(Some(current.clone()));
    let measurer = TextMeasurer::default();
    let (persistence, worker) = PersistenceController::controlled_for_test();
    let mut runtime = CommandRuntime::new(&mut input, &measurer, &mut session, persistence);

    start_session_command(&mut runtime, SessionCommand::OpenHome(None)).unwrap();
    worker.complete_next(); // save the current session
    runtime.receive();

    assert!(!worker.has_request(), "nothing to load");
    assert_eq!(loaded_line_x2(&current), 51);
    assert!(matches!(
        runtime.reports.as_slice(),
        [SessionCommandReport::Home]
    ));
    assert!(runtime.session.options().is_none());
    assert!(runtime.input.boards.active_frame().shapes.is_empty());
    assert_eq!(runtime.committed_targets.last(), Some(&(None, 0)));
}

#[test]
fn clear_completion_refreshes_consumer_seeds() {
    let temp = crate::test_temp::tempdir().unwrap();
    let mut options = named_options(temp.path(), "clear");
    options.autosave_enabled = true;
    let mut input = test_input_state();
    add_line(&mut input, 51);
    let mut session = SessionState::new(Some(options));
    let measurer = TextMeasurer::default();
    let (persistence, worker) = PersistenceController::controlled_for_test();
    let mut runtime = CommandRuntime::new(&mut input, &measurer, &mut session, persistence);
    runtime.enable_ui_runtime(&temp.path().join("runtime-ui.toml"));
    runtime.config.ui.toolbar.top_offset = 64.0;
    runtime.config.boards = Some(runtime.input.boards.to_config());
    for item in &mut runtime.config.boards.as_mut().unwrap().items {
        item.pinned = true;
    }

    start_session_command(&mut runtime, SessionCommand::Clear).unwrap();
    worker.complete_next();
    runtime.receive();

    assert!(runtime.input.boards.active_frame().shapes.is_empty());
    assert_eq!(runtime.chrome.top_offset(), (64.0, 0.0));
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
    assert!(!runtime.input.is_session_dirty());
    assert!(!runtime.session.is_dirty());
    let options = runtime.session.options().unwrap();
    assert!(
        runtime
            .session
            .autosave_timeout(Instant::now(), options)
            .is_none()
    );
    assert!(options.clear_marker_file_path().exists());
    assert!(!options.session_file_path().exists());
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
fn shutdown_starts_ready_to_poll_open_and_save_as_after_an_autosave_receipt() {
    for open in [true, false] {
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
        observe_input_dirty(&mut runtime, Instant::now());
        let snapshot = runtime
            .input
            .snapshot_for_persistence_with(&measurer, &current)
            .unwrap();

        runtime.submit_autosave(snapshot, current.clone());
        let command = if open {
            SessionCommand::Open(target.session_file_path())
        } else {
            SessionCommand::SaveAs(
                target.session_file_path(),
                stored_session::SaveAsOverwrite::Deny,
            )
        };
        start_session_command(&mut runtime, command).unwrap();
        worker.complete_next();
        runtime.receive();
        assert!(!runtime.persistence.is_active());
        assert_eq!(runtime.pending.as_ref().unwrap().request_id, None);
        assert!(runtime.reports.is_empty());

        std::thread::scope(|scope| {
            scope.spawn(move || {
                for _ in 0..if open { 3 } else { 2 } {
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
                let snapshot = runtime
                    .input
                    .snapshot_for_persistence_with(&measurer, &target)
                    .unwrap();
                stored_session::save_snapshot(&snapshot, runtime.session.options().unwrap())?;
                Ok(())
            })
            .unwrap();
        });
        assert_eq!(loaded_line_x2(&current), 51);
        assert_eq!(loaded_line_x2(&target), if open { 42 } else { 51 });
    }
}

#[test]
fn shutdown_reports_open_and_save_as_phase_failures_before_final_persistence() {
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

            worker.respond_with(|_| Err(anyhow!("controlled shutdown phase failure")));
            // The controller remains active with a receipt ready; shutdown must poll it.
            persist_after_pending_commands(&mut runtime, |runtime| {
                assert!(runtime.pending.is_none());
                let catalog_failure = open && completed == 3;
                let active = if catalog_failure { &target } else { &current };
                assert_eq!(
                    runtime.session.options().unwrap().session_file_path(),
                    active.session_file_path()
                );
                if catalog_failure {
                    assert!(runtime.errors.is_empty());
                    let [SessionCommandReport::Open(report)] = runtime.reports.as_slice() else {
                        panic!("expected committed Open report");
                    };
                    assert!(
                        report
                            .catalog_error
                            .as_ref()
                            .unwrap()
                            .to_string()
                            .contains("controlled shutdown phase failure")
                    );
                } else {
                    assert!(runtime.reports.is_empty());
                    assert_eq!(runtime.errors.len(), 1);
                    assert!(
                        runtime.errors[0]
                            .to_string()
                            .contains("controlled shutdown phase failure")
                    );
                }

                let snapshot = runtime
                    .input
                    .snapshot_for_persistence_with(&measurer, active)
                    .unwrap();
                stored_session::save_snapshot(&snapshot, active)?;
                assert_eq!(
                    loaded_line_x2(active),
                    if catalog_failure { 42 } else { 51 }
                );
                Ok(())
            })
            .unwrap();
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
