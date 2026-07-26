#![cfg(unix)]

use std::ffi::OsString;
use std::fs;
use std::io;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::os::unix::fs::symlink;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const BROKER_FD: RawFd = 3;
const BROKER_SHUTDOWN_FD: RawFd = 4;
const BROKER_FD_ENV: &str = "WAYSCRIBER_INTERNAL_PROCESS_BROKER_FD";
const BROKER_SHUTDOWN_FD_ENV: &str = "WAYSCRIBER_INTERNAL_PROCESS_BROKER_SHUTDOWN_FD";
const BROKER_TOKEN_ENV: &str = "WAYSCRIBER_INTERNAL_PROCESS_BROKER_TOKEN";
const CANONICAL_TOKEN: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const PING_REQUEST_ID: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const CHILD_TIMEOUT: Duration = Duration::from_secs(2);

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

struct TestDirectory {
    path: PathBuf,
}

impl TestDirectory {
    fn create(label: &str) -> io::Result<Self> {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        for attempt in 0..100 {
            let path = std::env::temp_dir().join(format!(
                "wayscriber-broker-bootstrap-{}-{nonce}-{label}-{attempt}",
                std::process::id()
            ));
            match fs::create_dir(&path) {
                Ok(()) => return Ok(Self { path }),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error),
            }
        }
        Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "could not create a unique broker-bootstrap test directory",
        ))
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

struct CapturedExit {
    status: ExitStatus,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

struct BoundedChild {
    child: Child,
    stdout_path: PathBuf,
    stderr_path: PathBuf,
}

impl BoundedChild {
    fn spawn(command: &mut Command, directory: &TestDirectory) -> io::Result<Self> {
        let stdout_path = directory.path().join("child.stdout");
        let stderr_path = directory.path().join("child.stderr");
        let stdout = fs::File::create(&stdout_path)?;
        let stderr = fs::File::create(&stderr_path)?;
        let child = command
            .stdout(Stdio::from(stdout))
            .stderr(Stdio::from(stderr))
            .spawn()?;
        Ok(Self {
            child,
            stdout_path,
            stderr_path,
        })
    }

    fn wait(&mut self, timeout: Duration) -> io::Result<CapturedExit> {
        let deadline = Instant::now() + timeout;
        loop {
            match self.child.try_wait() {
                Ok(Some(status)) => {
                    return Ok(CapturedExit {
                        status,
                        stdout: fs::read(&self.stdout_path)?,
                        stderr: fs::read(&self.stderr_path)?,
                    });
                }
                Ok(None) if Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(5));
                }
                Ok(None) => {
                    let _ = self.child.kill();
                    let _ = self.child.wait();
                    return Err(io::Error::new(
                        io::ErrorKind::TimedOut,
                        format!("broker-bootstrap child exceeded {timeout:?}"),
                    ));
                }
                Err(error) => {
                    let _ = self.child.kill();
                    let _ = self.child.wait();
                    return Err(error);
                }
            }
        }
    }
}

impl Drop for BoundedChild {
    fn drop(&mut self) {
        match self.child.try_wait() {
            Ok(Some(_)) => {}
            Ok(None) | Err(_) => {
                let _ = self.child.kill();
            }
        }
        let _ = self.child.wait();
    }
}

#[derive(Default)]
struct Markers {
    descriptor: Option<OsString>,
    shutdown: Option<OsString>,
    token: Option<OsString>,
}

impl Markers {
    fn complete() -> Self {
        Self {
            descriptor: Some(OsString::from(BROKER_FD.to_string())),
            shutdown: Some(OsString::from(BROKER_SHUTDOWN_FD.to_string())),
            token: Some(OsString::from(CANONICAL_TOKEN)),
        }
    }

    fn apply(self, command: &mut Command) {
        if let Some(value) = self.descriptor {
            command.env(BROKER_FD_ENV, value);
        }
        if let Some(value) = self.shutdown {
            command.env(BROKER_SHUTDOWN_FD_ENV, value);
        }
        if let Some(value) = self.token {
            command.env(BROKER_TOKEN_ENV, value);
        }
    }
}

struct StagedDescriptors {
    _control: OwnedFd,
    _shutdown: OwnedFd,
}

fn broker_command(directory: &TestDirectory) -> io::Result<Command> {
    let unusable_root = directory.path().join("unusable-root");
    fs::write(&unusable_root, b"not a directory")?;
    let unusable_child = unusable_root.join("child");

    let mut command = Command::new(env!("CARGO_BIN_EXE_wayscriber"));
    command
        .env_clear()
        .current_dir(directory.path())
        .env("HOME", &unusable_child)
        .env("XDG_CONFIG_HOME", &unusable_child)
        .env("XDG_CACHE_HOME", &unusable_child)
        .env("XDG_DATA_HOME", &unusable_child)
        .env("XDG_RUNTIME_DIR", &unusable_child)
        .env("XDG_PICTURES_DIR", &unusable_child)
        .env(
            "WAYSCRIBER_LOG_FILE",
            directory.path().join("wayscriber.log"),
        );
    Ok(command)
}

fn socket_pair(socket_type: i32) -> io::Result<(OwnedFd, OwnedFd)> {
    let mut descriptors = [0; 2];
    // SAFETY: descriptors has space for both fresh socket descriptors.
    if unsafe {
        libc::socketpair(
            libc::AF_UNIX,
            socket_type | libc::SOCK_CLOEXEC,
            0,
            descriptors.as_mut_ptr(),
        )
    } != 0
    {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: socketpair returned two fresh owned descriptors.
    Ok(unsafe {
        (
            OwnedFd::from_raw_fd(descriptors[0]),
            OwnedFd::from_raw_fd(descriptors[1]),
        )
    })
}

fn duplicate_for_child(descriptor: &OwnedFd) -> io::Result<OwnedFd> {
    // SAFETY: F_DUPFD_CLOEXEC duplicates the live test descriptor at or above five.
    let duplicate = unsafe { libc::fcntl(descriptor.as_raw_fd(), libc::F_DUPFD_CLOEXEC, 5) };
    if duplicate < 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: fcntl returned a fresh owned descriptor.
    Ok(unsafe { OwnedFd::from_raw_fd(duplicate) })
}

fn stage_broker_descriptors(
    command: &mut Command,
    control: &OwnedFd,
    shutdown: &OwnedFd,
) -> io::Result<StagedDescriptors> {
    let control = duplicate_for_child(control)?;
    let shutdown = duplicate_for_child(shutdown)?;
    let control_fd = control.as_raw_fd();
    let shutdown_fd = shutdown.as_raw_fd();

    // SAFETY: the closure performs only descriptor syscalls before exec, using
    // duplicates held live by StagedDescriptors until spawning completes.
    unsafe {
        command.pre_exec(move || {
            if libc::dup2(control_fd, BROKER_FD) < 0 {
                return Err(io::Error::last_os_error());
            }
            if libc::dup2(shutdown_fd, BROKER_SHUTDOWN_FD) < 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        });
    }

    Ok(StagedDescriptors {
        _control: control,
        _shutdown: shutdown,
    })
}

fn close_broker_descriptors(command: &mut Command) {
    // SAFETY: closing only the reserved broker descriptors makes the child
    // fixture deterministic; the child has no other work before exec.
    unsafe {
        command.pre_exec(|| {
            libc::close(BROKER_FD);
            libc::close(BROKER_SHUTDOWN_FD);
            Ok(())
        });
    }
}

fn send_graceful_shutdown(descriptor: &OwnedFd) -> io::Result<()> {
    send_socket_packet(descriptor, &[1_u8])
}

fn send_socket_packet(descriptor: &OwnedFd, packet: &[u8]) -> io::Result<()> {
    loop {
        // SAFETY: descriptor is a live test socket and packet is readable.
        let sent = unsafe {
            libc::send(
                descriptor.as_raw_fd(),
                packet.as_ptr().cast(),
                packet.len(),
                libc::MSG_NOSIGNAL,
            )
        };
        if sent == packet.len() as isize {
            return Ok(());
        }
        if sent < 0 {
            let error = io::Error::last_os_error();
            if error.kind() == io::ErrorKind::Interrupted {
                continue;
            }
            return Err(error);
        }
        return Err(io::Error::new(
            io::ErrorKind::WriteZero,
            "test socket packet was not written completely",
        ));
    }
}

fn receive_socket_packet(descriptor: &OwnedFd, timeout: Duration) -> io::Result<Vec<u8>> {
    let mut pollfd = libc::pollfd {
        fd: descriptor.as_raw_fd(),
        events: libc::POLLIN,
        revents: 0,
    };
    let deadline = Instant::now() + timeout;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "broker did not answer authenticated ping",
            ));
        }
        let timeout_ms = i32::try_from(remaining.as_millis().max(1)).unwrap_or(i32::MAX);
        pollfd.revents = 0;
        // SAFETY: pollfd points to one initialized test descriptor.
        let ready = unsafe { libc::poll(&mut pollfd, 1, timeout_ms) };
        if ready == 0 {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "broker did not answer authenticated ping",
            ));
        }
        if ready < 0 {
            let error = io::Error::last_os_error();
            if error.kind() == io::ErrorKind::Interrupted {
                continue;
            }
            return Err(error);
        }
        if pollfd.revents & libc::POLLIN == 0 {
            return Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "broker control socket closed before ping response",
            ));
        }

        let mut packet = vec![0_u8; 4096];
        // MSG_TRUNC exposes an unexpectedly oversized response.
        // SAFETY: packet is writable and descriptor is ready for one packet.
        let received = unsafe {
            libc::recv(
                descriptor.as_raw_fd(),
                packet.as_mut_ptr().cast(),
                packet.len(),
                libc::MSG_DONTWAIT | libc::MSG_TRUNC,
            )
        };
        if received >= 0 {
            let received = usize::try_from(received).unwrap_or(usize::MAX);
            if received > packet.len() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "broker ping response exceeded the test packet cap",
                ));
            }
            packet.truncate(received);
            return Ok(packet);
        }
        let error = io::Error::last_os_error();
        if matches!(
            error.kind(),
            io::ErrorKind::Interrupted | io::ErrorKind::WouldBlock
        ) {
            continue;
        }
        return Err(error);
    }
}

fn assert_authenticated_ping(descriptor: &OwnedFd) -> TestResult {
    let request = serde_json::json!({
        "token": CANONICAL_TOKEN,
        "request_id": PING_REQUEST_ID,
        "admission_deadline_monotonic_ns": admission_deadline_after(CHILD_TIMEOUT)?,
        "operation": { "operation": "ping" },
    });
    send_socket_packet(descriptor, &serde_json::to_vec(&request)?)?;
    let response = receive_socket_packet(descriptor, CHILD_TIMEOUT)?;
    let response: serde_json::Value = serde_json::from_slice(&response)?;
    assert_eq!(
        response,
        serde_json::json!({
            "request_id": PING_REQUEST_ID,
            "outcome": { "outcome": "acknowledged" },
        })
    );
    Ok(())
}

fn admission_deadline_after(duration: Duration) -> io::Result<u64> {
    let mut now = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    // SAFETY: now is a writable timespec and CLOCK_MONOTONIC is process-independent.
    if unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut now) } != 0 {
        return Err(io::Error::last_os_error());
    }
    let seconds = u64::try_from(now.tv_sec)
        .map_err(|_| io::Error::other("monotonic test clock returned negative seconds"))?;
    let nanos = u64::try_from(now.tv_nsec)
        .map_err(|_| io::Error::other("monotonic test clock returned negative nanoseconds"))?;
    let now = seconds
        .checked_mul(1_000_000_000)
        .and_then(|base| base.checked_add(nanos))
        .ok_or_else(|| io::Error::other("monotonic test clock overflowed"))?;
    let budget = u64::try_from(duration.as_nanos())
        .map_err(|_| io::Error::other("test admission budget overflowed"))?;
    now.checked_add(budget)
        .ok_or_else(|| io::Error::other("test admission deadline overflowed"))
}

fn file_read_request(
    request_id: &str,
    path: &Path,
    timeout: Duration,
    byte_limit: usize,
) -> TestResult<serde_json::Value> {
    Ok(serde_json::json!({
        "token": CANONICAL_TOKEN,
        "request_id": request_id,
        "admission_deadline_monotonic_ns": admission_deadline_after(CHILD_TIMEOUT)?,
        "operation": {
            "operation": "read_file",
            "path": path.as_os_str().as_bytes(),
            "timeout_ms": u64::try_from(timeout.as_millis()).map_or(u64::MAX, |value| value),
            "byte_limit": byte_limit,
        },
    }))
}

fn exchange_json(
    descriptor: &OwnedFd,
    request: serde_json::Value,
) -> TestResult<serde_json::Value> {
    send_socket_packet(descriptor, &serde_json::to_vec(&request)?)?;
    let response = receive_socket_packet(descriptor, CHILD_TIMEOUT)?;
    Ok(serde_json::from_slice(&response)?)
}

fn assert_quiet_exit(output: &CapturedExit, expected_code: i32) {
    assert_eq!(
        output.status.code(),
        Some(expected_code),
        "unexpected exit status; stdout={:?}; stderr={:?}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output.stdout.is_empty(),
        "internal broker classification emitted stdout: {:?}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(
        output.stderr.is_empty(),
        "internal broker classification emitted stderr: {:?}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn assert_no_application_artifact(directory: &TestDirectory) {
    assert!(!directory.path().join("wayscriber.log").exists());
    assert!(!directory.path().join("wayscriber").exists());
    assert!(
        !directory
            .path()
            .join("unusable-root")
            .join("child")
            .exists()
    );
}

fn assert_invalid_without_descriptors(label: &str, markers: Markers) -> TestResult {
    let directory = TestDirectory::create(label)?;
    let mut command = broker_command(&directory)?;
    markers.apply(&mut command);
    let mut child = BoundedChild::spawn(&mut command, &directory)?;
    let output = child.wait(CHILD_TIMEOUT)?;

    assert_quiet_exit(&output, 126);
    assert_no_application_artifact(&directory);
    Ok(())
}

fn assert_invalid_socket_types(label: &str, control_type: i32, shutdown_type: i32) -> TestResult {
    let directory = TestDirectory::create(label)?;
    let (control_parent, control_child) = socket_pair(control_type)?;
    let (shutdown_parent, shutdown_child) = socket_pair(shutdown_type)?;
    let mut command = broker_command(&directory)?;
    Markers::complete().apply(&mut command);
    let staged = stage_broker_descriptors(&mut command, &control_child, &shutdown_child)?;
    let mut child = BoundedChild::spawn(&mut command, &directory)?;
    drop((staged, control_child, shutdown_child));
    let output = child.wait(CHILD_TIMEOUT)?;

    assert_quiet_exit(&output, 126);
    assert_no_application_artifact(&directory);
    drop((control_parent, shutdown_parent));
    Ok(())
}

#[test]
fn complete_broker_bootstrap_enters_the_loop_and_shuts_down_cleanly() -> TestResult {
    let directory = TestDirectory::create("valid")?;
    let (control_parent, control_child) = socket_pair(libc::SOCK_SEQPACKET)?;
    let (shutdown_parent, shutdown_child) = socket_pair(libc::SOCK_SEQPACKET)?;
    let mut command = broker_command(&directory)?;
    Markers::complete().apply(&mut command);
    let staged = stage_broker_descriptors(&mut command, &control_child, &shutdown_child)?;
    let mut child = BoundedChild::spawn(&mut command, &directory)?;
    drop((staged, control_child, shutdown_child));

    assert_authenticated_ping(&control_parent)?;
    send_graceful_shutdown(&shutdown_parent)?;
    let output = child.wait(CHILD_TIMEOUT)?;

    assert_quiet_exit(&output, 0);
    assert_no_application_artifact(&directory);
    drop(control_parent);
    Ok(())
}

#[test]
fn file_reader_is_typed_bounded_and_leaves_the_broker_healthy_after_timeout() -> TestResult {
    let directory = TestDirectory::create("file-reader")?;
    let (control_parent, control_child) = socket_pair(libc::SOCK_SEQPACKET)?;
    let (shutdown_parent, shutdown_child) = socket_pair(libc::SOCK_SEQPACKET)?;
    let mut command = broker_command(&directory)?;
    Markers::complete().apply(&mut command);
    let staged = stage_broker_descriptors(&mut command, &control_child, &shutdown_child)?;
    let mut child = BoundedChild::spawn(&mut command, &directory)?;
    drop((staged, control_child, shutdown_child));

    let regular = directory.path().join("regular.bin");
    fs::write(&regular, b"abc")?;
    let ready = exchange_json(
        &control_parent,
        file_read_request(
            "00000000000000000000000000000001",
            &regular,
            Duration::from_secs(1),
            3,
        )?,
    )?;
    assert_eq!(
        ready["outcome"],
        serde_json::json!({
            "outcome": "file_read",
            "result": { "result": "ready" },
            "bytes": { "storage": "inline", "bytes": [97, 98, 99] },
        })
    );

    let empty = directory.path().join("empty.bin");
    fs::write(&empty, [])?;
    let empty = exchange_json(
        &control_parent,
        file_read_request(
            "00000000000000000000000000000002",
            &empty,
            Duration::from_secs(1),
            3,
        )?,
    )?;
    assert_eq!(empty["outcome"]["result"]["result"], "empty");

    let too_large = exchange_json(
        &control_parent,
        file_read_request(
            "00000000000000000000000000000003",
            &regular,
            Duration::from_secs(1),
            2,
        )?,
    )?;
    assert_eq!(too_large["outcome"]["result"]["result"], "too_large");

    let symlink_path = directory.path().join("regular-link.bin");
    symlink(&regular, &symlink_path)?;
    let symlink_result = exchange_json(
        &control_parent,
        file_read_request(
            "00000000000000000000000000000004",
            &symlink_path,
            Duration::from_secs(1),
            3,
        )?,
    )?;
    assert_eq!(symlink_result["outcome"]["result"]["result"], "not_regular");

    let directory_result = exchange_json(
        &control_parent,
        file_read_request(
            "00000000000000000000000000000005",
            directory.path(),
            Duration::from_secs(1),
            3,
        )?,
    )?;
    assert_eq!(
        directory_result["outcome"]["result"]["result"],
        "not_regular"
    );

    let missing = directory.path().join("missing.bin");
    let missing_result = exchange_json(
        &control_parent,
        file_read_request(
            "00000000000000000000000000000006",
            &missing,
            Duration::from_secs(1),
            3,
        )?,
    )?;
    assert_eq!(missing_result["outcome"]["result"]["result"], "read_failed");
    assert!(
        missing_result["outcome"]["result"]["reason"]
            .as_str()
            .is_some_and(|reason| !reason.is_empty()),
        "missing-file outcome must retain a typed diagnostic"
    );

    let slow = directory.path().join("deadline.bin");
    fs::write(&slow, vec![0x5a; 3 * 1024 * 1024])?;
    let timed_out = exchange_json(
        &control_parent,
        file_read_request(
            "00000000000000000000000000000007",
            &slow,
            Duration::ZERO,
            3 * 1024 * 1024,
        )?,
    )?;
    assert_eq!(timed_out["outcome"]["result"]["result"], "timed_out");

    assert_authenticated_ping(&control_parent)?;
    send_graceful_shutdown(&shutdown_parent)?;
    let output = child.wait(CHILD_TIMEOUT)?;
    assert_quiet_exit(&output, 0);
    assert_no_application_artifact(&directory);
    drop(control_parent);
    Ok(())
}

#[test]
fn partial_broker_markers_fail_closed() -> TestResult {
    let descriptor = Some(OsString::from(BROKER_FD.to_string()));
    let shutdown = Some(OsString::from(BROKER_SHUTDOWN_FD.to_string()));
    let token = Some(OsString::from(CANONICAL_TOKEN));
    let cases = [
        (
            "descriptor-only",
            Markers {
                descriptor: descriptor.clone(),
                ..Markers::default()
            },
        ),
        (
            "shutdown-only",
            Markers {
                shutdown: shutdown.clone(),
                ..Markers::default()
            },
        ),
        (
            "token-only",
            Markers {
                token: token.clone(),
                ..Markers::default()
            },
        ),
        (
            "descriptors-only",
            Markers {
                descriptor: descriptor.clone(),
                shutdown: shutdown.clone(),
                token: None,
            },
        ),
        (
            "descriptor-token",
            Markers {
                descriptor: descriptor.clone(),
                shutdown: None,
                token: token.clone(),
            },
        ),
        (
            "shutdown-token",
            Markers {
                descriptor: None,
                shutdown,
                token,
            },
        ),
    ];

    for (label, markers) in cases {
        assert_invalid_without_descriptors(label, markers)?;
    }
    Ok(())
}

#[test]
fn malformed_broker_markers_fail_closed() -> TestResult {
    let cases = [
        (
            "nonnumeric-control",
            Markers {
                descriptor: Some(OsString::from("not-a-number")),
                ..Markers::complete()
            },
        ),
        (
            "nonnumeric-shutdown",
            Markers {
                shutdown: Some(OsString::from("not-a-number")),
                ..Markers::complete()
            },
        ),
        (
            "nonunicode-control",
            Markers {
                descriptor: Some(OsString::from_vec(vec![0xff])),
                ..Markers::complete()
            },
        ),
        (
            "nonunicode-token",
            Markers {
                token: Some(OsString::from_vec(vec![0xff; 64])),
                ..Markers::complete()
            },
        ),
        (
            "short-token",
            Markers {
                token: Some(OsString::from("a".repeat(63))),
                ..Markers::complete()
            },
        ),
        (
            "uppercase-token",
            Markers {
                token: Some(OsString::from("A".repeat(64))),
                ..Markers::complete()
            },
        ),
        (
            "nonhex-token",
            Markers {
                token: Some(OsString::from("g".repeat(64))),
                ..Markers::complete()
            },
        ),
    ];

    for (label, markers) in cases {
        assert_invalid_without_descriptors(label, markers)?;
    }
    Ok(())
}

#[test]
fn invalid_broker_descriptors_fail_closed() -> TestResult {
    assert_invalid_without_descriptors(
        "wrong-identities",
        Markers {
            descriptor: Some(OsString::from("5")),
            shutdown: Some(OsString::from("6")),
            token: Some(OsString::from(CANONICAL_TOKEN)),
        },
    )?;

    let directory = TestDirectory::create("closed-descriptors")?;
    let mut command = broker_command(&directory)?;
    Markers::complete().apply(&mut command);
    close_broker_descriptors(&mut command);
    let mut child = BoundedChild::spawn(&mut command, &directory)?;
    let output = child.wait(CHILD_TIMEOUT)?;
    assert_quiet_exit(&output, 126);
    assert_no_application_artifact(&directory);

    assert_invalid_socket_types(
        "wrong-control-type",
        libc::SOCK_STREAM,
        libc::SOCK_SEQPACKET,
    )?;
    assert_invalid_socket_types(
        "wrong-shutdown-type",
        libc::SOCK_SEQPACKET,
        libc::SOCK_STREAM,
    )?;
    Ok(())
}
