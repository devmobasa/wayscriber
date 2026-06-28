use super::*;
use crate::paths::daemon_pid_file;
use std::env;
use std::fs;
use std::sync::Mutex;

static ENV_MUTEX: Mutex<()> = Mutex::new(());

#[test]
fn empty_toggle_request_reports_empty() {
    assert!(DaemonToggleRequest::default().is_empty());
}

#[test]
fn overlay_action_request_is_not_empty() {
    let request = DaemonToggleRequest {
        overlay_action: Some(TrayAction::LightDrawToggle),
        ..Default::default()
    };
    assert!(!request.is_empty());
}

#[test]
fn session_file_request_is_not_empty() {
    let request = DaemonToggleRequest {
        session_file: Some(PathBuf::from("/tmp/lecture.wayscriber-session")),
        ..Default::default()
    };
    assert!(!request.is_empty());
}

#[test]
fn toggle_request_reports_session_override() {
    let request = DaemonToggleRequest {
        resume_session: true,
        ..Default::default()
    };
    assert_eq!(request.session_resume_override(), Some(true));

    let request = DaemonToggleRequest {
        no_resume_session: true,
        ..Default::default()
    };
    assert_eq!(request.session_resume_override(), Some(false));
}

#[test]
fn session_file_request_rejects_no_resume_session() {
    let mut request = DaemonToggleRequest {
        no_resume_session: true,
        session_file: Some(PathBuf::from("/tmp/lecture.wayscriber-session")),
        ..Default::default()
    };

    let err = request
        .normalize_and_validate_session_file()
        .expect_err("session file conflicts with disabled resume");

    assert!(
        format!("{err:#}").contains("--session-file conflicts with --no-resume-session"),
        "{err:#}"
    );
}

#[test]
fn session_file_request_rejects_relative_path() {
    let mut request = DaemonToggleRequest {
        session_file: Some(PathBuf::from("lecture.wayscriber-session")),
        ..Default::default()
    };

    let err = request
        .normalize_and_validate_session_file()
        .expect_err("daemon protocol requires anchored paths");

    assert!(
        format!("{err:#}").contains("daemon --session-file request must use an absolute path"),
        "{err:#}"
    );
}

#[test]
fn daemon_toggle_response_round_trips_error_and_is_removed_after_wait() {
    let tmp = crate::test_temp::tempdir().unwrap();
    let command = DaemonToggleCommand {
        daemon_token: "daemon-token".into(),
        request: DaemonToggleRequest::default(),
        request_path: tmp.path().join("request.json"),
        response_path: tmp.path().join("responses").join("request.json"),
    };

    write_daemon_toggle_command_error(&command, "cannot switch target").unwrap();

    let err =
        wait_daemon_toggle_response_for(&command.response_path, MAX_DAEMON_TOGGLE_RESPONSE_WAIT)
            .expect_err("error response should surface to caller");
    assert!(
        format!("{err:#}").contains("cannot switch target"),
        "{err:#}"
    );
    assert!(!command.response_path.exists());
}

#[test]
fn daemon_toggle_response_round_trips_success_and_is_removed_after_wait() {
    let tmp = crate::test_temp::tempdir().unwrap();
    let command = DaemonToggleCommand {
        daemon_token: "daemon-token".into(),
        request: DaemonToggleRequest::default(),
        request_path: tmp.path().join("request.json"),
        response_path: tmp.path().join("responses").join("request.json"),
    };

    write_daemon_toggle_command_success(&command).unwrap();

    wait_daemon_toggle_response_for(&command.response_path, MAX_DAEMON_TOGGLE_RESPONSE_WAIT)
        .unwrap();
    assert!(!command.response_path.exists());
}

#[test]
fn daemon_toggle_response_wait_covers_request_age_and_overlay_stop_grace() {
    assert!(
        MAX_DAEMON_TOGGLE_RESPONSE_WAIT > MAX_DAEMON_TOGGLE_REQUEST_AGE + Duration::from_secs(2),
        "typed toggle response wait should exceed accepted request age plus overlay stop grace"
    );
}

#[test]
fn daemon_toggle_response_wait_error_marks_existing_request_canceled() {
    let tmp = crate::test_temp::tempdir().unwrap();
    let command = DaemonToggleCommand {
        daemon_token: "daemon-token".into(),
        request: DaemonToggleRequest::default(),
        request_path: tmp.path().join("request.json"),
        response_path: tmp.path().join("responses").join("request.json"),
    };
    fs::write(&command.request_path, b"pending request").unwrap();

    let err = wait_daemon_toggle_command_response_for(&command, Duration::ZERO)
        .expect_err("missing response should time out immediately");

    assert!(
        format!("{err:#}")
            .contains("timed out waiting for wayscriber daemon to process toggle request"),
        "{err:#}"
    );
    let canceled: DaemonToggleEnvelope =
        serde_json::from_slice(&fs::read(&command.request_path).unwrap()).unwrap();
    assert_eq!(canceled.daemon_token, "daemon-token");
    assert!(canceled.canceled);
    assert!(!command.response_path.exists());
}

#[test]
fn daemon_toggle_response_wait_error_does_not_create_missing_cancel_request() {
    let tmp = crate::test_temp::tempdir().unwrap();
    let command = DaemonToggleCommand {
        daemon_token: "daemon-token".into(),
        request: DaemonToggleRequest::default(),
        request_path: tmp.path().join("request.json"),
        response_path: tmp.path().join("responses").join("request.json"),
    };

    let err = wait_daemon_toggle_command_response_for(&command, Duration::ZERO)
        .expect_err("missing response should time out immediately");

    assert!(
        format!("{err:#}")
            .contains("timed out waiting for wayscriber daemon to process toggle request"),
        "{err:#}"
    );
    assert!(!command.request_path.exists());
    assert!(!command.response_path.exists());
}

#[test]
fn daemon_toggle_response_parse_error_marks_existing_request_canceled() {
    let tmp = crate::test_temp::tempdir().unwrap();
    let command = DaemonToggleCommand {
        daemon_token: "daemon-token".into(),
        request: DaemonToggleRequest::default(),
        request_path: tmp.path().join("request.json"),
        response_path: tmp.path().join("responses").join("request.json"),
    };
    fs::write(&command.request_path, b"pending request").unwrap();
    fs::create_dir_all(command.response_path.parent().unwrap()).unwrap();
    fs::write(&command.response_path, b"not json").unwrap();

    let err = wait_daemon_toggle_command_response_for(&command, Duration::ZERO)
        .expect_err("malformed response should preserve parse error");

    assert!(
        format!("{err:#}").contains("failed to parse daemon toggle response"),
        "{err:#}"
    );
    let canceled: DaemonToggleEnvelope =
        serde_json::from_slice(&fs::read(&command.request_path).unwrap()).unwrap();
    assert!(canceled.canceled);
    assert!(!command.response_path.exists());
}

#[test]
fn daemon_pid_file_round_trips_runtime_info() {
    let _guard = ENV_MUTEX
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let tmp = crate::test_temp::tempdir().unwrap();
    let prev = env::var_os(XDG_RUNTIME_DIR_ENV);
    unsafe {
        env::set_var(XDG_RUNTIME_DIR_ENV, tmp.path());
    }

    write_daemon_pid_file(1234, "daemon-token").unwrap();
    let info = read_daemon_runtime_info().unwrap_err();
    assert!(
        info.to_string()
            .contains("wayscriber daemon is not running")
    );

    match prev {
        Some(value) => unsafe { env::set_var(XDG_RUNTIME_DIR_ENV, value) },
        None => unsafe { env::remove_var(XDG_RUNTIME_DIR_ENV) },
    }
}

#[test]
fn stale_cleanup_removes_matching_runtime_while_lock_is_free() {
    let _guard = ENV_MUTEX
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let tmp = crate::test_temp::tempdir().unwrap();
    let prev = env::var_os(XDG_RUNTIME_DIR_ENV);
    unsafe {
        env::set_var(XDG_RUNTIME_DIR_ENV, tmp.path());
    }

    let runtime = DaemonRuntimeInfo {
        pid: 1234,
        token: Some("old-token".into()),
    };
    write_daemon_pid_file(runtime.pid, runtime.token.as_deref().unwrap()).unwrap();
    write_daemon_toggle_request(
        &DaemonToggleRequest {
            freeze: true,
            ..Default::default()
        },
        "old-token",
    )
    .unwrap();

    clear_stale_daemon_state_if_matches(&runtime);

    assert!(!daemon_pid_file().exists());
    assert!(!daemon_command_dir().exists());

    match prev {
        Some(value) => unsafe { env::set_var(XDG_RUNTIME_DIR_ENV, value) },
        None => unsafe { env::remove_var(XDG_RUNTIME_DIR_ENV) },
    }
}

#[test]
fn stale_cleanup_preserves_mismatched_runtime() {
    let _guard = ENV_MUTEX
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let tmp = crate::test_temp::tempdir().unwrap();
    let prev = env::var_os(XDG_RUNTIME_DIR_ENV);
    unsafe {
        env::set_var(XDG_RUNTIME_DIR_ENV, tmp.path());
    }

    let current = DaemonRuntimeInfo {
        pid: 5678,
        token: Some("new-token".into()),
    };
    write_daemon_pid_file(current.pid, current.token.as_deref().unwrap()).unwrap();
    write_daemon_toggle_request(
        &DaemonToggleRequest {
            freeze: true,
            ..Default::default()
        },
        "new-token",
    )
    .unwrap();

    clear_stale_daemon_state_if_matches(&DaemonRuntimeInfo {
        pid: 1234,
        token: Some("old-token".into()),
    });

    assert_eq!(read_daemon_runtime_file().unwrap(), current);
    assert!(daemon_command_dir().exists());

    match prev {
        Some(value) => unsafe { env::set_var(XDG_RUNTIME_DIR_ENV, value) },
        None => unsafe { env::remove_var(XDG_RUNTIME_DIR_ENV) },
    }
}

#[test]
fn take_daemon_toggle_request_round_trips_payload() {
    let _guard = ENV_MUTEX
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let tmp = crate::test_temp::tempdir().unwrap();
    let prev = env::var_os(XDG_RUNTIME_DIR_ENV);
    unsafe {
        env::set_var(XDG_RUNTIME_DIR_ENV, tmp.path());
    }

    let request = DaemonToggleRequest {
        mode: Some("whiteboard".into()),
        freeze: true,
        exit_after_capture: true,
        session_file: Some(PathBuf::from("/tmp/lecture.wayscriber-session")),
        ..Default::default()
    };
    write_daemon_toggle_request(&request, "daemon-token").unwrap();
    let batch = take_daemon_toggle_requests("daemon-token").unwrap();
    let requests = batch
        .commands
        .iter()
        .map(|command| command.request.clone())
        .collect::<Vec<_>>();
    assert_eq!(requests, vec![request]);
    assert!(batch.saw_command_files);
    assert_eq!(batch.commands.len(), 1);
    assert_eq!(batch.commands[0].daemon_token, "daemon-token");
    assert!(
        batch.commands[0]
            .request_path
            .starts_with(daemon_command_dir())
    );
    assert!(
        batch.commands[0]
            .response_path
            .starts_with(daemon_command_dir().join("responses"))
    );
    let batch = take_daemon_toggle_requests("daemon-token").unwrap();
    assert!(!batch.saw_command_files);
    assert!(batch.commands.is_empty());

    match prev {
        Some(value) => unsafe { env::set_var(XDG_RUNTIME_DIR_ENV, value) },
        None => unsafe { env::remove_var(XDG_RUNTIME_DIR_ENV) },
    }
}

#[test]
fn write_daemon_toggle_request_queues_multiple_files_without_leaking_temp_files() {
    let _guard = ENV_MUTEX
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let tmp = crate::test_temp::tempdir().unwrap();
    let prev = env::var_os(XDG_RUNTIME_DIR_ENV);
    unsafe {
        env::set_var(XDG_RUNTIME_DIR_ENV, tmp.path());
    }

    write_daemon_toggle_request(
        &DaemonToggleRequest {
            freeze: true,
            ..Default::default()
        },
        "daemon-token",
    )
    .unwrap();
    write_daemon_toggle_request(
        &DaemonToggleRequest {
            mode: Some("whiteboard".into()),
            ..Default::default()
        },
        "daemon-token",
    )
    .unwrap();

    let command_dir = daemon_command_dir();
    let entries = fs::read_dir(&command_dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    assert_eq!(entries.len(), 2);
    assert!(entries.iter().all(|name| name.ends_with(".json")));
    assert!(!entries.iter().any(|name| name.ends_with(".tmp")));

    match prev {
        Some(value) => unsafe { env::set_var(XDG_RUNTIME_DIR_ENV, value) },
        None => unsafe { env::remove_var(XDG_RUNTIME_DIR_ENV) },
    }
}

#[test]
fn take_daemon_toggle_request_drains_multiple_payloads_in_order() {
    let _guard = ENV_MUTEX
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let tmp = crate::test_temp::tempdir().unwrap();
    let prev = env::var_os(XDG_RUNTIME_DIR_ENV);
    unsafe {
        env::set_var(XDG_RUNTIME_DIR_ENV, tmp.path());
    }

    let first = DaemonToggleRequest {
        freeze: true,
        ..Default::default()
    };
    let second = DaemonToggleRequest {
        mode: Some("whiteboard".into()),
        ..Default::default()
    };
    write_daemon_toggle_request(&first, "daemon-token").unwrap();
    write_daemon_toggle_request(&second, "daemon-token").unwrap();

    let batch = take_daemon_toggle_requests("daemon-token").unwrap();
    let requests = batch
        .commands
        .into_iter()
        .map(|command| command.request)
        .collect::<Vec<_>>();
    assert_eq!(requests, vec![first, second]);
    assert!(batch.saw_command_files);
    assert!(
        daemon_command_dir()
            .read_dir()
            .map(|mut entries| entries.next().is_none())
            .unwrap_or(true)
    );

    match prev {
        Some(value) => unsafe { env::set_var(XDG_RUNTIME_DIR_ENV, value) },
        None => unsafe { env::remove_var(XDG_RUNTIME_DIR_ENV) },
    }
}

#[test]
fn take_daemon_toggle_request_ignores_mismatched_token() {
    let _guard = ENV_MUTEX
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let tmp = crate::test_temp::tempdir().unwrap();
    let prev = env::var_os(XDG_RUNTIME_DIR_ENV);
    unsafe {
        env::set_var(XDG_RUNTIME_DIR_ENV, tmp.path());
    }

    write_daemon_toggle_request(
        &DaemonToggleRequest {
            freeze: true,
            ..Default::default()
        },
        "other-daemon",
    )
    .unwrap();

    assert!(
        take_daemon_toggle_requests("daemon-token")
            .unwrap()
            .commands
            .is_empty()
    );

    match prev {
        Some(value) => unsafe { env::set_var(XDG_RUNTIME_DIR_ENV, value) },
        None => unsafe { env::remove_var(XDG_RUNTIME_DIR_ENV) },
    }
}

#[test]
fn take_daemon_toggle_request_ignores_stale_payload() {
    let _guard = ENV_MUTEX
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let tmp = crate::test_temp::tempdir().unwrap();
    let prev = env::var_os(XDG_RUNTIME_DIR_ENV);
    unsafe {
        env::set_var(XDG_RUNTIME_DIR_ENV, tmp.path());
    }

    let payload = serde_json::to_vec(&DaemonToggleEnvelope {
        daemon_token: "daemon-token".into(),
        requested_at_unix_ms: current_unix_millis().unwrap() - 60_000,
        canceled: false,
        request: DaemonToggleRequest {
            freeze: true,
            ..Default::default()
        },
    })
    .unwrap();
    fs::create_dir_all(daemon_command_dir()).unwrap();
    fs::write(daemon_command_dir().join("stale.json"), payload).unwrap();

    let batch = take_daemon_toggle_requests("daemon-token").unwrap();
    assert!(batch.saw_command_files);
    assert!(batch.commands.is_empty());

    match prev {
        Some(value) => unsafe { env::set_var(XDG_RUNTIME_DIR_ENV, value) },
        None => unsafe { env::remove_var(XDG_RUNTIME_DIR_ENV) },
    }
}

#[test]
fn take_daemon_toggle_request_ignores_canceled_payload_but_marks_typed_signal() {
    let _guard = ENV_MUTEX
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let tmp = crate::test_temp::tempdir().unwrap();
    let prev = env::var_os(XDG_RUNTIME_DIR_ENV);
    unsafe {
        env::set_var(XDG_RUNTIME_DIR_ENV, tmp.path());
    }

    let payload = serde_json::to_vec(&DaemonToggleEnvelope {
        daemon_token: "daemon-token".into(),
        requested_at_unix_ms: current_unix_millis().unwrap(),
        canceled: true,
        request: DaemonToggleRequest {
            freeze: true,
            ..Default::default()
        },
    })
    .unwrap();
    fs::create_dir_all(daemon_command_dir()).unwrap();
    fs::write(daemon_command_dir().join("canceled.json"), payload).unwrap();

    let batch = take_daemon_toggle_requests("daemon-token").unwrap();
    assert!(batch.saw_command_files);
    assert!(batch.commands.is_empty());

    match prev {
        Some(value) => unsafe { env::set_var(XDG_RUNTIME_DIR_ENV, value) },
        None => unsafe { env::remove_var(XDG_RUNTIME_DIR_ENV) },
    }
}
