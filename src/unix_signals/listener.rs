use std::fmt;
use std::io;
use std::os::fd::{AsRawFd, OwnedFd, RawFd};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

use super::{
    SIGNAL_WRITE_FD, begin_listener_teardown, clear_registered_signals, create_pipe,
    dispatch_pending_signals, finish_listener_teardown, install_handler, publish_listener_active,
    publish_registered_signals, reserve_listener_epoch, restore_handlers,
    rollback_listener_reservation, validate_signals, wait_for_admitted_handlers, write_pipe_hint,
};

pub(super) const MAX_SIGNAL_PIPE_BYTES_PER_PASS: usize = 4096;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SignalListenerFailure {
    UnexpectedEof,
    ReadFailed {
        kind: io::ErrorKind,
        raw_os_error: Option<i32>,
    },
    CallbackPanicked,
    ListenerPanicked,
}

impl fmt::Display for SignalListenerFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnexpectedEof => write!(f, "signal pipe closed unexpectedly"),
            Self::ReadFailed { kind, raw_os_error } => write!(
                f,
                "signal pipe read failed ({kind:?}, os error {raw_os_error:?})"
            ),
            Self::CallbackPanicked => write!(f, "signal callback panicked"),
            Self::ListenerPanicked => write!(f, "signal listener thread panicked"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SignalListenerHealth {
    Running,
    Failed(SignalListenerFailure),
    Stopping,
    Stopped,
}

struct ListenerShared {
    stop_requested: AtomicBool,
    health: Mutex<SignalListenerHealth>,
    on_signal: Box<dyn Fn(libc::c_int) + Send + Sync + 'static>,
    owner_wake: Arc<dyn Fn() + Send + Sync + 'static>,
    #[cfg(test)]
    injected_read_error: std::sync::atomic::AtomicI32,
    #[cfg(test)]
    thread_tid: std::sync::atomic::AtomicI32,
}

pub(crate) struct SignalListener {
    epoch: u64,
    thread: Option<JoinHandle<()>>,
    read_fd: Option<OwnedFd>,
    write_fd: Option<OwnedFd>,
    installed_handlers: Vec<(libc::c_int, libc::sigaction)>,
    shared: Arc<ListenerShared>,
    teardown_complete: bool,
}

pub(crate) fn spawn_listener<F, W>(
    signals: &[libc::c_int],
    on_signal: F,
    owner_wake: W,
) -> io::Result<SignalListener>
where
    F: Fn(libc::c_int) + Send + Sync + 'static,
    W: Fn() + Send + Sync + 'static,
{
    validate_signals(signals)?;
    let epoch = reserve_listener_epoch()?;
    let (read_fd, write_fd) = match create_pipe() {
        Ok(pipe) => pipe,
        Err(err) => {
            rollback_listener_reservation(epoch);
            return Err(err);
        }
    };

    publish_registered_signals(signals);
    SIGNAL_WRITE_FD.store(write_fd.as_raw_fd(), Ordering::Release);
    // The registered slots and stable self-pipe are ready before any custom
    // handler is installed. Admit each handler immediately so signals cannot
    // disappear between sigaction and listener-thread startup.
    publish_listener_active(epoch);

    let mut installed_handlers = Vec::with_capacity(signals.len());
    for &signal in signals {
        match install_handler(signal) {
            Ok(previous) => {
                installed_handlers.push((signal, previous));
                #[cfg(test)]
                super::test_hooks::pause_if_requested(
                    super::test_hooks::PAUSE_AFTER_HANDLER_INSTALL,
                );
            }
            Err(err) => {
                rollback_setup(epoch, &installed_handlers);
                return Err(err);
            }
        }
    }

    let shared = Arc::new(ListenerShared {
        stop_requested: AtomicBool::new(false),
        health: Mutex::new(SignalListenerHealth::Running),
        on_signal: Box::new(on_signal),
        owner_wake: Arc::new(owner_wake),
        #[cfg(test)]
        injected_read_error: std::sync::atomic::AtomicI32::new(0),
        #[cfg(test)]
        thread_tid: std::sync::atomic::AtomicI32::new(0),
    });
    let thread_shared = Arc::clone(&shared);
    let listener_read_fd = read_fd.as_raw_fd();
    let thread = match thread::Builder::new()
        .name("wayscriber-signals".to_string())
        .spawn(move || listener_thread(listener_read_fd, &thread_shared))
    {
        Ok(thread) => thread,
        Err(err) => {
            rollback_setup(epoch, &installed_handlers);
            return Err(err);
        }
    };

    Ok(SignalListener {
        epoch,
        thread: Some(thread),
        read_fd: Some(read_fd),
        write_fd: Some(write_fd),
        installed_handlers,
        shared,
        teardown_complete: false,
    })
}

fn rollback_setup(epoch: u64, installed_handlers: &[(libc::c_int, libc::sigaction)]) {
    // Setup owns this epoch and published it active before installing handlers.
    // Closing the gate first prevents new admissions while the old actions and
    // descriptors are restored.
    if let Err(err) = begin_listener_teardown(epoch) {
        log::error!("Signal listener setup rollback could not close its admission gate: {err}");
    }
    restore_handlers(installed_handlers);
    wait_for_admitted_handlers();
    SIGNAL_WRITE_FD.store(-1, Ordering::Release);
    clear_registered_signals();
    finish_listener_teardown(epoch);
}

impl SignalListener {
    pub(crate) fn health(&self) -> SignalListenerHealth {
        self.shared
            .health
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    pub(crate) fn stop_and_join(&mut self) -> io::Result<()> {
        if self.teardown_complete {
            return Ok(());
        }

        begin_listener_teardown(self.epoch)?;
        restore_handlers(&self.installed_handlers);
        wait_for_admitted_handlers();
        SIGNAL_WRITE_FD.store(-1, Ordering::Release);
        clear_registered_signals();

        {
            let mut health = self
                .shared
                .health
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            if matches!(*health, SignalListenerHealth::Running) {
                *health = SignalListenerHealth::Stopping;
            }
        }
        self.shared.stop_requested.store(true, Ordering::Release);
        if let Some(write_fd) = self.write_fd.as_ref() {
            let _ = write_pipe_hint(write_fd.as_raw_fd());
        }

        let join_result = self.thread.take().map(JoinHandle::join);
        self.read_fd.take();
        self.write_fd.take();
        self.installed_handlers.clear();
        finish_listener_teardown(self.epoch);
        *self
            .shared
            .health
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = SignalListenerHealth::Stopped;
        self.teardown_complete = true;

        match join_result {
            Some(Err(_)) => Err(io::Error::other(
                "signal listener thread panicked during join",
            )),
            _ => Ok(()),
        }
    }

    #[cfg(test)]
    pub(crate) fn inject_read_error(&self, raw_os_error: i32) {
        self.shared
            .injected_read_error
            .store(raw_os_error, Ordering::Release);
        if let Some(write_fd) = self.write_fd.as_ref() {
            write_pipe_hint(write_fd.as_raw_fd()).unwrap();
        }
    }

    #[cfg(test)]
    pub(crate) fn endpoint_fds(&self) -> (RawFd, RawFd) {
        (
            self.read_fd.as_ref().unwrap().as_raw_fd(),
            self.write_fd.as_ref().unwrap().as_raw_fd(),
        )
    }

    #[cfg(test)]
    pub(crate) fn retains_endpoints(&self) -> bool {
        self.read_fd.is_some() && self.write_fd.is_some()
    }

    #[cfg(test)]
    pub(crate) fn thread_tid(&self) -> libc::pid_t {
        self.shared.thread_tid.load(Ordering::Acquire)
    }
}

impl Drop for SignalListener {
    fn drop(&mut self) {
        let _ = self.stop_and_join();
    }
}

fn listener_thread(read_fd: RawFd, shared: &ListenerShared) {
    #[cfg(test)]
    {
        // SAFETY: gettid has no preconditions and is used only by tests to
        // observe that this exact thread reached its blocking read.
        let tid = unsafe { libc::syscall(libc::SYS_gettid) as libc::pid_t };
        shared.thread_tid.store(tid, Ordering::Release);
    }

    let outcome = catch_unwind(AssertUnwindSafe(|| read_signal_loop(read_fd, shared)));
    let failure = match outcome {
        Ok(Ok(())) => None,
        Ok(Err(failure)) => Some(failure),
        Err(_) => Some(SignalListenerFailure::ListenerPanicked),
    };
    if let Some(failure) = failure {
        publish_failure(shared, failure);
    }
}

fn read_signal_loop(read_fd: RawFd, shared: &ListenerShared) -> Result<(), SignalListenerFailure> {
    let mut buffer = [0_u8; MAX_SIGNAL_PIPE_BYTES_PER_PASS];
    loop {
        if should_stop(shared) {
            return Ok(());
        }

        #[cfg(test)]
        {
            let injected = shared.injected_read_error.swap(0, Ordering::AcqRel);
            if injected != 0 {
                let err = io::Error::from_raw_os_error(injected);
                return Err(read_failure(err));
            }
        }

        // SAFETY: the owner retains `read_fd` through thread join, and `buffer`
        // is writable for the requested bounded pass.
        let count = unsafe {
            libc::read(
                read_fd,
                buffer.as_mut_ptr().cast::<libc::c_void>(),
                buffer.len(),
            )
        };
        if count == 0 {
            return Err(SignalListenerFailure::UnexpectedEof);
        }
        if count < 0 {
            let err = io::Error::last_os_error();
            if err.kind() == io::ErrorKind::Interrupted {
                continue;
            }
            return Err(read_failure(err));
        }
        if should_stop(shared) {
            return Ok(());
        }

        let dispatched = catch_unwind(AssertUnwindSafe(|| {
            dispatch_pending_signals(shared.on_signal.as_ref())
        }))
        .map_err(|_| SignalListenerFailure::CallbackPanicked)?;
        if dispatched {
            wake_owner(shared);
        }
    }
}

fn read_failure(err: io::Error) -> SignalListenerFailure {
    SignalListenerFailure::ReadFailed {
        kind: err.kind(),
        raw_os_error: err.raw_os_error(),
    }
}

fn should_stop(shared: &ListenerShared) -> bool {
    if shared.stop_requested.load(Ordering::Acquire) {
        return true;
    }
    !matches!(
        *shared
            .health
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()),
        SignalListenerHealth::Running
    )
}

fn publish_failure(shared: &ListenerShared, failure: SignalListenerFailure) {
    let published = {
        let mut health = shared
            .health
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if matches!(*health, SignalListenerHealth::Running) {
            *health = SignalListenerHealth::Failed(failure);
            true
        } else {
            false
        }
    };
    if published {
        wake_owner(shared);
    }
}

fn wake_owner(shared: &ListenerShared) {
    let wake = Arc::clone(&shared.owner_wake);
    let _ = catch_unwind(AssertUnwindSafe(move || wake()));
}
