use std::io;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};

#[cfg(not(test))]
use std::ffi::CString;
#[cfg(not(test))]
use std::os::unix::ffi::OsStrExt;

use anyhow::{Context, Result};

#[cfg(test)]
use std::sync::mpsc::{Receiver, SyncSender, sync_channel};
#[cfg(test)]
use std::time::Duration;

#[cfg(not(test))]
use super::wire::{
    BROKER_FD, BROKER_FD_ENV, BROKER_SHUTDOWN_FD, BROKER_SHUTDOWN_FD_ENV, BROKER_TOKEN_ENV,
};

pub(super) struct BrokerBootstrap {
    pub(super) socket: OwnedFd,
    pub(super) shutdown: OwnedFd,
    pub(super) token: String,
    pub(super) server: BrokerServer,
}

pub(super) enum BrokerServer {
    #[cfg(not(test))]
    Process(libc::pid_t),
    #[cfg(test)]
    Thread(std::thread::JoinHandle<()>),
}

#[cfg(test)]
pub(super) struct BrokerAdmissionControl {
    paused: Receiver<()>,
    release: SyncSender<()>,
}

#[cfg(test)]
impl BrokerAdmissionControl {
    pub(super) fn wait_until_paused(&self, timeout: Duration) -> Result<()> {
        self.paused
            .recv_timeout(timeout)
            .context("test broker did not reach its admission gate")
    }

    pub(super) fn release(&self) -> Result<()> {
        self.release
            .send(())
            .context("test broker admission gate is no longer waiting")
    }
}

impl BrokerServer {
    pub(super) fn wait(self) {
        match self {
            #[cfg(not(test))]
            Self::Process(child_pid) => wait_for_broker_process(child_pid),
            #[cfg(test)]
            Self::Thread(thread) => {
                let _ = thread.join();
            }
        }
    }
}

#[cfg(test)]
pub(super) fn start() -> Result<BrokerBootstrap> {
    start_test_broker(None)
}

#[cfg(test)]
pub(super) fn start_with_admission_gate() -> Result<(BrokerBootstrap, BrokerAdmissionControl)> {
    let (paused, wait_for_pause) = sync_channel(1);
    let (release, wait_for_release) = sync_channel(1);
    let bootstrap = start_test_broker(Some((paused, wait_for_release)))?;
    Ok((
        bootstrap,
        BrokerAdmissionControl {
            paused: wait_for_pause,
            release,
        },
    ))
}

#[cfg(test)]
fn start_test_broker(
    admission_gate: Option<(SyncSender<()>, Receiver<()>)>,
) -> Result<BrokerBootstrap> {
    let (parent_socket, child_socket) = socket_pair("test broker")?;
    let (shutdown_writer, shutdown_reader) = socket_pair("test broker shutdown")?;
    let token = crate::daemon::protocol_v2::ProtocolToken::generate()?.to_string();
    let thread_token = token.clone();
    let thread = std::thread::Builder::new()
        .name("wayscriber-process-broker-test".into())
        .spawn(move || {
            let _socket = child_socket;
            let _shutdown = shutdown_reader;
            let result = if let Some((paused, release)) = admission_gate {
                super::server::run_loop_for_test_with_admission_gate(
                    _socket.as_raw_fd(),
                    _shutdown.as_raw_fd(),
                    &thread_token,
                    paused,
                    release,
                )
            } else {
                super::server::run_loop_for_test(
                    _socket.as_raw_fd(),
                    _shutdown.as_raw_fd(),
                    &thread_token,
                )
            };
            let _ = result;
        })
        .context("failed to start test broker thread")?;
    Ok(BrokerBootstrap {
        socket: parent_socket,
        shutdown: shutdown_writer,
        token,
        server: BrokerServer::Thread(thread),
    })
}

#[cfg(not(test))]
pub(super) fn start() -> Result<BrokerBootstrap> {
    let (parent_socket, child_socket) = socket_pair("broker")?;
    let (shutdown_writer, shutdown_reader) = socket_pair("broker shutdown")?;
    let token = crate::daemon::protocol_v2::ProtocolToken::generate()
        .context("failed to generate broker authentication token")?
        .to_string();
    let exe = std::env::current_exe().context("failed to resolve broker executable")?;
    let exe = CString::new(exe.as_os_str().as_bytes())?;
    let argv = [exe.as_ptr(), std::ptr::null()];
    let mut environment = std::env::vars_os()
        .filter(|(name, _)| {
            name != BROKER_FD_ENV
                && name != BROKER_TOKEN_ENV
                && name != BROKER_SHUTDOWN_FD_ENV
                && name != crate::env_vars::DAEMON_WATCHDOG_FD_ENV
        })
        .map(|(name, value)| {
            let mut bytes = name.as_bytes().to_vec();
            bytes.push(b'=');
            bytes.extend_from_slice(value.as_bytes());
            CString::new(bytes).map_err(anyhow::Error::from)
        })
        .collect::<Result<Vec<_>>>()?;
    environment.push(CString::new(format!("{BROKER_FD_ENV}={BROKER_FD}"))?);
    environment.push(CString::new(format!("{BROKER_TOKEN_ENV}={token}"))?);
    environment.push(CString::new(format!(
        "{BROKER_SHUTDOWN_FD_ENV}={BROKER_SHUTDOWN_FD}"
    ))?);
    let mut envp = environment
        .iter()
        .map(|value| value.as_ptr())
        .collect::<Vec<_>>();
    envp.push(std::ptr::null());
    let descriptor_limit = descriptor_close_limit()?;

    let null_descriptor: OwnedFd = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/null")
        .context("failed to open broker standard-I/O sink")?
        .into();
    let null_exec = duplicate_for_exec(&null_descriptor)?;
    let child_socket_exec = duplicate_for_exec(&child_socket)?;
    let shutdown_exec = duplicate_for_exec(&shutdown_reader)?;
    let null_fd = null_exec.as_raw_fd();
    let child_fd = child_socket_exec.as_raw_fd();
    let shutdown_fd = shutdown_exec.as_raw_fd();
    // SAFETY: clone has fork-like SIGCHLD semantics; the child branch uses
    // only fixed syscalls over buffers prepared above before exec.
    let pid = unsafe { libc::syscall(libc::SYS_clone, libc::SIGCHLD, 0, 0, 0, 0) as libc::pid_t };
    if pid < 0 {
        return Err(io::Error::last_os_error()).context("raw clone for broker failed");
    }
    // BEGIN RAW-CLONE CHILD STUB. Keep the matching checker boundary in
    // tools/check-process-sites.py synchronized with this marker.
    if pid == 0 {
        // Raw-clone child stub: no allocation, formatting, logging,
        // unwinding, Rust destructors, or dynamic loader calls are allowed.
        unsafe {
            if child_fd == BROKER_FD {
                let _ = libc::syscall(libc::SYS_fcntl, BROKER_FD, libc::F_SETFD, 0);
            } else if libc::syscall(libc::SYS_dup3, child_fd, BROKER_FD, 0) < 0 {
                libc::syscall(libc::SYS_exit_group, 126);
            }
            if libc::syscall(libc::SYS_dup3, shutdown_fd, BROKER_SHUTDOWN_FD, 0) < 0 {
                libc::syscall(libc::SYS_exit_group, 126);
            }
            if libc::syscall(libc::SYS_dup3, null_fd, libc::STDIN_FILENO, 0) < 0 {
                libc::syscall(libc::SYS_exit_group, 126);
            }
            if libc::syscall(libc::SYS_dup3, null_fd, libc::STDOUT_FILENO, 0) < 0 {
                libc::syscall(libc::SYS_exit_group, 126);
            }
            if libc::syscall(libc::SYS_dup3, null_fd, libc::STDERR_FILENO, 0) < 0 {
                libc::syscall(libc::SYS_exit_group, 126);
            }
            if libc::syscall(libc::SYS_setpgid, 0, 0) < 0 {
                libc::syscall(libc::SYS_exit_group, 126);
            }
            if libc::syscall(libc::SYS_close_range, 5_u32, u32::MAX, 0_u32) < 0 {
                // Linux before close_range, or a sandbox that denies it, still
                // gets deterministic descriptor hygiene. The upper bound was
                // captured before raw clone so this branch needs only fixed
                // close syscalls and touches no allocator or dynamic loader.
                let mut descriptor = 5_u32;
                while descriptor < descriptor_limit {
                    libc::syscall(libc::SYS_close, descriptor);
                    descriptor += 1;
                }
            }
            libc::syscall(libc::SYS_execve, exe.as_ptr(), argv.as_ptr(), envp.as_ptr());
            libc::syscall(libc::SYS_exit_group, 127);
            libc::_exit(127);
        }
    }
    // END RAW-CLONE CHILD STUB.
    drop(null_descriptor);
    drop(null_exec);
    drop(child_socket);
    drop(child_socket_exec);
    drop(shutdown_reader);
    drop(shutdown_exec);
    Ok(BrokerBootstrap {
        socket: parent_socket,
        shutdown: shutdown_writer,
        token,
        server: BrokerServer::Process(pid),
    })
}

#[cfg(not(test))]
fn descriptor_close_limit() -> Result<u32> {
    let mut limit = std::mem::MaybeUninit::<libc::rlimit>::uninit();
    // SAFETY: getrlimit initializes the complete output value on success.
    if unsafe { libc::getrlimit(libc::RLIMIT_NOFILE, limit.as_mut_ptr()) } != 0 {
        return Err(io::Error::last_os_error())
            .context("failed to resolve broker descriptor close bound");
    }
    let limit = unsafe {
        // SAFETY: getrlimit succeeded above.
        limit.assume_init()
    };
    let kernel_limit = std::fs::read_to_string("/proc/sys/fs/nr_open")
        .context("failed to read the kernel descriptor ceiling")?
        .trim()
        .parse::<libc::rlim_t>()
        .context("kernel descriptor ceiling is not numeric")?;
    // Use both hard ceilings so another embedding thread cannot open above a
    // concurrently raised soft limit between this preflight and raw clone.
    let raw_limit = limit
        .rlim_max
        .min(kernel_limit)
        .min((i32::MAX as libc::rlim_t) + 1);
    u32::try_from(raw_limit).map_err(|_| anyhow::anyhow!("broker descriptor close bound overflow"))
}

#[cfg(not(test))]
fn duplicate_for_exec(descriptor: &OwnedFd) -> Result<OwnedFd> {
    // SAFETY: F_DUPFD_CLOEXEC duplicates the live descriptor at or above five.
    let duplicate = unsafe { libc::fcntl(descriptor.as_raw_fd(), libc::F_DUPFD_CLOEXEC, 5) };
    if duplicate < 0 {
        return Err(io::Error::last_os_error()).context("failed to stage broker descriptor");
    }
    // SAFETY: fcntl returned a fresh owned descriptor.
    Ok(unsafe { OwnedFd::from_raw_fd(duplicate) })
}

fn socket_pair(label: &str) -> Result<(OwnedFd, OwnedFd)> {
    let mut sockets = [0; 2];
    // SAFETY: sockets has room for the returned descriptor pair.
    if unsafe {
        libc::socketpair(
            libc::AF_UNIX,
            libc::SOCK_SEQPACKET | libc::SOCK_CLOEXEC,
            0,
            sockets.as_mut_ptr(),
        )
    } != 0
    {
        return Err(io::Error::last_os_error())
            .with_context(|| format!("failed to create {label} socketpair"));
    }
    // SAFETY: socketpair returned two new descriptors.
    Ok(unsafe {
        (
            OwnedFd::from_raw_fd(sockets[0]),
            OwnedFd::from_raw_fd(sockets[1]),
        )
    })
}

#[cfg(not(test))]
pub(super) fn wait_for_broker_process(child_pid: libc::pid_t) {
    if child_pid <= 0 {
        return;
    }
    let mut status = 0;
    loop {
        // SAFETY: child_pid names the raw-clone broker child owned by its guard.
        let result = unsafe { libc::waitpid(child_pid, &mut status, 0) };
        if result == child_pid {
            break;
        }
        if result < 0 && io::Error::last_os_error().kind() == io::ErrorKind::Interrupted {
            continue;
        }
        break;
    }
}
