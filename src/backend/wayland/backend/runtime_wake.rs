use std::io;
use std::os::fd::{AsFd, AsRawFd, BorrowedFd, FromRawFd, OwnedFd};
use std::time::{Duration, Instant};

#[derive(Debug)]
pub(crate) struct RuntimeWakeSource {
    fd: OwnedFd,
}

#[derive(Debug)]
pub(crate) struct RuntimeWakeSender {
    fd: OwnedFd,
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
        Ok(Self { fd })
    }

    pub(crate) fn poll_fd(&self) -> BorrowedFd<'_> {
        self.fd.as_fd()
    }

    pub(crate) fn try_sender(&self) -> io::Result<RuntimeWakeSender> {
        duplicate_sender(self.fd.as_fd())
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
            // SAFETY: pollfd is valid and the source-owned descriptor remains open
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

impl RuntimeWakeSender {
    pub(crate) fn try_duplicate(&self) -> io::Result<Self> {
        duplicate_sender(self.fd.as_fd())
    }

    pub(crate) fn wake(&self) -> io::Result<()> {
        write_wake(self.fd.as_fd())
    }
}

fn duplicate_sender(fd: BorrowedFd<'_>) -> io::Result<RuntimeWakeSender> {
    fd.try_clone_to_owned().map(|fd| RuntimeWakeSender { fd })
}

fn write_wake(fd: BorrowedFd<'_>) -> io::Result<()> {
    let value = 1_u64;
    loop {
        // SAFETY: value points to a readable u64 and the borrowed eventfd remains
        // valid for the duration of this write.
        let result = unsafe {
            libc::write(
                fd.as_raw_fd(),
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

#[cfg(test)]
mod tests {
    use std::mem::ManuallyDrop;
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
        let source = RuntimeWakeSource::new().expect("test creates a runtime eventfd");
        source
            .try_sender()
            .expect("test duplicates the runtime eventfd")
            .wake()
            .expect("test sender writes a wake");

        assert!(poll_readable(&source, 0));
        assert!(source.drain().expect("test drains its published wake"));
        assert!(!poll_readable(&source, 0));
    }

    #[test]
    fn wake_unblocks_a_waiting_poll() {
        let source = RuntimeWakeSource::new().expect("test creates a runtime eventfd");
        let sender = source
            .try_sender()
            .expect("test duplicates the runtime eventfd");
        let (poll_entry_tx, poll_entry_rx) = mpsc::channel();
        let poller = thread::spawn(move || {
            // SAFETY: gettid has no preconditions and returns the calling Linux
            // thread's id, used only to observe its scheduler state in this test.
            let tid = unsafe { libc::syscall(libc::SYS_gettid) as libc::pid_t };
            poll_entry_tx
                .send(tid)
                .expect("test poller publishes its thread identity");
            assert!(
                source
                    .wait_readable(Some(Duration::from_secs(1)))
                    .expect("test poller waits on its valid runtime eventfd")
            );
        });

        let poller_tid = poll_entry_rx
            .recv()
            .expect("test poller publishes its thread identity");
        wait_until_task_is_blocked_in_poll(poller_tid);
        sender.wake().expect("test sender wakes the blocked poller");
        poller.join().expect("test poller exits after its wake");
    }

    #[test]
    fn multiple_wakes_coalesce() {
        let source = RuntimeWakeSource::new().expect("test creates a runtime eventfd");
        let sender = source
            .try_sender()
            .expect("test duplicates the runtime eventfd");
        for _ in 0..32 {
            sender.wake().expect("test sender writes a coalesced wake");
        }

        assert!(source.drain().expect("test drains its coalesced wakes"));
        assert!(!poll_readable(&source, 0));
    }

    #[test]
    fn sender_can_safely_outlive_source() {
        let source = RuntimeWakeSource::new().expect("test creates a runtime eventfd");
        let sender = source
            .try_sender()
            .expect("test duplicates the runtime eventfd");
        drop(source);

        sender
            .wake()
            .expect("test sender retains its duplicated eventfd after source drop");
    }

    #[test]
    fn source_can_safely_outlive_sender() {
        let source = RuntimeWakeSource::new().expect("test creates a runtime eventfd");
        let sender = source
            .try_sender()
            .expect("test duplicates the runtime eventfd");
        drop(sender);

        source
            .try_sender()
            .expect("test duplicates the still-owned source descriptor")
            .wake()
            .expect("test replacement sender wakes the surviving source");
        assert!(source.drain().expect("test drains the replacement wake"));
    }

    #[test]
    fn empty_drain_reports_no_consumed_wake() {
        let source = RuntimeWakeSource::new().expect("test creates a runtime eventfd");
        assert!(!source.drain().expect("test drains its empty eventfd"));
    }

    #[test]
    fn wait_readable_times_out_without_a_wake() {
        let source = RuntimeWakeSource::new().expect("test creates a runtime eventfd");
        assert!(
            !source
                .wait_readable(Some(Duration::ZERO))
                .expect("test polls its valid runtime eventfd")
        );
    }

    #[test]
    fn duplicated_sender_owns_a_distinct_cloexec_descriptor() {
        let source = RuntimeWakeSource::new().expect("test creates a runtime eventfd");
        let sender = source
            .try_sender()
            .expect("test duplicates the source descriptor");
        let duplicate = sender
            .try_duplicate()
            .expect("test duplicates the sender descriptor");

        assert_ne!(source.fd.as_raw_fd(), sender.fd.as_raw_fd());
        assert_ne!(source.fd.as_raw_fd(), duplicate.fd.as_raw_fd());
        assert_ne!(sender.fd.as_raw_fd(), duplicate.fd.as_raw_fd());
        for descriptor in [&sender.fd, &duplicate.fd] {
            // SAFETY: F_GETFD reads descriptor flags from this valid owned descriptor.
            let flags = unsafe { libc::fcntl(descriptor.as_raw_fd(), libc::F_GETFD) };
            assert!(flags >= 0, "F_GETFD failed: {}", io::Error::last_os_error());
            assert_ne!(flags & libc::FD_CLOEXEC, 0);
        }
    }

    #[test]
    fn sender_duplication_reports_a_closed_descriptor() {
        let source = RuntimeWakeSource::new().expect("test creates a runtime eventfd");
        let sender = ManuallyDrop::new(
            source
                .try_sender()
                .expect("test duplicates the source descriptor"),
        );
        // SAFETY: this test closes its uniquely owned descriptor exactly once and
        // prevents OwnedFd's destructor from closing the invalid descriptor again.
        let close_result = unsafe { libc::close(sender.fd.as_raw_fd()) };
        assert_eq!(close_result, 0);

        let error = sender
            .try_duplicate()
            .expect_err("test closed descriptor must reject duplication");
        assert_eq!(error.raw_os_error(), Some(libc::EBADF));
    }

    #[test]
    fn saturated_counter_is_already_a_successful_wake() {
        let source = RuntimeWakeSource::new().expect("test creates a runtime eventfd");
        let sender = source
            .try_sender()
            .expect("test duplicates the source descriptor");
        let saturated = u64::MAX - 1;
        // SAFETY: saturated points to a readable u64 and source owns a valid eventfd.
        let written = unsafe {
            libc::write(
                source.fd.as_raw_fd(),
                (&saturated as *const u64).cast(),
                size_of::<u64>(),
            )
        };
        assert_eq!(written, size_of::<u64>() as isize);

        sender
            .wake()
            .expect("test saturated eventfd coalesces another wake");
        assert!(source.drain().expect("test drains the saturated counter"));
    }

    #[test]
    fn independent_sources_do_not_cross_wake() {
        let first = RuntimeWakeSource::new().expect("test creates its first runtime eventfd");
        let second = RuntimeWakeSource::new().expect("test creates its second runtime eventfd");
        first
            .try_sender()
            .expect("test duplicates the first source descriptor")
            .wake()
            .expect("test wakes only the first source");

        assert!(poll_readable(&first, 0));
        assert!(!poll_readable(&second, 0));
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
            .expect("test readiness policy accepts readable buffered streams")
        );
        assert!(
            validate_poll_readiness(&pollfd, "runtime wake", TerminalReadinessPolicy::Reject,)
                .is_err()
        );
    }
}
