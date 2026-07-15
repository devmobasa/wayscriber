use std::io;
use std::os::fd::{AsFd, AsRawFd, BorrowedFd, FromRawFd, OwnedFd};
use std::sync::Arc;

pub(in crate::backend::wayland) const MAX_WAKE_DRAIN_READS: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::backend::wayland) struct RuntimeWakeDrain {
    pub(in crate::backend::wayland) reads: usize,
    pub(in crate::backend::wayland) limit_reached: bool,
}

#[derive(Debug)]
pub(in crate::backend::wayland) struct RuntimeWakeSource {
    fd: Arc<OwnedFd>,
}

#[derive(Clone, Debug)]
pub(in crate::backend::wayland) struct RuntimeWakeHandle {
    fd: Arc<OwnedFd>,
}

impl RuntimeWakeSource {
    pub(in crate::backend::wayland) fn new() -> io::Result<Self> {
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

    pub(in crate::backend::wayland) fn poll_fd(&self) -> BorrowedFd<'_> {
        self.fd.as_fd()
    }

    pub(in crate::backend::wayland) fn handle(&self) -> RuntimeWakeHandle {
        RuntimeWakeHandle {
            fd: Arc::clone(&self.fd),
        }
    }

    pub(in crate::backend::wayland) fn drain(&self) -> io::Result<RuntimeWakeDrain> {
        self.drain_with_limit(MAX_WAKE_DRAIN_READS)
    }

    fn drain_with_limit(&self, max_reads: usize) -> io::Result<RuntimeWakeDrain> {
        let mut reads = 0;
        while reads < max_reads {
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
                reads += 1;
                continue;
            }
            if result < 0 {
                let err = io::Error::last_os_error();
                match err.kind() {
                    io::ErrorKind::Interrupted => continue,
                    io::ErrorKind::WouldBlock => {
                        return Ok(RuntimeWakeDrain {
                            reads,
                            limit_reached: false,
                        });
                    }
                    _ => return Err(err),
                }
            }
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                format!("runtime wake eventfd returned a short read ({result} bytes)"),
            ));
        }

        Ok(RuntimeWakeDrain {
            reads,
            limit_reached: true,
        })
    }
}

impl RuntimeWakeHandle {
    pub(in crate::backend::wayland) fn wake(&self) -> io::Result<()> {
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
        assert_eq!(
            source.drain().unwrap(),
            RuntimeWakeDrain {
                reads: 1,
                limit_reached: false,
            }
        );
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
            assert!(poll_readable(&source, 1_000));
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

        assert_eq!(source.drain().unwrap().reads, 1);
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
    fn empty_drain_is_bounded_and_reports_no_residual_work() {
        let source = RuntimeWakeSource::new().unwrap();
        assert_eq!(
            source.drain().unwrap(),
            RuntimeWakeDrain {
                reads: 0,
                limit_reached: false,
            }
        );
    }

    #[test]
    fn bounded_drain_leaves_residual_readiness_for_the_next_pass() {
        let source = RuntimeWakeSource::new().unwrap();
        source.handle().wake().unwrap();

        assert_eq!(
            source.drain_with_limit(0).unwrap(),
            RuntimeWakeDrain {
                reads: 0,
                limit_reached: true,
            }
        );
        assert!(poll_readable(&source, 0));
        assert_eq!(source.drain().unwrap().reads, 1);
    }
}
