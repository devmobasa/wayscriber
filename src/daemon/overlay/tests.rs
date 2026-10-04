use super::*;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::time::Instant;

mod fake_overlay;

pub(in crate::daemon) fn with_visible_overlay(
    named_file: Option<PathBuf>,
    ignore_term: bool,
    body: impl FnOnce(&mut Daemon),
) {
    with_fixture(ignore_term, |daemon, candidate, _| {
        daemon.queue_overlay_launch(
            named_file.map(|session_file| DaemonToggleRequest {
                session_file: Some(session_file),
                ..Default::default()
            }),
            None,
        );
        let request = daemon.take_pending_launch();
        daemon
            .start_launch(request, &[candidate])
            .unwrap()
            .require_shown()
            .unwrap();

        body(daemon);

        daemon.hide_overlay().unwrap();
    });
}

/// Like [`with_visible_overlay`], for an overlay that reports `session`, a
/// path or `"home"`, once shown.
pub(in crate::daemon) fn with_reporting_overlay(session: &str, body: impl FnOnce(&mut Daemon)) {
    with_fixture(false, |daemon, _, root| {
        instruct(root, Some(session), false);
        show(daemon, root);

        body(daemon);

        daemon.hide_overlay().unwrap();
    });
}

pub(in crate::daemon) fn assert_token_only_pending_launch(daemon: &Daemon, token: &str) {
    let retained = daemon.pending_launch.as_ref().expect("token must be kept");

    assert_eq!(retained.activation_token(), Some(token));
    assert_eq!(retained.mode(), Some("transparent"));
    assert_eq!(retained.session_resume_override(Some(true)), Some(true));
}

/// Runs `body` with a broker whose overlay launches of this test binary act
/// as [`fake_overlay`] children. Receipts land in the runtime root passed to `body`.
fn with_fixture(ignore_term: bool, body: impl FnOnce(&mut Daemon, OverlaySpawnCandidate, &Path)) {
    let temp = crate::test_temp::tempdir().unwrap();
    let behavior = if ignore_term {
        fake_overlay::IGNORES_TERM
    } else {
        fake_overlay::STOPS_ON_TERM
    };

    crate::test_env::with_env_vars(
        &[
            (
                crate::env_vars::XDG_RUNTIME_DIR_ENV,
                Some(temp.path().as_os_str()),
            ),
            (fake_overlay::FAKE_OVERLAY_ENV, Some(OsStr::new(behavior))),
        ],
        || {
            let _broker = crate::process_broker::start_for_runtime().unwrap();
            let mut daemon = Daemon::new(
                Some("transparent".into()),
                false,
                Some(true),
                Some(PathBuf::from("/tmp/home.wayscriber-session")),
            );

            daemon.instance_token = crate::daemon::protocol_v2::ProtocolToken::generate()
                .unwrap()
                .to_string();
            let candidate = OverlaySpawnCandidate {
                program: std::env::current_exe().unwrap().into_os_string(),
                source: "test binary",
            };

            body(&mut daemon, candidate, temp.path());
        },
    );
}

fn receipt(root: &Path) -> serde_json::Value {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        if let Some(path) = fs::read_dir(root)
            .unwrap()
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .find(|path| path.extension().is_some_and(|ext| ext == "receipt"))
        {
            return serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
        }
        assert!(
            Instant::now() < deadline,
            "owned overlay did not publish its launch receipt"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn assert_retired(daemon: &Daemon, generation: &str) {
    assert_eq!(daemon.overlay.state(), OverlayState::Hidden);
    assert!(!daemon.overlay.active_flag().load(Ordering::Acquire));
    assert!(daemon.overlay.active_named_session_file().is_none());
    assert!(daemon.overlay.poll_fd().is_none());
    let proofs = crate::daemon::protocol_v2::command_root().join("children");
    for suffix in ["active", "enabled", "ready", "signals"] {
        assert!(!proofs.join(format!("{generation}.{suffix}")).exists());
    }
    assert!(!session_report(generation).exists());
}

fn session_report(generation: &str) -> PathBuf {
    crate::paths::daemon_command_dir()
        .join("overlay-targets")
        .join(format!("{generation}.target"))
}

/// Has the next fake overlay report `report`, `"home"` or a path, and exit
/// afterwards when `exit` is set.
fn instruct(root: &Path, report: Option<&str>, exit: bool) {
    fs::write(
        root.join(fake_overlay::SESSION_INSTRUCTION),
        serde_json::json!({ "report": report, "exit": exit }).to_string(),
    )
    .unwrap();
}

/// Shows the overlay and returns its launch receipt, removed so the next
/// show's can be told apart.
fn show(daemon: &mut Daemon, root: &Path) -> serde_json::Value {
    daemon.show_overlay().unwrap().require_shown().unwrap();
    let receipt = receipt(root);
    fs::remove_file(root.join(format!(
        "{}.receipt",
        receipt["generation"].as_str().unwrap()
    )))
    .unwrap();
    receipt
}

fn wait_until_retired(daemon: &mut Daemon) {
    let deadline = Instant::now() + Duration::from_secs(3);
    while daemon.overlay.state() == OverlayState::Visible {
        daemon.update_overlay_process_state().unwrap();
        assert!(Instant::now() < deadline, "exited child was not retired");
        std::thread::sleep(Duration::from_millis(5));
    }
}

const HOME: &str = "/tmp/home.wayscriber-session";
const REMEMBERED: &str = "/tmp/remembered.wayscriber-session";

#[test]
fn the_next_show_continues_the_session_the_overlay_reported() {
    with_fixture(false, |daemon, _, root| {
        instruct(root, Some(REMEMBERED), false);
        let first = show(daemon, root);
        assert_eq!(first["reports"], "1");
        assert_eq!(first["home"], HOME);
        assert!(first["preferred"].is_null());
        let generation = first["generation"].as_str().unwrap();
        assert!(session_report(generation).exists());

        daemon.hide_overlay().unwrap();
        assert_retired(daemon, generation);
        assert_eq!(
            daemon.remembered_session_file.as_deref(),
            Some(Path::new(REMEMBERED))
        );

        // An older overlay still starts at home; a current one continues.
        instruct(root, Some("home"), false);
        let second = show(daemon, root);
        assert_eq!(
            second["args"],
            serde_json::json!(["--active", "--mode", "transparent", "--session-file", HOME])
        );
        assert_eq!(second["home"], HOME);
        assert_eq!(second["preferred"], REMEMBERED);

        // Back home, the overlay exits on its own and the daemon forgets.
        daemon.overlay.signal(libc::SIGTERM).unwrap();
        wait_until_retired(daemon);
        assert_retired(daemon, second["generation"].as_str().unwrap());
        assert_eq!(daemon.remembered_session_file, None);

        instruct(root, None, false);
        let third = show(daemon, root);
        assert!(third["preferred"].is_null());
        daemon.hide_overlay().unwrap();
    });
}

#[test]
fn an_overlay_that_reports_and_exits_at_once_is_still_heard() {
    with_fixture(false, |daemon, _, root| {
        instruct(root, Some(REMEMBERED), true);
        let receipt = show(daemon, root);

        wait_until_retired(daemon);

        assert_retired(daemon, receipt["generation"].as_str().unwrap());
        assert_eq!(
            daemon.remembered_session_file.as_deref(),
            Some(Path::new(REMEMBERED))
        );
    });
}

#[test]
fn a_forced_stop_still_reads_the_last_report() {
    with_fixture(true, |daemon, _, root| {
        instruct(root, Some(REMEMBERED), false);
        let receipt = show(daemon, root);

        daemon.hide_overlay().unwrap();

        assert_retired(daemon, receipt["generation"].as_str().unwrap());
        assert_eq!(
            daemon.remembered_session_file.as_deref(),
            Some(Path::new(REMEMBERED))
        );
    });
}

#[test]
fn a_requested_session_file_outranks_the_remembered_one_for_that_show() {
    with_fixture(false, |daemon, _, root| {
        daemon.remembered_session_file = Some(PathBuf::from(REMEMBERED));
        let requested = "/tmp/requested.wayscriber-session";
        for (session_file, remembered_after) in [(requested, Some(REMEMBERED)), (HOME, None)] {
            daemon.queue_overlay_launch(
                Some(DaemonToggleRequest {
                    session_file: Some(PathBuf::from(session_file)),
                    ..Default::default()
                }),
                None,
            );

            let receipt = show(daemon, root);

            assert_eq!(receipt["args"][4], session_file);
            assert!(receipt["preferred"].is_null(), "{session_file}");
            // A request for home returns home: nothing remains to continue.
            assert_eq!(
                daemon.remembered_session_file.as_deref(),
                remembered_after.map(Path::new),
                "{session_file}"
            );
            daemon.hide_overlay().unwrap();
        }
    });
}

#[test]
fn real_child_hide_restart_and_natural_retirement_clear_target_and_visibility() {
    with_fixture(false, |daemon, _, root| {
        daemon.queue_overlay_launch(
            Some(DaemonToggleRequest {
                mode: Some("whiteboard".into()),
                session_file: Some(PathBuf::from("/tmp/away.wayscriber-session")),
                freeze: true,
                no_exit_after_capture: true,
                no_resume_session: true,
                ..Default::default()
            }),
            Some("owned-token".into()),
        );
        daemon.show_overlay().unwrap().require_shown().unwrap();
        let first = receipt(root);

        assert_eq!(
            first["args"],
            serde_json::json!([
                "--active",
                "--freeze",
                "--no-exit-after-capture",
                "--mode",
                "whiteboard",
                "--session-file",
                "/tmp/away.wayscriber-session"
            ])
        );
        assert_eq!(first["token"], "owned-token");
        assert_eq!(first["startup"], "owned-token");
        assert_eq!(first["resume"], "off");
        assert_eq!(first["detach"], "1");
        assert!(daemon.overlay.active_flag().load(Ordering::Acquire));
        assert_eq!(
            daemon.overlay.active_named_session_file(),
            Some(Path::new("/tmp/away.wayscriber-session"))
        );
        assert!(daemon.pending_launch.is_none());

        daemon.queue_overlay_launch(
            Some(DaemonToggleRequest {
                mode: Some("blackboard".into()),
                freeze: true,
                no_resume_session: true,
                ..Default::default()
            }),
            Some("hide-retained-token".into()),
        );

        daemon.hide_overlay().unwrap();
        assert_retired(daemon, first["generation"].as_str().unwrap());
        fs::remove_file(root.join(format!("{}.receipt", first["generation"].as_str().unwrap())))
            .unwrap();
        daemon.show_overlay().unwrap().require_shown().unwrap();
        let second = receipt(root);

        assert_ne!(first["generation"], second["generation"]);
        assert_eq!(
            second["args"],
            serde_json::json!([
                "--active",
                "--mode",
                "transparent",
                "--session-file",
                "/tmp/home.wayscriber-session"
            ])
        );
        assert_eq!(second["token"], "hide-retained-token");
        assert_eq!(second["startup"], "hide-retained-token");
        assert_eq!(second["resume"], "on");
        assert!(daemon.pending_launch.is_none());
        assert_eq!(
            daemon.overlay.active_named_session_file(),
            Some(Path::new("/tmp/home.wayscriber-session"))
        );

        daemon.overlay.signal(libc::SIGTERM).unwrap();
        let deadline = Instant::now() + Duration::from_secs(3);
        while daemon.overlay.state() == OverlayState::Visible {
            daemon.update_overlay_process_state().unwrap();
            assert!(Instant::now() < deadline, "exited child was not retired");
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_retired(daemon, second["generation"].as_str().unwrap());
    });
}

#[test]
fn real_spawn_failure_drops_options_and_token_before_retry() {
    with_fixture(false, |daemon, candidate, root| {
        // A real child that starts and exits before publishing readiness.
        let failed_program = root.join(fake_overlay::EXITS_BEFORE_READY);
        std::os::unix::fs::symlink(&candidate.program, &failed_program).unwrap();
        daemon.queue_overlay_launch(
            Some(DaemonToggleRequest {
                mode: Some("whiteboard".into()),
                no_resume_session: true,
                session_file: Some(PathBuf::from("/tmp/rejected.wayscriber-session")),
                ..Default::default()
            }),
            Some("rejected-token".into()),
        );
        let request = daemon.take_pending_launch();

        let failure = daemon
            .start_launch(
                request,
                &[OverlaySpawnCandidate {
                    program: failed_program.into_os_string(),
                    source: "exits before readiness",
                }],
            )
            .unwrap_err();

        // Callers report the summary alone, even with `{:#}`; the attempts stay
        // on the error for inspection.
        assert_eq!(
            format!("{failure:#}"),
            format!(
                "Unable to launch overlay process (tried current_exe/argv0/{})",
                crate::env_vars::PATH_ENV
            )
        );
        let attempts = failure
            .downcast_ref::<super::spawn::SpawnAttemptsFailed>()
            .expect("every candidate failed")
            .attempts();
        assert!(
            attempts.iter().any(|attempt| {
                attempt.contains("overlay child exited before publishing readiness")
            }),
            "{attempts:?}"
        );
        assert!(
            fs::read_dir(root)
                .unwrap()
                .filter_map(Result::ok)
                .any(|entry| {
                    entry
                        .path()
                        .extension()
                        .is_some_and(|extension| extension == fake_overlay::STARTED_THEN_EXITED)
                }),
            "the failing candidate never started"
        );
        assert!(daemon.pending_launch.is_none());
        assert_eq!(daemon.overlay.state(), OverlayState::Hidden);
        assert!(!daemon.overlay.active_flag().load(Ordering::Acquire));
        assert!(daemon.overlay.poll_fd().is_none());
        let deadline = Instant::now() + Duration::from_secs(3);
        while daemon.overlay_start_backoff().is_some() {
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(10));
        }

        let request = daemon.take_pending_launch();
        daemon
            .start_launch(request, &[candidate])
            .unwrap()
            .require_shown()
            .unwrap();
        let actual = receipt(root);

        assert_eq!(
            actual["args"],
            serde_json::json!([
                "--active",
                "--mode",
                "transparent",
                "--session-file",
                "/tmp/home.wayscriber-session"
            ])
        );
        assert!(actual["token"].is_null());
        assert!(actual["startup"].is_null());
        assert_eq!(actual["resume"], "on");
        daemon.hide_overlay().unwrap();
        assert_retired(daemon, actual["generation"].as_str().unwrap());
    });
}

#[test]
fn preparation_error_keeps_only_the_token_and_does_not_replay_options() {
    with_fixture(false, |daemon, candidate, _| {
        let commands = crate::daemon::protocol_v2::command_root();
        fs::create_dir_all(&commands).unwrap();
        fs::write(commands.join("children"), b"not a proof directory").unwrap();
        daemon.queue_overlay_launch(
            Some(DaemonToggleRequest {
                mode: Some("whiteboard".into()),
                no_resume_session: true,
                ..Default::default()
            }),
            Some("retained-token".into()),
        );
        let request = daemon.take_pending_launch();

        assert!(daemon.start_launch(request, &[candidate]).is_err());
        let retained = daemon.take_pending_launch();
        let actual = launch::build_overlay_launch(&retained, Some(true), None, None);

        assert_eq!(
            actual.arguments,
            [
                "--active",
                "--mode",
                "transparent",
                "--session-file",
                "/tmp/home.wayscriber-session"
            ]
            .map(std::ffi::OsString::from)
        );
        assert!(actual.environment.contains(&(
            crate::env_vars::XDG_ACTIVATION_TOKEN_ENV.into(),
            Some("retained-token".into())
        )));
        assert!(
            actual
                .environment
                .contains(&(crate::RESUME_SESSION_ENV.into(), Some("on".into())))
        );
    });
}
