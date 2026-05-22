use std::io;
use std::os::fd::RawFd;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicUsize, Ordering};
use std::thread::{self, JoinHandle};

const MAX_SIGNAL_SLOTS: usize = 8;

static SIGNAL_WRITE_FD: AtomicI32 = AtomicI32::new(-1);
static LISTENER_ACTIVE: AtomicBool = AtomicBool::new(false);
static REGISTERED_SIGNALS: [AtomicI32; MAX_SIGNAL_SLOTS] =
    [const { AtomicI32::new(0) }; MAX_SIGNAL_SLOTS];
static PENDING_SIGNALS: [AtomicUsize; MAX_SIGNAL_SLOTS] =
    [const { AtomicUsize::new(0) }; MAX_SIGNAL_SLOTS];

pub(crate) fn spawn_listener<F>(signals: &[libc::c_int], on_signal: F) -> io::Result<JoinHandle<()>>
where
    F: Fn(libc::c_int) + Send + 'static,
{
    if signals.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "at least one signal must be registered",
        ));
    }

    // This lightweight signal bridge intentionally owns the process handlers for
    // the requested signals rather than chaining prior handlers. Keep a single
    // listener per process so signal delivery stays deterministic.
    if LISTENER_ACTIVE.swap(true, Ordering::AcqRel) {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "a signal listener is already active",
        ));
    }

    if let Err(err) = register_signals(signals) {
        LISTENER_ACTIVE.store(false, Ordering::Release);
        return Err(err);
    }

    let (read_fd, write_fd) = match create_pipe() {
        Ok(pipe) => pipe,
        Err(err) => {
            clear_registered_signals();
            LISTENER_ACTIVE.store(false, Ordering::Release);
            return Err(err);
        }
    };

    SIGNAL_WRITE_FD.store(write_fd, Ordering::Release);

    let mut installed_handlers = Vec::with_capacity(signals.len());
    for &signal in signals {
        match install_handler(signal) {
            Ok(previous) => installed_handlers.push((signal, previous)),
            Err(err) => {
                restore_handlers(&installed_handlers);
                SIGNAL_WRITE_FD.store(-1, Ordering::Release);
                clear_registered_signals();
                close_fd(read_fd);
                close_fd(write_fd);
                LISTENER_ACTIVE.store(false, Ordering::Release);
                return Err(err);
            }
        }
    }

    Ok(thread::spawn(move || read_signal_loop(read_fd, on_signal)))
}

fn restore_handlers(handlers: &[(libc::c_int, libc::sigaction)]) {
    for (signal, previous) in handlers.iter().rev() {
        // SAFETY: `previous` was returned by `sigaction` for the same signal.
        let _ = unsafe { libc::sigaction(*signal, previous, std::ptr::null_mut()) };
    }
}

fn register_signals(signals: &[libc::c_int]) -> io::Result<()> {
    if signals.len() > MAX_SIGNAL_SLOTS {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "too many signals registered",
        ));
    }

    clear_registered_signals();
    for (index, &signal) in signals.iter().enumerate() {
        if signal <= 0 {
            clear_registered_signals();
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "signal numbers must be positive",
            ));
        }
        if signals[..index].contains(&signal) {
            clear_registered_signals();
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "duplicate signals are not supported",
            ));
        }

        REGISTERED_SIGNALS[index].store(signal, Ordering::Release);
    }

    Ok(())
}

fn clear_registered_signals() {
    for index in 0..MAX_SIGNAL_SLOTS {
        PENDING_SIGNALS[index].store(0, Ordering::Release);
        REGISTERED_SIGNALS[index].store(0, Ordering::Release);
    }
}

fn create_pipe() -> io::Result<(RawFd, RawFd)> {
    let mut fds = [-1; 2];
    // SAFETY: `fds` points to two valid c_int slots for libc to fill.
    if unsafe { libc::pipe(fds.as_mut_ptr()) } != 0 {
        return Err(io::Error::last_os_error());
    }

    if let Err(err) = configure_pipe(fds[0], fds[1]) {
        close_fd(fds[0]);
        close_fd(fds[1]);
        return Err(err);
    }

    Ok((fds[0], fds[1]))
}

fn configure_pipe(read_fd: RawFd, write_fd: RawFd) -> io::Result<()> {
    set_fd_flag(read_fd, libc::FD_CLOEXEC)?;
    set_fd_flag(write_fd, libc::FD_CLOEXEC)?;
    set_status_flag(write_fd, libc::O_NONBLOCK)
}

fn set_fd_flag(fd: RawFd, flag: libc::c_int) -> io::Result<()> {
    // SAFETY: `fd` is an open file descriptor owned by this module.
    let current = unsafe { libc::fcntl(fd, libc::F_GETFD) };
    if current < 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: `fd` is valid and `current | flag` is a valid F_SETFD bitset.
    if unsafe { libc::fcntl(fd, libc::F_SETFD, current | flag) } < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

fn set_status_flag(fd: RawFd, flag: libc::c_int) -> io::Result<()> {
    // SAFETY: `fd` is an open file descriptor owned by this module.
    let current = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if current < 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: `fd` is valid and `current | flag` is a valid F_SETFL bitset.
    if unsafe { libc::fcntl(fd, libc::F_SETFL, current | flag) } < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

fn install_handler(signal: libc::c_int) -> io::Result<libc::sigaction> {
    // SAFETY: Zeroed sigaction is immediately initialized before use.
    let mut action = unsafe { std::mem::zeroed::<libc::sigaction>() };
    action.sa_sigaction = signal_handler as *const () as usize;
    action.sa_flags = signal_action_flags();

    // SAFETY: `action.sa_mask` points to a valid sigset_t field.
    if unsafe { libc::sigemptyset(&mut action.sa_mask) } != 0 {
        return Err(io::Error::last_os_error());
    }

    // SAFETY: Zeroed sigaction is filled by libc when installation succeeds.
    let mut previous = unsafe { std::mem::zeroed::<libc::sigaction>() };
    // SAFETY: Installs a process-wide handler for the requested signal.
    if unsafe { libc::sigaction(signal, &action, &mut previous) } != 0 {
        return Err(io::Error::last_os_error());
    }

    Ok(previous)
}

fn signal_action_flags() -> libc::c_int {
    libc::SA_RESTART
}

extern "C" fn signal_handler(signal: libc::c_int) {
    let _errno_guard = ErrnoGuard::new();

    if !mark_pending(signal) {
        return;
    }

    let fd = SIGNAL_WRITE_FD.load(Ordering::Acquire);
    if fd < 0 {
        return;
    }

    let wakeup = 1u8;
    // SAFETY: `fd` is a nonblocking pipe write end. `write` is async-signal-safe.
    // The pipe is only a wakeup channel; pending signal counters above preserve
    // signal state if this best-effort write fails with EAGAIN.
    let _ = unsafe {
        libc::write(
            fd,
            (&wakeup as *const u8).cast::<libc::c_void>(),
            std::mem::size_of::<u8>(),
        )
    };
}

struct ErrnoGuard {
    location: *mut libc::c_int,
    saved: libc::c_int,
}

impl ErrnoGuard {
    fn new() -> Self {
        let location = errno_location();
        let saved = if location.is_null() {
            0
        } else {
            // SAFETY: `location` points to the current thread's errno slot.
            unsafe { *location }
        };
        Self { location, saved }
    }
}

impl Drop for ErrnoGuard {
    fn drop(&mut self) {
        if !self.location.is_null() {
            // SAFETY: `location` points to the current thread's errno slot.
            unsafe {
                *self.location = self.saved;
            }
        }
    }
}

#[cfg(any(
    target_os = "emscripten",
    target_os = "hurd",
    target_os = "linux",
    target_os = "redox"
))]
fn errno_location() -> *mut libc::c_int {
    // SAFETY: Returns the current thread's errno slot on these libc targets.
    unsafe { libc::__errno_location() }
}

#[cfg(any(target_os = "android", target_os = "cygwin", target_os = "netbsd"))]
fn errno_location() -> *mut libc::c_int {
    // SAFETY: Returns the current thread's errno slot on these libc targets.
    unsafe { libc::__errno() }
}

#[cfg(any(
    target_os = "freebsd",
    target_os = "ios",
    target_os = "macos",
    target_os = "tvos",
    target_os = "visionos",
    target_os = "watchos"
))]
fn errno_location() -> *mut libc::c_int {
    // SAFETY: Returns the current thread's errno slot on these libc targets.
    unsafe { libc::__error() }
}

#[cfg(target_os = "dragonfly")]
fn errno_location() -> *mut libc::c_int {
    // SAFETY: Returns the current thread's errno slot on DragonFly BSD.
    unsafe { libc::__errno_location() }
}

#[cfg(any(target_os = "illumos", target_os = "solaris"))]
fn errno_location() -> *mut libc::c_int {
    // SAFETY: Returns the current thread's errno slot on Solaris-family targets.
    unsafe { libc::___errno() }
}

#[cfg(not(any(
    target_os = "android",
    target_os = "cygwin",
    target_os = "dragonfly",
    target_os = "emscripten",
    target_os = "freebsd",
    target_os = "hurd",
    target_os = "illumos",
    target_os = "ios",
    target_os = "linux",
    target_os = "macos",
    target_os = "netbsd",
    target_os = "redox",
    target_os = "solaris",
    target_os = "tvos",
    target_os = "visionos",
    target_os = "watchos"
)))]
fn errno_location() -> *mut libc::c_int {
    std::ptr::null_mut()
}

fn mark_pending(signal: libc::c_int) -> bool {
    for index in 0..MAX_SIGNAL_SLOTS {
        if REGISTERED_SIGNALS[index].load(Ordering::Acquire) == signal {
            PENDING_SIGNALS[index].fetch_add(1, Ordering::Release);
            return true;
        }
    }
    false
}

fn dispatch_pending_signals<F>(on_signal: &F)
where
    F: Fn(libc::c_int),
{
    for index in 0..MAX_SIGNAL_SLOTS {
        let signal = REGISTERED_SIGNALS[index].load(Ordering::Acquire);
        if signal <= 0 {
            continue;
        }

        let pending = PENDING_SIGNALS[index].swap(0, Ordering::AcqRel);
        for _ in 0..pending {
            on_signal(signal);
        }
    }
}

fn read_signal_loop<F>(read_fd: RawFd, on_signal: F)
where
    F: Fn(libc::c_int),
{
    loop {
        match read_wakeup(read_fd) {
            Ok(true) => dispatch_pending_signals(&on_signal),
            Ok(false) => break,
            Err(err) if err.kind() == io::ErrorKind::Interrupted => continue,
            Err(err) => {
                log::warn!("Signal listener stopped: {}", err);
                break;
            }
        }
    }
    close_fd(read_fd);
}

fn read_wakeup(read_fd: RawFd) -> io::Result<bool> {
    let mut wakeup = 0u8;
    loop {
        // SAFETY: `wakeup` points to one writable byte, and `read_fd` is the
        // blocking pipe read end owned by the listener thread.
        let count = unsafe {
            libc::read(
                read_fd,
                (&mut wakeup as *mut u8).cast::<libc::c_void>(),
                std::mem::size_of::<u8>(),
            )
        };

        if count == 0 {
            return Ok(false);
        }
        if count == 1 {
            return Ok(true);
        }
        if count < 0 {
            return Err(io::Error::last_os_error());
        }
    }
}

fn close_fd(fd: RawFd) {
    if fd >= 0 {
        // SAFETY: Closing an owned file descriptor; errors are intentionally ignored.
        let _ = unsafe { libc::close(fd) };
    }
}

#[cfg(test)]
mod tests {
    use super::{
        PENDING_SIGNALS, SIGNAL_WRITE_FD, clear_registered_signals, close_fd,
        dispatch_pending_signals, errno_location, register_signals, signal_action_flags,
        signal_handler,
    };
    use std::cell::RefCell;
    use std::io;
    use std::sync::{Mutex, atomic::Ordering};

    static TEST_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn signal_handlers_restart_interrupted_syscalls() {
        let _guard = TEST_LOCK.lock().unwrap();

        assert_ne!(signal_action_flags() & libc::SA_RESTART, 0);
    }

    #[cfg(any(
        target_os = "android",
        target_os = "cygwin",
        target_os = "dragonfly",
        target_os = "emscripten",
        target_os = "freebsd",
        target_os = "hurd",
        target_os = "illumos",
        target_os = "ios",
        target_os = "linux",
        target_os = "macos",
        target_os = "netbsd",
        target_os = "redox",
        target_os = "solaris",
        target_os = "tvos",
        target_os = "visionos",
        target_os = "watchos"
    ))]
    #[test]
    fn signal_handler_preserves_errno() {
        let _guard = TEST_LOCK.lock().unwrap();

        clear_registered_signals();
        register_signals(&[libc::SIGTERM]).unwrap();
        SIGNAL_WRITE_FD.store(i32::MAX, Ordering::Release);
        set_errno(libc::E2BIG);

        signal_handler(libc::SIGTERM);

        assert_eq!(current_errno(), libc::E2BIG);
        SIGNAL_WRITE_FD.store(-1, Ordering::Release);
        clear_registered_signals();
    }

    #[test]
    fn spawn_listener_restores_installed_handlers_after_partial_failure() {
        let _guard = TEST_LOCK.lock().unwrap();
        let signal = libc::SIGWINCH;
        let before = current_sigaction(signal).unwrap();

        let err = super::spawn_listener(&[signal, i32::MAX], |_| {}).unwrap_err();

        assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
        let after = current_sigaction(signal).unwrap();
        assert_eq!(after.sa_sigaction, before.sa_sigaction);
        clear_registered_signals();
        SIGNAL_WRITE_FD.store(-1, Ordering::Release);
    }

    #[test]
    fn pending_signals_are_preserved_when_wakeup_write_is_unavailable() {
        let _guard = TEST_LOCK.lock().unwrap();

        clear_registered_signals();
        register_signals(&[libc::SIGTERM]).unwrap();
        SIGNAL_WRITE_FD.store(-1, Ordering::Release);

        signal_handler(libc::SIGTERM);

        let dispatched = RefCell::new(Vec::new());
        dispatch_pending_signals(&|signal| dispatched.borrow_mut().push(signal));
        assert_eq!(dispatched.into_inner(), vec![libc::SIGTERM]);
        assert_eq!(PENDING_SIGNALS[0].load(Ordering::Acquire), 0);

        clear_registered_signals();
    }

    #[test]
    fn pending_signal_counters_preserve_repeated_delivery() {
        let _guard = TEST_LOCK.lock().unwrap();

        clear_registered_signals();
        register_signals(&[libc::SIGUSR1]).unwrap();
        SIGNAL_WRITE_FD.store(-1, Ordering::Release);

        signal_handler(libc::SIGUSR1);
        signal_handler(libc::SIGUSR1);

        let dispatched = RefCell::new(Vec::new());
        dispatch_pending_signals(&|signal| dispatched.borrow_mut().push(signal));
        assert_eq!(dispatched.into_inner(), vec![libc::SIGUSR1, libc::SIGUSR1]);

        clear_registered_signals();
    }

    #[test]
    fn pending_state_survives_full_self_pipe() {
        let _guard = TEST_LOCK.lock().unwrap();

        clear_registered_signals();
        register_signals(&[libc::SIGTERM]).unwrap();
        let (read_fd, write_fd) = super::create_pipe().unwrap();
        SIGNAL_WRITE_FD.store(write_fd, Ordering::Release);

        let wakeup = 1u8;
        loop {
            // SAFETY: `write_fd` is a nonblocking pipe write end and `wakeup`
            // points to one readable byte.
            let count = unsafe {
                libc::write(
                    write_fd,
                    (&wakeup as *const u8).cast::<libc::c_void>(),
                    std::mem::size_of::<u8>(),
                )
            };
            if count == 1 {
                continue;
            }
            assert!(count < 0);
            let err = std::io::Error::last_os_error();
            assert_eq!(err.kind(), std::io::ErrorKind::WouldBlock);
            break;
        }

        signal_handler(libc::SIGTERM);

        let dispatched = RefCell::new(Vec::new());
        dispatch_pending_signals(&|signal| dispatched.borrow_mut().push(signal));
        assert_eq!(dispatched.into_inner(), vec![libc::SIGTERM]);

        SIGNAL_WRITE_FD.store(-1, Ordering::Release);
        clear_registered_signals();
        close_fd(read_fd);
        close_fd(write_fd);
    }

    fn current_errno() -> libc::c_int {
        let location = errno_location();
        assert!(!location.is_null());
        // SAFETY: `location` points to the current thread's errno slot.
        unsafe { *location }
    }

    fn set_errno(value: libc::c_int) {
        let location = errno_location();
        assert!(!location.is_null());
        // SAFETY: `location` points to the current thread's errno slot.
        unsafe {
            *location = value;
        }
    }

    fn current_sigaction(signal: libc::c_int) -> io::Result<libc::sigaction> {
        // SAFETY: Zeroed sigaction is filled by libc when the query succeeds.
        let mut action = unsafe { std::mem::zeroed::<libc::sigaction>() };
        // SAFETY: Null new action queries the current handler for `signal`.
        if unsafe { libc::sigaction(signal, std::ptr::null(), &mut action) } != 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(action)
    }
}
