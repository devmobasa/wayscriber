use anyhow::{Context, Result};
use log::warn;
use std::env;
use std::os::fd::{AsRawFd, RawFd};
use std::time::Duration;
use wayland_client::{EventQueue, backend::ReadEventsGuard, backend::WaylandError};

use super::super::state::WaylandState;
#[cfg(test)]
use super::runtime_wake::timeout_to_poll_ms;
use super::runtime_wake::{
    RuntimeWakeSource, TerminalReadinessPolicy, poll_with_retry, validate_poll_readiness,
};
use crate::RESUME_SESSION_ENV;

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
    signal: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RuntimeReadOutcome {
    wayland_read: bool,
    runtime_wake: bool,
    signal_ready: bool,
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

fn poll_runtime_fds_with(
    wayland_fd: RawFd,
    wake_fd: RawFd,
    signal_fd: RawFd,
    timeout: Option<Duration>,
    mut poll_once: impl FnMut(&mut [libc::pollfd], i32) -> std::io::Result<i32>,
) -> std::io::Result<RuntimePollReadiness> {
    poll_with_retry(timeout, |timeout_ms| {
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
            libc::pollfd {
                fd: signal_fd,
                events: libc::POLLIN,
                revents: 0,
            },
        ];
        match poll_once(&mut pollfds, timeout_ms) {
            Ok(0) => Ok(None),
            Ok(_) => {
                let readiness = RuntimePollReadiness {
                    // A Wayland socket can report final buffered protocol data
                    // together with HUP/ERR. Read and dispatch it first so the
                    // compositor's actual disconnect error is preserved.
                    wayland: validate_poll_readiness(
                        &pollfds[0],
                        "Wayland",
                        TerminalReadinessPolicy::ReadBuffered,
                    )?,
                    wake: validate_poll_readiness(
                        &pollfds[1],
                        "runtime wake",
                        TerminalReadinessPolicy::Reject,
                    )?,
                    signal: validate_poll_readiness(
                        &pollfds[2],
                        "signal source",
                        TerminalReadinessPolicy::Reject,
                    )?,
                };
                if !readiness.wayland && !readiness.wake && !readiness.signal {
                    return Err(std::io::Error::other(
                        "runtime poll reported readiness without a readable descriptor",
                    ));
                }
                Ok(Some(readiness))
            }
            Err(err) => Err(err),
        }
    })
    .map(|readiness| {
        readiness.unwrap_or(RuntimePollReadiness {
            wayland: false,
            wake: false,
            signal: false,
        })
    })
}

fn poll_runtime_fds(
    wayland_fd: RawFd,
    wake_fd: RawFd,
    signal_fd: RawFd,
    timeout: Option<Duration>,
) -> std::io::Result<RuntimePollReadiness> {
    poll_runtime_fds_with(
        wayland_fd,
        wake_fd,
        signal_fd,
        timeout,
        |pollfds, timeout_ms| {
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
        },
    )
}

fn read_events_with_runtime_sources(
    guard: impl PreparedWaylandRead,
    runtime_wake: &RuntimeWakeSource,
    signal_fd: RawFd,
    timeout: Option<Duration>,
) -> Result<RuntimeReadOutcome> {
    let readiness = poll_runtime_fds(
        guard.connection_raw_fd(),
        runtime_wake.poll_fd().as_raw_fd(),
        signal_fd,
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
    if readiness.wake {
        runtime_wake
            .drain()
            .context("failed to drain runtime wake descriptor")?;
    }

    Ok(RuntimeReadOutcome {
        wayland_read,
        runtime_wake: readiness.wake,
        signal_ready: readiness.signal,
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
    fn process_woken_sources(&mut self) -> Result<()>;
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
        if outcome.runtime_wake || outcome.signal_ready {
            ops.process_woken_sources()?;
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
    signal_fd: RawFd,
    on_woken_sources: F,
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
            .map(|guard| {
                read_events_with_runtime_sources(guard, self.runtime_wake, self.signal_fd, timeout)
            })
            .transpose()
    }

    fn process_woken_sources(&mut self) -> Result<()> {
        (self.on_woken_sources)(self.state)
    }
}

pub(super) fn dispatch_with_timeout(
    event_queue: &mut EventQueue<WaylandState>,
    state: &mut WaylandState,
    runtime_wake: &RuntimeWakeSource,
    signal_fd: RawFd,
    on_woken_sources: impl FnMut(&mut WaylandState) -> Result<()>,
    timeout: Option<Duration>,
) -> Result<()> {
    let mut ops = RealRuntimeDispatchOps {
        event_queue,
        state,
        runtime_wake,
        signal_fd,
        on_woken_sources,
    };
    dispatch_runtime_cycle(&mut ops, timeout)
}

pub(super) fn resume_override_from_env(runtime_override: Option<bool>) -> Option<bool> {
    if let Some(runtime) = runtime_override {
        return Some(runtime);
    }
    match env::var(RESUME_SESSION_ENV) {
        Ok(raw) => match parse_resume_override(&raw) {
            Some(value) => Some(value),
            None => {
                warn!(
                    "Ignoring invalid {} value '{}'; expected on/off/true/false",
                    RESUME_SESSION_ENV, raw
                );
                None
            }
        },
        Err(_) => None,
    }
}

fn parse_resume_override(raw: &str) -> Option<bool> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" | "resume" | "enable" | "enabled" => Some(true),
        "0" | "false" | "no" | "off" | "disable" | "disabled" => Some(false),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::io::Write;
    use std::os::unix::net::UnixStream;
    use std::sync::mpsc::{Receiver, Sender, channel};

    use super::*;

    #[test]
    fn timeout_to_poll_ms_supports_none_and_caps_large_values() {
        assert_eq!(timeout_to_poll_ms(None), -1);
        assert_eq!(timeout_to_poll_ms(Some(Duration::ZERO)), 0);
        assert_eq!(timeout_to_poll_ms(Some(Duration::from_nanos(1))), 1);
        assert_eq!(timeout_to_poll_ms(Some(Duration::from_nanos(999_999))), 1);
        assert_eq!(timeout_to_poll_ms(Some(Duration::from_millis(15))), 15);

        let huge = Duration::from_millis(i32::MAX as u64 + 1000);
        assert_eq!(timeout_to_poll_ms(Some(huge)), i32::MAX);
    }

    #[test]
    fn normalize_read_result_maps_would_block_to_zero() {
        let err = WaylandError::Io(std::io::Error::from(std::io::ErrorKind::WouldBlock));
        assert_eq!(
            normalize_read_result(Err(err)).expect("fixture maps its injected WouldBlock error"),
            0
        );
    }

    #[test]
    fn normalize_read_result_preserves_other_errors() {
        let err = WaylandError::Io(std::io::Error::from(std::io::ErrorKind::BrokenPipe));
        let actual = normalize_read_result(Err(err)).unwrap_err();
        match actual {
            WaylandError::Io(io_err) => {
                assert_eq!(io_err.kind(), std::io::ErrorKind::BrokenPipe);
            }
            other => assert!(
                matches!(other, WaylandError::Io(_)),
                "fixture expected an I/O error, got {other}"
            ),
        }
    }

    #[test]
    fn runtime_poll_observes_wake_only_readiness() {
        let (wayland_read, _wayland_write) =
            UnixStream::pair().expect("fixture creates its idle Wayland socket");
        let (signal_read, _signal_write) =
            UnixStream::pair().expect("fixture creates its idle signal socket");
        let wake = RuntimeWakeSource::new().expect("fixture creates its runtime wake");
        wake.try_sender()
            .expect("test duplicates its runtime eventfd")
            .wake()
            .expect("test publishes a runtime wake");

        assert_eq!(
            poll_runtime_fds(
                wayland_read.as_raw_fd(),
                wake.poll_fd().as_raw_fd(),
                signal_read.as_raw_fd(),
                Some(Duration::ZERO),
            )
            .expect("fixture polls its runtime descriptors"),
            RuntimePollReadiness {
                wayland: false,
                wake: true,
                signal: false,
            }
        );
    }

    #[test]
    fn runtime_poll_observes_wayland_only_readiness() {
        let (wayland_read, mut wayland_write) =
            UnixStream::pair().expect("fixture creates its Wayland socket");
        let (signal_read, _signal_write) =
            UnixStream::pair().expect("fixture creates its idle signal socket");
        let wake = RuntimeWakeSource::new().expect("fixture creates its runtime wake");
        wayland_write
            .write_all(&[1])
            .expect("fixture makes its Wayland socket readable");

        assert_eq!(
            poll_runtime_fds(
                wayland_read.as_raw_fd(),
                wake.poll_fd().as_raw_fd(),
                signal_read.as_raw_fd(),
                Some(Duration::ZERO),
            )
            .expect("fixture polls its runtime descriptors"),
            RuntimePollReadiness {
                wayland: true,
                wake: false,
                signal: false,
            }
        );
    }

    #[test]
    fn runtime_poll_preserves_readable_wayland_data_when_peer_hangs_up() {
        let (wayland_read, mut wayland_write) =
            UnixStream::pair().expect("fixture creates its Wayland socket");
        let (signal_read, _signal_write) =
            UnixStream::pair().expect("fixture creates its idle signal socket");
        let wake = RuntimeWakeSource::new().expect("fixture creates its runtime wake");
        wayland_write
            .write_all(&[1])
            .expect("fixture makes its Wayland socket readable before hangup");
        drop(wayland_write);

        assert_eq!(
            poll_runtime_fds(
                wayland_read.as_raw_fd(),
                wake.poll_fd().as_raw_fd(),
                signal_read.as_raw_fd(),
                Some(Duration::ZERO),
            )
            .expect("fixture polls its runtime descriptors"),
            RuntimePollReadiness {
                wayland: true,
                wake: false,
                signal: false,
            }
        );
    }

    #[test]
    fn runtime_wake_preempts_a_future_deadline() {
        let (wayland_read, _wayland_write) =
            UnixStream::pair().expect("fixture creates its idle Wayland socket");
        let (signal_read, _signal_write) =
            UnixStream::pair().expect("fixture creates its idle signal socket");
        let wake = RuntimeWakeSource::new().expect("fixture creates its runtime wake");
        wake.try_sender()
            .expect("test duplicates its runtime eventfd")
            .wake()
            .expect("test publishes a runtime wake");

        assert_eq!(
            poll_runtime_fds(
                wayland_read.as_raw_fd(),
                wake.poll_fd().as_raw_fd(),
                signal_read.as_raw_fd(),
                Some(Duration::from_secs(30)),
            )
            .expect("fixture observes wake readiness before its future deadline"),
            RuntimePollReadiness {
                wayland: false,
                wake: true,
                signal: false,
            }
        );
    }

    #[test]
    fn runtime_poll_observes_combined_wayland_and_wake_readiness() {
        let (wayland_read, mut wayland_write) =
            UnixStream::pair().expect("fixture creates its Wayland socket");
        let (signal_read, _signal_write) =
            UnixStream::pair().expect("fixture creates its idle signal socket");
        let wake = RuntimeWakeSource::new().expect("fixture creates its runtime wake");
        wayland_write
            .write_all(&[1])
            .expect("fixture makes its Wayland socket readable");
        wake.try_sender()
            .expect("test duplicates its runtime eventfd")
            .wake()
            .expect("test publishes a runtime wake");

        assert_eq!(
            poll_runtime_fds(
                wayland_read.as_raw_fd(),
                wake.poll_fd().as_raw_fd(),
                signal_read.as_raw_fd(),
                Some(Duration::ZERO),
            )
            .expect("fixture observes combined runtime readiness"),
            RuntimePollReadiness {
                wayland: true,
                wake: true,
                signal: false,
            }
        );
    }

    #[test]
    fn runtime_poll_observes_signal_only_readiness() {
        let (wayland_read, _wayland_write) =
            UnixStream::pair().expect("fixture creates its idle Wayland socket");
        let (signal_read, mut signal_write) =
            UnixStream::pair().expect("fixture creates its signal socket");
        let wake = RuntimeWakeSource::new().expect("fixture creates its idle runtime wake");
        signal_write
            .write_all(&[1])
            .expect("fixture makes its signal socket readable");

        assert_eq!(
            poll_runtime_fds(
                wayland_read.as_raw_fd(),
                wake.poll_fd().as_raw_fd(),
                signal_read.as_raw_fd(),
                Some(Duration::ZERO),
            )
            .expect("fixture polls its signal-ready runtime descriptors"),
            RuntimePollReadiness {
                wayland: false,
                wake: false,
                signal: true,
            }
        );
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum PreparedReadObservation {
        Read,
        Cancelled,
    }

    struct FakePreparedWaylandRead {
        stream: UnixStream,
        observations: Sender<PreparedReadObservation>,
        read: bool,
    }

    impl PreparedWaylandRead for FakePreparedWaylandRead {
        fn connection_raw_fd(&self) -> RawFd {
            self.stream.as_raw_fd()
        }

        fn read(mut self) -> Result<usize, WaylandError> {
            self.read = true;
            let _ = self.observations.send(PreparedReadObservation::Read);
            Ok(1)
        }
    }

    impl Drop for FakePreparedWaylandRead {
        fn drop(&mut self) {
            if !self.read {
                let _ = self.observations.send(PreparedReadObservation::Cancelled);
            }
        }
    }

    fn fake_prepared_read(
        stream: UnixStream,
    ) -> (FakePreparedWaylandRead, Receiver<PreparedReadObservation>) {
        let (observations, received) = channel();
        (
            FakePreparedWaylandRead {
                stream,
                observations,
                read: false,
            },
            received,
        )
    }

    #[test]
    fn wake_only_readiness_cancels_the_prepared_wayland_read() {
        let (wayland_read, _wayland_write) =
            UnixStream::pair().expect("fixture creates its idle Wayland socket");
        let (signal_read, _signal_write) =
            UnixStream::pair().expect("fixture creates its idle signal socket");
        let (guard, observations) = fake_prepared_read(wayland_read);
        let wake = RuntimeWakeSource::new().expect("fixture creates its runtime wake");
        wake.try_sender()
            .expect("test duplicates its runtime eventfd")
            .wake()
            .expect("test publishes a runtime wake");

        let outcome = read_events_with_runtime_sources(
            guard,
            &wake,
            signal_read.as_raw_fd(),
            Some(Duration::ZERO),
        )
        .expect("fixture reads its wake-only runtime readiness");

        assert!(!outcome.wayland_read);
        assert!(outcome.runtime_wake);
        assert!(!outcome.signal_ready);
        assert_eq!(
            observations
                .recv()
                .expect("fixture observes its prepared-read cancellation"),
            PreparedReadObservation::Cancelled
        );
        assert!(observations.try_recv().is_err());
    }

    #[test]
    fn combined_readiness_reads_wayland_and_drains_runtime_wake() {
        let (wayland_read, mut wayland_write) =
            UnixStream::pair().expect("fixture creates its Wayland socket");
        let (signal_read, _signal_write) =
            UnixStream::pair().expect("fixture creates its idle signal socket");
        wayland_write
            .write_all(&[1])
            .expect("fixture makes its Wayland socket readable");
        let (guard, observations) = fake_prepared_read(wayland_read);
        let wake = RuntimeWakeSource::new().expect("fixture creates its runtime wake");
        wake.try_sender()
            .expect("test duplicates its runtime eventfd")
            .wake()
            .expect("test publishes a runtime wake");

        let outcome = read_events_with_runtime_sources(
            guard,
            &wake,
            signal_read.as_raw_fd(),
            Some(Duration::ZERO),
        )
        .expect("fixture reads its combined runtime readiness");

        assert!(outcome.wayland_read);
        assert!(outcome.runtime_wake);
        assert!(!outcome.signal_ready);
        assert_eq!(
            observations
                .recv()
                .expect("fixture observes its prepared Wayland read"),
            PreparedReadObservation::Read
        );
        assert!(observations.try_recv().is_err());
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum DispatchCall {
        DispatchPending,
        TakeToolbarFlush,
        Flush,
        PrepareAndPoll,
        ProcessWokenSources,
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

        fn process_woken_sources(&mut self) -> Result<()> {
            self.calls.push(DispatchCall::ProcessWokenSources);
            Ok(())
        }
    }

    #[test]
    fn pending_wayland_events_return_without_prepare_read_or_poll() {
        let mut ops = FakeRuntimeDispatchOps::new([1], None);

        dispatch_runtime_cycle(&mut ops, None)
            .expect("fixture dispatches its pending Wayland event");

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

        dispatch_runtime_cycle(&mut ops, None)
            .expect("fixture dispatches its pending toolbar flush");

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
                runtime_wake: true,
                signal_ready: false,
            }),
        );

        dispatch_runtime_cycle(&mut ops, None).expect("fixture dispatches its runtime wake");

        assert_eq!(
            ops.calls,
            [
                DispatchCall::DispatchPending,
                DispatchCall::Flush,
                DispatchCall::PrepareAndPoll,
                DispatchCall::ProcessWokenSources,
            ]
        );
    }

    #[test]
    fn signal_only_readiness_processes_woken_sources_exactly_once() {
        let mut ops = FakeRuntimeDispatchOps::new(
            [0],
            Some(RuntimeReadOutcome {
                wayland_read: false,
                runtime_wake: false,
                signal_ready: true,
            }),
        );

        dispatch_runtime_cycle(&mut ops, None)
            .expect("fixture dispatches its signal source readiness");

        assert_eq!(
            ops.calls,
            [
                DispatchCall::DispatchPending,
                DispatchCall::Flush,
                DispatchCall::PrepareAndPoll,
                DispatchCall::ProcessWokenSources,
            ]
        );
    }

    #[test]
    fn combined_readiness_processes_wake_once_before_wayland_dispatch() {
        let mut ops = FakeRuntimeDispatchOps::new(
            [0, 1],
            Some(RuntimeReadOutcome {
                wayland_read: true,
                runtime_wake: true,
                signal_ready: false,
            }),
        );

        dispatch_runtime_cycle(&mut ops, None).expect("fixture dispatches its combined readiness");

        assert_eq!(
            ops.calls,
            [
                DispatchCall::DispatchPending,
                DispatchCall::Flush,
                DispatchCall::PrepareAndPoll,
                DispatchCall::ProcessWokenSources,
                DispatchCall::DispatchPending,
            ]
        );
    }

    #[test]
    fn runtime_poll_retries_interruption() {
        let mut calls = 0;
        let readiness = poll_runtime_fds_with(10, 11, 12, None, |pollfds, _| {
            calls += 1;
            if calls == 1 {
                return Err(std::io::Error::from(std::io::ErrorKind::Interrupted));
            }
            pollfds[1].revents = libc::POLLIN;
            Ok(1)
        })
        .expect("fixture poll retries its injected interruption");

        assert_eq!(calls, 2);
        assert_eq!(
            readiness,
            RuntimePollReadiness {
                wayland: false,
                wake: true,
                signal: false,
            }
        );
    }

    #[test]
    fn runtime_poll_rejects_invalid_descriptor_readiness() {
        let err = poll_runtime_fds_with(10, 11, 12, None, |pollfds, _| {
            pollfds[1].revents = libc::POLLNVAL;
            Ok(1)
        })
        .unwrap_err();

        assert!(err.to_string().contains("runtime wake"));
        assert!(err.to_string().contains("readiness"));
    }

    #[test]
    fn runtime_poll_timeout_reports_no_readiness() {
        let readiness = poll_runtime_fds_with(10, 11, 12, Some(Duration::ZERO), |_, timeout_ms| {
            assert_eq!(timeout_ms, 0);
            Ok(0)
        })
        .expect("fixture observes its nonblocking poll timeout");

        assert_eq!(
            readiness,
            RuntimePollReadiness {
                wayland: false,
                wake: false,
                signal: false,
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
    fn runtime_override_is_already_the_resolved_value() {
        assert_eq!(
            Some(true).or_else(|| parse_resume_override("off")),
            Some(true)
        );
    }

    #[test]
    fn resume_override_parser_accepts_expected_values() {
        assert_eq!(parse_resume_override("enabled"), Some(true));
        assert_eq!(parse_resume_override("0"), Some(false));
        assert_eq!(parse_resume_override("maybe"), None);
    }
}
