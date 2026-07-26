#![cfg(target_os = "linux")]

use std::fs;
use std::io;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const PROCESS_TIMEOUT: Duration = Duration::from_secs(5);

type TestResult = Result<(), Box<dyn std::error::Error>>;

struct ChildSubreaper {
    previous: libc::c_int,
}

impl ChildSubreaper {
    fn enable() -> io::Result<Self> {
        let mut previous = 0;
        // SAFETY: PR_GET_CHILD_SUBREAPER writes one c_int to the provided slot.
        if unsafe {
            libc::prctl(
                libc::PR_GET_CHILD_SUBREAPER,
                &mut previous as *mut libc::c_int,
                0,
                0,
                0,
            )
        } != 0
        {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: PR_SET_CHILD_SUBREAPER changes only this test process's child
        // adoption policy. This integration-test binary contains one test.
        if unsafe { libc::prctl(libc::PR_SET_CHILD_SUBREAPER, 1, 0, 0, 0) } != 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(Self { previous })
    }
}

impl Drop for ChildSubreaper {
    fn drop(&mut self) {
        // SAFETY: restore the process setting captured by enable().
        unsafe {
            libc::prctl(libc::PR_SET_CHILD_SUBREAPER, self.previous, 0, 0, 0);
        }
    }
}

struct TestDirectory {
    path: PathBuf,
}

impl TestDirectory {
    fn create() -> io::Result<Self> {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        for attempt in 0..100 {
            let path = std::env::temp_dir().join(format!(
                "wayscriber-broker-owner-{}-{nonce}-{attempt}",
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
            "could not create a unique broker-owner test directory",
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
    stderr: Vec<u8>,
}

struct BoundedChild {
    child: Option<Child>,
    stderr_path: PathBuf,
}

impl BoundedChild {
    fn spawn(command: &mut Command, directory: &TestDirectory) -> io::Result<Self> {
        let stdout_path = directory.path().join("parent.stdout");
        let stderr_path = directory.path().join("parent.stderr");
        let stdout = fs::File::create(stdout_path)?;
        let stderr = fs::File::create(&stderr_path)?;
        let child = command
            .stdout(Stdio::from(stdout))
            .stderr(Stdio::from(stderr))
            .spawn()?;
        Ok(Self {
            child: Some(child),
            stderr_path,
        })
    }

    fn id(&self) -> io::Result<u32> {
        self.child
            .as_ref()
            .map(Child::id)
            .ok_or_else(|| io::Error::other("Wayscriber fixture process was already consumed"))
    }

    fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
        self.child
            .as_mut()
            .ok_or_else(|| io::Error::other("Wayscriber fixture process was already consumed"))?
            .try_wait()
    }

    fn wait(&mut self, timeout: Duration) -> io::Result<CapturedExit> {
        let deadline = Instant::now() + timeout;
        loop {
            match self.try_wait()? {
                Some(status) => {
                    self.child = None;
                    return Ok(CapturedExit {
                        status,
                        stderr: fs::read(&self.stderr_path)?,
                    });
                }
                None if Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(5));
                }
                None => {
                    if let Some(child) = self.child.as_mut() {
                        let _ = child.kill();
                        let _ = child.wait();
                    }
                    self.child = None;
                    return Err(io::Error::new(
                        io::ErrorKind::TimedOut,
                        format!("Wayscriber fixture exceeded {timeout:?}"),
                    ));
                }
            }
        }
    }
}

impl Drop for BoundedChild {
    fn drop(&mut self) {
        if let Some(child) = self.child.as_mut() {
            match child.try_wait() {
                Ok(Some(_)) => {}
                Ok(None) | Err(_) => {
                    let _ = child.kill();
                }
            }
            let _ = child.wait();
        }
    }
}

fn accept_before_parent_exit(
    listener: &UnixListener,
    parent: &mut BoundedChild,
    timeout: Duration,
) -> io::Result<UnixStream> {
    let deadline = Instant::now() + timeout;
    loop {
        match listener.accept() {
            Ok((stream, _)) => return Ok(stream),
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {}
            Err(error) => return Err(error),
        }
        if parent.try_wait()?.is_some() {
            return Err(io::Error::other(
                "Wayscriber exited before opening the isolated Wayland fixture",
            ));
        }
        if Instant::now() >= deadline {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "Wayscriber did not open the isolated Wayland fixture",
            ));
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn wait_for_broker_child(
    parent_pid: u32,
    parent: &mut BoundedChild,
    timeout: Duration,
) -> io::Result<libc::pid_t> {
    let children_path = format!("/proc/{parent_pid}/task/{parent_pid}/children");
    let deadline = Instant::now() + timeout;
    loop {
        let children = fs::read_to_string(&children_path)?;
        let children = children
            .split_whitespace()
            .map(str::parse::<libc::pid_t>)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        match children.as_slice() {
            [broker_pid] => return Ok(*broker_pid),
            [] => {}
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("Wayscriber fixture owned unexpected children: {children:?}"),
                ));
            }
        }
        if parent.try_wait()?.is_some() {
            return Err(io::Error::other(
                "Wayscriber exited before exposing its broker child",
            ));
        }
        if Instant::now() >= deadline {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "Wayscriber broker child did not appear in /proc",
            ));
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn process_parent(pid: libc::pid_t) -> io::Result<libc::pid_t> {
    let status = fs::read_to_string(format!("/proc/{pid}/status"))?;
    let value = status
        .lines()
        .find_map(|line| line.strip_prefix("PPid:"))
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "process has no PPid field"))?;
    value
        .trim()
        .parse()
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

fn process_state(pid: libc::pid_t) -> io::Result<char> {
    let status = fs::read_to_string(format!("/proc/{pid}/status"))?;
    let value = status
        .lines()
        .find_map(|line| line.strip_prefix("State:"))
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "process has no State field"))?;
    value
        .trim()
        .chars()
        .next()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "process State field is empty"))
}

fn process_has_pidfd(pid: libc::pid_t) -> io::Result<bool> {
    for entry in fs::read_dir(format!("/proc/{pid}/fd"))? {
        let entry = entry?;
        match fs::read_link(entry.path()) {
            Ok(target) if target.to_string_lossy().contains("anon_inode:[pidfd]") => {
                return Ok(true);
            }
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
    }
    Ok(false)
}

fn blocked_signal_mask(pid: libc::pid_t, task: Option<libc::pid_t>) -> io::Result<u64> {
    let path = match task {
        Some(task) => format!("/proc/{pid}/task/{task}/status"),
        None => format!("/proc/{pid}/status"),
    };
    let status = fs::read_to_string(path)?;
    let value = status
        .lines()
        .find_map(|line| line.strip_prefix("SigBlk:"))
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "process has no SigBlk field"))?;
    u64::from_str_radix(value.trim(), 16)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

fn runtime_signal_bits() -> u64 {
    [libc::SIGUSR1, libc::SIGUSR2, libc::SIGTERM, libc::SIGINT]
        .into_iter()
        .fold(0_u64, |mask, signal| mask | (1_u64 << (signal - 1)))
}

fn termination_signal_bits() -> u64 {
    [libc::SIGTERM, libc::SIGINT]
        .into_iter()
        .fold(0_u64, |mask, signal| mask | (1_u64 << (signal - 1)))
}

fn parent_task_ids(pid: libc::pid_t) -> io::Result<Vec<libc::pid_t>> {
    fs::read_dir(format!("/proc/{pid}/task"))?
        .map(|entry| {
            let entry = entry?;
            entry
                .file_name()
                .to_string_lossy()
                .parse()
                .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
        })
        .collect()
}

fn task_name(pid: libc::pid_t, task: libc::pid_t) -> io::Result<String> {
    fs::read_to_string(format!("/proc/{pid}/task/{task}/comm")).map(|name| name.trim().to_string())
}

fn signal_set(signals: &[libc::c_int]) -> io::Result<libc::sigset_t> {
    let mut mask = std::mem::MaybeUninit::<libc::sigset_t>::uninit();
    if unsafe {
        // SAFETY: sigemptyset initializes the complete fixture mask.
        libc::sigemptyset(mask.as_mut_ptr())
    } != 0
    {
        return Err(io::Error::last_os_error());
    }
    let mut mask = unsafe {
        // SAFETY: sigemptyset succeeded above.
        mask.assume_init()
    };
    for signal in signals {
        if unsafe {
            // SAFETY: mask is initialized and each value is a supported signal.
            libc::sigaddset(&mut mask, *signal)
        } != 0
        {
            return Err(io::Error::last_os_error());
        }
    }
    Ok(mask)
}

fn install_blocked_termination_baseline_before_exec(command: &mut Command) -> io::Result<()> {
    let runtime_signals = signal_set(&[libc::SIGUSR1, libc::SIGUSR2, libc::SIGTERM, libc::SIGINT])?;
    let termination_signals = signal_set(&[libc::SIGTERM, libc::SIGINT])?;
    unsafe {
        // SAFETY: the child closure performs only async-signal-safe
        // sigprocmask operations on masks prepared before fork. It establishes
        // a deterministic caller baseline with termination blocked and the two
        // application-control signals unblocked.
        command.pre_exec(move || {
            if libc::sigprocmask(libc::SIG_UNBLOCK, &runtime_signals, std::ptr::null_mut()) != 0 {
                return Err(io::Error::last_os_error());
            }
            if libc::sigprocmask(libc::SIG_BLOCK, &termination_signals, std::ptr::null_mut()) != 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        });
    }
    Ok(())
}

fn close_standard_io_before_exec(command: &mut Command) {
    unsafe {
        // SAFETY: each close is async-signal-safe. This deliberately gives the
        // fixture no descriptors 0, 1, or 2 so ordinary app initialization can
        // place private capabilities there before broker bootstrap normalizes
        // its own standard descriptors.
        command.pre_exec(|| {
            libc::close(libc::STDIN_FILENO);
            libc::close(libc::STDOUT_FILENO);
            libc::close(libc::STDERR_FILENO);
            Ok(())
        });
    }
}

fn inherit_self_watchdog(command: &mut Command) -> io::Result<OwnedFd> {
    let raw = unsafe {
        // SAFETY: pidfd_open reads only the current process identity and
        // returns one fresh descriptor on success.
        libc::syscall(libc::SYS_pidfd_open, std::process::id(), 0) as libc::c_int
    };
    if raw < 0 {
        return Err(io::Error::last_os_error());
    }
    let descriptor = unsafe {
        // SAFETY: pidfd_open returned one fresh descriptor transferred here.
        OwnedFd::from_raw_fd(raw)
    };
    // Exercise the high-fd path used by a broker-transferred daemon watchdog,
    // rather than relying on dup3 replacement of descriptor three or four.
    let high = unsafe {
        // SAFETY: F_DUPFD borrows the live pidfd and returns a fresh descriptor.
        libc::fcntl(descriptor.as_raw_fd(), libc::F_DUPFD, 9)
    };
    if high < 0 {
        return Err(io::Error::last_os_error());
    }
    let descriptor = unsafe {
        // SAFETY: fcntl returned one fresh descriptor transferred exactly once.
        OwnedFd::from_raw_fd(high)
    };
    // SAFETY: this single-test integration binary retains descriptor ownership;
    // clearing close-on-exec deliberately lends the high-numbered capability
    // to the Wayscriber fixture. App bootstrap takes a close-on-exec duplicate
    // and protects the borrowed source before creating the raw-clone broker.
    if unsafe { libc::fcntl(descriptor.as_raw_fd(), libc::F_SETFD, 0) } != 0 {
        return Err(io::Error::last_os_error());
    }
    command.env(
        "WAYSCRIBER_INTERNAL_DAEMON_WATCHDOG_FD",
        descriptor.as_raw_fd().to_string(),
    );
    Ok(descriptor)
}

fn wait_for_process_absence(pid: libc::pid_t, timeout: Duration) -> io::Result<()> {
    let path = PathBuf::from(format!("/proc/{pid}"));
    let deadline = Instant::now() + timeout;
    loop {
        match fs::symlink_metadata(&path) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
            Ok(_) => {}
            Err(error) => return Err(error),
        }
        if Instant::now() >= deadline {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                format!("broker process {pid} remained after its owner exited"),
            ));
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn assert_broker_standard_io_is_null(pid: libc::pid_t) -> io::Result<()> {
    for descriptor in [libc::STDIN_FILENO, libc::STDOUT_FILENO, libc::STDERR_FILENO] {
        let target = fs::read_link(format!("/proc/{pid}/fd/{descriptor}"))?;
        if target != Path::new("/dev/null") {
            return Err(io::Error::other(format!(
                "broker descriptor {descriptor} unexpectedly targets {target:?}"
            )));
        }
    }
    Ok(())
}

fn assert_no_adopted_children() -> io::Result<()> {
    let mut status = 0;
    // SAFETY: waitpid writes to status and WNOHANG never blocks.
    let result = unsafe { libc::waitpid(-1, &mut status, libc::WNOHANG) };
    if result < 0 && io::Error::last_os_error().raw_os_error() == Some(libc::ECHILD) {
        return Ok(());
    }
    Err(io::Error::other(if result == 0 {
        "a live broker descendant was adopted instead of being shut down and reaped".to_string()
    } else if result > 0 {
        format!("broker descendant {result} was reaped by the test instead of its owner")
    } else {
        format!(
            "waitpid failed while checking broker ownership: {}",
            io::Error::last_os_error()
        )
    }))
}

#[test]
fn production_owner_handshakes_then_shuts_down_and_reaps_its_raw_clone_broker() -> TestResult {
    let _subreaper = ChildSubreaper::enable()?;
    let directory = TestDirectory::create()?;
    let runtime = directory.path().join("runtime");
    let home = directory.path().join("home");
    let config = directory.path().join("config");
    let cache = directory.path().join("cache");
    let data = directory.path().join("data");
    let pictures = directory.path().join("pictures");
    for path in [&runtime, &home, &config, &cache, &data, &pictures] {
        fs::create_dir(path)?;
    }
    fs::set_permissions(&runtime, fs::Permissions::from_mode(0o700))?;

    let display = "wayland-broker-owner-test";
    let listener = UnixListener::bind(runtime.join(display))?;
    listener.set_nonblocking(true)?;

    let mut command = Command::new(env!("CARGO_BIN_EXE_wayscriber"));
    command
        .arg("--active")
        .env_clear()
        .env("HOME", &home)
        .env("XDG_CONFIG_HOME", &config)
        .env("XDG_CACHE_HOME", &cache)
        .env("XDG_DATA_HOME", &data)
        .env("XDG_RUNTIME_DIR", &runtime)
        .env("XDG_PICTURES_DIR", &pictures)
        .env("WAYLAND_DISPLAY", display)
        .env("WAYSCRIBER_NO_DETACH", "1")
        .env(
            "WAYSCRIBER_LOG_FILE",
            directory.path().join("wayscriber.log"),
        );
    let inherited_watchdog = inherit_self_watchdog(&mut command)?;
    install_blocked_termination_baseline_before_exec(&mut command)?;
    close_standard_io_before_exec(&mut command);
    let mut parent = BoundedChild::spawn(&mut command, &directory)?;
    drop(inherited_watchdog);
    let parent_pid = parent.id()?;

    // The active process reaches Wayland only after the raw-clone broker's
    // authenticated actor handshake and root signal-owner installation.
    let wayland_connection = accept_before_parent_exit(&listener, &mut parent, PROCESS_TIMEOUT)?;
    let broker_pid = wait_for_broker_child(parent_pid, &mut parent, PROCESS_TIMEOUT)?;
    assert_eq!(process_parent(broker_pid)?, parent_pid as libc::pid_t);
    assert_ne!(
        process_state(broker_pid)?,
        'Z',
        "the broker handshake must come from a live actor, not an unreaped child"
    );
    assert!(
        process_has_pidfd(parent_pid as libc::pid_t)?,
        "active fixture must retain its root-owned daemon-watchdog pidfd"
    );
    assert!(
        !process_has_pidfd(broker_pid)?,
        "prepared watchdog capability and its borrowed source must not reach the broker exec"
    );
    assert_broker_standard_io_is_null(broker_pid)?;

    let runtime_signals = runtime_signal_bits();
    let tasks = parent_task_ids(parent_pid as libc::pid_t)?;
    assert!(
        tasks.len() >= 3,
        "active fixture must retain its root plus watchdog and broker-actor workers"
    );
    for task in tasks {
        let task_name = task_name(parent_pid as libc::pid_t, task)?;
        assert_eq!(
            blocked_signal_mask(parent_pid as libc::pid_t, Some(task))? & runtime_signals,
            runtime_signals,
            "ordinary Wayscriber task {task} ({task_name}) did not inherit the root signal mask"
        );
    }
    assert_eq!(
        blocked_signal_mask(broker_pid, None)? & runtime_signals,
        termination_signal_bits(),
        "prepared broker must retain the caller's blocked-termination baseline"
    );

    // Closing the fake compositor makes the active runtime return without ever
    // mapping UI. Its root owner must then stop the actor, signal the broker,
    // and waitpid it before restoring the root signal mask.
    drop(wayland_connection);
    drop(listener);
    let exit = parent.wait(PROCESS_TIMEOUT)?;
    assert!(!exit.status.success());
    assert!(
        exit.stderr.is_empty(),
        "the closed standard-error fixture unexpectedly retained output"
    );

    // The production parent consumes the broker's waitpid status, so this
    // black-box child-of-child test cannot distinguish exit code 0 from another
    // reaped exit without adding product instrumentation. It does prove that a
    // live, directly owned broker existed before shutdown and that the owner
    // removed and reaped it rather than orphaning it. The shutdown-channel unit
    // `graceful_shutdown_packet_is_accepted_before_peer_hangup` unit test covers
    // recognition of the graceful packet itself.
    wait_for_process_absence(broker_pid, PROCESS_TIMEOUT)?;
    assert_no_adopted_children()?;
    Ok(())
}
