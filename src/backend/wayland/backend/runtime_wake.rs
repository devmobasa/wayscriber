use std::io;
use std::os::fd::{AsFd, AsRawFd, BorrowedFd, FromRawFd, OwnedFd};
use std::sync::Arc;
use std::time::{Duration, Instant};

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
    pollfd: &libc::pollfd,
    label: &str,
    terminal_policy: TerminalReadinessPolicy,
) -> io::Result<bool> {
    let readable = pollfd.revents & libc::POLLIN != 0;
    if pollfd.revents & libc::POLLNVAL != 0 {
        return Err(io::Error::other(format!(
            "{label} poll descriptor failed with readiness {:#x}",
            pollfd.revents
        )));
    }
    let terminal = pollfd.revents & (libc::POLLERR | libc::POLLHUP);
    if terminal != 0 && !(terminal_policy == TerminalReadinessPolicy::ReadBuffered && readable) {
        return Err(io::Error::other(format!(
            "{label} poll descriptor failed with readiness {:#x}",
            pollfd.revents
        )));
    }
    if pollfd.revents != 0 && !readable {
        return Err(io::Error::other(format!(
            "{label} poll descriptor returned unexpected readiness {:#x}",
            pollfd.revents
        )));
    }
    Ok(readable)
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
                timeout_ms = deadline
                    .map(|deadline| {
                        timeout_to_poll_ms(Some(deadline.saturating_duration_since(Instant::now())))
                    })
                    .unwrap_or(-1);
            }
            result => return result,
        }
    }
}

impl RuntimeWakeSource {
    pub(crate) fn new() -> io::Result<Self> {
        // SAFETY: eventfd returns a new owned descriptor on success. EFD_NONBLOCK
        // keeps both producer writes and consumer drains bounded, and EFD_CLOEXEC
        // prevents subprocesses from extending the runtime descriptor lifetime.
        let raw_fd = unsafe { libc::eventfd(0, libc::EFD_NONBLOCK | libc::EFD_CLOEXEC) };
        if raw_fd < 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: raw_fd was just returned by eventfd and has not been wrapped.
        let fd = unsafe { OwnedFd::from_raw_fd(raw_fd) };
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
        loop {
            let mut value = 0_u64;
            // SAFETY: value points to a writable u64 and the owned eventfd remains
            // valid for the duration of this read.
            let result = unsafe {
                libc::read(
                    self.fd.as_raw_fd(),
                    (&mut value as *mut u64).cast(),
                    size_of::<u64>(),
                )
            };
            if result == size_of::<u64>() as isize {
                return Ok(true);
            }
            if result < 0 {
                let err = io::Error::last_os_error();
                match err.kind() {
                    io::ErrorKind::Interrupted => continue,
                    io::ErrorKind::WouldBlock => return Ok(false),
                    _ => return Err(err),
                }
            }
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                format!("runtime wake eventfd returned a short read ({result} bytes)"),
            ));
        }
    }

    /// Waits for and drains this eventfd. `None` blocks until a producer wake.
    #[cfg(test)]
    pub(crate) fn wait_readable(&self, timeout: Option<Duration>) -> io::Result<bool> {
        let readable = poll_with_retry(timeout, |timeout_ms| {
            let mut pollfd = libc::pollfd {
                fd: self.fd.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            };
            // SAFETY: pollfd is valid and the Arc-owned descriptor remains open
            // throughout this bounded or producer-woken wait.
            let ready = unsafe { libc::poll(&mut pollfd, 1, timeout_ms) };
            if ready == 0 {
                return Ok(None);
            }
            if ready < 0 {
                return Err(io::Error::last_os_error());
            }
            if !validate_poll_readiness(&pollfd, "runtime wake", TerminalReadinessPolicy::Reject)? {
                return Err(io::Error::other(format!(
                    "runtime wake poll reported readiness without a readable descriptor ({:#x})",
                    pollfd.revents
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
}

pub(super) fn timeout_to_poll_ms(timeout: Option<Duration>) -> i32 {
    timeout
        .map(|duration| {
            // poll(2) accepts integer milliseconds. Round positive fractions
            // up so an unexpired deadline never becomes a zero-timeout spin.
            duration
                .as_nanos()
                .div_ceil(1_000_000)
                .min(i32::MAX as u128) as i32
        })
        .unwrap_or(-1)
}

impl RuntimeWakeHandle {
    pub(crate) fn wake(&self) -> io::Result<()> {
        let value = 1_u64;
        loop {
            // SAFETY: value points to a readable u64 and the shared eventfd remains
            // valid for the duration of this write.
            let result = unsafe {
                libc::write(
                    self.fd.as_raw_fd(),
                    (&value as *const u64).cast(),
                    size_of::<u64>(),
                )
            };
            if result == size_of::<u64>() as isize {
                return Ok(());
            }
            if result < 0 {
                let err = io::Error::last_os_error();
                match err.kind() {
                    io::ErrorKind::Interrupted => continue,
                    // A saturated eventfd is already readable, so the wake is
                    // successfully coalesced rather than lost.
                    io::ErrorKind::WouldBlock => return Ok(()),
                    _ => return Err(err),
                }
            }
            return Err(io::Error::new(
                io::ErrorKind::WriteZero,
                format!("runtime wake eventfd returned a short write ({result} bytes)"),
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;
    use std::thread;
    use std::time::{Duration, Instant};

    use super::*;

    fn poll_readable(source: &RuntimeWakeSource, timeout_ms: i32) -> bool {
        let mut pollfd = libc::pollfd {
            fd: source.poll_fd().as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        // SAFETY: pollfd is valid for the duration of this call.
        let ready = unsafe { libc::poll(&mut pollfd, 1, timeout_ms) };
        assert!(ready >= 0, "poll failed: {}", io::Error::last_os_error());
        ready > 0 && pollfd.revents & libc::POLLIN != 0
    }

    fn wait_until_task_is_blocked_in_poll(tid: libc::pid_t) {
        let stat_path = format!("/proc/self/task/{tid}/stat");
        let deadline = Instant::now() + Duration::from_secs(1);
        loop {
            let state = std::fs::read_to_string(&stat_path).ok().and_then(|stat| {
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
        let source = RuntimeWakeSource::new().unwrap();
        source.handle().wake().unwrap();

        assert!(poll_readable(&source, 0));
        assert!(source.drain().unwrap());
        assert!(!poll_readable(&source, 0));
    }

    #[test]
    fn wake_unblocks_a_waiting_poll() {
        let source = RuntimeWakeSource::new().unwrap();
        let handle = source.handle();
        let (poll_entry_tx, poll_entry_rx) = mpsc::channel();
        let poller = thread::spawn(move || {
            // SAFETY: gettid has no preconditions and returns the calling Linux
            // thread's id, used only to observe its scheduler state in this test.
            let tid = unsafe { libc::syscall(libc::SYS_gettid) as libc::pid_t };
            poll_entry_tx.send(tid).unwrap();
            assert!(source.wait_readable(Some(Duration::from_secs(1))).unwrap());
        });

        let poller_tid = poll_entry_rx.recv().unwrap();
        wait_until_task_is_blocked_in_poll(poller_tid);
        handle.wake().unwrap();
        poller.join().unwrap();
    }

    #[test]
    fn multiple_wakes_coalesce() {
        let source = RuntimeWakeSource::new().unwrap();
        let handle = source.handle();
        for _ in 0..32 {
            handle.wake().unwrap();
        }

        assert!(source.drain().unwrap());
        assert!(!poll_readable(&source, 0));
    }

    #[test]
    fn handle_can_safely_outlive_source() {
        let source = RuntimeWakeSource::new().unwrap();
        let handle = source.handle();
        drop(source);

        handle.wake().unwrap();
    }

    #[test]
    fn empty_drain_reports_no_consumed_wake() {
        let source = RuntimeWakeSource::new().unwrap();
        assert!(!source.drain().unwrap());
    }

    #[test]
    fn wait_readable_times_out_without_a_wake() {
        let source = RuntimeWakeSource::new().unwrap();
        assert!(!source.wait_readable(Some(Duration::ZERO)).unwrap());
    }

    #[test]
    fn terminal_readiness_policy_distinguishes_streams_from_wake_descriptors() {
        let pollfd = libc::pollfd {
            fd: 7,
            events: libc::POLLIN,
            revents: libc::POLLIN | libc::POLLHUP,
        };

        assert!(
            validate_poll_readiness(
                &pollfd,
                "buffered stream",
                TerminalReadinessPolicy::ReadBuffered,
            )
            .unwrap()
        );
        assert!(
            validate_poll_readiness(&pollfd, "runtime wake", TerminalReadinessPolicy::Reject,)
                .is_err()
        );
    }
}
