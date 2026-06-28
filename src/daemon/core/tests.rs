use super::*;
use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};

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
    let mut daemon = Daemon::new(None, false, None, None);
    let child = std::process::Command::new("sleep")
        .arg("10")
        .spawn()
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
        .overlay_pid
        .store(child.id(), std::sync::atomic::Ordering::Release);
    daemon.overlay_child = Some(child);
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
