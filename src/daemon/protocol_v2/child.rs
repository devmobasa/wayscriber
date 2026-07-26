use std::fs;
use std::io::ErrorKind;
use std::os::fd::{AsFd, AsRawFd, BorrowedFd, FromRawFd, OwnedFd};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::thread::JoinHandle;
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use serde::{Deserialize, Serialize};

use super::wire::fresh_id;

#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OverlayChildPhase {
    Stopped,
    Reserved,
    Starting,
    Committing,
    Ready,
    StopPending,
}

#[derive(Debug)]
struct OwnedOverlayChild {
    generation: String,
    display_pid: u32,
    pidfd: OwnedFd,
    child: crate::process_broker::BrokerChild,
}

#[derive(Debug)]
enum OverlayChildState {
    Stopped,
    Reserved { generation: String },
    Starting(OwnedOverlayChild),
    Committing(OwnedOverlayChild),
    Ready(OwnedOverlayChild),
    StopPending(OwnedOverlayChild),
}

impl OverlayChildState {
    fn generation(&self) -> Option<&str> {
        match self {
            Self::Stopped => None,
            Self::Reserved { generation } => Some(generation),
            Self::Starting(owned)
            | Self::Committing(owned)
            | Self::Ready(owned)
            | Self::StopPending(owned) => Some(&owned.generation),
        }
    }

    fn owned(&self) -> Option<&OwnedOverlayChild> {
        match self {
            Self::Stopped | Self::Reserved { .. } => None,
            Self::Starting(owned)
            | Self::Committing(owned)
            | Self::Ready(owned)
            | Self::StopPending(owned) => Some(owned),
        }
    }

    fn owned_mut(&mut self) -> Option<&mut OwnedOverlayChild> {
        match self {
            Self::Stopped | Self::Reserved { .. } => None,
            Self::Starting(owned)
            | Self::Committing(owned)
            | Self::Ready(owned)
            | Self::StopPending(owned) => Some(owned),
        }
    }

    #[cfg(test)]
    fn phase(&self) -> OverlayChildPhase {
        match self {
            Self::Stopped => OverlayChildPhase::Stopped,
            Self::Reserved { .. } => OverlayChildPhase::Reserved,
            Self::Starting(_) => OverlayChildPhase::Starting,
            Self::Committing(_) => OverlayChildPhase::Committing,
            Self::Ready(_) => OverlayChildPhase::Ready,
            Self::StopPending(_) => OverlayChildPhase::StopPending,
        }
    }
}

#[derive(Debug)]
pub(crate) struct OverlayChildOwner {
    root: PathBuf,
    state: OverlayChildState,
}

impl OverlayChildOwner {
    pub(crate) fn new(root: PathBuf) -> Self {
        Self {
            root,
            state: OverlayChildState::Stopped,
        }
    }
    #[cfg(test)]
    pub(crate) fn phase(&self) -> OverlayChildPhase {
        self.state.phase()
    }

    pub(crate) fn display_pid(&self) -> Option<u32> {
        self.state.owned().map(|owned| owned.display_pid)
    }

    pub(crate) fn poll_fd(&self) -> Option<BorrowedFd<'_>> {
        self.state.owned().map(|owned| owned.pidfd.as_fd())
    }

    pub(crate) fn generation(&self) -> Option<&str> {
        self.state.generation()
    }

    pub(crate) fn reserve(&mut self) -> Result<()> {
        if !matches!(&self.state, OverlayChildState::Stopped) {
            bail!("overlay child owner is not stopped");
        }
        let generation = fresh_id()?;
        let ready_path = ready_path(&self.root, &generation);
        if let Err(error) = fs::remove_file(&ready_path)
            && error.kind() != ErrorKind::NotFound
        {
            return Err(error).context("failed to clear stale overlay readiness record");
        }
        if let Err(error) = fs::remove_file(active_path(&self.root, &generation))
            && error.kind() != ErrorKind::NotFound
        {
            return Err(error).context("failed to clear stale overlay active record");
        }
        if let Err(error) = fs::remove_file(enabled_path(&self.root, &generation))
            && error.kind() != ErrorKind::NotFound
        {
            return Err(error).context("failed to clear stale overlay enable record");
        }
        if let Err(error) = fs::remove_file(signals_path(&self.root, &generation))
            && error.kind() != ErrorKind::NotFound
        {
            return Err(error).context("failed to clear stale overlay signal record");
        }
        self.state = OverlayChildState::Reserved { generation };
        Ok(())
    }

    pub(crate) fn start(&mut self, child: crate::process_broker::BrokerChild) -> Result<()> {
        let previous = std::mem::replace(&mut self.state, OverlayChildState::Stopped);
        let generation = match previous {
            OverlayChildState::Reserved { generation } => generation,
            other => {
                self.state = other;
                let _ = child.kill_wait();
                bail!("overlay child owner is not reserved");
            }
        };
        let display_pid = child.id();
        let pidfd = match super::linux::open_pidfd(display_pid) {
            Ok(pidfd) => pidfd,
            Err(error) => {
                let _ = child.kill_wait();
                self.state = OverlayChildState::Reserved { generation };
                return Err(error).with_context(|| {
                    format!("failed to identify overlay child {display_pid} by pidfd")
                });
            }
        };
        self.state = OverlayChildState::Starting(OwnedOverlayChild {
            generation,
            display_pid,
            pidfd,
            child,
        });
        Ok(())
    }

    pub(crate) fn mark_committing(&mut self) -> Result<()> {
        let previous = std::mem::replace(&mut self.state, OverlayChildState::Stopped);
        match previous {
            OverlayChildState::Starting(owned) => {
                self.state = OverlayChildState::Committing(owned);
                Ok(())
            }
            other => {
                self.state = other;
                bail!("only a starting overlay child can commit")
            }
        }
    }

    pub(crate) fn mark_ready(&mut self) -> Result<()> {
        let previous = std::mem::replace(&mut self.state, OverlayChildState::Stopped);
        match previous {
            OverlayChildState::Committing(owned) => {
                self.state = OverlayChildState::Ready(owned);
                Ok(())
            }
            other => {
                self.state = other;
                bail!("only a committing overlay child can become ready")
            }
        }
    }

    pub(crate) fn wait_until_ready(&mut self, timeout: Duration, daemon_token: &str) -> Result<()> {
        super::wire::validate_token(daemon_token)?;
        let deadline = super::BootClock::now()?.checked_add(timeout)?;
        let (generation, expected_pid) = match &self.state {
            OverlayChildState::Starting(owned) => (owned.generation.clone(), owned.display_pid),
            _ => bail!("only a starting overlay child can wait for readiness"),
        };
        let path = ready_path(&self.root, &generation);
        loop {
            match super::linux::read_bounded_regular_file(&path, 1024) {
                Ok(bytes) => {
                    let record: OverlayReadyRecord =
                        super::wire::parse_canonical_json(&bytes, 1024)?;
                    if record.protocol_version != super::wire::DAEMON_CHILD_PROTOCOL_VERSION
                        || record.generation != generation
                        || record.pid != expected_pid
                        || super::linux::process_start_ticks(record.pid)?
                            != record.process_start_ticks
                    {
                        bail!("overlay readiness identity mismatch");
                    }
                    let signals_bytes = match super::linux::read_bounded_regular_file(
                        &signals_path(&self.root, &generation),
                        1024,
                    ) {
                        Ok(bytes) => bytes,
                        Err(error) if error.kind() == ErrorKind::NotFound => {
                            if super::BootClock::now()? >= deadline {
                                let _ = self.force_kill_and_wait();
                                bail!(
                                    "overlay signal authority did not become ready before deadline"
                                );
                            }
                            std::thread::sleep(Duration::from_millis(10));
                            continue;
                        }
                        Err(error) => {
                            return Err(error)
                                .context("failed to read overlay signal-authority proof");
                        }
                    };
                    let signals: OverlayActiveRecord =
                        super::wire::parse_canonical_json(&signals_bytes, 1024)?;
                    if signals.protocol_version != record.protocol_version
                        || signals.generation != record.generation
                        || signals.pid != record.pid
                        || signals.process_start_ticks != record.process_start_ticks
                    {
                        bail!("overlay signal-authority proof does not match readiness");
                    }
                    let enabled = OverlayEnabledRecord {
                        protocol_version: record.protocol_version,
                        generation: record.generation.clone(),
                        pid: record.pid,
                        process_start_ticks: record.process_start_ticks,
                        daemon_token: daemon_token.to_owned(),
                    };
                    let enabled_bytes = super::wire::canonical_json(&enabled, 1024)?;
                    crate::durable_io::write_atomic(
                        &enabled_path(&self.root, &generation),
                        &enabled_bytes,
                        crate::durable_io::AtomicWriteOptions {
                            overwrite: crate::durable_io::OverwriteMode::CreateNew,
                            permissions: crate::durable_io::PermissionPolicy::FixedMode(0o600),
                            symlink: crate::durable_io::SymlinkPolicy::Reject,
                            sync_file: true,
                            sync_parent: true,
                        },
                    )?;
                    fs::remove_file(&path)?;
                    fs::remove_file(signals_path(&self.root, &generation))?;
                    self.mark_committing()?;
                    self.mark_ready()?;
                    return Ok(());
                }
                Err(error) if error.kind() == ErrorKind::NotFound => {}
                Err(error) => return Err(error).context("failed to read overlay readiness record"),
            }
            if self.try_wait()?.is_some() {
                bail!("overlay child exited before publishing readiness");
            }
            if super::BootClock::now()? >= deadline {
                let _ = self.force_kill_and_wait();
                bail!("overlay child did not become ready before deadline");
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    pub(crate) fn abort_reservation(&mut self) {
        let generation = self.state.generation().map(str::to_owned);
        let has_child = self.state.owned().is_some();
        if let Some(generation) = generation {
            clear_generation_records(&self.root, &generation);
        }
        if has_child {
            let _ = self.force_kill_and_wait();
        } else {
            self.state = OverlayChildState::Stopped;
        }
    }

    pub(crate) fn signal(&self, signal: i32) -> Result<()> {
        let owned = self
            .state
            .owned()
            .ok_or_else(|| anyhow!("overlay child is stopped"))?;
        // The retained pidfd proves that this generation still identifies the
        // same kernel process. The opaque broker handle is the signal
        // authority; the daemon never sends a signal using the display PID.
        let _identity = &owned.pidfd;
        owned.child.signal(signal)
    }

    pub(crate) fn try_wait(&mut self) -> Result<Option<i32>> {
        let Some(owned) = self.state.owned_mut() else {
            return Ok(None);
        };
        match owned
            .child
            .try_wait()
            .context("failed to query overlay child")?
        {
            Some(status) => {
                let previous = std::mem::replace(&mut self.state, OverlayChildState::Stopped);
                if let Some(generation) = previous.generation() {
                    clear_generation_records(&self.root, generation);
                }
                Ok(Some(status))
            }
            None => Ok(None),
        }
    }

    pub(crate) fn begin_stop(&mut self) -> Result<()> {
        let previous = std::mem::replace(&mut self.state, OverlayChildState::Stopped);
        match previous {
            OverlayChildState::Stopped | OverlayChildState::Reserved { .. } => Ok(()),
            OverlayChildState::Starting(owned)
            | OverlayChildState::Committing(owned)
            | OverlayChildState::Ready(owned)
            | OverlayChildState::StopPending(owned) => {
                self.state = OverlayChildState::StopPending(owned);
                self.signal(libc::SIGTERM)
            }
        }
    }

    pub(crate) fn force_kill_and_wait(&mut self) -> Result<i32> {
        let owned = self
            .state
            .owned()
            .ok_or_else(|| anyhow!("overlay child is already stopped"))?;
        let status = owned
            .child
            .kill_wait()
            .context("broker failed to kill and reap overlay child")?;
        let previous = std::mem::replace(&mut self.state, OverlayChildState::Stopped);
        if let Some(generation) = previous.generation() {
            clear_generation_records(&self.root, generation);
        }
        Ok(status)
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct OverlayReadyRecord {
    protocol_version: u16,
    generation: String,
    pid: u32,
    process_start_ticks: u64,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct OverlayActiveRecord {
    protocol_version: u16,
    generation: String,
    pid: u32,
    process_start_ticks: u64,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct OverlayEnabledRecord {
    protocol_version: u16,
    generation: String,
    pid: u32,
    process_start_ticks: u64,
    daemon_token: String,
}

fn ready_dir(root: &Path) -> PathBuf {
    root.join("children")
}

fn ready_path(root: &Path, generation: &str) -> PathBuf {
    ready_dir(root).join(format!("{generation}.ready"))
}

fn active_path(root: &Path, generation: &str) -> PathBuf {
    ready_dir(root).join(format!("{generation}.active"))
}

fn enabled_path(root: &Path, generation: &str) -> PathBuf {
    ready_dir(root).join(format!("{generation}.enabled"))
}

fn signals_path(root: &Path, generation: &str) -> PathBuf {
    ready_dir(root).join(format!("{generation}.signals"))
}

fn clear_generation_records(root: &Path, generation: &str) {
    let _ = fs::remove_file(ready_path(root, generation));
    let _ = fs::remove_file(active_path(root, generation));
    let _ = fs::remove_file(enabled_path(root, generation));
    let _ = fs::remove_file(signals_path(root, generation));
}

pub(crate) fn recover_stale_child_records(root: &Path) -> Result<()> {
    let directory = ready_dir(root);
    let entries = match fs::read_dir(&directory) {
        Ok(entries) => entries.take(129).collect::<std::io::Result<Vec<_>>>()?,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error).context("failed to enumerate overlay child proofs"),
    };
    if entries.len() > 128 {
        bail!("overlay child proof directory exceeds recovery cap");
    }
    for entry in entries {
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| anyhow!("overlay child proof name is not UTF-8"))?;
        let (generation, kind) = name
            .rsplit_once('.')
            .ok_or_else(|| anyhow!("invalid overlay child proof name"))?;
        super::wire::validate_id(generation)?;
        let (pid, process_start_ticks, record_generation, protocol_version) = match kind {
            "ready" => {
                let bytes = super::linux::read_bounded_regular_file(&entry.path(), 1024)?;
                let record: OverlayReadyRecord = super::wire::parse_canonical_json(&bytes, 1024)?;
                (
                    record.pid,
                    record.process_start_ticks,
                    record.generation,
                    record.protocol_version,
                )
            }
            "active" | "signals" => {
                let bytes = super::linux::read_bounded_regular_file(&entry.path(), 1024)?;
                let record: OverlayActiveRecord = super::wire::parse_canonical_json(&bytes, 1024)?;
                (
                    record.pid,
                    record.process_start_ticks,
                    record.generation,
                    record.protocol_version,
                )
            }
            "enabled" => {
                let bytes = super::linux::read_bounded_regular_file(&entry.path(), 1024)?;
                let record: OverlayEnabledRecord = super::wire::parse_canonical_json(&bytes, 1024)?;
                super::wire::validate_token(&record.daemon_token)?;
                (
                    record.pid,
                    record.process_start_ticks,
                    record.generation,
                    record.protocol_version,
                )
            }
            _ => bail!("unknown overlay child proof kind"),
        };
        if protocol_version != super::wire::DAEMON_CHILD_PROTOCOL_VERSION
            || record_generation != generation
        {
            bail!("overlay child proof identity mismatch during recovery");
        }
        let still_live = match super::linux::process_start_ticks(pid) {
            Ok(actual) if actual == process_start_ticks => {
                super::linux::open_pidfd(pid)
                    .context("failed to validate live prior overlay child")?;
                true
            }
            Ok(_) => false,
            Err(error) if error.kind() == ErrorKind::NotFound => false,
            Err(error) => return Err(error).context("failed to inspect prior overlay child"),
        };
        if still_live {
            bail!("a prior overlay child is still live during generation recovery");
        }
        fs::remove_file(entry.path())?;
    }
    Ok(())
}

pub(crate) fn open_daemon_watchdog() -> Result<OwnedFd> {
    super::linux::open_pidfd(std::process::id())
        .context("failed to open daemon pidfd for overlay watchdog")
}

/// Root-owned lifetime for an inherited daemon-death watchdog.
///
/// The worker owns the inherited pidfd and one side of a shutdown socket.
/// The app root retains the only producer for that socket and joins the worker
/// before restoring its signal mask.
pub(crate) struct DaemonWatchdogOwner {
    shutdown: Option<OwnedFd>,
    thread: Option<JoinHandle<()>>,
}

impl DaemonWatchdogOwner {
    pub(crate) fn finish(&mut self) -> Result<()> {
        if self.thread.is_none() {
            self.shutdown = None;
            return Ok(());
        }
        let publish_result = self.shutdown.as_ref().map_or(Ok(()), |shutdown| {
            publish_watchdog_shutdown(shutdown.as_fd())
                .context("failed to stop daemon watchdog worker")
        });
        // Closing the socket producer is a second, independent stop signal. The
        // worker treats its peer hangup as graceful root teardown, so a
        // failed write cannot strand a signal-masked watchdog thread.
        self.shutdown = None;
        let Some(thread) = self.thread.take() else {
            return publish_result;
        };
        let join_result = thread
            .join()
            .map_err(|_| anyhow!("daemon watchdog worker panicked during shutdown"));
        match (publish_result, join_result) {
            (Ok(()), Ok(())) => Ok(()),
            (Err(error), Ok(())) | (Ok(()), Err(error)) => Err(error),
            (Err(publish_error), Err(join_error)) => Err(anyhow!(
                "{publish_error:#}; watchdog join also failed: {join_error:#}"
            )),
        }
    }
}

impl Drop for DaemonWatchdogOwner {
    fn drop(&mut self) {
        if let Err(error) = self.finish() {
            log::error!("Failed to join daemon watchdog worker: {error:#}");
        }
    }
}

/// Parsed watchdog bootstrap state that owns no thread.
///
/// Preparing borrows the inherited descriptor, takes a close-on-exec duplicate,
/// marks the borrowed source close-on-exec, and leaves process environment
/// unchanged for safe embedding. The broker and helper exec boundaries also
/// filter the private marker. Activation happens only after the root signal
/// mask is installed.
pub(crate) struct PreparedDaemonWatchdog {
    descriptor: OwnedFd,
}

impl PreparedDaemonWatchdog {
    pub(crate) fn start(self) -> Result<DaemonWatchdogOwner> {
        start_daemon_watchdog(self.descriptor)
    }
}

pub(crate) fn prepare_daemon_watchdog_from_environment() -> Result<Option<PreparedDaemonWatchdog>> {
    let Some(raw) = std::env::var_os(crate::env_vars::DAEMON_WATCHDOG_FD_ENV) else {
        return Ok(None);
    };
    let raw = raw
        .to_str()
        .ok_or_else(|| anyhow!("daemon watchdog descriptor is not UTF-8"))?
        .parse::<i32>()
        .context("daemon watchdog descriptor is not numeric")?;
    if raw <= libc::STDERR_FILENO {
        bail!("daemon watchdog descriptor aliases standard I/O");
    }
    // Treat the environment value as a borrowed capability. A safe library
    // entry cannot prove that it owns the caller's raw descriptor or mutate
    // process environment while embedding threads may exist. The duplicate is
    // root-owned and close-on-exec. The marked capability authorizes protecting
    // its borrowed source from later exec without taking or closing it; broker
    // and helper boundaries also filter the private marker.
    let duplicate = unsafe {
        // SAFETY: F_DUPFD_CLOEXEC borrows `raw` and returns a distinct owned
        // descriptor without changing or taking ownership of the source.
        libc::fcntl(raw, libc::F_DUPFD_CLOEXEC, 5)
    };
    if duplicate < 0 {
        return Err(std::io::Error::last_os_error())
            .context("failed to duplicate inherited daemon watchdog descriptor");
    }
    let descriptor = unsafe {
        // SAFETY: fcntl returned one fresh descriptor transferred exactly once.
        OwnedFd::from_raw_fd(duplicate)
    };
    let source_flags = unsafe {
        // SAFETY: F_GETFD only inspects the borrowed marked descriptor.
        libc::fcntl(raw, libc::F_GETFD)
    };
    if source_flags < 0
        || unsafe {
            // SAFETY: F_SETFD changes only exec inheritance for the borrowed
            // capability explicitly named by the private watchdog marker.
            libc::fcntl(raw, libc::F_SETFD, source_flags | libc::FD_CLOEXEC)
        } != 0
    {
        return Err(std::io::Error::last_os_error())
            .context("failed to protect inherited daemon watchdog source descriptor");
    }
    super::linux::validate_pidfd(descriptor.as_fd())
        .context("inherited daemon watchdog is not a live pidfd")?;
    Ok(Some(PreparedDaemonWatchdog { descriptor }))
}

fn start_daemon_watchdog(descriptor: OwnedFd) -> Result<DaemonWatchdogOwner> {
    let (shutdown_source, shutdown_sender) = watchdog_shutdown_socket_pair()
        .context("failed to create daemon watchdog shutdown socket pair")?;
    let thread = std::thread::Builder::new()
        .name("wayscriber-daemon-watchdog".into())
        .spawn(move || {
            let mut pollfds = [
                libc::pollfd {
                    fd: descriptor.as_raw_fd(),
                    events: libc::POLLIN,
                    revents: 0,
                },
                libc::pollfd {
                    fd: shutdown_source.as_raw_fd(),
                    events: libc::POLLIN,
                    revents: 0,
                },
            ];
            loop {
                // SAFETY: pollfds is initialized and both descriptors remain
                // owned by this worker for the duration of the call.
                let result =
                    unsafe { libc::poll(pollfds.as_mut_ptr(), pollfds.len() as libc::nfds_t, -1) };
                if result > 0
                    && pollfds[0].revents
                        & (libc::POLLIN | libc::POLLERR | libc::POLLHUP | libc::POLLNVAL)
                        != 0
                {
                    // SAFETY: loss of the owning daemon is an irrevocable
                    // fail-stop condition for its internal overlay child.
                    unsafe { super::linux::fail_stop(70) }
                }
                if result > 0 && pollfds[1].revents & (libc::POLLIN | libc::POLLHUP) != 0 {
                    return;
                }
                if result < 0
                    && std::io::Error::last_os_error().kind() == std::io::ErrorKind::Interrupted
                {
                    continue;
                }
                if result > 0 {
                    // SAFETY: losing the root-owned shutdown event makes the
                    // watchdog lifetime indeterminate, so fail-stop preserves
                    // the overlay's daemon-owned process contract.
                    unsafe { super::linux::fail_stop(70) }
                }
                // SAFETY: an unusable parent-death channel cannot safely
                // preserve overlay ownership.
                unsafe { super::linux::fail_stop(70) }
            }
        })
        .context("failed to start daemon watchdog")?;
    Ok(DaemonWatchdogOwner {
        shutdown: Some(shutdown_sender),
        thread: Some(thread),
    })
}

fn watchdog_shutdown_socket_pair() -> std::io::Result<(OwnedFd, OwnedFd)> {
    let mut descriptors = [-1; 2];
    // SAFETY: descriptors has room for socketpair's two connected descriptors.
    if unsafe {
        libc::socketpair(
            libc::AF_UNIX,
            libc::SOCK_SEQPACKET | libc::SOCK_NONBLOCK | libc::SOCK_CLOEXEC,
            0,
            descriptors.as_mut_ptr(),
        )
    } != 0
    {
        return Err(std::io::Error::last_os_error());
    }
    // SAFETY: socketpair returned two fresh descriptors transferred exactly once.
    Ok(unsafe {
        (
            OwnedFd::from_raw_fd(descriptors[0]),
            OwnedFd::from_raw_fd(descriptors[1]),
        )
    })
}

fn publish_watchdog_shutdown(descriptor: BorrowedFd<'_>) -> std::io::Result<()> {
    let value = 1_u8;
    loop {
        // SAFETY: value is readable and the owner retains the connected socket
        // through this nonblocking send. MSG_NOSIGNAL makes peer loss a typed
        // BrokenPipe error instead of a process-wide SIGPIPE.
        let written = unsafe {
            libc::send(
                descriptor.as_raw_fd(),
                (&value as *const u8).cast(),
                size_of::<u8>(),
                libc::MSG_NOSIGNAL,
            )
        };
        if written == size_of::<u8>() as isize {
            return Ok(());
        }
        if written < 0 {
            let error = std::io::Error::last_os_error();
            return match error.kind() {
                ErrorKind::Interrupted => continue,
                // Saturation already leaves the shutdown event readable.
                ErrorKind::WouldBlock => Ok(()),
                _ => Err(error),
            };
        }
        return Err(std::io::Error::new(
            ErrorKind::WriteZero,
            format!("daemon watchdog shutdown event returned a short write ({written} bytes)"),
        ));
    }
}

pub(crate) fn publish_ready_from_environment(root: &Path) -> Result<()> {
    let Some(generation) = std::env::var_os(crate::env_vars::OVERLAY_CHILD_GENERATION_ENV) else {
        return Ok(());
    };
    let generation = generation
        .to_str()
        .ok_or_else(|| anyhow!("overlay generation is not UTF-8"))?;
    publish_ready(root, generation)
}

fn publish_ready(root: &Path, generation: &str) -> Result<()> {
    super::wire::validate_id(generation)?;
    let directory = ready_dir(root);
    fs::create_dir_all(&directory)?;
    fs::set_permissions(&directory, fs::Permissions::from_mode(0o700))?;
    let process_start_ticks = super::linux::current_process_start_ticks()?;
    let active = OverlayActiveRecord {
        protocol_version: super::wire::DAEMON_CHILD_PROTOCOL_VERSION,
        generation: generation.to_owned(),
        pid: std::process::id(),
        process_start_ticks,
    };
    let active_bytes = super::wire::canonical_json(&active, 1024)?;
    crate::durable_io::write_atomic(
        &active_path(root, generation),
        &active_bytes,
        crate::durable_io::AtomicWriteOptions {
            overwrite: crate::durable_io::OverwriteMode::CreateNew,
            permissions: crate::durable_io::PermissionPolicy::FixedMode(0o600),
            symlink: crate::durable_io::SymlinkPolicy::Reject,
            sync_file: true,
            sync_parent: true,
        },
    )?;
    let record = OverlayReadyRecord {
        protocol_version: super::wire::DAEMON_CHILD_PROTOCOL_VERSION,
        generation: generation.to_owned(),
        pid: std::process::id(),
        process_start_ticks,
    };
    let bytes = super::wire::canonical_json(&record, 1024)?;
    crate::durable_io::write_atomic(
        &ready_path(root, generation),
        &bytes,
        crate::durable_io::AtomicWriteOptions {
            overwrite: crate::durable_io::OverwriteMode::CreateNew,
            permissions: crate::durable_io::PermissionPolicy::FixedMode(0o600),
            symlink: crate::durable_io::SymlinkPolicy::Reject,
            sync_file: true,
            sync_parent: true,
        },
    )?;
    Ok(())
}

#[cfg(test)]
pub(in crate::daemon::protocol_v2) fn publish_ready_for_test(
    root: &Path,
    generation: &str,
) -> Result<()> {
    publish_ready(root, generation)
}

pub(crate) fn publish_signal_ready_from_environment(root: &Path) -> Result<()> {
    let Some(generation) = std::env::var_os(crate::env_vars::OVERLAY_CHILD_GENERATION_ENV) else {
        return Ok(());
    };
    let generation = generation
        .to_str()
        .ok_or_else(|| anyhow!("overlay generation is not UTF-8"))?;
    super::wire::validate_id(generation)?;
    let active_bytes =
        super::linux::read_bounded_regular_file(&active_path(root, generation), 1024)?;
    let active: OverlayActiveRecord = super::wire::parse_canonical_json(&active_bytes, 1024)?;
    if active.pid != std::process::id()
        || active.process_start_ticks != super::linux::current_process_start_ticks()?
        || active.generation != generation
    {
        bail!("cannot publish signal readiness for a different overlay child");
    }
    crate::durable_io::write_atomic(
        &signals_path(root, generation),
        &active_bytes,
        crate::durable_io::AtomicWriteOptions {
            overwrite: crate::durable_io::OverwriteMode::CreateNew,
            permissions: crate::durable_io::PermissionPolicy::FixedMode(0o600),
            symlink: crate::durable_io::SymlinkPolicy::Reject,
            sync_file: true,
            sync_parent: true,
        },
    )?;
    Ok(())
}

pub(crate) enum ActiveGeneration {
    Inactive,
    Pending,
    Enabled { daemon_token: String },
}

pub(crate) fn active_generation_from_environment(root: &Path) -> Result<ActiveGeneration> {
    let generation = std::env::var_os(crate::env_vars::OVERLAY_CHILD_GENERATION_ENV);
    let generation = generation
        .as_deref()
        .map(|generation| {
            generation
                .to_str()
                .ok_or_else(|| anyhow!("overlay generation is not UTF-8"))
        })
        .transpose()?;
    active_generation(root, generation)
}

pub(in crate::daemon::protocol_v2) fn active_generation(
    root: &Path,
    generation: Option<&str>,
) -> Result<ActiveGeneration> {
    let Some(generation) = generation else {
        return Ok(ActiveGeneration::Inactive);
    };
    super::wire::validate_id(generation)?;
    let bytes = match super::linux::read_bounded_regular_file(&active_path(root, generation), 1024)
    {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == ErrorKind::NotFound => {
            return Ok(ActiveGeneration::Inactive);
        }
        Err(error) => return Err(error).context("failed to read active overlay proof"),
    };
    let record: OverlayActiveRecord = super::wire::parse_canonical_json(&bytes, 1024)?;
    let current_pid = std::process::id();
    let current_start = super::linux::current_process_start_ticks()?;
    if record.protocol_version != super::wire::DAEMON_CHILD_PROTOCOL_VERSION
        || record.generation != generation
        || record.pid != current_pid
        || record.process_start_ticks != current_start
    {
        bail!("active overlay proof does not match this child generation");
    }
    let enabled_bytes =
        match super::linux::read_bounded_regular_file(&enabled_path(root, generation), 1024) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == ErrorKind::NotFound => {
                return Ok(ActiveGeneration::Pending);
            }
            Err(error) => {
                return Err(error).context("failed to read overlay action-enable proof");
            }
        };
    let enabled: OverlayEnabledRecord = super::wire::parse_canonical_json(&enabled_bytes, 1024)?;
    super::wire::validate_token(&enabled.daemon_token)?;
    if enabled.protocol_version != record.protocol_version
        || enabled.generation != record.generation
        || enabled.pid != record.pid
        || enabled.process_start_ticks != record.process_start_ticks
    {
        bail!("overlay action-enable proof does not match active child proof");
    }
    Ok(ActiveGeneration::Enabled {
        daemon_token: enabled.daemon_token,
    })
}

#[cfg(test)]
pub(crate) fn enable_current_generation_for_test(
    root: &Path,
    generation: &str,
    daemon_token: &str,
) -> Result<()> {
    super::wire::validate_token(daemon_token)?;
    super::wire::validate_id(generation)?;
    let bytes = super::linux::read_bounded_regular_file(&active_path(root, generation), 1024)?;
    let active: OverlayActiveRecord = super::wire::parse_canonical_json(&bytes, 1024)?;
    let enabled = super::wire::canonical_json(
        &OverlayEnabledRecord {
            protocol_version: active.protocol_version,
            generation: active.generation,
            pid: active.pid,
            process_start_ticks: active.process_start_ticks,
            daemon_token: daemon_token.to_owned(),
        },
        1024,
    )?;
    crate::durable_io::write_atomic(
        &enabled_path(root, generation),
        &enabled,
        crate::durable_io::AtomicWriteOptions::private_runtime_file(),
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn daemon_watchdog_root_shutdown_event_stops_and_joins_worker() {
        let pidfd = super::super::linux::open_self_pidfd()
            .expect("fixture opens a live pidfd for its own process");
        let mut owner = start_daemon_watchdog(pidfd)
            .expect("fixture starts its root-owned daemon watchdog worker");

        owner
            .finish()
            .expect("fixture stops and joins its daemon watchdog worker");
        owner
            .finish()
            .expect("finished daemon watchdog ownership is idempotent");
    }

    #[test]
    fn daemon_watchdog_root_socket_close_stops_worker_without_a_write() {
        let pidfd = super::super::linux::open_self_pidfd()
            .expect("socket-close fixture opens a live pidfd for its own process");
        let mut owner = start_daemon_watchdog(pidfd)
            .expect("socket-close fixture starts its root-owned daemon watchdog worker");

        owner.shutdown = None;
        let worker = owner
            .thread
            .take()
            .expect("socket-close fixture owns the watchdog worker handle");
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(1);
        while !worker.is_finished() && std::time::Instant::now() < deadline {
            std::thread::yield_now();
        }
        assert!(
            worker.is_finished(),
            "closing the root shutdown producer must wake the watchdog poll"
        );
        worker
            .join()
            .expect("socket-close fixture watchdog exits without failure");
    }

    #[test]
    fn child_generation_and_pidfd_own_signal_authority() {
        let temp = crate::test_temp::tempdir().expect("isolated child protocol fixture");
        let root = temp.path().join("daemon-commands").join("v2");
        let broker = crate::process_broker::start_for_runtime()
            .expect("fixture starts its process-broker owner");
        let child = broker
            .handle()
            .spawn(
                crate::process_broker::HelperKind::TestSleep,
                crate::process_broker::HelperLifetime::OwnedChild,
                std::ffi::OsStr::new("sleep"),
                [std::ffi::OsStr::new("30")],
                Vec::new(),
            )
            .expect("fixture spawns its long-running overlay child");
        let display_pid = child.id();
        let mut owner = OverlayChildOwner::new(root);
        assert_eq!(owner.phase(), OverlayChildPhase::Stopped);
        assert_eq!(owner.generation(), None);
        assert_eq!(owner.display_pid(), None);
        assert!(owner.poll_fd().is_none());
        owner
            .reserve()
            .expect("fixture reserves an overlay generation");
        let generation = owner
            .generation()
            .expect("fixture reservation installs a generation")
            .to_owned();
        assert_eq!(owner.phase(), OverlayChildPhase::Reserved);
        assert_eq!(owner.generation(), Some(generation.as_str()));
        assert_eq!(owner.display_pid(), None);
        assert!(owner.poll_fd().is_none());
        assert!(owner.reserve().is_err());
        assert!(owner.mark_committing().is_err());
        assert_eq!(owner.phase(), OverlayChildPhase::Reserved);
        assert_eq!(owner.generation(), Some(generation.as_str()));
        owner
            .start(child)
            .expect("fixture installs its broker-owned overlay child");
        assert_eq!(owner.generation(), Some(generation.as_str()));
        assert_eq!(owner.display_pid(), Some(display_pid));
        assert!(owner.poll_fd().is_some());
        assert_eq!(owner.phase(), OverlayChildPhase::Starting);
        assert!(owner.mark_ready().is_err());
        assert_eq!(owner.phase(), OverlayChildPhase::Starting);
        assert_eq!(owner.generation(), Some(generation.as_str()));
        owner
            .mark_committing()
            .expect("fixture advances its child to committing");
        assert_eq!(owner.phase(), OverlayChildPhase::Committing);
        assert_eq!(owner.generation(), Some(generation.as_str()));
        assert_eq!(owner.display_pid(), Some(display_pid));
        owner
            .mark_ready()
            .expect("fixture advances its child to ready");
        assert_eq!(owner.phase(), OverlayChildPhase::Ready);
        assert_eq!(owner.generation(), Some(generation.as_str()));
        assert_eq!(owner.display_pid(), Some(display_pid));
        owner
            .begin_stop()
            .expect("fixture starts graceful stop through its pidfd-identified child");
        assert_eq!(owner.phase(), OverlayChildPhase::StopPending);
        assert_eq!(owner.generation(), Some(generation.as_str()));
        assert_eq!(owner.display_pid(), Some(display_pid));
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(1);
        while owner
            .try_wait()
            .expect("fixture polls its owned child")
            .is_none()
        {
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert_eq!(owner.phase(), OverlayChildPhase::Stopped);
        assert_eq!(owner.generation(), None);
        assert_eq!(owner.display_pid(), None);
        assert!(owner.poll_fd().is_none());
        assert!(!generation.is_empty());
    }

    #[test]
    fn rejected_transitions_preserve_the_current_structural_state() {
        let temp = crate::test_temp::tempdir().expect("isolated child protocol fixture");
        let root = temp.path().join("daemon-commands").join("v2");
        let mut owner = OverlayChildOwner::new(root);

        assert!(owner.mark_committing().is_err());
        assert!(owner.mark_ready().is_err());
        assert_eq!(owner.phase(), OverlayChildPhase::Stopped);
        assert_eq!(owner.generation(), None);
        assert_eq!(owner.display_pid(), None);
    }

    #[test]
    fn readiness_requires_exact_generation_pid_and_start_identity() {
        let temp = crate::test_temp::tempdir().expect("fixture creates its runtime directory");
        let root = temp.path().join("daemon-commands").join("v2");
        let broker = crate::process_broker::start_for_runtime()
            .expect("fixture starts its process-broker owner");
        let mut owner = OverlayChildOwner::new(root.clone());
        owner
            .reserve()
            .expect("fixture reserves an overlay generation");
        let generation = owner
            .generation()
            .expect("fixture reservation installs a generation")
            .to_owned();
        let child = broker
            .handle()
            .spawn(
                crate::process_broker::HelperKind::TestSleep,
                crate::process_broker::HelperLifetime::OwnedChild,
                std::ffi::OsStr::new("sleep"),
                [std::ffi::OsStr::new("30")],
                Vec::new(),
            )
            .expect("fixture spawns its long-running overlay child");
        let pid = child.id();
        owner
            .start(child)
            .expect("fixture installs its broker-owned overlay child");
        fs::create_dir_all(ready_dir(&root)).expect("fixture creates its readiness directory");
        let record = OverlayReadyRecord {
            protocol_version: super::super::wire::DAEMON_CHILD_PROTOCOL_VERSION,
            generation,
            pid,
            process_start_ticks: super::super::linux::process_start_ticks(pid)
                .expect("fixture reads its live child start identity"),
        };
        let bytes = super::super::wire::canonical_json(&record, 1024)
            .expect("fixture serializes its readiness record");
        fs::write(ready_path(&root, &record.generation), bytes)
            .expect("fixture writes its readiness record");
        let signals = OverlayActiveRecord {
            protocol_version: record.protocol_version,
            generation: record.generation.clone(),
            pid: record.pid,
            process_start_ticks: record.process_start_ticks,
        };
        let bytes = super::super::wire::canonical_json(&signals, 1024)
            .expect("fixture serializes its signal-authority record");
        fs::write(signals_path(&root, &record.generation), bytes)
            .expect("fixture writes its signal-authority record");

        let daemon_token = super::super::ProtocolToken::generate()
            .expect("fixture generates its daemon protocol token")
            .to_string();
        owner
            .wait_until_ready(Duration::from_secs(1), &daemon_token)
            .expect("fixture child presents matching readiness and signal proofs");
        assert_eq!(owner.phase(), OverlayChildPhase::Ready);
        owner
            .force_kill_and_wait()
            .expect("fixture terminates and reaps its overlay child");
    }

    #[test]
    fn recovery_removes_only_proofs_for_dead_child_identities() {
        let temp = crate::test_temp::tempdir().expect("fixture creates its runtime directory");
        let root = temp.path().join("daemon-commands").join("v2");
        let generation = super::super::ProtocolId::generate()
            .expect("fixture generates its stale child identity")
            .to_string();
        fs::create_dir_all(ready_dir(&root)).expect("fixture creates its readiness directory");
        let active = OverlayActiveRecord {
            protocol_version: super::super::wire::DAEMON_CHILD_PROTOCOL_VERSION,
            generation: generation.clone(),
            pid: u32::MAX,
            process_start_ticks: 1,
        };
        let bytes = super::super::wire::canonical_json(&active, 1024)
            .expect("fixture serializes its stale active-child proof");
        fs::write(active_path(&root, &generation), bytes)
            .expect("fixture writes its stale active-child proof");

        recover_stale_child_records(&root).expect("fixture recovers its stale child proofs");
        assert!(!active_path(&root, &generation).exists());
    }
}
