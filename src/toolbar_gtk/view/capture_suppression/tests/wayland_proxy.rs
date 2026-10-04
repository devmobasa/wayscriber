//! Runs one GTK test behind a private headless Weston with one frame event withheld.
//!
//! All protocol messages and SCM_RIGHTS descriptors pass through unchanged, except
//! the selected popup callback and its `delete_id` event. The protocol tables
//! compiled into `wayland-client` and `wayland-protocols` provide opcodes and
//! new-object types; this fixture neither renders nor invents feedback.

mod protocol;
mod relay;
mod wire;

use std::fs::File;
use std::io::Read;
use std::os::unix::fs::FileTypeExt;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Output, Stdio};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// Names the control socket for the GTK test running behind the proxy.
pub(super) const CONTROL_ENV: &str = "WAYSCRIBER_GTK_WAYLAND_CONTROL";

const UPSTREAM_SOCKET: &str = "upstream";
const PROXY_SOCKET: &str = "proxy";
const CONTROL_SOCKET: &str = "control";
const WESTON_START_TIMEOUT: Duration = Duration::from_secs(10);
const WESTON_STOP_TIMEOUT: Duration = Duration::from_secs(3);
const TEST_TIMEOUT: Duration = Duration::from_secs(20);
const POLL_INTERVAL: Duration = Duration::from_millis(10);

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub(super) struct ProxyStatus {
    pub(super) held: bool,
    pub(super) popup_commits: u64,
    pub(super) popup_callbacks_delivered: u64,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub(super) enum ProxyResponse {
    Status(ProxyStatus),
    Error { error: String },
}

impl ProxyResponse {
    fn error(message: impl Into<String>) -> Self {
        Self::Error {
            error: message.into(),
        }
    }
}

/// Runs `test_name` from this test binary in a private D-Bus session whose
/// Wayland display is the proxy, and returns its captured output.
pub(super) fn run_in_private_wayland(child_env: &str, test_name: &str) -> Output {
    let runtime = tempfile::Builder::new()
        .prefix("wayscriber-popup-test-")
        .tempdir()
        .expect("private XDG_RUNTIME_DIR");
    let weston = Weston::start(runtime.path());
    let proxy = relay::Proxy::start(runtime.path(), weston.socket.clone(), protocol::schema())
        .expect("listen for proxied Wayland clients");

    let output = run_test_session(runtime.path(), child_env, test_name);

    // Stop relaying before the compositor, and remove the runtime directory last.
    drop(proxy);
    drop(weston);
    output
}

fn private_environment<'a>(
    command: &'a mut Command,
    runtime: &Path,
    display: &str,
) -> &'a mut Command {
    command
        .env("XDG_RUNTIME_DIR", runtime)
        .env("WAYLAND_DISPLAY", display)
        .env("GDK_BACKEND", "wayland")
        .env("GSK_RENDERER", "gl")
        .env("LIBGL_ALWAYS_SOFTWARE", "1")
        .env("GTK_A11Y", "test")
        .env("GDK_DEBUG", "no-portals")
        .env_remove("DISPLAY")
        .env_remove("WAYLAND_SOCKET")
        .env_remove("DBUS_SESSION_BUS_ADDRESS")
}

struct Weston {
    process: Child,
    socket: PathBuf,
}

impl Weston {
    fn start(runtime: &Path) -> Self {
        let log_path = runtime.join("weston.log");
        let log = File::create(&log_path).expect("create the Weston log");
        let mut command = Command::new("weston");
        command
            .args([
                "--backend=headless-backend.so",
                "--renderer=pixman",
                "--no-config",
            ])
            .arg(format!("--socket={UPSTREAM_SOCKET}"))
            .arg("--idle-time=0")
            .stdout(log.try_clone().expect("share the Weston log"))
            .stderr(log);
        private_environment(&mut command, runtime, UPSTREAM_SOCKET);

        let mut weston = Self {
            process: command.spawn().expect("start the private headless Weston"),
            socket: runtime.join(UPSTREAM_SOCKET),
        };
        weston.wait_for_socket(&log_path);
        weston
    }

    fn wait_for_socket(&mut self, log_path: &Path) {
        let deadline = Instant::now() + WESTON_START_TIMEOUT;
        while !is_socket(&self.socket) {
            let exited = self.process.try_wait().expect("poll Weston").is_some();
            if exited || Instant::now() >= deadline {
                let log = std::fs::read_to_string(log_path).unwrap_or_default();
                panic!("private Weston did not create its socket:\n{log}");
            }

            std::thread::sleep(POLL_INTERVAL);
        }
    }
}

impl Drop for Weston {
    fn drop(&mut self) {
        if matches!(self.process.try_wait(), Ok(None)) {
            // SAFETY: kill only signals the unreaped child this guard owns.
            unsafe { libc::kill(self.process.id() as libc::pid_t, libc::SIGTERM) };
        }
        if !matches!(
            wait_until(&mut self.process, WESTON_STOP_TIMEOUT),
            Ok(Some(_))
        ) {
            let _ = self.process.kill();
        }
        let _ = self.process.wait();
    }
}

fn is_socket(path: &Path) -> bool {
    std::fs::metadata(path).is_ok_and(|metadata| metadata.file_type().is_socket())
}

fn run_test_session(runtime: &Path, child_env: &str, test_name: &str) -> Output {
    let mut command = Command::new("dbus-run-session");
    command
        .arg("--")
        .arg(std::env::current_exe().expect("test binary"))
        .args([test_name, "--exact", "--test-threads=1", "--nocapture"]);
    private_environment(&mut command, runtime, PROXY_SOCKET)
        .env(child_env, "1")
        .env("G_DEBUG", "fatal-criticals")
        .env(CONTROL_ENV, runtime.join(CONTROL_SOCKET))
        // GIO activates gvfsd on the private bus. Its FUSE helper would mount
        // inside the runtime directory, and the final SIGKILL can strand that mount.
        .env("GVFS_DISABLE_FUSE", "1")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    // SAFETY: setsid is async-signal-safe and touches no parent memory.
    unsafe { command.pre_exec(start_session) };

    let mut session = TestSession(Some(command.spawn().expect("start dbus-run-session")));
    let leader = session.leader();
    let stdout = drain(leader.stdout.take());
    let stderr = drain(leader.stderr.take());
    let exited = wait_until(leader, TEST_TIMEOUT).expect("poll the private GTK test session");
    if exited.is_none() {
        eprintln!("private GTK test exceeded {TEST_TIMEOUT:?}; killing its session");
    }
    let status = session.finish();

    Output {
        status,
        stdout: stdout.join().expect("collect test stdout"),
        stderr: stderr.join().expect("collect test stderr"),
    }
}

fn start_session() -> std::io::Result<()> {
    // SAFETY: setsid has no memory-safety preconditions.
    if unsafe { libc::setsid() } < 0 {
        return Err(std::io::Error::last_os_error());
    }

    Ok(())
}

/// Owns the session leader so the whole session is killed however the run ends.
struct TestSession(Option<Child>);

impl TestSession {
    fn leader(&mut self) -> &mut Child {
        self.0.as_mut().expect("session leader is still owned")
    }

    fn finish(mut self) -> ExitStatus {
        let mut leader = self.0.take().expect("session leader is still owned");
        kill_session(&leader);
        leader.wait().expect("reap the private GTK test session")
    }
}

impl Drop for TestSession {
    fn drop(&mut self) {
        if let Some(mut leader) = self.0.take() {
            kill_session(&leader);
            let _ = leader.wait();
        }
    }
}

/// Kills the whole session, including the private bus and any services it activated.
fn kill_session(leader: &Child) {
    // SAFETY: killpg only sends a signal to the group this leader created.
    unsafe { libc::killpg(leader.id() as libc::pid_t, libc::SIGKILL) };
}

fn drain(pipe: Option<impl Read + Send + 'static>) -> JoinHandle<Vec<u8>> {
    let mut pipe = pipe.expect("piped test output");
    std::thread::spawn(move || {
        let mut bytes = Vec::new();
        pipe.read_to_end(&mut bytes).expect("read test output");
        bytes
    })
}

fn wait_until(process: &mut Child, timeout: Duration) -> std::io::Result<Option<ExitStatus>> {
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(status) = process.try_wait()? {
            return Ok(Some(status));
        }
        if Instant::now() >= deadline {
            return Ok(None);
        }

        std::thread::sleep(POLL_INTERVAL);
    }
}
