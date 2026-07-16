use anyhow::{Context, Result};
use log::warn;
use std::env;
use std::os::fd::{AsRawFd, RawFd};
use std::time::{Duration, Instant};
use wayland_client::{EventQueue, backend::ReadEventsGuard, backend::WaylandError};

use super::super::state::WaylandState;
use super::runtime_wake::{RuntimeWakeDrain, RuntimeWakeSource};
use crate::{RESUME_SESSION_ENV, runtime_session_override};

pub(super) fn friendly_capture_error(error: &str) -> String {
    let lower = error.to_lowercase();

    if is_missing_tool(&lower, "slurp") {
        return "Missing screenshot tool: slurp. Install slurp + grim and try again.".to_string();
    }
    if is_missing_tool(&lower, "grim") {
        return "Missing screenshot tool: grim. Install grim and try again.".to_string();
    }
    if is_missing_tool(&lower, "wl-copy") {
        return "Missing clipboard tool: wl-clipboard (wl-copy). Install it and try again."
            .to_string();
    }
    if lower.contains("requestcancelled") || lower.contains("cancelled") {
        "Screen capture cancelled by user".to_string()
    } else if lower.contains("permission") {
        "Permission denied. Enable screen sharing in system settings.".to_string()
    } else if lower.contains("portal returned error code") {
        "Screen capture failed. If you use Hyprland, Niri, or another wlroots desktop, install grim + slurp. Otherwise check the desktop screen capture service."
            .to_string()
    } else if lower.contains("busy") {
        "Screen capture in progress. Try again in a moment.".to_string()
    } else {
        "Screen capture failed. Please try again.".to_string()
    }
}

fn is_missing_tool(lower: &str, tool: &str) -> bool {
    lower.contains(tool)
        && (lower.contains("no such file")
            || lower.contains("not found")
            || lower.contains("failed to run")
            || lower.contains("failed to spawn"))
}

fn timeout_to_poll_ms(timeout: Option<Duration>) -> i32 {
    timeout
        .map(|dur| dur.as_millis().min(i32::MAX as u128) as i32)
        .unwrap_or(-1)
}

fn normalize_read_result(result: Result<usize, WaylandError>) -> Result<usize, WaylandError> {
    match result {
        Ok(n) => Ok(n),
        Err(WaylandError::Io(err)) if err.kind() == std::io::ErrorKind::WouldBlock => Ok(0),
        Err(e) => Err(e),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RuntimePollReadiness {
    wayland: bool,
    wake: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RuntimeReadOutcome {
    wayland_read: bool,
    wake_drain: Option<RuntimeWakeDrain>,
}

trait PreparedWaylandRead {
    fn connection_raw_fd(&self) -> RawFd;
    fn read(self) -> Result<usize, WaylandError>;
}

impl PreparedWaylandRead for ReadEventsGuard {
    fn connection_raw_fd(&self) -> RawFd {
        self.connection_fd().as_raw_fd()
    }

    fn read(self) -> Result<usize, WaylandError> {
        ReadEventsGuard::read(self)
    }
}

fn validate_poll_readiness(pollfd: &libc::pollfd, label: &str) -> std::io::Result<bool> {
    let terminal = pollfd.revents & (libc::POLLERR | libc::POLLHUP | libc::POLLNVAL);
    if terminal != 0 {
        return Err(std::io::Error::other(format!(
            "{label} poll descriptor failed with readiness {:#x}",
            pollfd.revents
        )));
    }
    let unexpected = pollfd.revents & !libc::POLLIN;
    if unexpected != 0 {
        return Err(std::io::Error::other(format!(
            "{label} poll descriptor returned unexpected readiness {:#x}",
            pollfd.revents
        )));
    }
    Ok(pollfd.revents & libc::POLLIN != 0)
}

fn poll_runtime_fds_with(
    wayland_fd: RawFd,
    wake_fd: RawFd,
    timeout: Option<Duration>,
    mut poll_once: impl FnMut(&mut [libc::pollfd], i32) -> std::io::Result<i32>,
) -> std::io::Result<RuntimePollReadiness> {
    let deadline = timeout.and_then(|timeout| Instant::now().checked_add(timeout));
    let mut timeout_ms = timeout_to_poll_ms(timeout);
    loop {
        let mut pollfds = [
            libc::pollfd {
                fd: wayland_fd,
                events: libc::POLLIN,
                revents: 0,
            },
            libc::pollfd {
                fd: wake_fd,
                events: libc::POLLIN,
                revents: 0,
            },
        ];
        match poll_once(&mut pollfds, timeout_ms) {
            Ok(0) => {
                return Ok(RuntimePollReadiness {
                    wayland: false,
                    wake: false,
                });
            }
            Ok(_) => {
                let readiness = RuntimePollReadiness {
                    wayland: validate_poll_readiness(&pollfds[0], "Wayland")?,
                    wake: validate_poll_readiness(&pollfds[1], "runtime wake")?,
                };
                if !readiness.wayland && !readiness.wake {
                    return Err(std::io::Error::other(
                        "runtime poll reported readiness without a readable descriptor",
                    ));
                }
                return Ok(readiness);
            }
            Err(err) if err.kind() == std::io::ErrorKind::Interrupted => {
                timeout_ms = deadline
                    .map(|deadline| {
                        timeout_to_poll_ms(Some(deadline.saturating_duration_since(Instant::now())))
                    })
                    .unwrap_or(-1);
            }
            Err(err) => return Err(err),
        }
    }
}

fn poll_runtime_fds(
    wayland_fd: RawFd,
    wake_fd: RawFd,
    timeout: Option<Duration>,
) -> std::io::Result<RuntimePollReadiness> {
    poll_runtime_fds_with(wayland_fd, wake_fd, timeout, |pollfds, timeout_ms| {
        // SAFETY: pollfds is a live mutable slice for the duration of this call,
        // and both descriptors remain borrowed by their runtime owners.
        let ready = unsafe {
            libc::poll(
                pollfds.as_mut_ptr(),
                pollfds.len() as libc::nfds_t,
                timeout_ms,
            )
        };
        if ready < 0 {
            Err(std::io::Error::last_os_error())
        } else {
            Ok(ready)
        }
    })
}

fn read_events_with_runtime_wake(
    guard: impl PreparedWaylandRead,
    runtime_wake: &RuntimeWakeSource,
    timeout: Option<Duration>,
) -> Result<RuntimeReadOutcome> {
    let readiness = poll_runtime_fds(
        guard.connection_raw_fd(),
        runtime_wake.poll_fd().as_raw_fd(),
        timeout,
    )
    .context("runtime readiness poll failed")?;

    let wayland_read = if readiness.wayland {
        let _ =
            normalize_read_result(guard.read()).map_err(|err| anyhow::anyhow!(err.to_string()))?;
        true
    } else {
        // Dropping the guard cancels the prepared read when only a runtime wake
        // or a real deadline made the poll return.
        drop(guard);
        false
    };
    let wake_drain = readiness
        .wake
        .then(|| {
            runtime_wake
                .drain()
                .context("failed to drain runtime wake descriptor")
        })
        .transpose()?;

    Ok(RuntimeReadOutcome {
        wayland_read,
        wake_drain,
    })
}

trait RuntimeDispatchOps {
    fn dispatch_pending(&mut self) -> Result<usize>;
    fn take_toolbar_drag_flush_requested(&mut self) -> bool;
    fn flush(&mut self) -> Result<()>;
    fn poll_prepared_read(
        &mut self,
        timeout: Option<Duration>,
    ) -> Result<Option<RuntimeReadOutcome>>;
    fn process_runtime_wake(&mut self) -> Result<()>;
}

fn dispatch_runtime_cycle(
    ops: &mut impl RuntimeDispatchOps,
    timeout: Option<Duration>,
) -> Result<()> {
    let dispatched = ops.dispatch_pending()?;
    if dispatched > 0 {
        if ops.take_toolbar_drag_flush_requested() {
            ops.flush()?;
        }
        return Ok(());
    }

    ops.flush()?;
    if let Some(outcome) = ops.poll_prepared_read(timeout)? {
        if outcome.wake_drain.is_some_and(|drain| drain.limit_reached) {
            log::debug!(
                "Runtime wake drain reached its bounded read limit; residual readiness will force another outer pass"
            );
        }
        if outcome.wake_drain.is_some() {
            ops.process_runtime_wake()?;
        }
        if outcome.wayland_read {
            ops.dispatch_pending()?;
        }
    }

    Ok(())
}

struct RealRuntimeDispatchOps<'a, F> {
    event_queue: &'a mut EventQueue<WaylandState>,
    state: &'a mut WaylandState,
    runtime_wake: &'a RuntimeWakeSource,
    on_runtime_wake: F,
}

impl<F> RuntimeDispatchOps for RealRuntimeDispatchOps<'_, F>
where
    F: FnMut(&mut WaylandState) -> Result<()>,
{
    fn dispatch_pending(&mut self) -> Result<usize> {
        self.event_queue
            .dispatch_pending(self.state)
            .map_err(|err| anyhow::anyhow!(err.to_string()))
    }

    fn take_toolbar_drag_flush_requested(&mut self) -> bool {
        self.state.take_toolbar_drag_flush_requested()
    }

    fn flush(&mut self) -> Result<()> {
        self.event_queue
            .flush()
            .map_err(|err| anyhow::anyhow!("Wayland flush error: {err}"))
    }

    fn poll_prepared_read(
        &mut self,
        timeout: Option<Duration>,
    ) -> Result<Option<RuntimeReadOutcome>> {
        self.event_queue
            .prepare_read()
            .map(|guard| read_events_with_runtime_wake(guard, self.runtime_wake, timeout))
            .transpose()
    }

    fn process_runtime_wake(&mut self) -> Result<()> {
        (self.on_runtime_wake)(self.state)
    }
}

pub(super) fn dispatch_with_timeout(
    event_queue: &mut EventQueue<WaylandState>,
    state: &mut WaylandState,
    runtime_wake: &RuntimeWakeSource,
    on_runtime_wake: impl FnMut(&mut WaylandState) -> Result<()>,
    timeout: Option<Duration>,
) -> Result<()> {
    let mut ops = RealRuntimeDispatchOps {
        event_queue,
        state,
        runtime_wake,
        on_runtime_wake,
    };
    dispatch_runtime_cycle(&mut ops, timeout)
}

pub(super) fn resume_override_from_env() -> Option<bool> {
    if let Some(runtime) = runtime_session_override() {
        return Some(runtime);
    }
    match env::var(RESUME_SESSION_ENV) {
        Ok(raw) => {
            let normalized = raw.trim().to_ascii_lowercase();
            match normalized.as_str() {
                "1" | "true" | "yes" | "on" | "resume" | "enable" | "enabled" => Some(true),
                "0" | "false" | "no" | "off" | "disable" | "disabled" => Some(false),
                _ => {
                    warn!(
                        "Ignoring invalid {} value '{}'; expected on/off/true/false",
                        RESUME_SESSION_ENV, raw
                    );
                    None
                }
            }
        }
        Err(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::io::Write;
    use std::os::unix::net::UnixStream;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Mutex, OnceLock};

    use super::*;
    use crate::set_runtime_session_override;

    fn env_mutex() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    #[test]
    fn timeout_to_poll_ms_supports_none_and_caps_large_values() {
        assert_eq!(timeout_to_poll_ms(None), -1);
        assert_eq!(timeout_to_poll_ms(Some(Duration::from_millis(15))), 15);

        let huge = Duration::from_millis(i32::MAX as u64 + 1000);
        assert_eq!(timeout_to_poll_ms(Some(huge)), i32::MAX);
    }

    #[test]
    fn normalize_read_result_maps_would_block_to_zero() {
        let err = WaylandError::Io(std::io::Error::from(std::io::ErrorKind::WouldBlock));
        assert_eq!(normalize_read_result(Err(err)).unwrap(), 0);
    }

    #[test]
    fn normalize_read_result_preserves_other_errors() {
        let err = WaylandError::Io(std::io::Error::from(std::io::ErrorKind::BrokenPipe));
        let actual = normalize_read_result(Err(err)).unwrap_err();
        match actual {
            WaylandError::Io(io_err) => {
                assert_eq!(io_err.kind(), std::io::ErrorKind::BrokenPipe);
            }
            other => panic!("expected io error, got {other}"),
        }
    }

    #[test]
    fn runtime_poll_observes_wake_only_readiness() {
        let (wayland_read, _wayland_write) = UnixStream::pair().unwrap();
        let wake = RuntimeWakeSource::new().unwrap();
        wake.handle().wake().unwrap();

        assert_eq!(
            poll_runtime_fds(
                wayland_read.as_raw_fd(),
                wake.poll_fd().as_raw_fd(),
                Some(Duration::ZERO),
            )
            .unwrap(),
            RuntimePollReadiness {
                wayland: false,
                wake: true,
            }
        );
    }

    #[test]
    fn runtime_poll_observes_wayland_only_readiness() {
        let (wayland_read, mut wayland_write) = UnixStream::pair().unwrap();
        let wake = RuntimeWakeSource::new().unwrap();
        wayland_write.write_all(&[1]).unwrap();

        assert_eq!(
            poll_runtime_fds(
                wayland_read.as_raw_fd(),
                wake.poll_fd().as_raw_fd(),
                Some(Duration::ZERO),
            )
            .unwrap(),
            RuntimePollReadiness {
                wayland: true,
                wake: false,
            }
        );
    }

    #[test]
    fn runtime_wake_preempts_a_future_deadline() {
        let (wayland_read, _wayland_write) = UnixStream::pair().unwrap();
        let wake = RuntimeWakeSource::new().unwrap();
        wake.handle().wake().unwrap();

        assert_eq!(
            poll_runtime_fds(
                wayland_read.as_raw_fd(),
                wake.poll_fd().as_raw_fd(),
                Some(Duration::from_secs(30)),
            )
            .unwrap(),
            RuntimePollReadiness {
                wayland: false,
                wake: true,
            }
        );
    }

    #[test]
    fn runtime_poll_observes_combined_wayland_and_wake_readiness() {
        let (wayland_read, mut wayland_write) = UnixStream::pair().unwrap();
        let wake = RuntimeWakeSource::new().unwrap();
        wayland_write.write_all(&[1]).unwrap();
        wake.handle().wake().unwrap();

        assert_eq!(
            poll_runtime_fds(
                wayland_read.as_raw_fd(),
                wake.poll_fd().as_raw_fd(),
                Some(Duration::ZERO),
            )
            .unwrap(),
            RuntimePollReadiness {
                wayland: true,
                wake: true,
            }
        );
    }

    struct FakePreparedWaylandRead {
        stream: UnixStream,
        reads: Arc<AtomicUsize>,
        cancellations: Arc<AtomicUsize>,
        read: bool,
    }

    impl PreparedWaylandRead for FakePreparedWaylandRead {
        fn connection_raw_fd(&self) -> RawFd {
            self.stream.as_raw_fd()
        }

        fn read(mut self) -> Result<usize, WaylandError> {
            self.read = true;
            self.reads.fetch_add(1, Ordering::Relaxed);
            Ok(1)
        }
    }

    impl Drop for FakePreparedWaylandRead {
        fn drop(&mut self) {
            if !self.read {
                self.cancellations.fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    fn fake_prepared_read(
        stream: UnixStream,
    ) -> (FakePreparedWaylandRead, Arc<AtomicUsize>, Arc<AtomicUsize>) {
        let reads = Arc::new(AtomicUsize::new(0));
        let cancellations = Arc::new(AtomicUsize::new(0));
        (
            FakePreparedWaylandRead {
                stream,
                reads: Arc::clone(&reads),
                cancellations: Arc::clone(&cancellations),
                read: false,
            },
            reads,
            cancellations,
        )
    }

    #[test]
    fn wake_only_readiness_cancels_the_prepared_wayland_read() {
        let (wayland_read, _wayland_write) = UnixStream::pair().unwrap();
        let (guard, reads, cancellations) = fake_prepared_read(wayland_read);
        let wake = RuntimeWakeSource::new().unwrap();
        wake.handle().wake().unwrap();

        let outcome = read_events_with_runtime_wake(guard, &wake, Some(Duration::ZERO)).unwrap();

        assert!(!outcome.wayland_read);
        assert!(outcome.wake_drain.is_some());
        assert_eq!(reads.load(Ordering::Relaxed), 0);
        assert_eq!(cancellations.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn combined_readiness_reads_wayland_and_drains_runtime_wake() {
        let (wayland_read, mut wayland_write) = UnixStream::pair().unwrap();
        wayland_write.write_all(&[1]).unwrap();
        let (guard, reads, cancellations) = fake_prepared_read(wayland_read);
        let wake = RuntimeWakeSource::new().unwrap();
        wake.handle().wake().unwrap();

        let outcome = read_events_with_runtime_wake(guard, &wake, Some(Duration::ZERO)).unwrap();

        assert!(outcome.wayland_read);
        assert!(outcome.wake_drain.is_some());
        assert_eq!(reads.load(Ordering::Relaxed), 1);
        assert_eq!(cancellations.load(Ordering::Relaxed), 0);
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum DispatchCall {
        DispatchPending,
        TakeToolbarFlush,
        Flush,
        PrepareAndPoll,
        ProcessRuntimeWake,
    }

    struct FakeRuntimeDispatchOps {
        pending_counts: VecDeque<usize>,
        toolbar_flush_requested: bool,
        prepared_outcome: Option<RuntimeReadOutcome>,
        calls: Vec<DispatchCall>,
    }

    impl FakeRuntimeDispatchOps {
        fn new(
            pending_counts: impl IntoIterator<Item = usize>,
            prepared_outcome: Option<RuntimeReadOutcome>,
        ) -> Self {
            Self {
                pending_counts: pending_counts.into_iter().collect(),
                toolbar_flush_requested: false,
                prepared_outcome,
                calls: Vec::new(),
            }
        }
    }

    impl RuntimeDispatchOps for FakeRuntimeDispatchOps {
        fn dispatch_pending(&mut self) -> Result<usize> {
            self.calls.push(DispatchCall::DispatchPending);
            Ok(self.pending_counts.pop_front().unwrap_or(0))
        }

        fn take_toolbar_drag_flush_requested(&mut self) -> bool {
            self.calls.push(DispatchCall::TakeToolbarFlush);
            self.toolbar_flush_requested
        }

        fn flush(&mut self) -> Result<()> {
            self.calls.push(DispatchCall::Flush);
            Ok(())
        }

        fn poll_prepared_read(
            &mut self,
            _timeout: Option<Duration>,
        ) -> Result<Option<RuntimeReadOutcome>> {
            self.calls.push(DispatchCall::PrepareAndPoll);
            Ok(self.prepared_outcome)
        }

        fn process_runtime_wake(&mut self) -> Result<()> {
            self.calls.push(DispatchCall::ProcessRuntimeWake);
            Ok(())
        }
    }

    #[test]
    fn pending_wayland_events_return_without_prepare_read_or_poll() {
        let mut ops = FakeRuntimeDispatchOps::new([1], None);

        dispatch_runtime_cycle(&mut ops, None).unwrap();

        assert_eq!(
            ops.calls,
            [
                DispatchCall::DispatchPending,
                DispatchCall::TakeToolbarFlush,
            ]
        );
    }

    #[test]
    fn pending_toolbar_drag_performs_only_its_conditional_flush() {
        let mut ops = FakeRuntimeDispatchOps::new([1], None);
        ops.toolbar_flush_requested = true;

        dispatch_runtime_cycle(&mut ops, None).unwrap();

        assert_eq!(
            ops.calls,
            [
                DispatchCall::DispatchPending,
                DispatchCall::TakeToolbarFlush,
                DispatchCall::Flush,
            ]
        );
    }

    #[test]
    fn wake_only_readiness_processes_runtime_wake_exactly_once() {
        let mut ops = FakeRuntimeDispatchOps::new(
            [0],
            Some(RuntimeReadOutcome {
                wayland_read: false,
                wake_drain: Some(RuntimeWakeDrain {
                    reads: 1,
                    limit_reached: false,
                }),
            }),
        );

        dispatch_runtime_cycle(&mut ops, None).unwrap();

        assert_eq!(
            ops.calls,
            [
                DispatchCall::DispatchPending,
                DispatchCall::Flush,
                DispatchCall::PrepareAndPoll,
                DispatchCall::ProcessRuntimeWake,
            ]
        );
    }

    #[test]
    fn combined_readiness_processes_wake_once_before_wayland_dispatch() {
        let mut ops = FakeRuntimeDispatchOps::new(
            [0, 1],
            Some(RuntimeReadOutcome {
                wayland_read: true,
                wake_drain: Some(RuntimeWakeDrain {
                    reads: 1,
                    limit_reached: false,
                }),
            }),
        );

        dispatch_runtime_cycle(&mut ops, None).unwrap();

        assert_eq!(
            ops.calls,
            [
                DispatchCall::DispatchPending,
                DispatchCall::Flush,
                DispatchCall::PrepareAndPoll,
                DispatchCall::ProcessRuntimeWake,
                DispatchCall::DispatchPending,
            ]
        );
    }

    #[test]
    fn runtime_poll_retries_interruption() {
        let mut calls = 0;
        let readiness = poll_runtime_fds_with(10, 11, None, |pollfds, _| {
            calls += 1;
            if calls == 1 {
                return Err(std::io::Error::from(std::io::ErrorKind::Interrupted));
            }
            pollfds[1].revents = libc::POLLIN;
            Ok(1)
        })
        .unwrap();

        assert_eq!(calls, 2);
        assert_eq!(
            readiness,
            RuntimePollReadiness {
                wayland: false,
                wake: true,
            }
        );
    }

    #[test]
    fn runtime_poll_rejects_invalid_descriptor_readiness() {
        let err = poll_runtime_fds_with(10, 11, None, |pollfds, _| {
            pollfds[1].revents = libc::POLLNVAL;
            Ok(1)
        })
        .unwrap_err();

        assert!(err.to_string().contains("runtime wake"));
        assert!(err.to_string().contains("readiness"));
    }

    #[test]
    fn runtime_poll_timeout_reports_no_readiness() {
        let readiness = poll_runtime_fds_with(10, 11, Some(Duration::ZERO), |_, timeout_ms| {
            assert_eq!(timeout_ms, 0);
            Ok(0)
        })
        .unwrap();

        assert_eq!(
            readiness,
            RuntimePollReadiness {
                wayland: false,
                wake: false,
            }
        );
    }

    #[test]
    fn friendly_capture_error_covers_known_classes() {
        assert_eq!(
            friendly_capture_error("failed to spawn slurp: No such file"),
            "Missing screenshot tool: slurp. Install slurp + grim and try again."
        );
        assert_eq!(
            friendly_capture_error("grim not found"),
            "Missing screenshot tool: grim. Install grim and try again."
        );
        assert_eq!(
            friendly_capture_error("wl-copy failed to run"),
            "Missing clipboard tool: wl-clipboard (wl-copy). Install it and try again."
        );
        assert_eq!(
            friendly_capture_error("RequestCancelled by user"),
            "Screen capture cancelled by user"
        );
        assert_eq!(
            friendly_capture_error("permission denied"),
            "Permission denied. Enable screen sharing in system settings."
        );
        assert_eq!(
            friendly_capture_error("portal returned error code 2"),
            "Screen capture failed. If you use Hyprland, Niri, or another wlroots desktop, install grim + slurp. Otherwise check the desktop screen capture service."
        );
        assert_eq!(
            friendly_capture_error("resource busy"),
            "Screen capture in progress. Try again in a moment."
        );
        assert_eq!(
            friendly_capture_error("something unexpected"),
            "Screen capture failed. Please try again."
        );
    }

    #[test]
    fn resume_override_from_env_prefers_runtime_override() {
        let _guard = env_mutex().lock().unwrap();

        // SAFETY: test serialized by env mutex.
        unsafe {
            std::env::set_var(RESUME_SESSION_ENV, "off");
        }
        set_runtime_session_override(Some(true));

        assert_eq!(resume_override_from_env(), Some(true));

        set_runtime_session_override(None);
        // SAFETY: test serialized by env mutex.
        unsafe {
            std::env::remove_var(RESUME_SESSION_ENV);
        }
    }

    #[test]
    fn resume_override_from_env_parses_expected_values() {
        let _guard = env_mutex().lock().unwrap();
        set_runtime_session_override(None);

        // SAFETY: test serialized by env mutex.
        unsafe {
            std::env::set_var(RESUME_SESSION_ENV, "enabled");
        }
        assert_eq!(resume_override_from_env(), Some(true));

        // SAFETY: test serialized by env mutex.
        unsafe {
            std::env::set_var(RESUME_SESSION_ENV, "0");
        }
        assert_eq!(resume_override_from_env(), Some(false));

        // SAFETY: test serialized by env mutex.
        unsafe {
            std::env::set_var(RESUME_SESSION_ENV, "maybe");
        }
        assert_eq!(resume_override_from_env(), None);

        // SAFETY: test serialized by env mutex.
        unsafe {
            std::env::remove_var(RESUME_SESSION_ENV);
        }
    }
}
