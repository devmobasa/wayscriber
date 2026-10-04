use super::*;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::time::Instant;

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

fn with_fixture(ignore_term: bool, body: impl FnOnce(&mut Daemon, OverlaySpawnCandidate, &Path)) {
    let temp = crate::test_temp::tempdir().unwrap();
    crate::test_env::with_env_var(
        crate::env_vars::XDG_RUNTIME_DIR_ENV,
        Some(temp.path().as_os_str()),
        || {
            let _broker = crate::process_broker::start_for_runtime().unwrap();
            let program = temp.path().join("wayscriber-overlay-fixture");
            let proof_dir = crate::daemon::protocol_v2::command_root().join("children");
            let script = format!(
                r#"#!/usr/bin/python3
import json, os, pathlib, signal, sys, time
root = pathlib.Path({root})
proof = pathlib.Path({proof})
proof.mkdir(parents=True, exist_ok=True, mode=0o700)
generation = os.environ["WAYSCRIBER_OVERLAY_CHILD_GENERATION"]
record = {{"protocol_version": {version}, "generation": generation, "pid": os.getpid(), "process_start_ticks": int(pathlib.Path("/proc/self/stat").read_text().split(") ", 1)[1].split()[19])}}
def publish(path, data):
    temp = path.with_suffix(path.suffix + ".tmp")
    temp.write_text(json.dumps(data, separators=(",", ":")))
    temp.chmod(0o600)
    temp.replace(path)
if {ignore_term}:
    signal.signal(signal.SIGTERM, signal.SIG_IGN)
for suffix in ("active", "signals", "ready"):
    publish(proof / (generation + "." + suffix), record)
while not (proof / (generation + ".enabled")).exists():
    time.sleep(0.002)
publish(root / (generation + ".receipt"), {{"args": sys.argv[1:], "token": os.environ.get("XDG_ACTIVATION_TOKEN"), "startup": os.environ.get("DESKTOP_STARTUP_ID"), "resume": os.environ.get("WAYSCRIBER_RESUME_SESSION"), "detach": os.environ.get("WAYSCRIBER_NO_DETACH"), "pid": os.getpid(), "generation": generation}})
while True:
    time.sleep(0.01)
"#,
                root = serde_json::to_string(&temp.path().to_string_lossy()).unwrap(),
                proof = serde_json::to_string(&proof_dir.to_string_lossy()).unwrap(),
                version = 2,
                ignore_term = if ignore_term { "True" } else { "False" }
            );
            fs::write(&program, script).unwrap();
            fs::set_permissions(&program, fs::Permissions::from_mode(0o700)).unwrap();
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
                program: program.into_os_string(),
                source: "fixture",
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
}

#[test]
fn real_child_hide_restart_and_natural_retirement_clear_target_and_visibility() {
    with_fixture(false, |daemon, candidate, root| {
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
        let request = daemon.take_pending_launch();

        daemon
            .start_launch(request, std::slice::from_ref(&candidate))
            .unwrap()
            .require_shown()
            .unwrap();
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

        daemon.hide_overlay().unwrap();
        assert_retired(daemon, first["generation"].as_str().unwrap());
        fs::remove_file(root.join(format!("{}.receipt", first["generation"].as_str().unwrap())))
            .unwrap();
        let request = daemon.take_pending_launch();
        daemon
            .start_launch(request, &[candidate])
            .unwrap()
            .require_shown()
            .unwrap();
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
        assert!(second["token"].is_null());
        assert_eq!(second["resume"], "on");
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
        let failed_program = root.join("wayscriber-failing-fixture");
        fs::write(&failed_program, "#!/bin/sh\nexit 7\n").unwrap();
        fs::set_permissions(&failed_program, fs::Permissions::from_mode(0o700)).unwrap();
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
                    source: "failed fixture",
                }],
            )
            .unwrap_err();

        assert!(
            failure
                .to_string()
                .contains("Unable to launch overlay process")
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
        let actual = launch::build_overlay_launch(&retained, Some(true), None);

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
