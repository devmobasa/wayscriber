use std::io;
use std::os::fd::{AsFd, AsRawFd, BorrowedFd, FromRawFd, OwnedFd};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SignalProfile {
    Daemon,
    Overlay,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ShutdownSignal {
    Interrupt,
    Terminate,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SignalEvent {
    ToggleOverlay,
    TrayAction,
    Shutdown(ShutdownSignal),
}

/// The event-source seam consumed by the daemon and overlay roots.
///
/// Production supplies one process signal descriptor. Tests supply independent
/// pollable fakes, so signal behavior does not require process-global fixtures.
pub(crate) trait SignalEventSource {
    fn poll_fd(&self) -> io::Result<BorrowedFd<'_>>;
    fn drain(&mut self) -> io::Result<Vec<SignalEvent>>;
}

#[cfg(target_os = "linux")]
enum SignalOwnerState {
    Active {
        admission: OwnedFd,
        descriptor: OwnedFd,
        previous_mask: libc::sigset_t,
    },
    RestorePending {
        admission: OwnedFd,
        previous_mask: libc::sigset_t,
    },
    Finished,
}

/// Root-owned Linux signal adapter.
///
/// Installation blocks the selected signals on the calling thread. Threads
/// created afterward inherit that mask and the owning runtime polls signalfd
/// directly; no process-global handler state or signal-listener thread exists.
/// A PID-scoped abstract socket admits exactly one real owner and rejects a
/// competing runtime before either thread's mask is changed.
#[cfg(target_os = "linux")]
pub(crate) struct SignalOwner {
    profile: SignalProfile,
    state: SignalOwnerState,
    // Signal masks are thread-local. Keeping the owner !Send makes the thread
    // that installed the mask responsible for restoring it.
    _thread_affinity: std::marker::PhantomData<*mut ()>,
}

#[cfg(target_os = "linux")]
impl SignalOwner {
    pub(crate) fn install(profile: SignalProfile) -> io::Result<Self> {
        let mask = signal_mask(profile)?;
        // Admission must precede the thread-local mask change. A concurrent
        // runtime therefore fails without partially installing signal state.
        let admission = acquire_signal_owner_admission()?;
        let previous_mask = block_signals(&mask)?;
        let raw_descriptor = unsafe {
            // SAFETY: `mask` is initialized and remains borrowed for this call.
            libc::signalfd(-1, &mask, libc::SFD_CLOEXEC | libc::SFD_NONBLOCK)
        };
        if raw_descriptor < 0 {
            let descriptor_error = io::Error::last_os_error();
            return match restore_signals(&previous_mask) {
                Ok(()) => Err(descriptor_error),
                Err(restore_error) => Err(io::Error::other(format!(
                    "signalfd setup failed ({descriptor_error}); restoring the previous signal mask also failed ({restore_error})"
                ))),
            };
        }

        let descriptor = unsafe {
            // SAFETY: signalfd returned one fresh descriptor and ownership is
            // transferred exactly once into this owner.
            OwnedFd::from_raw_fd(raw_descriptor)
        };
        Ok(Self {
            profile,
            state: SignalOwnerState::Active {
                admission,
                descriptor,
                previous_mask,
            },
            _thread_affinity: std::marker::PhantomData,
        })
    }

    pub(crate) fn finish(&mut self) -> io::Result<()> {
        self.finish_with(restore_signals)
    }

    fn finish_with(
        &mut self,
        restore: impl FnOnce(&libc::sigset_t) -> io::Result<()>,
    ) -> io::Result<()> {
        let state = std::mem::replace(&mut self.state, SignalOwnerState::Finished);
        let (admission, previous_mask) = match state {
            SignalOwnerState::Active {
                admission,
                descriptor,
                previous_mask,
            } => {
                drop(descriptor);
                (admission, previous_mask)
            }
            SignalOwnerState::RestorePending {
                admission,
                previous_mask,
            } => (admission, previous_mask),
            SignalOwnerState::Finished => return Ok(()),
        };

        match restore(&previous_mask) {
            Ok(()) => Ok(()),
            Err(error) => {
                self.state = SignalOwnerState::RestorePending {
                    admission,
                    previous_mask,
                };
                Err(error)
            }
        }
    }

    fn active_descriptor(&self) -> io::Result<&OwnedFd> {
        match &self.state {
            SignalOwnerState::Active { descriptor, .. } => Ok(descriptor),
            SignalOwnerState::RestorePending { .. } | SignalOwnerState::Finished => {
                Err(io::Error::new(
                    io::ErrorKind::NotConnected,
                    "signal owner is already stopped",
                ))
            }
        }
    }
}

#[cfg(target_os = "linux")]
fn acquire_signal_owner_admission() -> io::Result<OwnedFd> {
    let raw_descriptor = unsafe {
        // SAFETY: socket has no pointer arguments and returns a fresh descriptor
        // on success. CLOEXEC keeps this process-local admission capability out
        // of every later helper exec.
        libc::socket(libc::AF_UNIX, libc::SOCK_DGRAM | libc::SOCK_CLOEXEC, 0)
    };
    if raw_descriptor < 0 {
        return Err(io::Error::last_os_error());
    }
    let descriptor = unsafe {
        // SAFETY: socket returned one fresh descriptor transferred exactly once.
        OwnedFd::from_raw_fd(raw_descriptor)
    };

    let mut address = std::mem::MaybeUninit::<libc::sockaddr_un>::zeroed();
    let address = unsafe {
        // SAFETY: an all-zero sockaddr_un is a valid starting representation;
        // the family and complete abstract-name extent are initialized below.
        address.assume_init_mut()
    };
    address.sun_family = libc::AF_UNIX as libc::sa_family_t;
    let name = format!("wayscriber-signal-owner-{}", std::process::id());
    let name = name.as_bytes();
    let path_capacity = address.sun_path.len().saturating_sub(1);
    if name.len() > path_capacity {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "process-scoped signal admission name exceeds sockaddr_un capacity",
        ));
    }
    for (slot, byte) in address.sun_path[1..].iter_mut().zip(name.iter().copied()) {
        *slot = byte as libc::c_char;
    }
    let address_length = std::mem::offset_of!(libc::sockaddr_un, sun_path) + 1 + name.len();
    let address_length = libc::socklen_t::try_from(address_length).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "process-scoped signal admission address length overflow",
        )
    })?;
    let bind_result = unsafe {
        // SAFETY: address is initialized through the exact abstract-name extent
        // passed to bind, and descriptor remains owned for this call.
        libc::bind(
            descriptor.as_raw_fd(),
            (address as *mut libc::sockaddr_un).cast::<libc::sockaddr>(),
            address_length,
        )
    };
    if bind_result == 0 {
        return Ok(descriptor);
    }

    let error = io::Error::last_os_error();
    if error.raw_os_error() == Some(libc::EADDRINUSE) {
        Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "a real signal owner is already active in this process",
        ))
    } else {
        Err(error)
    }
}

#[cfg(target_os = "linux")]
impl SignalEventSource for SignalOwner {
    fn poll_fd(&self) -> io::Result<BorrowedFd<'_>> {
        Ok(self.active_descriptor()?.as_fd())
    }

    fn drain(&mut self) -> io::Result<Vec<SignalEvent>> {
        let descriptor = self.active_descriptor()?.as_raw_fd();
        let mut events = Vec::new();
        loop {
            let mut info = std::mem::MaybeUninit::<libc::signalfd_siginfo>::uninit();
            let count = unsafe {
                // SAFETY: `info` has exactly the size passed to read and the
                // owner retains the nonblocking signalfd for this operation.
                libc::read(
                    descriptor,
                    info.as_mut_ptr().cast::<libc::c_void>(),
                    std::mem::size_of::<libc::signalfd_siginfo>(),
                )
            };
            if count < 0 {
                let error = io::Error::last_os_error();
                match error.kind() {
                    io::ErrorKind::Interrupted => continue,
                    io::ErrorKind::WouldBlock => return Ok(events),
                    _ => return Err(error),
                }
            }
            if count == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "signal descriptor closed unexpectedly",
                ));
            }
            if count as usize != std::mem::size_of::<libc::signalfd_siginfo>() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("signal descriptor returned a short record ({count} bytes)"),
                ));
            }
            let info = unsafe {
                // SAFETY: a complete signalfd_siginfo record was read above.
                info.assume_init()
            };
            events.push(decode_signal(self.profile, info.ssi_signo as libc::c_int)?);
        }
    }
}

#[cfg(target_os = "linux")]
impl Drop for SignalOwner {
    fn drop(&mut self) {
        let _ = self.finish();
    }
}

#[cfg(target_os = "linux")]
fn signal_mask(profile: SignalProfile) -> io::Result<libc::sigset_t> {
    let mut mask = std::mem::MaybeUninit::<libc::sigset_t>::uninit();
    if unsafe {
        // SAFETY: sigemptyset initializes the entire output mask.
        libc::sigemptyset(mask.as_mut_ptr())
    } != 0
    {
        return Err(io::Error::last_os_error());
    }
    let mut mask = unsafe {
        // SAFETY: sigemptyset succeeded above.
        mask.assume_init()
    };
    for &signal in signals_for_profile(profile) {
        if unsafe {
            // SAFETY: `mask` is initialized and every supplied signal number
            // is a supported process signal.
            libc::sigaddset(&mut mask, signal)
        } != 0
        {
            return Err(io::Error::last_os_error());
        }
    }
    Ok(mask)
}

#[cfg(target_os = "linux")]
fn block_signals(mask: &libc::sigset_t) -> io::Result<libc::sigset_t> {
    let mut previous = std::mem::MaybeUninit::<libc::sigset_t>::uninit();
    let error = unsafe {
        // SAFETY: both masks point to correctly sized storage. pthread_sigmask
        // returns an error number directly rather than setting errno.
        libc::pthread_sigmask(libc::SIG_BLOCK, mask, previous.as_mut_ptr())
    };
    if error != 0 {
        return Err(io::Error::from_raw_os_error(error));
    }
    Ok(unsafe {
        // SAFETY: pthread_sigmask initialized the previous mask on success.
        previous.assume_init()
    })
}

#[cfg(target_os = "linux")]
fn restore_signals(previous: &libc::sigset_t) -> io::Result<()> {
    let error = unsafe {
        // SAFETY: the owner restores the mask captured on this same thread.
        libc::pthread_sigmask(libc::SIG_SETMASK, previous, std::ptr::null_mut())
    };
    if error == 0 {
        Ok(())
    } else {
        Err(io::Error::from_raw_os_error(error))
    }
}

fn signals_for_profile(profile: SignalProfile) -> &'static [libc::c_int] {
    match profile {
        SignalProfile::Daemon => &[libc::SIGUSR1, libc::SIGTERM, libc::SIGINT],
        SignalProfile::Overlay => &[libc::SIGUSR1, libc::SIGUSR2, libc::SIGTERM, libc::SIGINT],
    }
}

fn decode_signal(profile: SignalProfile, signal: libc::c_int) -> io::Result<SignalEvent> {
    match signal {
        libc::SIGUSR1 => Ok(SignalEvent::ToggleOverlay),
        libc::SIGUSR2 if profile == SignalProfile::Overlay => Ok(SignalEvent::TrayAction),
        libc::SIGTERM => Ok(SignalEvent::Shutdown(ShutdownSignal::Terminate)),
        libc::SIGINT => Ok(SignalEvent::Shutdown(ShutdownSignal::Interrupt)),
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("signal profile {profile:?} received unexpected signal {signal}"),
        )),
    }
}

#[cfg(all(unix, not(target_os = "linux")))]
pub(crate) struct SignalOwner;

#[cfg(all(unix, not(target_os = "linux")))]
impl SignalOwner {
    pub(crate) fn install(_profile: SignalProfile) -> io::Result<Self> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "root-owned signal descriptors are currently supported on Linux",
        ))
    }

    pub(crate) fn finish(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(all(unix, not(target_os = "linux")))]
impl SignalEventSource for SignalOwner {
    fn poll_fd(&self) -> io::Result<BorrowedFd<'_>> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "signal descriptor is unavailable on this platform",
        ))
    }

    fn drain(&mut self) -> io::Result<Vec<SignalEvent>> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "signal descriptor is unavailable on this platform",
        ))
    }
}

#[cfg(test)]
pub(crate) struct FakeSignalSource {
    read: std::os::unix::net::UnixStream,
    write: std::os::unix::net::UnixStream,
    pending: std::collections::VecDeque<SignalEvent>,
    failure: Option<io::ErrorKind>,
}

#[cfg(test)]
impl FakeSignalSource {
    pub(crate) fn new() -> io::Result<Self> {
        let (read, write) = std::os::unix::net::UnixStream::pair()?;
        read.set_nonblocking(true)?;
        write.set_nonblocking(true)?;
        Ok(Self {
            read,
            write,
            pending: std::collections::VecDeque::new(),
            failure: None,
        })
    }

    pub(crate) fn publish(&mut self, event: SignalEvent) -> io::Result<()> {
        self.pending.push_back(event);
        match std::io::Write::write(&mut self.write, &[1]) {
            Ok(1) => Ok(()),
            Ok(count) => Err(io::Error::new(
                io::ErrorKind::WriteZero,
                format!("fake signal source wrote {count} wake bytes"),
            )),
            Err(error) => Err(error),
        }
    }

    pub(crate) fn fail_next_drain(&mut self, kind: io::ErrorKind) -> io::Result<()> {
        self.failure = Some(kind);
        match std::io::Write::write(&mut self.write, &[1]) {
            Ok(1) => Ok(()),
            Ok(count) => Err(io::Error::new(
                io::ErrorKind::WriteZero,
                format!("fake signal source wrote {count} wake bytes"),
            )),
            Err(error) => Err(error),
        }
    }
}

#[cfg(test)]
impl SignalEventSource for FakeSignalSource {
    fn poll_fd(&self) -> io::Result<BorrowedFd<'_>> {
        Ok(self.read.as_fd())
    }

    fn drain(&mut self) -> io::Result<Vec<SignalEvent>> {
        let mut buffer = [0_u8; 64];
        loop {
            match std::io::Read::read(&mut self.read, &mut buffer) {
                Ok(0) => {
                    return Err(io::Error::new(
                        io::ErrorKind::UnexpectedEof,
                        "fake signal source closed unexpectedly",
                    ));
                }
                Ok(_) => continue,
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => break,
                Err(error) => return Err(error),
            }
        }
        if let Some(kind) = self.failure.take() {
            return Err(io::Error::new(kind, "injected fake signal-source failure"));
        }
        Ok(self.pending.drain(..).collect())
    }
}

#[cfg(test)]
mod tests;
