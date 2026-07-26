use super::super::types::{DaemonPublishError, MAX_OVERLAY_ACTION_INTENTS};
use super::*;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread;

type RunnerInvocation = (Option<String>, Option<bool>);

fn runner_probe() -> (Box<BackendRunner>, Receiver<RunnerInvocation>) {
    let (sender, receiver) = mpsc::channel();
    let runner = Box::new(move |mode, session_resume_override| {
        sender
            .send((mode, session_resume_override))
            .map_err(|_| anyhow::anyhow!("runner probe receiver disconnected"))
    });
    (runner, receiver)
}

fn runtime_paths(temp: &crate::test_temp::TempDir) -> crate::paths::PreparedRuntimePaths {
    let paths =
        crate::paths::PathResolver::from_environment(crate::paths::PathEnvironment::for_test(&[(
            crate::env_vars::XDG_RUNTIME_DIR_ENV,
            temp.path().as_os_str(),
        )]));
    crate::paths::PreparedRuntimePaths::prepare(&paths)
        .expect("fixture prepares a private runtime identity")
}

fn stopped_overlay_child() -> OverlayChildOwner {
    OverlayChildOwner::new(std::path::PathBuf::from(
        "/tmp/wayscriber-stopped-overlay-child-fixture",
    ))
}

#[test]
fn inline_runner_receives_request_override_before_daemon_default() {
    let (runner, invocations) = runner_probe();
    let mut daemon = Daemon::new(None, false, Some(false), None);
    daemon.backend_runner = Some(runner);

    daemon
        .process_single_toggle(
            Some(DaemonToggleRequest {
                resume_session: true,
                ..Default::default()
            }),
            None,
            false,
        )
        .expect("explicit request override runs through the inline fixture");
    daemon
        .process_single_toggle(
            Some(DaemonToggleRequest {
                mode: Some("whiteboard".to_string()),
                ..Default::default()
            }),
            None,
            false,
        )
        .expect("daemon default runs through the inline fixture");

    assert_eq!(
        invocations.try_iter().collect::<Vec<_>>(),
        [
            (None, Some(true)),
            (Some("whiteboard".to_string()), Some(false)),
        ]
    );
}

#[test]
fn independent_daemon_owners_keep_events_and_coalesced_visibility_isolated() {
    let first_wake = RuntimeWakeSource::new().expect("fixture creates first owner wake");
    let second_wake = RuntimeWakeSource::new().expect("fixture creates second owner wake");
    let (first_inbox, mut first_senders) = daemon_event_channel();
    let (second_inbox, mut second_senders) = daemon_event_channel();
    let mut first = Daemon::new(None, false, None, None);
    let mut second = Daemon::new(None, false, None, None);
    first.daemon_event_inbox = Some(first_inbox);
    second.daemon_event_inbox = Some(second_inbox);

    let first_visibility = first_senders.visibility(
        first_wake
            .try_sender()
            .expect("fixture duplicates first visibility wake"),
    );
    first_visibility
        .publish(Some("first-old".to_string()), false, "first fixture")
        .expect("first owner accepts its initial visibility intent");
    first_visibility
        .publish(Some("first-new".to_string()), true, "first fixture")
        .expect("first owner accepts its newer visibility intent");
    let mut first_actions = first_senders
        .overlay_actions(
            first_wake
                .try_sender()
                .expect("fixture duplicates first action wake"),
        )
        .expect("first fixture claims its only action publisher");
    first_actions
        .publish(TrayAction::LightDrawOn)
        .expect("first owner accepts its action");
    second_senders
        .visibility(
            second_wake
                .try_sender()
                .expect("fixture duplicates second visibility wake"),
        )
        .publish(Some("second".to_string()), false, "second fixture")
        .expect("second owner accepts its visibility intent");
    let mut second_actions = second_senders
        .overlay_actions(
            second_wake
                .try_sender()
                .expect("fixture duplicates second action wake"),
        )
        .expect("second fixture claims its only action publisher");
    second_actions
        .publish(TrayAction::CaptureRegion)
        .expect("second owner accepts its action");

    first.drain_daemon_events();
    assert_eq!(
        first.pending_visibility_intent,
        Some(VisibilityIntent {
            activation_token: Some("first-new".to_string()),
            signal_requested: true,
        })
    );
    assert_eq!(
        first
            .pending_overlay_actions
            .iter()
            .copied()
            .collect::<Vec<_>>(),
        [TrayAction::LightDrawOn]
    );
    assert!(second.pending_visibility_intent.is_none());
    assert!(second.pending_overlay_actions.is_empty());

    second.drain_daemon_events();
    assert_eq!(
        second.pending_visibility_intent,
        Some(VisibilityIntent {
            activation_token: Some("second".to_string()),
            signal_requested: false,
        })
    );
    assert_eq!(
        second
            .pending_overlay_actions
            .iter()
            .copied()
            .collect::<Vec<_>>(),
        [TrayAction::CaptureRegion]
    );
}

#[test]
fn action_capacity_spans_ingress_root_pending_and_admission_retry() {
    let wake = RuntimeWakeSource::new().expect("fixture creates the daemon action wake");
    let (inbox, mut senders) = daemon_event_channel();
    let mut daemon = Daemon::new(None, false, None, None);
    daemon.daemon_event_inbox = Some(inbox);
    let mut publisher = senders
        .overlay_actions(
            wake.try_sender()
                .expect("fixture duplicates the daemon action wake"),
        )
        .expect("fixture claims its only daemon action publisher");
    let expected = (0..MAX_OVERLAY_ACTION_INTENTS)
        .map(|index| {
            if index % 2 == 0 {
                TrayAction::ToggleHelp
            } else {
                TrayAction::CaptureRegion
            }
        })
        .collect::<Vec<_>>();

    for action in expected.iter().copied() {
        publisher
            .publish(action)
            .expect("fixture has capacity for the documented action bound");
    }
    daemon.drain_daemon_events();
    assert_eq!(
        daemon
            .pending_overlay_actions
            .iter()
            .copied()
            .collect::<Vec<_>>(),
        expected
    );
    assert!(matches!(
        publisher.publish(TrayAction::ToggleFreeze),
        Err(DaemonPublishError::QueueFull)
    ));

    let (claimed, claimed_retry) = daemon
        .claim_overlay_action_batch()
        .expect("fixture claims its root-owned pending action batch");
    assert!(!claimed_retry);
    let mut failures = Vec::new();
    daemon.retain_action_admission_retry(claimed, 0, &mut failures);
    assert!(failures.is_empty());
    assert_eq!(daemon.pending_action_admission_retry, expected);
    assert!(matches!(
        publisher.publish(TrayAction::ToggleFreeze),
        Err(DaemonPublishError::QueueFull)
    ));

    daemon.action_admission_retry_at = Some(BootDeadline::from_nanos(0));
    let (retry, claimed_retry) = daemon
        .claim_overlay_action_batch()
        .expect("fixture retry deadline is already due");
    assert!(claimed_retry);
    assert_eq!(retry, expected);
    let completed = retry.len();
    daemon.retain_action_admission_retry(Vec::new(), completed, &mut failures);
    assert!(failures.is_empty());

    publisher
        .publish(TrayAction::ToggleFreeze)
        .expect("terminal ownership returns capacity to the only publisher");
    daemon.drain_daemon_events();
    assert_eq!(
        daemon
            .pending_overlay_actions
            .iter()
            .copied()
            .collect::<Vec<_>>(),
        [TrayAction::ToggleFreeze]
    );
}

#[test]
fn daemon_lifecycle_wait_wakes_for_v2_maintenance_deadline() {
    let wake = RuntimeWakeSource::new().expect("fixture creates its daemon lifecycle wake");
    let signals = crate::unix_signals::FakeSignalSource::new()
        .expect("fixture creates its daemon signal source");
    let deadline =
        BootDeadlineSource::new().expect("fixture creates its v2 maintenance deadline source");
    deadline
        .arm(
            super::super::protocol_v2::BootClock::now()
                .expect("fixture reads the Linux boot clock")
                .checked_add(Duration::from_millis(5))
                .expect("fixture deadline is representable"),
        )
        .expect("fixture arms its v2 maintenance deadline");
    let readiness = wait_for_daemon_lifecycle(
        &wake,
        &signals,
        None,
        Some(&deadline),
        &stopped_overlay_child(),
    )
    .expect("fixture lifecycle wait observes its armed deadline");
    assert_eq!(
        readiness,
        DaemonLifecycleReadiness {
            signal: false,
            command_queue: false,
            deadline: true,
        }
    );
    assert!(
        deadline
            .drain()
            .expect("fixture drains its fired maintenance deadline")
    );
}

#[test]
fn action_admission_retry_uses_the_existing_v2_deadline_source() {
    let wake = RuntimeWakeSource::new().expect("fixture creates its daemon lifecycle wake");
    let signals = crate::unix_signals::FakeSignalSource::new()
        .expect("fixture creates its daemon signal source");
    let mut daemon = Daemon::new(None, false, None, None);
    daemon.v2_deadline_source = Some(
        BootDeadlineSource::new().expect("fixture creates its v2 maintenance deadline source"),
    );
    daemon.action_admission_retry_at = Some(BootDeadline::from_nanos(1));
    daemon
        .arm_v2_lifecycle_deadline()
        .expect("fixture arms the already-due action retry deadline");

    let readiness = wait_for_daemon_lifecycle(
        &wake,
        &signals,
        None,
        daemon.v2_deadline_source.as_ref(),
        &stopped_overlay_child(),
    )
    .expect("fixture lifecycle wait observes its action retry deadline");
    assert_eq!(
        readiness,
        DaemonLifecycleReadiness {
            signal: false,
            command_queue: false,
            deadline: true,
        }
    );
    let deadline_source = daemon
        .v2_deadline_source
        .as_ref()
        .expect("fixture installed a v2 maintenance deadline source");
    assert!(
        deadline_source
            .drain()
            .expect("fixture drains its fired action retry deadline")
    );
}

#[test]
fn signal_source_failure_is_terminal_to_the_daemon_owner() {
    let mut signals = crate::unix_signals::FakeSignalSource::new()
        .expect("fixture creates its daemon signal source");
    signals
        .fail_next_drain(std::io::ErrorKind::BrokenPipe)
        .expect("fixture wakes its failed daemon signal source");
    let mut daemon = Daemon::new(None, false, None, None);
    let err = daemon
        .drain_signal_events(&mut signals)
        .expect_err("injected signal-source failure must stop the daemon owner");
    assert!(err.to_string().contains("daemon signal source failed"));
}

#[test]
fn daemon_lifecycle_wait_reports_signal_readiness() {
    let wake = RuntimeWakeSource::new().expect("fixture creates its daemon lifecycle wake");
    let mut signals = crate::unix_signals::FakeSignalSource::new()
        .expect("fixture creates its daemon signal source");
    signals
        .publish(crate::unix_signals::SignalEvent::Shutdown(
            crate::unix_signals::ShutdownSignal::Terminate,
        ))
        .expect("fixture publishes its daemon shutdown signal");

    let readiness =
        wait_for_daemon_lifecycle(&wake, &signals, None, None, &stopped_overlay_child())
            .expect("fixture lifecycle wait observes signal readiness");

    assert_eq!(
        readiness,
        DaemonLifecycleReadiness {
            signal: true,
            command_queue: false,
            deadline: false,
        }
    );
    let mut daemon = Daemon::new(None, false, None, None);
    daemon
        .drain_signal_events(&mut signals)
        .expect("fixture routes its daemon shutdown signal");
    assert!(daemon.should_quit);
}

#[test]
fn light_draw_off_request_does_not_show_hidden_overlay() {
    let (runner, calls) = runner_probe();
    let mut daemon = Daemon::with_backend_runner(None, runner);

    daemon
        .process_single_toggle(
            Some(DaemonToggleRequest {
                overlay_action: Some(TrayAction::LightDrawOff),
                ..Default::default()
            }),
            None,
            false,
        )
        .expect("fixture handles LightDrawOff while already hidden");

    assert!(matches!(calls.try_recv(), Err(TryRecvError::Empty)));
    assert_eq!(daemon.test_state(), OverlayState::Hidden);
    assert!(daemon.pending_toggle_request.is_none());
    assert!(daemon.pending_activation_token.is_none());
}

#[test]
fn visible_overlay_rejects_different_named_session_request() {
    let runner = Box::new(|_, _| Ok(()));
    let mut daemon = Daemon::with_backend_runner(None, runner);
    daemon.overlay_state = OverlayState::Visible;
    daemon.active_named_session_file =
        Some(std::path::PathBuf::from("/tmp/current.wayscriber-session"));

    let err = daemon
        .process_single_toggle(
            Some(DaemonToggleRequest {
                session_file: Some(std::path::PathBuf::from("/tmp/other.wayscriber-session")),
                ..Default::default()
            }),
            None,
            false,
        )
        .expect_err("different visible named target should be rejected");

    assert!(
        format!("{err:#}").contains("cannot switch named session target while overlay is visible"),
        "{err:#}"
    );
    assert_eq!(daemon.test_state(), OverlayState::Visible);
    assert_eq!(
        daemon.active_named_session_file.as_deref(),
        Some(std::path::Path::new("/tmp/current.wayscriber-session"))
    );
}

#[test]
fn visible_overlay_rejection_writes_daemon_toggle_error_response() {
    let temp = crate::test_temp::tempdir().expect("tempdir");
    let runner = Box::new(|_, _| Ok(()));
    let mut daemon = Daemon::with_backend_runner(None, runner);
    daemon.overlay_state = OverlayState::Visible;
    daemon.active_named_session_file =
        Some(std::path::PathBuf::from("/tmp/current.wayscriber-session"));
    let command = DaemonToggleCommand {
        daemon_token: "daemon-token".into(),
        request: DaemonToggleRequest {
            session_file: Some(std::path::PathBuf::from("/tmp/other.wayscriber-session")),
            ..Default::default()
        },
        request_path: temp.path().join("request.json"),
        response_path: temp.path().join("responses").join("request.json"),
    };

    let mut suppress_overlay_action_signal = false;
    daemon.process_queued_toggle_command(command.clone(), &mut suppress_overlay_action_signal);

    let err = read_daemon_toggle_response(&command.response_path)
        .expect_err("visible target mismatch should be written to response");
    assert!(
        format!("{err:#}").contains("cannot switch named session target while overlay is visible"),
        "{err:#}"
    );
    assert_eq!(daemon.test_state(), OverlayState::Visible);
    assert!(!suppress_overlay_action_signal);
}

#[test]
fn typed_signal_with_no_executable_commands_does_not_fallback_to_raw_toggle() {
    let (runner, calls) = runner_probe();
    let mut daemon = Daemon::with_backend_runner(None, runner);

    daemon
        .process_signal_toggle_commands(DaemonToggleCommands {
            commands: Vec::new(),
            saw_command_files: true,
        })
        .expect("typed command marker should suppress raw fallback");

    assert!(matches!(calls.try_recv(), Err(TryRecvError::Empty)));
    assert_eq!(daemon.test_state(), OverlayState::Hidden);
}

#[test]
fn duplicate_plain_toggle_requests_are_debounced() {
    let (runner, calls) = runner_probe();
    let mut daemon = Daemon::with_backend_runner(None, runner);

    daemon
        .process_single_toggle(Some(DaemonToggleRequest::default()), None, false)
        .expect("fixture processes its first plain toggle");
    daemon
        .process_single_toggle(Some(DaemonToggleRequest::default()), None, false)
        .expect("fixture processes its duplicate plain toggle");

    assert_eq!(calls.try_iter().count(), 1);
    assert_eq!(daemon.test_state(), OverlayState::Hidden);
}

#[test]
fn typed_visibility_toggle_request_is_not_debounced() {
    let (runner, modes) = runner_probe();
    let mut daemon = Daemon::with_backend_runner(None, runner);

    daemon
        .process_single_toggle(Some(DaemonToggleRequest::default()), None, false)
        .expect("fixture processes its initial plain toggle");
    daemon
        .process_single_toggle(
            Some(DaemonToggleRequest {
                mode: Some("whiteboard".to_string()),
                ..Default::default()
            }),
            None,
            false,
        )
        .expect("fixture processes its typed visibility toggle");

    assert_eq!(
        modes.try_iter().collect::<Vec<_>>(),
        [(None, None), (Some("whiteboard".to_string()), None),]
    );
    assert_eq!(daemon.test_state(), OverlayState::Hidden);
}

#[cfg(unix)]
#[test]
fn duplicate_plain_toggle_after_slow_hide_is_debounced() {
    let broker = crate::process_broker::start_for_runtime()
        .expect("fixture starts its process-broker owner");
    let mut daemon = Daemon::new(None, false, None, None);
    let child = broker
        .handle()
        .spawn(
            crate::process_broker::HelperKind::TestSleep,
            crate::process_broker::HelperLifetime::OwnedChild,
            std::ffi::OsStr::new("sleep"),
            [std::ffi::OsStr::new("10")],
            Vec::new(),
        )
        .expect("spawn slow-terminating test process");
    let child_pid = child.id();
    assert_eq!(unsafe { libc::kill(child_pid as i32, libc::SIGSTOP) }, 0);
    let mut stopped = false;
    for _ in 0..20 {
        let mut status = 0;
        let result = unsafe {
            libc::waitpid(
                child_pid as i32,
                &mut status,
                libc::WNOHANG | libc::WUNTRACED,
            )
        };
        if result == child_pid as i32 && libc::WIFSTOPPED(status) {
            stopped = true;
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }
    assert!(stopped, "test child should stop before hide starts");
    daemon
        .overlay_child
        .reserve()
        .expect("fixture reserves an overlay generation");
    daemon
        .overlay_child
        .start(child)
        .expect("fixture installs its stopped test child");
    daemon
        .overlay_child
        .mark_committing()
        .expect("fixture advances its child to committing");
    daemon
        .overlay_child
        .mark_ready()
        .expect("fixture advances its child to ready");
    daemon.overlay_state = OverlayState::Visible;

    let hide_started = Instant::now();
    daemon
        .process_single_toggle(Some(DaemonToggleRequest::default()), None, false)
        .expect("fixture hides its deliberately slow overlay child");
    assert!(
        hide_started.elapsed() >= DUPLICATE_SHORTCUT_SUPPRESSION_WINDOW,
        "test setup should keep hide slow enough to cross the debounce window"
    );
    assert_eq!(daemon.test_state(), OverlayState::Hidden);

    let (runner, calls) = runner_probe();
    daemon.backend_runner = Some(runner);

    daemon
        .process_single_toggle(Some(DaemonToggleRequest::default()), None, false)
        .expect("fixture processes the duplicate after the slow hide");

    assert!(matches!(calls.try_recv(), Err(TryRecvError::Empty)));
    assert_eq!(daemon.test_state(), OverlayState::Hidden);
}

#[test]
fn plain_toggle_after_debounce_window_is_processed() {
    let (runner, calls) = runner_probe();
    let mut daemon = Daemon::with_backend_runner(None, runner);

    daemon
        .process_single_toggle(Some(DaemonToggleRequest::default()), None, false)
        .expect("fixture processes its initial plain toggle");
    daemon.last_plain_visibility_toggle_completed_at =
        Some(Instant::now() - DUPLICATE_SHORTCUT_SUPPRESSION_WINDOW - Duration::from_millis(1));
    daemon
        .process_single_toggle(Some(DaemonToggleRequest::default()), None, false)
        .expect("fixture processes the toggle after the debounce window");

    assert_eq!(calls.try_iter().count(), 2);
    assert_eq!(daemon.test_state(), OverlayState::Hidden);
}

#[test]
fn published_v2_runtime_drives_a_typed_request_to_terminal_response() {
    let temp = crate::test_temp::tempdir().expect("fixture creates its v2 runtime directory");
    let runtime_paths = runtime_paths(&temp);
    let root = runtime_paths.protocol_v2_root();

    let token = ProtocolToken::generate().expect("fixture generates its daemon protocol token");
    let token_text = token.to_string();
    let owner =
        CommandOwner::open(&token_text, root.clone()).expect("fixture opens its v2 command owner");
    let queue_path = owner.queue_path();
    let action_journal = ActionJournal::open(root).expect("fixture opens its action journal");
    let runtime = DaemonRuntimeRecordV2::current(token)
        .expect("fixture builds its current daemon runtime identity");
    super::super::protocol_v2::write_runtime_record_v2(&runtime_paths.daemon_pid_file(), &runtime)
        .expect("fixture publishes its v2 runtime record");

    let (runner, observed_modes) = runner_probe();
    let mut daemon = Daemon::with_backend_runner(None, runner);
    daemon.protocol_mode = DaemonControlProtocolMode::dark_harness();
    daemon.instance_token = token_text;
    daemon.v2_command_owner = Some(owner);
    daemon.v2_action_journal = Some(action_journal);

    std::thread::scope(|scope| {
        let caller = scope.spawn(|| {
            crate::daemon::send_daemon_toggle_request(
                &DaemonToggleRequest {
                    mode: Some("whiteboard".into()),
                    ..Default::default()
                },
                &runtime_paths,
            )
        });
        let deadline = Instant::now() + Duration::from_secs(2);
        while std::fs::read_dir(&queue_path)
            .expect("fixture reads its v2 command queue")
            .next()
            .is_none()
        {
            assert!(Instant::now() < deadline, "v2 caller did not publish");
            std::thread::sleep(Duration::from_millis(2));
        }
        daemon
            .process_v2_commands()
            .expect("fixture daemon processes its published v2 command");
        caller
            .join()
            .expect("fixture caller thread exits normally")
            .expect("fixture caller observes a terminal daemon response");
    });

    assert_eq!(
        observed_modes.try_iter().collect::<Vec<_>>(),
        [(Some("whiteboard".into()), None)]
    );
    assert_eq!(daemon.test_state(), OverlayState::Hidden);
}

#[test]
fn failed_anonymous_action_admission_does_not_allow_the_tail_to_overtake() {
    let temp = crate::test_temp::tempdir().expect("fixture creates its action journal root");
    let root = temp.path().join("daemon-commands").join("v2");

    let token = ProtocolToken::generate()
        .expect("fixture generates its daemon protocol token")
        .to_string();
    let _owner =
        CommandOwner::open(&token, root.clone()).expect("fixture opens its v2 command owner");
    let journal = ActionJournal::open(root).expect("fixture opens its action journal");
    journal
        .fail_next_anonymous_publications(1)
        .expect("fixture installs one anonymous-publication failure");
    let runner = Box::new(|_, _| Ok(()));
    let mut daemon = Daemon::with_backend_runner(None, runner);
    daemon.protocol_mode = DaemonControlProtocolMode::dark_harness();
    daemon.instance_token = token.clone();
    daemon.v2_action_journal = Some(journal.clone());
    let wake = RuntimeWakeSource::new().expect("fixture creates its daemon action wake");
    let (inbox, mut senders) = daemon_event_channel();
    daemon.daemon_event_inbox = Some(inbox);
    let mut publisher = senders
        .overlay_actions(
            wake.try_sender()
                .expect("test duplicates its daemon action runtime eventfd"),
        )
        .expect("fixture claims its only daemon action publisher");
    publisher
        .publish(TrayAction::ToggleFreeze)
        .expect("fixture queues the first retained action");
    publisher
        .publish(TrayAction::ToggleHelp)
        .expect("fixture queues the second retained action");
    daemon.drain_daemon_events();

    let (claimed, claimed_retry) = daemon
        .claim_overlay_action_batch()
        .expect("fixture claims its initial action batch");
    assert!(!claimed_retry);
    let error = daemon
        .process_overlay_action_intents(claimed)
        .expect_err("injected first admission failure retains the whole batch");
    assert!(
        error
            .to_string()
            .contains("injected anonymous action admission failure")
    );

    let overtaking_action = journal
        .claim_next(&token, |_, _| Ok(true))
        .expect("fixture probes its durable action journal")
        .map(|claimed| {
            let action = claimed.action();
            claimed
                .finish(false, Some("test cleanup"))
                .expect("fixture completes an unexpectedly admitted action");
            action
        });
    assert_eq!(overtaking_action, None);
    assert!(daemon.pending_overlay_actions.is_empty());
    assert_eq!(daemon.pending_action_admission_retry.len(), 2);

    publisher
        .publish(TrayAction::CaptureRegion)
        .expect("fixture queues an action behind the retained batch");
    assert!(
        wake.drain()
            .expect("fixture drains the queued overtaking-action wake")
    );
    daemon.drain_daemon_events();
    assert_eq!(daemon.pending_overlay_actions.len(), 1);
    assert_eq!(daemon.pending_action_admission_retry.len(), 2);
    daemon.action_admission_retry_at = Some(
        BootClock::now()
            .expect("fixture reads the Linux boot clock")
            .checked_add(Duration::from_secs(60))
            .expect("fixture retry deadline is representable"),
    );
    assert!(
        daemon
            .claim_overlay_action_batch()
            .expect("fixture checks the not-yet-due action retry")
            .0
            .is_empty()
    );
    assert_eq!(daemon.pending_overlay_actions.len(), 1);
    assert_eq!(daemon.pending_action_admission_retry.len(), 2);
    daemon.action_admission_retry_at = Some(BootDeadline::from_nanos(0));
    let (retained, claimed_retry) = daemon
        .claim_overlay_action_batch()
        .expect("fixture claims the now-due retained action batch");
    assert!(claimed_retry);
    assert_eq!(retained, [TrayAction::ToggleFreeze, TrayAction::ToggleHelp]);
    daemon
        .process_overlay_action_intents(retained)
        .expect("fixture admits and delivers the retained action batch");
    assert_eq!(daemon.pending_overlay_actions.len(), 1);
    assert!(daemon.pending_action_admission_retry.is_empty());
    let (newly_queued, claimed_retry) = daemon
        .claim_overlay_action_batch()
        .expect("fixture claims the action queued behind the retained batch");
    assert!(!claimed_retry);
    assert_eq!(newly_queued, [TrayAction::CaptureRegion]);
    daemon
        .process_overlay_action_intents(newly_queued)
        .expect("fixture admits and delivers the queued tail action");

    let mut durable_order = Vec::new();
    while let Some(claimed) = journal
        .claim_next(&token, |_, _| Ok(true))
        .expect("fixture drains the durable action journal")
    {
        durable_order.push(claimed.action());
        claimed
            .finish(false, Some("test cleanup"))
            .expect("fixture completes its claimed durable action");
    }
    assert_eq!(
        durable_order,
        [
            TrayAction::ToggleFreeze,
            TrayAction::ToggleHelp,
            TrayAction::CaptureRegion,
        ]
    );
    assert!(daemon.pending_overlay_actions.is_empty());
    assert!(daemon.pending_action_admission_retry.is_empty());
}
