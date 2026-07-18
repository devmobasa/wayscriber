use super::*;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};
use std::thread;

#[test]
fn daemon_lifecycle_wait_wakes_for_v2_maintenance_deadline() {
    let wake = RuntimeWakeSource::new().unwrap();
    let deadline = BootDeadlineSource::new().unwrap();
    deadline
        .arm(
            super::super::protocol_v2::BootClock::now()
                .unwrap()
                .checked_add(Duration::from_millis(5))
                .unwrap(),
        )
        .unwrap();
    let readiness =
        wait_for_daemon_lifecycle(&wake, None, Some(&deadline), &OverlayChildOwner::default())
            .unwrap();
    assert_eq!(
        readiness,
        DaemonLifecycleReadiness {
            command_queue: false,
            deadline: true,
        }
    );
    assert!(deadline.drain().unwrap());
}

#[cfg(unix)]
#[test]
fn listener_failure_invalidates_v1_readiness_and_runs_existing_cleanup() {
    let _signal_guard = crate::unix_signals::test_signal_lock();
    let _env_guard = crate::test_env::lock();
    let temp = crate::test_temp::tempdir().unwrap();
    let previous_runtime_dir = std::env::var_os(crate::env_vars::XDG_RUNTIME_DIR_ENV);
    // SAFETY: this test holds the process environment mutex.
    unsafe {
        std::env::set_var(crate::env_vars::XDG_RUNTIME_DIR_ENV, temp.path());
    }

    let wake = RuntimeWakeSource::new().unwrap();
    let wake_handle = wake.handle();
    let listener = crate::unix_signals::spawn_listener(
        &[libc::SIGWINCH],
        |_| {},
        move || wake_handle.wake().unwrap(),
    )
    .unwrap();
    listener.inject_read_error(libc::EIO);
    wait_for_daemon_lifecycle(&wake, None, None, &OverlayChildOwner::default()).unwrap();

    let mut daemon = Daemon::new(None, false, None, None);
    daemon.protocol_mode = DaemonControlProtocolMode::rollback_compatibility();
    daemon.signal_listener = Some(listener);
    crate::daemon::write_daemon_pid_file(std::process::id(), &daemon.instance_token).unwrap();
    assert!(crate::paths::daemon_pid_file().exists());

    let err = daemon
        .run_control_loop_and_invalidate_on_failure(&wake)
        .unwrap_err();
    assert!(err.to_string().contains("daemon signal listener failed"));
    assert!(!crate::paths::daemon_pid_file().exists());

    let cleanup_err = daemon.shutdown_after_run().unwrap_err();
    assert!(cleanup_err.to_string().contains("failed before teardown"));
    assert!(daemon.signal_listener.is_none());
    assert!(daemon.should_quit.load(Ordering::Acquire));

    // SAFETY: this test still holds the process environment mutex.
    unsafe {
        match previous_runtime_dir {
            Some(value) => std::env::set_var(crate::env_vars::XDG_RUNTIME_DIR_ENV, value),
            None => std::env::remove_var(crate::env_vars::XDG_RUNTIME_DIR_ENV),
        }
    }
}

#[test]
fn light_draw_off_request_does_not_show_hidden_overlay() {
    let called = Arc::new(AtomicUsize::new(0));
    let called_clone = Arc::clone(&called);
    let runner: Arc<BackendRunner> = Arc::new(move |_| {
        called_clone.fetch_add(1, AtomicOrdering::SeqCst);
        Ok(())
    });
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
        .unwrap();

    assert_eq!(called.load(AtomicOrdering::SeqCst), 0);
    assert_eq!(daemon.test_state(), OverlayState::Hidden);
    assert!(daemon.pending_toggle_request.is_none());
    assert!(daemon.pending_activation_token.is_none());
}

#[test]
fn visible_overlay_rejects_different_named_session_request() {
    let runner: Arc<BackendRunner> = Arc::new(|_| Ok(()));
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
    let runner: Arc<BackendRunner> = Arc::new(|_| Ok(()));
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
    let called = Arc::new(AtomicUsize::new(0));
    let called_clone = Arc::clone(&called);
    let runner: Arc<BackendRunner> = Arc::new(move |_| {
        called_clone.fetch_add(1, AtomicOrdering::SeqCst);
        Ok(())
    });
    let mut daemon = Daemon::with_backend_runner(None, runner);

    daemon
        .process_signal_toggle_commands(DaemonToggleCommands {
            commands: Vec::new(),
            saw_command_files: true,
        })
        .expect("typed command marker should suppress raw fallback");

    assert_eq!(called.load(AtomicOrdering::SeqCst), 0);
    assert_eq!(daemon.test_state(), OverlayState::Hidden);
}

#[test]
fn duplicate_plain_toggle_requests_are_debounced() {
    let called = Arc::new(AtomicUsize::new(0));
    let called_clone = Arc::clone(&called);
    let runner: Arc<BackendRunner> = Arc::new(move |_| {
        called_clone.fetch_add(1, AtomicOrdering::SeqCst);
        Ok(())
    });
    let mut daemon = Daemon::with_backend_runner(None, runner);

    daemon
        .process_single_toggle(Some(DaemonToggleRequest::default()), None, false)
        .unwrap();
    daemon
        .process_single_toggle(Some(DaemonToggleRequest::default()), None, false)
        .unwrap();

    assert_eq!(called.load(AtomicOrdering::SeqCst), 1);
    assert_eq!(daemon.test_state(), OverlayState::Hidden);
}

#[test]
fn typed_visibility_toggle_request_is_not_debounced() {
    let modes = Arc::new(std::sync::Mutex::new(Vec::new()));
    let modes_clone = Arc::clone(&modes);
    let runner: Arc<BackendRunner> = Arc::new(move |mode| {
        modes_clone
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .push(mode);
        Ok(())
    });
    let mut daemon = Daemon::with_backend_runner(None, runner);

    daemon
        .process_single_toggle(Some(DaemonToggleRequest::default()), None, false)
        .unwrap();
    daemon
        .process_single_toggle(
            Some(DaemonToggleRequest {
                mode: Some("whiteboard".to_string()),
                ..Default::default()
            }),
            None,
            false,
        )
        .unwrap();

    let modes = modes
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    assert_eq!(modes.as_slice(), &[None, Some("whiteboard".to_string())]);
    assert_eq!(daemon.test_state(), OverlayState::Hidden);
}

#[cfg(unix)]
#[test]
fn duplicate_plain_toggle_after_slow_hide_is_debounced() {
    let broker = crate::process_broker::start_for_runtime().unwrap();
    let mut daemon = Daemon::new(None, false, None, None);
    let child = broker
        .broker()
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
    daemon.overlay_child.reserve().unwrap();
    daemon.overlay_child.start(child).unwrap();
    daemon.overlay_child.mark_committing().unwrap();
    daemon.overlay_child.mark_ready().unwrap();
    daemon
        .overlay_active
        .store(true, std::sync::atomic::Ordering::Release);
    daemon.overlay_state = OverlayState::Visible;

    let hide_started = Instant::now();
    daemon
        .process_single_toggle(Some(DaemonToggleRequest::default()), None, false)
        .unwrap();
    assert!(
        hide_started.elapsed() >= DUPLICATE_SHORTCUT_SUPPRESSION_WINDOW,
        "test setup should keep hide slow enough to cross the debounce window"
    );
    assert_eq!(daemon.test_state(), OverlayState::Hidden);

    let called = Arc::new(AtomicUsize::new(0));
    let called_clone = Arc::clone(&called);
    daemon.backend_runner = Some(Arc::new(move |_| {
        called_clone.fetch_add(1, AtomicOrdering::SeqCst);
        Ok(())
    }));

    daemon
        .process_single_toggle(Some(DaemonToggleRequest::default()), None, false)
        .unwrap();

    assert_eq!(called.load(AtomicOrdering::SeqCst), 0);
    assert_eq!(daemon.test_state(), OverlayState::Hidden);
}

#[test]
fn plain_toggle_after_debounce_window_is_processed() {
    let called = Arc::new(AtomicUsize::new(0));
    let called_clone = Arc::clone(&called);
    let runner: Arc<BackendRunner> = Arc::new(move |_| {
        called_clone.fetch_add(1, AtomicOrdering::SeqCst);
        Ok(())
    });
    let mut daemon = Daemon::with_backend_runner(None, runner);

    daemon
        .process_single_toggle(Some(DaemonToggleRequest::default()), None, false)
        .unwrap();
    daemon.last_plain_visibility_toggle_completed_at =
        Some(Instant::now() - DUPLICATE_SHORTCUT_SUPPRESSION_WINDOW - Duration::from_millis(1));
    daemon
        .process_single_toggle(Some(DaemonToggleRequest::default()), None, false)
        .unwrap();

    assert_eq!(called.load(AtomicOrdering::SeqCst), 2);
    assert_eq!(daemon.test_state(), OverlayState::Hidden);
}

#[test]
fn published_v2_runtime_drives_a_typed_request_to_terminal_response() {
    let _env_guard = crate::test_env::lock();
    let temp = crate::test_temp::tempdir().unwrap();
    let previous_runtime_dir = std::env::var_os(crate::env_vars::XDG_RUNTIME_DIR_ENV);
    // SAFETY: this test holds the process environment mutex.
    unsafe {
        std::env::set_var(crate::env_vars::XDG_RUNTIME_DIR_ENV, temp.path());
    }

    let token = ProtocolToken::generate().unwrap();
    let token_text = token.to_string();
    let owner = CommandOwner::open(&token_text).unwrap();
    let queue_path = owner.queue_path();
    let action_journal = ActionJournal::open().unwrap();
    let runtime = DaemonRuntimeRecordV2::current(token).unwrap();
    super::super::protocol_v2::write_runtime_record_v2(&crate::paths::daemon_pid_file(), &runtime)
        .unwrap();

    let observed_modes = Arc::new(Mutex::new(Vec::new()));
    let runner_modes = Arc::clone(&observed_modes);
    let runner: Arc<BackendRunner> = Arc::new(move |mode| {
        runner_modes
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .push(mode);
        Ok(())
    });
    let mut daemon = Daemon::with_backend_runner(None, runner);
    daemon.protocol_mode = DaemonControlProtocolMode::dark_harness();
    daemon.instance_token = token_text;
    daemon.v2_command_owner = Some(owner);
    daemon.v2_action_journal = Some(action_journal);

    std::thread::scope(|scope| {
        let caller = scope.spawn(|| {
            crate::daemon::send_daemon_toggle_request(&DaemonToggleRequest {
                mode: Some("whiteboard".into()),
                ..Default::default()
            })
        });
        let deadline = Instant::now() + Duration::from_secs(2);
        while std::fs::read_dir(&queue_path).unwrap().next().is_none() {
            assert!(Instant::now() < deadline, "v2 caller did not publish");
            std::thread::sleep(Duration::from_millis(2));
        }
        daemon.process_v2_commands().unwrap();
        caller.join().unwrap().unwrap();
    });

    assert_eq!(
        observed_modes
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .as_slice(),
        &[Some("whiteboard".into())]
    );
    assert_eq!(daemon.test_state(), OverlayState::Hidden);

    // SAFETY: this test still holds the process environment mutex.
    unsafe {
        match previous_runtime_dir {
            Some(value) => std::env::set_var(crate::env_vars::XDG_RUNTIME_DIR_ENV, value),
            None => std::env::remove_var(crate::env_vars::XDG_RUNTIME_DIR_ENV),
        }
    }
}
