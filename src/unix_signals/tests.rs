use super::listener::MAX_SIGNAL_PIPE_BYTES_PER_PASS;
use super::*;
use std::io;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Barrier, mpsc};
use std::thread;
use std::time::{Duration, Instant};

const TEST_SIGNAL: libc::c_int = libc::SIGWINCH;

#[test]
fn signal_handlers_restart_interrupted_syscalls() {
    let _guard = test_signal_lock();
    assert_ne!(signal_action_flags() & libc::SA_RESTART, 0);
}

#[test]
fn setup_failure_restores_partial_installation_and_releases_singleton() {
    let _guard = test_signal_lock();
    let before = current_sigaction(TEST_SIGNAL).unwrap();

    let err = match spawn_listener(&[TEST_SIGNAL, i32::MAX], |_| {}, || {}) {
        Ok(_) => panic!("invalid signal unexpectedly installed"),
        Err(err) => err,
    };

    assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
    let after = current_sigaction(TEST_SIGNAL).unwrap();
    assert_eq!(after.sa_sigaction, before.sa_sigaction);
    assert_eq!(SIGNAL_WRITE_FD.load(Ordering::Acquire), -1);
    assert_eq!(
        gate_state(HANDLER_GATE_TOKEN.load(Ordering::Acquire)),
        GATE_INACTIVE
    );

    let mut replacement = spawn_listener(&[TEST_SIGNAL], |_| {}, || {}).unwrap();
    replacement.stop_and_join().unwrap();
}

#[test]
fn signal_handler_preserves_errno() {
    let _guard = test_signal_lock();
    let mut listener = spawn_listener(&[TEST_SIGNAL], |_| {}, || {}).unwrap();
    set_errno(libc::E2BIG);

    signal_handler(TEST_SIGNAL);

    assert_eq!(current_errno(), libc::E2BIG);
    listener.stop_and_join().unwrap();
}

#[test]
fn pending_state_survives_an_unavailable_self_pipe() {
    let _guard = test_signal_lock();
    let callbacks = Arc::new(AtomicUsize::new(0));
    let callback_count = Arc::clone(&callbacks);
    let mut listener = spawn_listener(
        &[TEST_SIGNAL],
        move |_| {
            callback_count.fetch_add(1, Ordering::AcqRel);
        },
        || {},
    )
    .unwrap();
    let write_fd = listener.endpoint_fds().1;
    SIGNAL_WRITE_FD.store(-1, Ordering::Release);

    signal_handler(TEST_SIGNAL);

    SIGNAL_WRITE_FD.store(write_fd, Ordering::Release);
    assert!(dispatch_pending_signals(&|_| {
        callbacks.fetch_add(1, Ordering::AcqRel);
    }));
    assert_eq!(callbacks.load(Ordering::Acquire), 1);
    listener.stop_and_join().unwrap();
}

#[test]
fn wake_before_listener_wait_is_retained_and_callback_precedes_owner_wake() {
    let _guard = test_signal_lock();
    let owner_wake = crate::backend::wayland::RuntimeWakeSource::new().unwrap();
    let wake_handle = owner_wake.handle();
    let callback_published = Arc::new(AtomicBool::new(false));
    let callback_state = Arc::clone(&callback_published);
    let mut listener = spawn_listener(
        &[TEST_SIGNAL],
        move |_| callback_state.store(true, Ordering::Release),
        move || wake_handle.wake().unwrap(),
    )
    .unwrap();

    signal_handler(TEST_SIGNAL);

    wait_for_owner_wake(&owner_wake);
    assert!(callback_published.load(Ordering::Acquire));
    listener.stop_and_join().unwrap();
}

#[cfg(target_os = "linux")]
#[test]
fn signal_delivered_while_listener_is_blocked_wakes_owner() {
    let _guard = test_signal_lock();
    let owner_wake = crate::backend::wayland::RuntimeWakeSource::new().unwrap();
    let wake_handle = owner_wake.handle();
    let callbacks = Arc::new(AtomicUsize::new(0));
    let callback_count = Arc::clone(&callbacks);
    let mut listener = spawn_listener(
        &[TEST_SIGNAL],
        move |_| {
            callback_count.fetch_add(1, Ordering::AcqRel);
        },
        move || wake_handle.wake().unwrap(),
    )
    .unwrap();
    wait_until_listener_blocks(&listener);

    signal_handler(TEST_SIGNAL);

    wait_for_owner_wake(&owner_wake);
    assert_eq!(callbacks.load(Ordering::Acquire), 1);
    listener.stop_and_join().unwrap();
}

#[test]
fn signal_burst_coalesces_while_callback_is_in_flight() {
    let _guard = test_signal_lock();
    let entered = Arc::new(Barrier::new(2));
    let release = Arc::new(Barrier::new(2));
    let callbacks = Arc::new(AtomicUsize::new(0));
    let callback_entered = Arc::clone(&entered);
    let callback_release = Arc::clone(&release);
    let callback_count = Arc::clone(&callbacks);
    let mut listener = spawn_listener(
        &[TEST_SIGNAL],
        move |_| {
            let call = callback_count.fetch_add(1, Ordering::AcqRel);
            if call == 0 {
                callback_entered.wait();
                callback_release.wait();
            }
        },
        || {},
    )
    .unwrap();

    signal_handler(TEST_SIGNAL);
    entered.wait();
    for _ in 0..64 {
        signal_handler(TEST_SIGNAL);
    }
    release.wait();
    wait_until(|| callbacks.load(Ordering::Acquire) == 2);

    assert_eq!(callbacks.load(Ordering::Acquire), 2);
    listener.stop_and_join().unwrap();
}

#[test]
fn read_error_publishes_failure_before_owner_wake_and_retains_endpoints() {
    let _guard = test_signal_lock();
    let owner_wake = crate::backend::wayland::RuntimeWakeSource::new().unwrap();
    let wake_handle = owner_wake.handle();
    let mut listener =
        spawn_listener(&[TEST_SIGNAL], |_| {}, move || wake_handle.wake().unwrap()).unwrap();
    let endpoints = listener.endpoint_fds();

    listener.inject_read_error(libc::EIO);
    wait_for_owner_wake(&owner_wake);

    assert!(matches!(
        listener.health(),
        SignalListenerHealth::Failed(SignalListenerFailure::ReadFailed {
            raw_os_error: Some(libc::EIO),
            ..
        })
    ));
    assert!(fd_is_open(endpoints.0));
    assert!(fd_is_open(endpoints.1));
    assert!(listener.retains_endpoints());
    listener.stop_and_join().unwrap();
    assert!(!listener.retains_endpoints());
}

#[test]
fn callback_panic_publishes_failure_before_independent_owner_wake() {
    let _guard = test_signal_lock();
    let owner_wake = crate::backend::wayland::RuntimeWakeSource::new().unwrap();
    let wake_handle = owner_wake.handle();
    let mut listener = spawn_listener(
        &[TEST_SIGNAL],
        |_| panic!("injected callback panic"),
        move || wake_handle.wake().unwrap(),
    )
    .unwrap();

    signal_handler(TEST_SIGNAL);
    wait_for_owner_wake(&owner_wake);

    assert_eq!(
        listener.health(),
        SignalListenerHealth::Failed(SignalListenerFailure::CallbackPanicked)
    );
    listener.stop_and_join().unwrap();
}

#[test]
fn old_epoch_paused_before_admission_cannot_access_next_generation_descriptor() {
    let _guard = test_signal_lock();
    reset_handler_hooks(test_hooks::PAUSE_AFTER_TOKEN_READ);
    let mut first = spawn_listener(&[TEST_SIGNAL], |_| {}, || {}).unwrap();
    let old_handler = thread::spawn(|| signal_handler(TEST_SIGNAL));
    wait_until(|| test_hooks::HANDLER_PAUSED.load(Ordering::Acquire));

    first.stop_and_join().unwrap();
    let mut second = spawn_listener(&[TEST_SIGNAL], |_| {}, || {}).unwrap();
    let accesses_before = test_hooks::DESCRIPTOR_ACCESSES.load(Ordering::Acquire);
    test_hooks::RESUME_HANDLER.store(true, Ordering::Release);
    old_handler.join().unwrap();

    assert_eq!(
        test_hooks::DESCRIPTOR_ACCESSES.load(Ordering::Acquire),
        accesses_before
    );
    reset_handler_hooks(0);
    second.stop_and_join().unwrap();
}

#[test]
fn descriptor_close_waits_for_every_admitted_handler() {
    let _guard = test_signal_lock();
    reset_handler_hooks(test_hooks::PAUSE_AFTER_ADMISSION);
    let listener = spawn_listener(&[TEST_SIGNAL], |_| {}, || {}).unwrap();
    let endpoints = listener.endpoint_fds();
    let admitted_handler = thread::spawn(|| signal_handler(TEST_SIGNAL));
    wait_until(|| test_hooks::HANDLER_PAUSED.load(Ordering::Acquire));

    let (stopped_tx, stopped_rx) = mpsc::channel();
    let stopper = thread::spawn(move || {
        let mut listener = listener;
        let result = listener.stop_and_join();
        stopped_tx.send(result).unwrap();
    });
    assert!(stopped_rx.recv_timeout(Duration::from_millis(20)).is_err());
    assert!(fd_is_open(endpoints.0));
    assert!(fd_is_open(endpoints.1));

    test_hooks::RESUME_HANDLER.store(true, Ordering::Release);
    admitted_handler.join().unwrap();
    stopped_rx
        .recv_timeout(Duration::from_secs(1))
        .unwrap()
        .unwrap();
    stopper.join().unwrap();
    reset_handler_hooks(0);
}

#[test]
fn continuous_signals_cannot_starve_stop_and_join() {
    let _guard = test_signal_lock();
    let listener = spawn_listener(&[TEST_SIGNAL], |_| {}, || {}).unwrap();
    let keep_sending = Arc::new(AtomicBool::new(true));
    let sender_flag = Arc::clone(&keep_sending);
    let sender = thread::spawn(move || {
        while sender_flag.load(Ordering::Acquire) {
            signal_handler(TEST_SIGNAL);
            thread::yield_now();
        }
    });
    let (stopped_tx, stopped_rx) = mpsc::channel();
    let stopper = thread::spawn(move || {
        let mut listener = listener;
        stopped_tx.send(listener.stop_and_join()).unwrap();
    });

    let result = stopped_rx.recv_timeout(Duration::from_secs(1));
    keep_sending.store(false, Ordering::Release);
    sender.join().unwrap();
    result
        .expect("continuous signals starved teardown")
        .unwrap();
    stopper.join().unwrap();
}

#[test]
fn stop_restores_handlers_and_is_idempotent() {
    let _guard = test_signal_lock();
    let before = current_sigaction(TEST_SIGNAL).unwrap();
    let mut listener = spawn_listener(&[TEST_SIGNAL], |_| {}, || {}).unwrap();
    let installed = current_sigaction(TEST_SIGNAL).unwrap();
    assert_ne!(installed.sa_sigaction, before.sa_sigaction);

    listener.stop_and_join().unwrap();
    listener.stop_and_join().unwrap();

    let after = current_sigaction(TEST_SIGNAL).unwrap();
    assert_eq!(after.sa_sigaction, before.sa_sigaction);
    assert_eq!(listener.health(), SignalListenerHealth::Stopped);
}

#[test]
fn stale_old_generation_handler_cannot_write_to_reused_numeric_descriptor() {
    let _guard = test_signal_lock();
    reset_handler_hooks(test_hooks::PAUSE_AFTER_TOKEN_READ);
    let mut listener = spawn_listener(&[TEST_SIGNAL], |_| {}, || {}).unwrap();
    let old_write_fd = listener.endpoint_fds().1;
    let old_handler = thread::spawn(|| signal_handler(TEST_SIGNAL));
    wait_until(|| test_hooks::HANDLER_PAUSED.load(Ordering::Acquire));
    listener.stop_and_join().unwrap();

    let (reuse_read, reuse_write) = create_pipe().unwrap();
    let reuse_read = if reuse_read.as_raw_fd() == old_write_fd {
        // SAFETY: duplicates the live read endpoint so `old_write_fd` can be
        // deliberately reused for the write endpoint below.
        let duplicate = unsafe { libc::dup(reuse_read.as_raw_fd()) };
        assert!(duplicate >= 0);
        drop(reuse_read);
        // SAFETY: `dup` returned a fresh descriptor owned by this test.
        unsafe { OwnedFd::from_raw_fd(duplicate) }
    } else {
        reuse_read
    };
    set_status_flag(reuse_read.as_raw_fd(), libc::O_NONBLOCK).unwrap();
    if reuse_write.as_raw_fd() != old_write_fd {
        // SAFETY: duplicates the live pipe endpoint onto the deliberately
        // reused numeric descriptor for this isolated race test.
        assert!(unsafe { libc::dup2(reuse_write.as_raw_fd(), old_write_fd) } >= 0);
    }
    test_hooks::RESUME_HANDLER.store(true, Ordering::Release);
    old_handler.join().unwrap();

    let mut byte = 0_u8;
    // SAFETY: `byte` is writable and the pipe read descriptor remains open.
    let count = unsafe {
        libc::read(
            reuse_read.as_raw_fd(),
            (&mut byte as *mut u8).cast::<libc::c_void>(),
            1,
        )
    };
    assert_eq!(count, -1);
    assert_eq!(io::Error::last_os_error().kind(), io::ErrorKind::WouldBlock);
    if reuse_write.as_raw_fd() != old_write_fd {
        close_fd(old_write_fd);
    }
    reset_handler_hooks(0);
}

#[test]
fn listener_pass_is_explicitly_bounded() {
    assert_eq!(MAX_SIGNAL_PIPE_BYTES_PER_PASS, 4096);
}

fn wait_for_owner_wake(source: &crate::backend::wayland::RuntimeWakeSource) {
    let mut pollfd = libc::pollfd {
        fd: source.poll_fd().as_raw_fd(),
        events: libc::POLLIN,
        revents: 0,
    };
    // SAFETY: source owns the descriptor throughout this bounded wait.
    let ready = unsafe { libc::poll(&mut pollfd, 1, 1_000) };
    assert_eq!(ready, 1, "owner was not woken");
    assert_ne!(pollfd.revents & libc::POLLIN, 0);
    source.drain().unwrap();
}

#[cfg(target_os = "linux")]
fn wait_until_listener_blocks(listener: &SignalListener) {
    wait_until(|| {
        let tid = listener.thread_tid();
        if tid == 0 {
            return false;
        }
        std::fs::read_to_string(format!("/proc/self/task/{tid}/stat"))
            .ok()
            .and_then(|stat| {
                stat.rsplit_once(") ")
                    .and_then(|(_, suffix)| suffix.chars().next())
            })
            == Some('S')
    });
}

fn wait_until(mut predicate: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(1);
    while !predicate() {
        assert!(Instant::now() < deadline, "condition was not observed");
        thread::yield_now();
    }
}

fn reset_handler_hooks(stage: u8) {
    test_hooks::PAUSE_STAGE.store(stage, Ordering::Release);
    test_hooks::HANDLER_PAUSED.store(false, Ordering::Release);
    test_hooks::RESUME_HANDLER.store(false, Ordering::Release);
    test_hooks::DESCRIPTOR_ACCESSES.store(0, Ordering::Release);
}

fn fd_is_open(fd: RawFd) -> bool {
    // SAFETY: F_GETFD only queries the supplied descriptor.
    (unsafe { libc::fcntl(fd, libc::F_GETFD) }) >= 0
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
    unsafe { *location = value };
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
