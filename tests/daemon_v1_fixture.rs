use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

const FROZEN_V1_SOURCE: &str = include_str!("fixtures/frozen_daemon_v1.rs");
static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);

struct TempDir(PathBuf);

impl TempDir {
    fn new() -> std::io::Result<Self> {
        for _ in 0..100 {
            let id = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "wayscriber-daemon-v1-fixture-{}-{id}",
                std::process::id()
            ));
            match fs::create_dir(&path) {
                Ok(()) => return Ok(Self(path)),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error),
            }
        }
        Err(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            "failed to create a unique fixture directory",
        ))
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

struct ChildGuard(Child);

impl ChildGuard {
    fn child_mut(&mut self) -> &mut Child {
        &mut self.0
    }
}

impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn compile_fixture(root: &Path) -> PathBuf {
    let source = root.join("frozen_daemon_v1.rs");
    let binary = root.join("frozen-daemon-v1");
    fs::write(&source, FROZEN_V1_SOURCE).unwrap();
    let rustc = std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
    let output = Command::new(rustc)
        .arg("--edition=2024")
        .arg("-C")
        .arg("debuginfo=0")
        .arg("-o")
        .arg(&binary)
        .arg(&source)
        .output()
        .expect("compile frozen v1 fixture");
    assert!(
        output.status.success(),
        "fixture compilation failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    binary
}

fn wait_for(path: &Path) {
    let deadline = Instant::now() + Duration::from_secs(2);
    while !path.exists() {
        assert!(
            Instant::now() < deadline,
            "timed out waiting for {}",
            path.display()
        );
        std::thread::sleep(Duration::from_millis(2));
    }
}

fn read_report(root: &Path) -> (usize, usize) {
    wait_for(&root.join("fixture-report"));
    let report = fs::read_to_string(root.join("fixture-report")).unwrap();
    let value = |name: &str| {
        report
            .lines()
            .find_map(|line| line.strip_prefix(&format!("{name}=")))
            .unwrap()
            .parse::<usize>()
            .unwrap()
    };
    (value("signals"), value("typed"))
}

fn wait_for_report(root: &Path, expected_signals: usize, expected_typed: usize) {
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        if read_report(root) == (expected_signals, expected_typed) {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "fixture report did not reach expected counts"
        );
        std::thread::sleep(Duration::from_millis(2));
    }
}

fn run_client(binary: &Path, mode: &str, root: &Path) -> Output {
    Command::new(binary)
        .arg(mode)
        .arg(root)
        .output()
        .expect("run frozen v1 client")
}

fn stop_daemon(daemon: &mut Child) {
    daemon.kill().unwrap();
    daemon.wait().unwrap();
}

#[test]
fn frozen_v1_client_preserves_request_signal_and_response_behavior() {
    let temp = TempDir::new().unwrap();
    let binary = compile_fixture(temp.path());
    let runtime = temp.path().join("runtime-v1");
    let mut daemon = ChildGuard(
        Command::new(&binary)
            .arg("daemon-v1")
            .arg(&runtime)
            .spawn()
            .unwrap(),
    );
    wait_for(&runtime.join("wayscriber.pid"));
    assert_eq!(
        fs::read_to_string(runtime.join("wayscriber.pid")).unwrap(),
        format!(
            "{{\"pid\":{},\"token\":\"frozen-v1-token\"}}",
            daemon.child_mut().id()
        )
    );

    let modes = [
        "client-freeze",
        "client-exit-after-capture",
        "client-no-exit-after-capture",
        "client-resume-session",
        "client-no-resume-session",
    ];
    for (index, mode) in modes.into_iter().enumerate() {
        let typed = run_client(&binary, mode, &runtime);
        assert!(
            typed.status.success(),
            "{mode}: {}",
            String::from_utf8_lossy(&typed.stderr)
        );
        wait_for_report(&runtime, index + 1, index + 1);
    }

    let empty = run_client(&binary, "client-empty", &runtime);
    assert!(
        empty.status.success(),
        "{}",
        String::from_utf8_lossy(&empty.stderr)
    );
    wait_for_report(&runtime, 6, 5);

    let requests = fs::read_to_string(runtime.join("fixture-requests")).unwrap();
    let request_bodies = requests
        .lines()
        .map(|line| line.split_once(' ').unwrap().1)
        .collect::<Vec<_>>();
    assert_eq!(
        request_bodies,
        [
            r#"{"daemon_token":"frozen-v1-token","requested_at_unix_ms":0,"canceled":false,"request":{"freeze":true,"exit_after_capture":false,"no_exit_after_capture":false,"resume_session":false,"no_resume_session":false}}"#,
            r#"{"daemon_token":"frozen-v1-token","requested_at_unix_ms":0,"canceled":false,"request":{"freeze":false,"exit_after_capture":true,"no_exit_after_capture":false,"resume_session":false,"no_resume_session":false}}"#,
            r#"{"daemon_token":"frozen-v1-token","requested_at_unix_ms":0,"canceled":false,"request":{"freeze":false,"exit_after_capture":false,"no_exit_after_capture":true,"resume_session":false,"no_resume_session":false}}"#,
            r#"{"daemon_token":"frozen-v1-token","requested_at_unix_ms":0,"canceled":false,"request":{"freeze":false,"exit_after_capture":false,"no_exit_after_capture":false,"resume_session":true,"no_resume_session":false}}"#,
            r#"{"daemon_token":"frozen-v1-token","requested_at_unix_ms":0,"canceled":false,"request":{"freeze":false,"exit_after_capture":false,"no_exit_after_capture":false,"resume_session":false,"no_resume_session":true}}"#,
        ]
    );

    stop_daemon(daemon.child_mut());
}

#[test]
fn frozen_v1_client_fails_closed_for_nonempty_v2_request_but_keeps_empty_signal() {
    let temp = TempDir::new().unwrap();
    let binary = compile_fixture(temp.path());
    let runtime = temp.path().join("runtime-v2");
    let mut daemon = ChildGuard(
        Command::new(&binary)
            .arg("daemon-v2")
            .arg(&runtime)
            .spawn()
            .unwrap(),
    );
    wait_for(&runtime.join("wayscriber.pid"));

    let typed = run_client(&binary, "client-freeze", &runtime);
    assert!(!typed.status.success());
    assert!(
        String::from_utf8_lossy(&typed.stderr)
            .contains("running daemon does not support typed control")
    );
    assert!(!runtime.join("daemon-commands").exists());
    wait_for_report(&runtime, 0, 0);

    let empty = run_client(&binary, "client-empty", &runtime);
    assert!(
        empty.status.success(),
        "{}",
        String::from_utf8_lossy(&empty.stderr)
    );
    wait_for_report(&runtime, 1, 0);

    stop_daemon(daemon.child_mut());
}

#[test]
fn frozen_v1_daemon_ignores_unpublished_temporary_request_files() {
    let temp = TempDir::new().unwrap();
    let binary = compile_fixture(temp.path());
    let runtime = temp.path().join("runtime-v1-temp-request");
    let command_dir = runtime.join("daemon-commands");
    fs::create_dir_all(&command_dir).unwrap();
    let unpublished = command_dir.join("00000000000000000000000000000000-00000000.tmp-1234");
    fs::write(&unpublished, br#"{"daemon_token":"frozen-v1-token"}"#).unwrap();

    let mut daemon = ChildGuard(
        Command::new(&binary)
            .arg("daemon-v1")
            .arg(&runtime)
            .spawn()
            .unwrap(),
    );
    wait_for(&runtime.join("wayscriber.pid"));
    wait_for_report(&runtime, 0, 0);

    assert!(unpublished.exists());
    stop_daemon(daemon.child_mut());
}
