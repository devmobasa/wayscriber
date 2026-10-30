use std::io;
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::sync::Arc;
use std::time::{Duration, Instant};

use rustix::event::{self, EventfdFlags, PollFd, PollFlags, Timespec};
use rustix::io::{self as rustix_io, Errno};

#[derive(Debug)]
pub(crate) struct RuntimeWakeSource {
    fd: Arc<OwnedFd>,
}

#[derive(Clone, Debug)]
pub(crate) struct RuntimeWakeHandle {
    fd: Arc<OwnedFd>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum TerminalReadinessPolicy {
    Reject,
    ReadBuffered,
}

pub(super) fn validate_poll_readiness(
    revents: PollFlags,
    label: &str,
    terminal_policy: TerminalReadinessPolicy,
) -> io::Result<bool> {
    let readable = revents.contains(PollFlags::IN);
    if revents.contains(PollFlags::NVAL) {
        return Err(io::Error::other(format!(
            "{label} poll descriptor failed with readiness {:#x}",
            revents.bits()
        )));
    }
    let terminal = revents.intersects(PollFlags::ERR | PollFlags::HUP);
    if terminal && !(terminal_policy == TerminalReadinessPolicy::ReadBuffered && readable) {
        return Err(io::Error::other(format!(
            "{label} poll descriptor failed with readiness {:#x}",
            revents.bits()
        )));
    }
    if !revents.is_empty() && !readable {
        return Err(io::Error::other(format!(
            "{label} poll descriptor returned unexpected readiness {:#x}",
            revents.bits()
        )));
    }
    Ok(readable)
}

/// `poll(2)` with a millisecond timeout from [`timeout_to_poll_ms`], where a
/// negative value blocks.
pub(super) fn poll_for_ms(fds: &mut [PollFd<'_>], timeout_ms: i32) -> io::Result<usize> {
    let timeout = (timeout_ms >= 0).then(|| Timespec {
        tv_sec: (timeout_ms / 1_000).into(),
        tv_nsec: (timeout_ms % 1_000 * 1_000_000).into(),
    });
    Ok(event::poll(fds, timeout.as_ref())?)
}

pub(super) fn poll_with_retry<T>(
    timeout: Option<Duration>,
    mut attempt: impl FnMut(i32) -> io::Result<Option<T>>,
) -> io::Result<Option<T>> {
    let deadline = timeout.and_then(|timeout| Instant::now().checked_add(timeout));
    let mut timeout_ms = timeout_to_poll_ms(timeout);
    loop {
        match attempt(timeout_ms) {
            Err(err) if err.kind() == io::ErrorKind::Interrupted => {
                timeout_ms = deadline.map_or(-1, |deadline| {
                    timeout_to_poll_ms(Some(deadline.saturating_duration_since(Instant::now())))
                });
            }
            result => return result,
        }
    }
}

impl RuntimeWakeSource {
    pub(crate) fn try_new() -> io::Result<Self> {
        // NONBLOCK keeps both producer writes and consumer drains bounded, and
        // CLOEXEC prevents subprocesses from extending the descriptor lifetime.
        let fd = event::eventfd(0, EventfdFlags::NONBLOCK | EventfdFlags::CLOEXEC)?;
        Ok(Self { fd: Arc::new(fd) })
    }

    pub(crate) fn poll_fd(&self) -> BorrowedFd<'_> {
        self.fd.as_fd()
    }

    pub(crate) fn handle(&self) -> RuntimeWakeHandle {
        RuntimeWakeHandle {
            fd: Arc::clone(&self.fd),
        }
    }

    /// Drains the non-semaphore eventfd with at most one successful read.
    /// One eventfd read consumes the entire accumulated counter, so a second
    /// successful-path read would only issue a guaranteed-EAGAIN syscall.
    pub(crate) fn drain(&self) -> io::Result<bool> {
        let mut value = [0_u8; size_of::<u64>()];
        loop {
            match rustix_io::read(&self.fd, &mut value) {
                Ok(read) if read == value.len() => return Ok(true),
                Ok(read) => {
                    return Err(io::Error::new(
                        io::ErrorKind::UnexpectedEof,
                        format!("runtime wake eventfd returned a short read ({read} bytes)"),
                    ));
                }
                Err(Errno::INTR) => {}
                Err(Errno::WOULDBLOCK) => return Ok(false),
                Err(errno) => return Err(errno.into()),
            }
        }
    }

    /// Waits for and drains this eventfd. `None` blocks until a producer wake.
    #[cfg(test)]
    pub(crate) fn wait_readable(&self, timeout: Option<Duration>) -> io::Result<bool> {
        let readable = poll_with_retry(timeout, |timeout_ms| {
            let mut pollfds = [PollFd::new(&self.fd, PollFlags::IN)];
            if poll_for_ms(&mut pollfds, timeout_ms)? == 0 {
                return Ok(None);
            }
            let revents = pollfds[0].revents();
            if !validate_poll_readiness(revents, "runtime wake", TerminalReadinessPolicy::Reject)? {
                return Err(io::Error::other(format!(
                    "runtime wake poll reported readiness without a readable descriptor ({:#x})",
                    revents.bits()
                )));
            }
            Ok(Some(()))
        })?;
        if readable.is_some() {
            self.drain()
        } else {
            Ok(false)
        }
    }

    /// Reports whether a wake arrives within `timeout`, without draining it.
    #[cfg(test)]
    pub(crate) fn is_readable_within(&self, timeout: Duration) -> io::Result<bool> {
        let readable = poll_with_retry(Some(timeout), |timeout_ms| {
            let mut pollfds = [PollFd::new(&self.fd, PollFlags::IN)];
            if poll_for_ms(&mut pollfds, timeout_ms)? == 0 {
                return Ok(None);
            }
            Ok(Some(pollfds[0].revents().contains(PollFlags::IN)))
        })?;
        Ok(readable == Some(true))
    }
}

pub(super) fn timeout_to_poll_ms(timeout: Option<Duration>) -> i32 {
    timeout.map_or(-1, |duration| {
        // poll(2) accepts integer milliseconds. Round positive fractions
        // up so an unexpired deadline never becomes a zero-timeout spin.
        duration
            .as_nanos()
            .div_ceil(1_000_000)
            .min(i32::MAX as u128) as i32
    })
}

impl RuntimeWakeHandle {
    pub(crate) fn wake(&self) -> io::Result<()> {
        let value = 1_u64.to_ne_bytes();
        loop {
            match rustix_io::write(&self.fd, &value) {
                Ok(written) if written == value.len() => return Ok(()),
                Ok(written) => {
                    return Err(io::Error::new(
                        io::ErrorKind::WriteZero,
                        format!("runtime wake eventfd returned a short write ({written} bytes)"),
                    ));
                }
                Err(Errno::INTR) => {}
                // A saturated eventfd is already readable, so the wake is
                // successfully coalesced rather than lost.
                Err(Errno::WOULDBLOCK) => return Ok(()),
                Err(errno) => return Err(errno.into()),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;
    use std::time::{Duration, Instant};
    use std::{fs, thread};

    use rustix::process::Pid;
    use rustix::thread as rustix_thread;

    use super::*;

    fn wait_until_task_is_blocked_in_poll(tid: Pid) {
        let stat_path = format!("/proc/self/task/{tid}/stat");
        let deadline = Instant::now() + Duration::from_secs(1);
        loop {
            let state = fs::read_to_string(&stat_path).ok().and_then(|stat| {
                stat.rsplit_once(") ")
                    .and_then(|(_, suffix)| suffix.chars().next())
            });
            // The polling thread publishes its tid before calling poll and performs
            // no other blocking operation afterward. Once Linux reports it as
            // sleeping, the poll syscall is actively waiting on the eventfd.
            if state == Some('S') {
                return;
            }
            assert!(
                Instant::now() < deadline,
                "polling thread did not enter a blocked poll (last state: {state:?})"
            );
            thread::yield_now();
        }
    }

    #[test]
    fn wake_before_poll_is_observed_and_drained() {
        let source = RuntimeWakeSource::try_new().unwrap();
        source.handle().wake().unwrap();

        assert!(source.is_readable_within(Duration::ZERO).unwrap());
        assert!(source.drain().unwrap());
        assert!(!source.is_readable_within(Duration::ZERO).unwrap());
    }

    #[test]
    fn wake_unblocks_a_waiting_poll() {
        let source = RuntimeWakeSource::try_new().unwrap();
        let handle = source.handle();
        let (poll_entry_tx, poll_entry_rx) = mpsc::channel();
        let poller = thread::spawn(move || {
            poll_entry_tx.send(rustix_thread::gettid()).unwrap();
            assert!(source.wait_readable(Some(Duration::from_secs(1))).unwrap());
        });

        let poller_tid = poll_entry_rx.recv().unwrap();
        wait_until_task_is_blocked_in_poll(poller_tid);
        handle.wake().unwrap();
        poller.join().unwrap();
    }

    #[test]
    fn multiple_wakes_coalesce() {
        let source = RuntimeWakeSource::try_new().unwrap();
        let handle = source.handle();
        for _ in 0..32 {
            handle.wake().unwrap();
        }

        assert!(source.drain().unwrap());
        assert!(!source.is_readable_within(Duration::ZERO).unwrap());
    }

    #[test]
    fn handle_can_safely_outlive_source() {
        let source = RuntimeWakeSource::try_new().unwrap();
        let handle = source.handle();
        drop(source);

        handle.wake().unwrap();
    }

    #[test]
    fn empty_drain_reports_no_consumed_wake() {
        let source = RuntimeWakeSource::try_new().unwrap();
        assert!(!source.drain().unwrap());
    }

    #[test]
    fn wait_readable_times_out_without_a_wake() {
        let source = RuntimeWakeSource::try_new().unwrap();
        assert!(!source.wait_readable(Some(Duration::ZERO)).unwrap());
    }

    #[test]
    fn terminal_readiness_policy_distinguishes_streams_from_wake_descriptors() {
        let revents = PollFlags::IN | PollFlags::HUP;

        assert!(
            validate_poll_readiness(
                revents,
                "buffered stream",
                TerminalReadinessPolicy::ReadBuffered,
            )
            .unwrap()
        );
        assert!(
            validate_poll_readiness(revents, "runtime wake", TerminalReadinessPolicy::Reject)
                .is_err()
        );
    }
}
