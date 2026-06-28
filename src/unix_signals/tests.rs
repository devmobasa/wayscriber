use super::{
    PENDING_SIGNALS, SIGNAL_WRITE_FD, clear_registered_signals, close_fd, dispatch_pending_signals,
    errno_location, register_signals, signal_action_flags, signal_handler,
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
