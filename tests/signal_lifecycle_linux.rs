#![cfg(target_os = "linux")]

use std::fs;
use std::io;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const PROCESS_TIMEOUT: Duration = Duration::from_secs(8);

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
        // SAFETY: this integration-test binary contains one test, so its
        // process-wide child-adoption policy has one serialized owner.
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
                "wayscriber-signal-lifecycle-{}-{nonce}-{attempt}",
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
            "could not create a unique signal-lifecycle test directory",
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
        let stdout = fs::File::create(directory.path().join("daemon.stdout"))?;
        let stderr_path = directory.path().join("daemon.stderr");
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
            .ok_or_else(|| io::Error::other("daemon fixture was already consumed"))
    }

    fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
        self.child
            .as_mut()
            .ok_or_else(|| io::Error::other("daemon fixture was already consumed"))?
            .try_wait()
    }

    fn stderr(&self) -> String {
        fs::read(&self.stderr_path)
            .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
            .unwrap_or_else(|error| format!("<failed to read daemon stderr: {error}>"))
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
                        format!("daemon fixture exceeded {timeout:?}"),
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

fn unblock_runtime_signals_before_exec(command: &mut Command) -> io::Result<()> {
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
    for signal in [libc::SIGUSR1, libc::SIGUSR2, libc::SIGTERM, libc::SIGINT] {
        if unsafe {
            // SAFETY: mask is initialized and each value is a supported signal.
            libc::sigaddset(&mut mask, signal)
        } != 0
        {
            return Err(io::Error::last_os_error());
        }
    }
    unsafe {
        // SAFETY: the child closure performs only the async-signal-safe
        // sigprocmask operation on a mask prepared before fork.
        command.pre_exec(move || {
            if libc::sigprocmask(libc::SIG_UNBLOCK, &mask, std::ptr::null_mut()) == 0 {
                Ok(())
            } else {
                Err(io::Error::last_os_error())
            }
        });
    }
    Ok(())
}

fn wait_for_path_and_broker(
    ready_path: &Path,
    parent_pid: u32,
    parent: &mut BoundedChild,
) -> io::Result<libc::pid_t> {
    let children_path = format!("/proc/{parent_pid}/task/{parent_pid}/children");
    let deadline = Instant::now() + PROCESS_TIMEOUT;
    loop {
        if let Some(status) = parent.try_wait()? {
            return Err(io::Error::other(format!(
                "daemon fixture exited before publishing readiness ({status}): {}",
                parent.stderr()
            )));
        }
        let children = match fs::read_to_string(&children_path) {
            Ok(children) => children,
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error),
        }
        .split_whitespace()
        .map(str::parse::<libc::pid_t>)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        if ready_path.is_file() {
            match children.as_slice() {
                [broker] => return Ok(*broker),
                [] => {}
                _ => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("daemon fixture owned unexpected children: {children:?}"),
                    ));
                }
            }
        }
        if Instant::now() >= deadline {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "daemon fixture did not publish readiness and broker ownership",
            ));
        }
        std::thread::sleep(Duration::from_millis(5));
    }
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
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "process has no SigBlk"))?;
    u64::from_str_radix(value.trim(), 16)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

fn daemon_signal_bits() -> u64 {
    [libc::SIGUSR1, libc::SIGTERM, libc::SIGINT]
        .into_iter()
        .fold(0_u64, |mask, signal| mask | (1_u64 << (signal - 1)))
}

fn task_ids(pid: libc::pid_t) -> io::Result<Vec<libc::pid_t>> {
    fs::read_dir(format!("/proc/{pid}/task"))?
        .map(|entry| {
            entry?
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

fn wait_for_process_absence(pid: libc::pid_t) -> io::Result<()> {
    let path = PathBuf::from(format!("/proc/{pid}"));
    let deadline = Instant::now() + PROCESS_TIMEOUT;
    loop {
        match fs::symlink_metadata(&path) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
            Ok(_) => {}
            Err(error) => return Err(error),
        }
        if Instant::now() >= deadline {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                format!("broker process {pid} remained after daemon shutdown"),
            ));
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn assert_no_adopted_children() -> io::Result<()> {
    let mut status = 0;
    // SAFETY: waitpid writes to status and WNOHANG never blocks.
    let result = unsafe { libc::waitpid(-1, &mut status, libc::WNOHANG) };
    if result < 0 && io::Error::last_os_error().raw_os_error() == Some(libc::ECHILD) {
        return Ok(());
    }
    Err(io::Error::other(if result == 0 {
        "a live daemon descendant was adopted instead of being shut down".to_string()
    } else if result > 0 {
        format!("daemon descendant {result} was reaped by the test instead of its owner")
    } else {
        format!(
            "waitpid failed while checking daemon descendants: {}",
            io::Error::last_os_error()
        )
    }))
}

#[test]
fn process_directed_sigterm_exits_daemon_through_root_signal_owner() -> TestResult {
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

    let mut command = Command::new(env!("CARGO_BIN_EXE_wayscriber"));
    command
        .args(["--daemon", "--no-tray"])
        .env_clear()
        .env("HOME", &home)
        .env("XDG_CONFIG_HOME", &config)
        .env("XDG_CACHE_HOME", &cache)
        .env("XDG_DATA_HOME", &data)
        .env("XDG_RUNTIME_DIR", &runtime)
        .env("XDG_PICTURES_DIR", &pictures)
        .env("WAYLAND_DISPLAY", "wayland-signal-lifecycle-no-socket")
        .env("WAYSCRIBER_DISABLE_UPDATE_CHECK", "1")
        .env(
            "WAYSCRIBER_LOG_FILE",
            directory.path().join("wayscriber.log"),
        );
    unblock_runtime_signals_before_exec(&mut command)?;
    let mut daemon = BoundedChild::spawn(&mut command, &directory)?;
    let daemon_pid = daemon.id()?;
    let readiness = runtime.join("wayscriber").join("wayscriber.pid");
    let broker_pid = wait_for_path_and_broker(&readiness, daemon_pid, &mut daemon)?;

    let tasks = task_ids(daemon_pid as libc::pid_t)?;
    assert!(
        tasks.iter().any(|task| *task != daemon_pid as libc::pid_t),
        "daemon fixture must retain at least one ordinary post-install worker"
    );
    let signal_bits = daemon_signal_bits();
    let mut observed_workers = 0usize;
    for task in tasks {
        let task_name = match task_name(daemon_pid as libc::pid_t, task) {
            Ok(name) => name,
            Err(error)
                if error.kind() == io::ErrorKind::NotFound
                    || error.raw_os_error() == Some(libc::ESRCH) =>
            {
                continue;
            }
            Err(error) => return Err(error.into()),
        };
        let task_mask = match blocked_signal_mask(daemon_pid as libc::pid_t, Some(task)) {
            Ok(mask) => mask,
            Err(error)
                if error.kind() == io::ErrorKind::NotFound
                    || error.raw_os_error() == Some(libc::ESRCH) =>
            {
                // Startup may briefly publish short-lived helper tasks between
                // reading the task directory and opening one status file.
                continue;
            }
            Err(error) => {
                return Err(io::Error::new(
                    error.kind(),
                    format!("failed to inspect daemon task {task} signal mask: {error}"),
                )
                .into());
            }
        };
        if task != daemon_pid as libc::pid_t {
            observed_workers += 1;
        }
        assert_eq!(
            task_mask & signal_bits,
            signal_bits,
            "ordinary daemon task {task} ({task_name}) did not inherit the root signal mask"
        );
    }
    assert!(
        observed_workers > 0,
        "daemon fixture did not retain an inspectable post-install worker"
    );
    let broker_mask = blocked_signal_mask(broker_pid, None).map_err(|error| {
        io::Error::new(
            error.kind(),
            format!("failed to inspect broker {broker_pid} signal mask: {error}"),
        )
    })?;
    assert_eq!(
        broker_mask & signal_bits,
        0,
        "broker must retain the fixture's unblocked pre-entry signal baseline"
    );

    // SAFETY: daemon_pid names the live child whose v2 readiness record and
    // directly-owned broker were observed above. kill sends a process-directed
    // signal and does not select a particular thread.
    if unsafe { libc::kill(daemon_pid as libc::pid_t, libc::SIGTERM) } != 0 {
        return Err(io::Error::last_os_error().into());
    }
    let exit = daemon.wait(PROCESS_TIMEOUT)?;
    let stderr = String::from_utf8_lossy(&exit.stderr);
    assert!(
        exit.status.success(),
        "daemon did not complete signalfd-driven graceful shutdown: {stderr}"
    );
    assert!(
        !readiness.exists(),
        "graceful daemon shutdown retained its readiness record"
    );
    wait_for_process_absence(broker_pid)?;
    assert_no_adopted_children()?;
    Ok(())
}
