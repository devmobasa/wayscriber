use std::fs;
use std::io::ErrorKind;
use std::os::fd::{AsFd, AsRawFd, BorrowedFd, FromRawFd, OwnedFd};
use std::os::unix::fs::PermissionsExt;
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use serde::{Deserialize, Serialize};

use super::wire::fresh_id;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OverlayChildPhase {
    Stopped,
    Probing,
    Reserved,
    Starting,
    Committing,
    Ready,
    StopPending,
}

#[derive(Debug)]
struct OwnedOverlayChild {
    display_pid: u32,
    pidfd: OwnedFd,
    child: crate::process_broker::BrokerChild,
}

#[derive(Debug)]
pub(crate) struct OverlayChildOwner {
    phase: OverlayChildPhase,
    generation: Option<String>,
    owned: Option<OwnedOverlayChild>,
}

impl Default for OverlayChildOwner {
    fn default() -> Self {
        Self {
            phase: OverlayChildPhase::Stopped,
            generation: None,
            owned: None,
        }
    }
}

impl OverlayChildOwner {
    #[cfg(test)]
    pub(crate) fn phase(&self) -> OverlayChildPhase {
        self.phase
    }

    pub(crate) fn display_pid(&self) -> Option<u32> {
        self.owned.as_ref().map(|owned| owned.display_pid)
    }

    pub(crate) fn poll_fd(&self) -> Option<BorrowedFd<'_>> {
        self.owned.as_ref().map(|owned| owned.pidfd.as_fd())
    }

    pub(crate) fn generation(&self) -> Option<&str> {
        self.generation.as_deref()
    }

    pub(crate) fn reserve(&mut self) -> Result<&str> {
        if self.owned.is_some() || self.phase != OverlayChildPhase::Stopped {
            bail!("overlay child owner is not stopped");
        }
        self.phase = OverlayChildPhase::Probing;
        let generation = match fresh_id() {
            Ok(generation) => generation,
            Err(error) => {
                self.phase = OverlayChildPhase::Stopped;
                return Err(error);
            }
        };
        let ready_path = ready_path(&generation);
        if let Err(error) = fs::remove_file(&ready_path)
            && error.kind() != ErrorKind::NotFound
        {
            self.phase = OverlayChildPhase::Stopped;
            return Err(error).context("failed to clear stale overlay readiness record");
        }
        if let Err(error) = fs::remove_file(active_path(&generation))
            && error.kind() != ErrorKind::NotFound
        {
            self.phase = OverlayChildPhase::Stopped;
            return Err(error).context("failed to clear stale overlay active record");
        }
        if let Err(error) = fs::remove_file(enabled_path(&generation))
            && error.kind() != ErrorKind::NotFound
        {
            self.phase = OverlayChildPhase::Stopped;
            return Err(error).context("failed to clear stale overlay enable record");
        }
        if let Err(error) = fs::remove_file(signals_path(&generation))
            && error.kind() != ErrorKind::NotFound
        {
            self.phase = OverlayChildPhase::Stopped;
            return Err(error).context("failed to clear stale overlay signal record");
        }
        self.generation = Some(generation);
        self.phase = OverlayChildPhase::Reserved;
        Ok(self.generation().expect("reserved generation exists"))
    }

    pub(crate) fn start(&mut self, child: crate::process_broker::BrokerChild) -> Result<()> {
        if self.phase != OverlayChildPhase::Reserved {
            let _ = child.kill_wait();
            bail!("overlay child owner is not reserved");
        }
        if self.generation.is_none() {
            let _ = child.kill_wait();
            bail!("reserved overlay generation disappeared");
        }
        let display_pid = child.id();
        let pidfd = match super::linux::open_pidfd(display_pid) {
            Ok(pidfd) => pidfd,
            Err(error) => {
                let _ = child.kill_wait();
                return Err(error).with_context(|| {
                    format!("failed to identify overlay child {display_pid} by pidfd")
                });
            }
        };
        self.owned = Some(OwnedOverlayChild {
            display_pid,
            pidfd,
            child,
        });
        self.phase = OverlayChildPhase::Starting;
        Ok(())
    }

    pub(crate) fn mark_committing(&mut self) -> Result<()> {
        if self.phase != OverlayChildPhase::Starting || self.owned.is_none() {
            bail!("only a starting overlay child can commit");
        }
        self.phase = OverlayChildPhase::Committing;
        Ok(())
    }

    pub(crate) fn mark_ready(&mut self) -> Result<()> {
        if self.phase != OverlayChildPhase::Committing || self.owned.is_none() {
            bail!("only a committing overlay child can become ready");
        }
        self.phase = OverlayChildPhase::Ready;
        Ok(())
    }

    pub(crate) fn wait_until_ready(&mut self, timeout: Duration, daemon_token: &str) -> Result<()> {
        super::wire::validate_token(daemon_token)?;
        let deadline = super::BootClock::now()?.checked_add(timeout)?;
        let generation = self
            .generation()
            .ok_or_else(|| anyhow!("overlay child has no generation"))?
            .to_owned();
        let expected_pid = self
            .display_pid()
            .ok_or_else(|| anyhow!("overlay child has no display pid"))?;
        let path = ready_path(&generation);
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
                        &signals_path(&generation),
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
                        &enabled_path(&generation),
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
                    fs::remove_file(signals_path(&generation))?;
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
        if let Some(generation) = self.generation().map(str::to_owned) {
            clear_generation_records(&generation);
        }
        if self.display_pid().is_some_and(|pid| pid != 0) {
            let _ = self.force_kill_and_wait();
        } else {
            self.owned = None;
            self.generation = None;
            self.phase = OverlayChildPhase::Stopped;
        }
    }

    pub(crate) fn signal(&self, signal: i32) -> Result<()> {
        let owned = self
            .owned
            .as_ref()
            .ok_or_else(|| anyhow!("overlay child is stopped"))?;
        // The retained pidfd proves that this generation still identifies the
        // same kernel process. The opaque broker handle is the signal
        // authority; the daemon never sends a signal using the display PID.
        let _identity = &owned.pidfd;
        owned.child.signal(signal)
    }

    pub(crate) fn try_wait(&mut self) -> Result<Option<i32>> {
        let Some(owned) = self.owned.as_mut() else {
            return Ok(None);
        };
        match owned
            .child
            .try_wait()
            .context("failed to query overlay child")?
        {
            Some(status) => {
                if let Some(generation) = self.generation().map(str::to_owned) {
                    clear_generation_records(&generation);
                }
                self.owned = None;
                self.generation = None;
                self.phase = OverlayChildPhase::Stopped;
                Ok(Some(status))
            }
            None => Ok(None),
        }
    }

    pub(crate) fn begin_stop(&mut self) -> Result<()> {
        if self.owned.is_none() {
            self.generation = None;
            self.phase = OverlayChildPhase::Stopped;
            return Ok(());
        }
        self.phase = OverlayChildPhase::StopPending;
        self.signal(libc::SIGTERM)
    }

    pub(crate) fn force_kill_and_wait(&mut self) -> Result<i32> {
        let owned = self
            .owned
            .as_mut()
            .ok_or_else(|| anyhow!("overlay child is already stopped"))?;
        let status = owned
            .child
            .kill_wait()
            .context("broker failed to kill and reap overlay child")?;
        if let Some(generation) = self.generation().map(str::to_owned) {
            clear_generation_records(&generation);
        }
        self.owned = None;
        self.generation = None;
        self.phase = OverlayChildPhase::Stopped;
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

fn ready_dir() -> std::path::PathBuf {
    super::command_root().join("children")
}

fn ready_path(generation: &str) -> std::path::PathBuf {
    ready_dir().join(format!("{generation}.ready"))
}

fn active_path(generation: &str) -> std::path::PathBuf {
    ready_dir().join(format!("{generation}.active"))
}

fn enabled_path(generation: &str) -> std::path::PathBuf {
    ready_dir().join(format!("{generation}.enabled"))
}

fn signals_path(generation: &str) -> std::path::PathBuf {
    ready_dir().join(format!("{generation}.signals"))
}

fn clear_generation_records(generation: &str) {
    let _ = fs::remove_file(ready_path(generation));
    let _ = fs::remove_file(active_path(generation));
    let _ = fs::remove_file(enabled_path(generation));
    let _ = fs::remove_file(signals_path(generation));
}

pub(crate) fn recover_stale_child_records() -> Result<()> {
    let directory = ready_dir();
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

pub(crate) fn start_daemon_watchdog_from_environment() -> Result<()> {
    let Some(raw) = std::env::var_os(crate::env_vars::DAEMON_WATCHDOG_FD_ENV) else {
        return Ok(());
    };
    let raw = raw
        .to_str()
        .ok_or_else(|| anyhow!("daemon watchdog descriptor is not UTF-8"))?
        .parse::<i32>()
        .context("daemon watchdog descriptor is not numeric")?;
    if raw <= libc::STDERR_FILENO {
        bail!("daemon watchdog descriptor aliases standard I/O");
    }
    // SAFETY: a daemon-launched overlay receives sole ownership of this
    // inherited descriptor. The environment marker is removed immediately so
    // no later exec can reinterpret the descriptor number.
    let descriptor = unsafe { OwnedFd::from_raw_fd(raw) };
    // SAFETY: descriptor is owned and F_SETFD changes only exec inheritance.
    if unsafe { libc::fcntl(descriptor.as_raw_fd(), libc::F_SETFD, libc::FD_CLOEXEC) } != 0 {
        return Err(std::io::Error::last_os_error())
            .context("failed to protect daemon watchdog descriptor");
    }
    super::linux::validate_pidfd(descriptor.as_fd())
        .context("inherited daemon watchdog is not a live pidfd")?;
    // SAFETY: application startup has not launched runtime worker threads yet;
    // removing this private bootstrap marker prevents accidental inheritance.
    unsafe {
        std::env::remove_var(crate::env_vars::DAEMON_WATCHDOG_FD_ENV);
    }
    std::thread::Builder::new()
        .name("wayscriber-daemon-watchdog".into())
        .spawn(move || {
            let mut pollfd = libc::pollfd {
                fd: descriptor.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            };
            loop {
                // SAFETY: pollfd points to one initialized entry and the
                // descriptor remains owned by this thread.
                let result = unsafe { libc::poll(&mut pollfd, 1, -1) };
                if result > 0 {
                    // SAFETY: loss of the owning daemon is an irrevocable
                    // fail-stop condition for its internal overlay child.
                    unsafe { super::linux::fail_stop(70) }
                }
                if result < 0
                    && std::io::Error::last_os_error().kind() == std::io::ErrorKind::Interrupted
                {
                    continue;
                }
                // SAFETY: an unusable parent-death channel cannot safely
                // preserve overlay ownership.
                unsafe { super::linux::fail_stop(70) }
            }
        })
        .context("failed to start daemon watchdog")?;
    Ok(())
}

pub(crate) fn publish_ready_from_environment() -> Result<()> {
    let Some(generation) = std::env::var_os(crate::env_vars::OVERLAY_CHILD_GENERATION_ENV) else {
        return Ok(());
    };
    let generation = generation
        .to_str()
        .ok_or_else(|| anyhow!("overlay generation is not UTF-8"))?;
    super::wire::validate_id(generation)?;
    let directory = ready_dir();
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
        &active_path(generation),
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
        &ready_path(generation),
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

pub(crate) fn publish_signal_ready_from_environment() -> Result<()> {
    let Some(generation) = std::env::var_os(crate::env_vars::OVERLAY_CHILD_GENERATION_ENV) else {
        return Ok(());
    };
    let generation = generation
        .to_str()
        .ok_or_else(|| anyhow!("overlay generation is not UTF-8"))?;
    super::wire::validate_id(generation)?;
    let active_bytes = super::linux::read_bounded_regular_file(&active_path(generation), 1024)?;
    let active: OverlayActiveRecord = super::wire::parse_canonical_json(&active_bytes, 1024)?;
    if active.pid != std::process::id()
        || active.process_start_ticks != super::linux::current_process_start_ticks()?
        || active.generation != generation
    {
        bail!("cannot publish signal readiness for a different overlay child");
    }
    crate::durable_io::write_atomic(
        &signals_path(generation),
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

pub(crate) fn active_generation_from_environment() -> Result<ActiveGeneration> {
    let Some(generation) = std::env::var_os(crate::env_vars::OVERLAY_CHILD_GENERATION_ENV) else {
        return Ok(ActiveGeneration::Inactive);
    };
    let generation = generation
        .to_str()
        .ok_or_else(|| anyhow!("overlay generation is not UTF-8"))?;
    super::wire::validate_id(generation)?;
    let bytes = match super::linux::read_bounded_regular_file(&active_path(generation), 1024) {
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
        match super::linux::read_bounded_regular_file(&enabled_path(generation), 1024) {
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
pub(crate) fn enable_current_generation_for_test(daemon_token: &str) -> Result<()> {
    super::wire::validate_token(daemon_token)?;
    let generation = std::env::var(crate::env_vars::OVERLAY_CHILD_GENERATION_ENV)?;
    let bytes = super::linux::read_bounded_regular_file(&active_path(&generation), 1024)?;
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
        &enabled_path(&generation),
        &enabled,
        crate::durable_io::AtomicWriteOptions::private_runtime_file(),
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn child_generation_and_pidfd_own_signal_authority() {
        let broker = crate::process_broker::start_for_runtime().unwrap();
        let child = broker
            .broker()
            .spawn(
                crate::process_broker::HelperKind::TestSleep,
                crate::process_broker::HelperLifetime::OwnedChild,
                std::ffi::OsStr::new("sleep"),
                [std::ffi::OsStr::new("30")],
                Vec::new(),
            )
            .unwrap();
        let display_pid = child.id();
        let mut owner = OverlayChildOwner::default();
        owner.reserve().unwrap();
        owner.start(child).unwrap();
        let generation = owner.generation().unwrap().to_owned();
        assert_eq!(owner.display_pid(), Some(display_pid));
        assert_eq!(owner.phase(), OverlayChildPhase::Starting);
        owner.mark_committing().unwrap();
        owner.mark_ready().unwrap();
        owner.signal(libc::SIGTERM).unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(1);
        while owner.try_wait().unwrap().is_none() {
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert_eq!(owner.phase(), OverlayChildPhase::Stopped);
        assert!(!generation.is_empty());
    }

    #[test]
    fn readiness_requires_exact_generation_pid_and_start_identity() {
        let _environment = crate::test_env::lock();
        let temp = crate::test_temp::tempdir().unwrap();
        let previous = std::env::var_os(crate::env_vars::XDG_RUNTIME_DIR_ENV);
        // SAFETY: serialized by the test environment mutex.
        unsafe { std::env::set_var(crate::env_vars::XDG_RUNTIME_DIR_ENV, temp.path()) };
        let broker = crate::process_broker::start_for_runtime().unwrap();
        let mut owner = OverlayChildOwner::default();
        let generation = owner.reserve().unwrap().to_owned();
        let child = broker
            .broker()
            .spawn(
                crate::process_broker::HelperKind::TestSleep,
                crate::process_broker::HelperLifetime::OwnedChild,
                std::ffi::OsStr::new("sleep"),
                [std::ffi::OsStr::new("30")],
                Vec::new(),
            )
            .unwrap();
        let pid = child.id();
        owner.start(child).unwrap();
        fs::create_dir_all(ready_dir()).unwrap();
        let record = OverlayReadyRecord {
            protocol_version: super::super::wire::DAEMON_CHILD_PROTOCOL_VERSION,
            generation,
            pid,
            process_start_ticks: super::super::linux::process_start_ticks(pid).unwrap(),
        };
        let bytes = super::super::wire::canonical_json(&record, 1024).unwrap();
        fs::write(ready_path(&record.generation), bytes).unwrap();
        let signals = OverlayActiveRecord {
            protocol_version: record.protocol_version,
            generation: record.generation.clone(),
            pid: record.pid,
            process_start_ticks: record.process_start_ticks,
        };
        let bytes = super::super::wire::canonical_json(&signals, 1024).unwrap();
        fs::write(signals_path(&record.generation), bytes).unwrap();

        let daemon_token = super::super::ProtocolToken::generate().unwrap().to_string();
        owner
            .wait_until_ready(Duration::from_secs(1), &daemon_token)
            .unwrap();
        assert_eq!(owner.phase(), OverlayChildPhase::Ready);
        owner.force_kill_and_wait().unwrap();

        // SAFETY: this test still holds the environment mutex.
        unsafe {
            if let Some(previous) = previous {
                std::env::set_var(crate::env_vars::XDG_RUNTIME_DIR_ENV, previous);
            } else {
                std::env::remove_var(crate::env_vars::XDG_RUNTIME_DIR_ENV);
            }
        }
    }

    #[test]
    fn recovery_removes_only_proofs_for_dead_child_identities() {
        let _environment = crate::test_env::lock();
        let temp = crate::test_temp::tempdir().unwrap();
        let previous = std::env::var_os(crate::env_vars::XDG_RUNTIME_DIR_ENV);
        // SAFETY: serialized by the test environment mutex.
        unsafe { std::env::set_var(crate::env_vars::XDG_RUNTIME_DIR_ENV, temp.path()) };
        let generation = super::super::ProtocolId::generate().unwrap().to_string();
        fs::create_dir_all(ready_dir()).unwrap();
        let active = OverlayActiveRecord {
            protocol_version: super::super::wire::DAEMON_CHILD_PROTOCOL_VERSION,
            generation: generation.clone(),
            pid: u32::MAX,
            process_start_ticks: 1,
        };
        let bytes = super::super::wire::canonical_json(&active, 1024).unwrap();
        fs::write(active_path(&generation), bytes).unwrap();

        recover_stale_child_records().unwrap();
        assert!(!active_path(&generation).exists());

        // SAFETY: this test still holds the environment mutex.
        unsafe {
            if let Some(previous) = previous {
                std::env::set_var(crate::env_vars::XDG_RUNTIME_DIR_ENV, previous);
            } else {
                std::env::remove_var(crate::env_vars::XDG_RUNTIME_DIR_ENV);
            }
        }
    }
}
