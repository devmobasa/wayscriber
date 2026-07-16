use std::io;
use std::os::fd::{FromRawFd, OwnedFd, RawFd};
use std::sync::atomic::{AtomicI32, AtomicU64, AtomicUsize, Ordering};

mod listener;

#[cfg(test)]
pub(crate) use listener::SignalListenerFailure;
pub(crate) use listener::{SignalListener, SignalListenerHealth, spawn_listener};

const MAX_SIGNAL_SLOTS: usize = 8;
const GATE_STATE_BITS: u32 = 2;
const GATE_STATE_MASK: u64 = (1 << GATE_STATE_BITS) - 1;
const GATE_INACTIVE: u64 = 0;
const GATE_ACTIVE: u64 = 1;
const GATE_STOPPING: u64 = 2;
const GATE_SETTING_UP: u64 = 3;
const MAX_GATE_EPOCH: u64 = u64::MAX >> GATE_STATE_BITS;

#[cfg(not(target_has_atomic = "64"))]
compile_error!("wayscriber signal handling requires lock-free 64-bit atomics");
#[cfg(not(target_has_atomic = "ptr"))]
compile_error!("wayscriber signal handling requires lock-free pointer-sized atomics");
#[cfg(not(target_has_atomic = "32"))]
compile_error!("wayscriber signal handling requires lock-free 32-bit atomics");
#[cfg(not(target_has_atomic = "8"))]
compile_error!("wayscriber signal handling requires lock-free 8-bit atomics");

static HANDLER_GATE_TOKEN: AtomicU64 = AtomicU64::new(GATE_INACTIVE);
static ACTIVE_HANDLERS: AtomicUsize = AtomicUsize::new(0);
static SIGNAL_WRITE_FD: AtomicI32 = AtomicI32::new(-1);
static REGISTERED_SIGNALS: [AtomicI32; MAX_SIGNAL_SLOTS] =
    [const { AtomicI32::new(0) }; MAX_SIGNAL_SLOTS];
static PENDING_SIGNALS: [std::sync::atomic::AtomicBool; MAX_SIGNAL_SLOTS] =
    [const { std::sync::atomic::AtomicBool::new(false) }; MAX_SIGNAL_SLOTS];

fn gate_token(epoch: u64, state: u64) -> u64 {
    (epoch << GATE_STATE_BITS) | state
}

fn gate_epoch(token: u64) -> u64 {
    token >> GATE_STATE_BITS
}

fn gate_state(token: u64) -> u64 {
    token & GATE_STATE_MASK
}

fn validate_signals(signals: &[libc::c_int]) -> io::Result<()> {
    if signals.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "at least one signal must be registered",
        ));
    }
    if signals.len() > MAX_SIGNAL_SLOTS {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "too many signals registered",
        ));
    }

    for (index, &signal) in signals.iter().enumerate() {
        if signal <= 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "signal numbers must be positive",
            ));
        }
        if signals[..index].contains(&signal) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "duplicate signals are not supported",
            ));
        }
    }
    Ok(())
}

fn reserve_listener_epoch() -> io::Result<u64> {
    loop {
        let current = HANDLER_GATE_TOKEN.load(Ordering::Acquire);
        if gate_state(current) != GATE_INACTIVE || ACTIVE_HANDLERS.load(Ordering::Acquire) != 0 {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "a signal listener is already active",
            ));
        }
        let epoch = gate_epoch(current)
            .checked_add(1)
            .filter(|epoch| *epoch <= MAX_GATE_EPOCH)
            .ok_or_else(|| io::Error::other("signal listener epoch space exhausted"))?;
        let setting_up = gate_token(epoch, GATE_SETTING_UP);
        match HANDLER_GATE_TOKEN.compare_exchange(
            current,
            setting_up,
            Ordering::AcqRel,
            Ordering::Acquire,
        ) {
            Ok(_) => return Ok(epoch),
            Err(_) => continue,
        }
    }
}

fn publish_listener_active(epoch: u64) {
    HANDLER_GATE_TOKEN.store(gate_token(epoch, GATE_ACTIVE), Ordering::Release);
}

fn rollback_listener_reservation(epoch: u64) {
    HANDLER_GATE_TOKEN.store(gate_token(epoch, GATE_INACTIVE), Ordering::Release);
}

fn begin_listener_teardown(epoch: u64) -> io::Result<()> {
    HANDLER_GATE_TOKEN
        .compare_exchange(
            gate_token(epoch, GATE_ACTIVE),
            gate_token(epoch, GATE_STOPPING),
            Ordering::AcqRel,
            Ordering::Acquire,
        )
        .map(|_| ())
        .map_err(|actual| {
            io::Error::other(format!(
                "signal listener gate changed unexpectedly during teardown ({actual:#x})"
            ))
        })
}

fn finish_listener_teardown(epoch: u64) {
    HANDLER_GATE_TOKEN.store(gate_token(epoch, GATE_INACTIVE), Ordering::Release);
}

fn wait_for_admitted_handlers() {
    while ACTIVE_HANDLERS.load(Ordering::Acquire) != 0 {
        std::thread::yield_now();
    }
}

fn publish_registered_signals(signals: &[libc::c_int]) {
    clear_registered_signals();
    for (index, &signal) in signals.iter().enumerate() {
        REGISTERED_SIGNALS[index].store(signal, Ordering::Release);
    }
}

fn clear_registered_signals() {
    for index in 0..MAX_SIGNAL_SLOTS {
        PENDING_SIGNALS[index].store(false, Ordering::Release);
        REGISTERED_SIGNALS[index].store(0, Ordering::Release);
    }
}

fn create_pipe() -> io::Result<(OwnedFd, OwnedFd)> {
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

    // SAFETY: pipe returned two fresh descriptors and ownership is transferred
    // exactly once into these wrappers.
    Ok(unsafe { (OwnedFd::from_raw_fd(fds[0]), OwnedFd::from_raw_fd(fds[1])) })
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

fn restore_handlers(handlers: &[(libc::c_int, libc::sigaction)]) {
    for (signal, previous) in handlers.iter().rev() {
        // SAFETY: `previous` was returned by `sigaction` for the same signal.
        let _ = unsafe { libc::sigaction(*signal, previous, std::ptr::null_mut()) };
    }
}

fn signal_action_flags() -> libc::c_int {
    libc::SA_RESTART
}

extern "C" fn signal_handler(signal: libc::c_int) {
    let _errno_guard = ErrnoGuard::new();
    let token = HANDLER_GATE_TOKEN.load(Ordering::Acquire);
    if gate_state(token) != GATE_ACTIVE {
        return;
    }

    #[cfg(test)]
    test_hooks::pause_if_requested(test_hooks::PAUSE_AFTER_TOKEN_READ);

    ACTIVE_HANDLERS.fetch_add(1, Ordering::AcqRel);

    #[cfg(test)]
    test_hooks::pause_if_requested(test_hooks::PAUSE_AFTER_ADMISSION);

    if HANDLER_GATE_TOKEN.load(Ordering::Acquire) != token {
        ACTIVE_HANDLERS.fetch_sub(1, Ordering::AcqRel);
        return;
    }

    #[cfg(test)]
    test_hooks::DESCRIPTOR_ACCESSES.fetch_add(1, Ordering::AcqRel);

    if mark_pending(signal) {
        let fd = SIGNAL_WRITE_FD.load(Ordering::Acquire);
        if fd >= 0 {
            let wakeup = 1u8;
            // SAFETY: the exact active gate protects this stable nonblocking
            // descriptor until every admitted handler has decremented.
            let _ = unsafe {
                libc::write(
                    fd,
                    (&wakeup as *const u8).cast::<libc::c_void>(),
                    std::mem::size_of::<u8>(),
                )
            };
        }
    }

    ACTIVE_HANDLERS.fetch_sub(1, Ordering::AcqRel);
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
            unsafe { *self.location = self.saved };
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
            return !PENDING_SIGNALS[index].swap(true, Ordering::AcqRel);
        }
    }
    false
}

fn dispatch_pending_signals(on_signal: &dyn Fn(libc::c_int)) -> bool {
    let mut dispatched = false;
    for index in 0..MAX_SIGNAL_SLOTS {
        let signal = REGISTERED_SIGNALS[index].load(Ordering::Acquire);
        if signal > 0 && PENDING_SIGNALS[index].swap(false, Ordering::AcqRel) {
            dispatched = true;
            on_signal(signal);
        }
    }
    dispatched
}

fn write_pipe_hint(fd: RawFd) -> io::Result<()> {
    let wakeup = 1u8;
    loop {
        // SAFETY: `fd` is the owner-retained nonblocking pipe write end.
        let result = unsafe {
            libc::write(
                fd,
                (&wakeup as *const u8).cast::<libc::c_void>(),
                std::mem::size_of::<u8>(),
            )
        };
        if result == 1 {
            return Ok(());
        }
        if result < 0 {
            let err = io::Error::last_os_error();
            match err.kind() {
                io::ErrorKind::Interrupted => continue,
                io::ErrorKind::WouldBlock => return Ok(()),
                _ => return Err(err),
            }
        }
        return Err(io::Error::new(
            io::ErrorKind::WriteZero,
            format!("signal pipe returned a short write ({result} bytes)"),
        ));
    }
}

fn close_fd(fd: RawFd) {
    if fd >= 0 {
        // SAFETY: Closing an owned descriptor during setup rollback.
        let _ = unsafe { libc::close(fd) };
    }
}

#[cfg(test)]
mod test_hooks {
    use std::sync::atomic::{AtomicBool, AtomicU8, AtomicUsize, Ordering};

    pub(super) const PAUSE_AFTER_TOKEN_READ: u8 = 1;
    pub(super) const PAUSE_AFTER_ADMISSION: u8 = 2;
    pub(super) static PAUSE_STAGE: AtomicU8 = AtomicU8::new(0);
    pub(super) static HANDLER_PAUSED: AtomicBool = AtomicBool::new(false);
    pub(super) static RESUME_HANDLER: AtomicBool = AtomicBool::new(false);
    pub(super) static DESCRIPTOR_ACCESSES: AtomicUsize = AtomicUsize::new(0);

    pub(super) fn pause_if_requested(stage: u8) {
        if PAUSE_STAGE.load(Ordering::Acquire) != stage {
            return;
        }
        HANDLER_PAUSED.store(true, Ordering::Release);
        while !RESUME_HANDLER.load(Ordering::Acquire) {
            std::hint::spin_loop();
            std::thread::yield_now();
        }
    }
}

#[cfg(test)]
pub(crate) fn test_signal_lock() -> std::sync::MutexGuard<'static, ()> {
    use std::sync::{Mutex, OnceLock};
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[cfg(test)]
pub(crate) fn deliver_signal_for_test(signal: libc::c_int) {
    signal_handler(signal);
}

#[cfg(test)]
mod tests;
