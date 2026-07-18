use std::fmt;
use std::fs::{File, OpenOptions};
use std::io::{self, Read};
use std::os::fd::{AsFd, AsRawFd, BorrowedFd, FromRawFd, OwnedFd};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::Path;
use std::time::Duration;

const BOOT_ID_PATH: &str = "/proc/sys/kernel/random/boot_id";
const TIME_NAMESPACE_PATH: &str = "/proc/self/ns/time";
const PID_NAMESPACE_PATH: &str = "/proc/self/ns/pid";

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct ProtocolId([u8; 16]);

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct ProtocolToken([u8; 32]);

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BootIdentity(String);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct NamespaceIdentity {
    pub(crate) dev: u64,
    pub(crate) ino: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct FileIdentity {
    pub(crate) dev: u64,
    pub(crate) ino: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct BootDeadline(u64);

pub(crate) struct BootClock;

#[derive(Debug)]
pub(crate) struct BootDeadlineSource {
    fd: OwnedFd,
}

const COMMAND_WATCH_BUFFER_BYTES: usize = 16 * 1024;
const COMMAND_WATCH_EVENTS_PER_PASS: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CommandWatchDrain {
    pub(crate) scan_pending: bool,
    pub(crate) more_pending: bool,
}

#[derive(Debug)]
pub(crate) struct CommandQueueWatcher {
    fd: OwnedFd,
    watch_descriptor: i32,
    queue_path: std::path::PathBuf,
    queue_identity: FileIdentity,
    _queue_fd: File,
    buffer: Box<[u8; COMMAND_WATCH_BUFFER_BYTES]>,
    buffered_length: usize,
    buffered_offset: usize,
}

fn fill_random(bytes: &mut [u8]) -> io::Result<()> {
    let mut filled = 0;
    while filled < bytes.len() {
        // SAFETY: the remaining slice is writable for its full length. getrandom
        // writes at most that length and does not retain the pointer.
        let result = unsafe {
            libc::getrandom(bytes[filled..].as_mut_ptr().cast(), bytes.len() - filled, 0)
        };
        if result > 0 {
            filled += result as usize;
            continue;
        }
        if result == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "getrandom returned zero bytes",
            ));
        }
        let error = io::Error::last_os_error();
        if error.kind() == io::ErrorKind::Interrupted {
            continue;
        }
        return Err(error);
    }
    Ok(())
}

fn lowercase_hex(bytes: &[u8], output: &mut fmt::Formatter<'_>) -> fmt::Result {
    for byte in bytes {
        write!(output, "{byte:02x}")?;
    }
    Ok(())
}

impl ProtocolId {
    pub(crate) fn generate() -> io::Result<Self> {
        let mut bytes = [0; 16];
        fill_random(&mut bytes)?;
        Ok(Self(bytes))
    }
}

impl fmt::Debug for ProtocolId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, formatter)
    }
}

impl fmt::Display for ProtocolId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        lowercase_hex(&self.0, formatter)
    }
}

impl ProtocolToken {
    pub(crate) fn generate() -> io::Result<Self> {
        let mut bytes = [0; 32];
        fill_random(&mut bytes)?;
        Ok(Self(bytes))
    }
}

impl fmt::Debug for ProtocolToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ProtocolToken([redacted])")
    }
}

impl fmt::Display for ProtocolToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        lowercase_hex(&self.0, formatter)
    }
}

impl BootIdentity {
    pub(crate) fn read() -> io::Result<Self> {
        let mut value = String::new();
        File::open(BOOT_ID_PATH)?
            .take(64)
            .read_to_string(&mut value)?;
        let value = value.trim();
        let bytes = value.as_bytes();
        let valid = bytes.len() == 36
            && bytes.iter().enumerate().all(|(index, byte)| match index {
                8 | 13 | 18 | 23 => *byte == b'-',
                _ => byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'),
            });
        if !valid {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "kernel boot ID is not a canonical lowercase UUID",
            ));
        }
        Ok(Self(value.to_owned()))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

impl NamespaceIdentity {
    fn read(path: &str) -> io::Result<Self> {
        // procfs namespace entries are kernel magic links. Following this fixed
        // path is intentional; serialized or caller-controlled paths never reach
        // this helper.
        let file = File::open(path)?;
        let metadata = file.metadata()?;
        Ok(Self {
            dev: metadata.dev(),
            ino: metadata.ino(),
        })
    }

    pub(crate) fn current_time() -> io::Result<Self> {
        Self::read(TIME_NAMESPACE_PATH)
    }

    pub(crate) fn current_pid() -> io::Result<Self> {
        Self::read(PID_NAMESPACE_PATH)
    }
}

pub(crate) fn read_bounded_regular_file(path: &Path, cap: usize) -> io::Result<Vec<u8>> {
    let mut options = OpenOptions::new();
    options
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK);
    let file = options.open(path)?;
    let metadata = file.metadata()?;
    if !metadata.is_file() || metadata.len() > cap as u64 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "protocol path is not a bounded regular file",
        ));
    }
    let expected = usize::try_from(metadata.len())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "file length overflow"))?;
    let mut bytes = Vec::with_capacity(expected);
    file.take((cap + 1) as u64).read_to_end(&mut bytes)?;
    if bytes.len() > cap {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "protocol record exceeds its read bound",
        ));
    }
    Ok(bytes)
}

pub(crate) fn open_nofollow_directory(path: &Path) -> io::Result<(File, FileIdentity)> {
    let mut options = OpenOptions::new();
    options
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK);
    let file = options.open(path)?;
    let metadata = file.metadata()?;
    if !metadata.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "protocol directory is not a directory",
        ));
    }
    Ok((
        file,
        FileIdentity {
            dev: metadata.dev(),
            ino: metadata.ino(),
        },
    ))
}

pub(crate) fn revalidate_path_identity(path: &Path, expected: FileIdentity) -> io::Result<()> {
    let (_, actual) = open_nofollow_directory(path)?;
    if actual != expected {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "protocol directory identity changed",
        ));
    }
    Ok(())
}

pub(crate) fn current_process_start_ticks() -> io::Result<u64> {
    process_start_ticks(std::process::id())
}

pub(crate) fn process_start_ticks(pid: u32) -> io::Result<u64> {
    let path = format!("/proc/{pid}/stat");
    let contents = std::fs::read_to_string(path)?;
    let close = contents.rfind(')').ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "proc stat has no command terminator",
        )
    })?;
    // Fields following the command start at field 3 (state); starttime is
    // field 22, therefore index 19 in this suffix.
    let value = contents[close + 1..]
        .split_whitespace()
        .nth(19)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "proc stat is truncated"))?
        .parse::<u64>()
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "invalid process start time"))?;
    if value == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "process start time is zero",
        ));
    }
    Ok(value)
}

pub(crate) fn open_pidfd(pid: u32) -> io::Result<OwnedFd> {
    let pid = i32::try_from(pid)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "pid does not fit pid_t"))?;
    // SAFETY: pidfd_open has no pointer arguments. The returned descriptor is
    // owned by the caller on success.
    let raw = unsafe { libc::syscall(libc::SYS_pidfd_open, pid, 0) };
    if raw < 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: raw is a new pidfd not wrapped elsewhere.
    Ok(unsafe { OwnedFd::from_raw_fd(raw as i32) })
}

pub(crate) fn validate_pidfd(fd: BorrowedFd<'_>) -> io::Result<()> {
    // Signal zero performs identity/permission validation without delivering a
    // signal. EINVAL rejects an arbitrary descriptor masquerading as a pidfd.
    // SAFETY: pidfd_send_signal receives no pointer arguments in this use.
    if unsafe {
        libc::syscall(
            libc::SYS_pidfd_send_signal,
            fd.as_raw_fd(),
            0,
            std::ptr::null::<libc::siginfo_t>(),
            0,
        )
    } == 0
    {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

impl BootClock {
    pub(crate) fn now() -> io::Result<BootDeadline> {
        let mut value = libc::timespec {
            tv_sec: 0,
            tv_nsec: 0,
        };
        // SAFETY: value is a valid writable timespec and CLOCK_BOOTTIME has no
        // additional preconditions.
        if unsafe { libc::clock_gettime(libc::CLOCK_BOOTTIME, &mut value) } != 0 {
            return Err(io::Error::last_os_error());
        }
        if value.tv_sec < 0 || !(0..1_000_000_000).contains(&value.tv_nsec) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "CLOCK_BOOTTIME returned an invalid timespec",
            ));
        }
        let seconds = u64::try_from(value.tv_sec)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "negative boot time"))?;
        let nanos = u64::try_from(value.tv_nsec)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "negative boot nanoseconds"))?;
        let total = seconds
            .checked_mul(1_000_000_000)
            .and_then(|base| base.checked_add(nanos))
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "boot time overflow"))?;
        Ok(BootDeadline(total))
    }
}

impl BootDeadline {
    pub(crate) const fn from_nanos(value: u64) -> Self {
        Self(value)
    }

    pub(crate) fn checked_add(self, duration: Duration) -> io::Result<Self> {
        let nanos = u64::try_from(duration.as_nanos()).map_err(|_| {
            io::Error::new(io::ErrorKind::InvalidInput, "deadline duration overflow")
        })?;
        self.0
            .checked_add(nanos)
            .map(Self)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "deadline overflow"))
    }

    pub(crate) const fn as_nanos(self) -> u64 {
        self.0
    }
}

impl BootDeadlineSource {
    pub(crate) fn new() -> io::Result<Self> {
        // SAFETY: timerfd_create returns a new owned descriptor on success.
        let raw = unsafe {
            libc::timerfd_create(libc::CLOCK_BOOTTIME, libc::TFD_NONBLOCK | libc::TFD_CLOEXEC)
        };
        if raw < 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: raw is a newly created descriptor not wrapped elsewhere.
        Ok(Self {
            fd: unsafe { OwnedFd::from_raw_fd(raw) },
        })
    }

    pub(crate) fn poll_fd(&self) -> BorrowedFd<'_> {
        self.fd.as_fd()
    }

    pub(crate) fn arm(&self, deadline: BootDeadline) -> io::Result<()> {
        let seconds = deadline.0 / 1_000_000_000;
        let nanos = deadline.0 % 1_000_000_000;
        let value = libc::itimerspec {
            it_interval: libc::timespec {
                tv_sec: 0,
                tv_nsec: 0,
            },
            it_value: libc::timespec {
                tv_sec: libc::time_t::try_from(seconds).map_err(|_| {
                    io::Error::new(io::ErrorKind::InvalidInput, "deadline seconds overflow")
                })?,
                tv_nsec: libc::c_long::try_from(nanos).map_err(|_| {
                    io::Error::new(io::ErrorKind::InvalidInput, "deadline nanos overflow")
                })?,
            },
        };
        // SAFETY: value is a fully initialized itimerspec and fd owns a timerfd.
        if unsafe {
            libc::timerfd_settime(
                self.fd.as_raw_fd(),
                libc::TFD_TIMER_ABSTIME,
                &value,
                std::ptr::null_mut(),
            )
        } != 0
        {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }

    pub(crate) fn disarm(&self) -> io::Result<()> {
        let value = libc::itimerspec {
            it_interval: libc::timespec {
                tv_sec: 0,
                tv_nsec: 0,
            },
            it_value: libc::timespec {
                tv_sec: 0,
                tv_nsec: 0,
            },
        };
        // SAFETY: value is initialized and fd owns a timerfd.
        if unsafe { libc::timerfd_settime(self.fd.as_raw_fd(), 0, &value, std::ptr::null_mut()) }
            != 0
        {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }

    pub(crate) fn drain(&self) -> io::Result<bool> {
        let mut expirations = 0_u64;
        loop {
            // SAFETY: expirations is writable and fd remains valid for the read.
            let result = unsafe {
                libc::read(
                    self.fd.as_raw_fd(),
                    (&mut expirations as *mut u64).cast(),
                    size_of::<u64>(),
                )
            };
            if result == size_of::<u64>() as isize {
                return Ok(true);
            }
            if result < 0 {
                let error = io::Error::last_os_error();
                return match error.kind() {
                    io::ErrorKind::Interrupted => continue,
                    io::ErrorKind::WouldBlock => Ok(false),
                    _ => Err(error),
                };
            }
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                format!("timerfd returned a short read ({result} bytes)"),
            ));
        }
    }
}

impl CommandQueueWatcher {
    pub(crate) fn new(queue_path: &Path) -> io::Result<Self> {
        let (queue_fd, queue_identity) = open_nofollow_directory(queue_path)?;
        // SAFETY: inotify_init1 returns a newly owned descriptor on success.
        let raw = unsafe { libc::inotify_init1(libc::IN_NONBLOCK | libc::IN_CLOEXEC) };
        if raw < 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: raw is a new descriptor not wrapped elsewhere.
        let fd = unsafe { OwnedFd::from_raw_fd(raw) };
        let path = std::ffi::CString::new(queue_path.as_os_str().as_encoded_bytes())
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "queue path contains NUL"))?;
        let mask = libc::IN_MOVED_TO
            | libc::IN_CREATE
            | libc::IN_DELETE_SELF
            | libc::IN_MOVE_SELF
            | libc::IN_IGNORED
            | libc::IN_Q_OVERFLOW;
        // SAFETY: path is NUL-terminated and fd owns an inotify instance.
        let watch_descriptor =
            unsafe { libc::inotify_add_watch(fd.as_raw_fd(), path.as_ptr(), mask) };
        if watch_descriptor < 0 {
            return Err(io::Error::last_os_error());
        }
        revalidate_path_identity(queue_path, queue_identity)?;
        Ok(Self {
            fd,
            watch_descriptor,
            queue_path: queue_path.to_owned(),
            queue_identity,
            _queue_fd: queue_fd,
            buffer: Box::new([0; COMMAND_WATCH_BUFFER_BYTES]),
            buffered_length: 0,
            buffered_offset: 0,
        })
    }

    pub(crate) fn poll_fd(&self) -> BorrowedFd<'_> {
        self.fd.as_fd()
    }

    pub(crate) fn revalidate(&self) -> io::Result<()> {
        revalidate_path_identity(&self.queue_path, self.queue_identity)
    }

    fn refill(&mut self) -> io::Result<bool> {
        self.buffered_length = 0;
        self.buffered_offset = 0;
        loop {
            // SAFETY: buffer is writable for its full fixed length and fd is
            // a nonblocking inotify descriptor.
            let read = unsafe {
                libc::read(
                    self.fd.as_raw_fd(),
                    self.buffer.as_mut_ptr().cast(),
                    self.buffer.len(),
                )
            };
            if read > 0 {
                self.buffered_length = read as usize;
                return Ok(true);
            }
            if read == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "inotify returned EOF",
                ));
            }
            let error = io::Error::last_os_error();
            match error.kind() {
                io::ErrorKind::Interrupted => continue,
                io::ErrorKind::WouldBlock => return Ok(false),
                _ => return Err(error),
            }
        }
    }

    pub(crate) fn drain(&mut self) -> io::Result<CommandWatchDrain> {
        if self.buffered_offset == self.buffered_length && !self.refill()? {
            return Ok(CommandWatchDrain {
                scan_pending: false,
                more_pending: false,
            });
        }
        let mut events = 0;
        let mut scan_pending = false;
        while self.buffered_offset < self.buffered_length && events < COMMAND_WATCH_EVENTS_PER_PASS
        {
            let remaining = &self.buffer[self.buffered_offset..self.buffered_length];
            if remaining.len() < size_of::<libc::inotify_event>() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "truncated inotify event header",
                ));
            }
            // SAFETY: length was checked and read_unaligned copies the fixed
            // header without retaining the pointer.
            let event = unsafe {
                std::ptr::read_unaligned(remaining.as_ptr().cast::<libc::inotify_event>())
            };
            let event_length = size_of::<libc::inotify_event>()
                .checked_add(event.len as usize)
                .ok_or_else(|| {
                    io::Error::new(io::ErrorKind::InvalidData, "inotify length overflow")
                })?;
            if remaining.len() < event_length {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "truncated inotify event body",
                ));
            }
            if event.mask & (libc::IN_DELETE_SELF | libc::IN_MOVE_SELF | libc::IN_IGNORED) != 0
                || (event.wd != self.watch_descriptor && event.mask & libc::IN_Q_OVERFLOW == 0)
            {
                return Err(io::Error::new(
                    io::ErrorKind::BrokenPipe,
                    "command queue watch lost directory ownership",
                ));
            }
            if event.mask & libc::IN_Q_OVERFLOW != 0 {
                self.revalidate()?;
                scan_pending = true;
            }
            if event.mask & (libc::IN_MOVED_TO | libc::IN_CREATE) != 0 {
                scan_pending = true;
            }
            self.buffered_offset += event_length;
            events += 1;
        }
        let more_pending = self.buffered_offset < self.buffered_length;
        if !more_pending {
            self.buffered_offset = 0;
            self.buffered_length = 0;
        }
        Ok(CommandWatchDrain {
            scan_pending,
            more_pending,
        })
    }
}

#[cfg(test)]
pub(crate) fn open_self_pidfd() -> io::Result<OwnedFd> {
    open_pidfd(std::process::id())
}

/// Immediately terminate the process without Rust or libc cleanup.
///
/// # Safety
///
/// The caller must have irrevocably entered a fail-stop state. This function
/// never runs destructors, flushes logs, or returns.
pub(crate) unsafe fn fail_stop(status: i32) -> ! {
    // SAFETY: the function contract requires a non-returning process exit.
    unsafe {
        libc::syscall(libc::SYS_exit_group, status);
        libc::_exit(status);
    }
}
