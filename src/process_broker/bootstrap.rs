use std::io;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

#[cfg(not(test))]
use std::ffi::CString;
#[cfg(not(test))]
use std::os::unix::ffi::OsStrExt;

use anyhow::{Context, Result};

use super::client::{BrokerInner, ProcessBroker};
use super::wire::BrokerOperation;
#[cfg(not(test))]
use super::wire::{
    BROKER_FD, BROKER_FD_ENV, BROKER_SHUTDOWN_FD, BROKER_SHUTDOWN_FD_ENV, BROKER_TOKEN_ENV,
};

#[cfg(test)]
pub(super) fn start() -> Result<ProcessBroker> {
    let (parent_socket, child_socket) = socket_pair("test broker")?;
    let (shutdown_writer, shutdown_reader) = socket_pair("test broker shutdown")?;
    let token = crate::daemon::protocol_v2::ProtocolToken::generate()?.to_string();
    let thread_token = token.clone();
    let thread = std::thread::Builder::new()
        .name("wayscriber-process-broker-test".into())
        .spawn(move || {
            let _socket = child_socket;
            let _shutdown = shutdown_reader;
            let _ = super::server::run_loop_for_test(
                _socket.as_raw_fd(),
                _shutdown.as_raw_fd(),
                &thread_token,
            );
        })
        .context("failed to start test broker thread")?;
    let broker = ProcessBroker {
        inner: Arc::new(BrokerInner {
            socket: parent_socket,
            shutdown: shutdown_writer,
            token,
            child_pid: 0,
            exchange_lock: Mutex::new(()),
            healthy: AtomicBool::new(true),
            test_thread: Mutex::new(Some(thread)),
        }),
    };
    broker
        .request(BrokerOperation::Ping)
        .context("test process broker handshake failed")?;
    Ok(broker)
}

#[cfg(not(test))]
pub(super) fn start() -> Result<ProcessBroker> {
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

    let child_socket_exec = duplicate_for_exec(&child_socket)?;
    let shutdown_exec = duplicate_for_exec(&shutdown_reader)?;
    let child_fd = child_socket_exec.as_raw_fd();
    let shutdown_fd = shutdown_exec.as_raw_fd();
    // SAFETY: clone has fork-like SIGCHLD semantics; the child branch uses
    // only fixed syscalls over buffers prepared above before exec.
    let pid = unsafe { libc::syscall(libc::SYS_clone, libc::SIGCHLD, 0, 0, 0, 0) as libc::pid_t };
    if pid < 0 {
        return Err(io::Error::last_os_error()).context("raw clone for broker failed");
    }
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
            if libc::syscall(libc::SYS_setpgid, 0, 0) < 0 {
                libc::syscall(libc::SYS_exit_group, 126);
            }
            let _ = libc::syscall(libc::SYS_close_range, 5_u32, u32::MAX, 0_u32);
            libc::syscall(libc::SYS_execve, exe.as_ptr(), argv.as_ptr(), envp.as_ptr());
            libc::syscall(libc::SYS_exit_group, 127);
            libc::_exit(127);
        }
    }
    drop(child_socket);
    drop(child_socket_exec);
    drop(shutdown_reader);
    drop(shutdown_exec);
    let broker = ProcessBroker {
        inner: Arc::new(BrokerInner {
            socket: parent_socket,
            shutdown: shutdown_writer,
            token,
            child_pid: pid,
            exchange_lock: Mutex::new(()),
            healthy: AtomicBool::new(true),
        }),
    };
    if let Err(error) = broker.request(BrokerOperation::Ping) {
        // SAFETY: pid is the raw-clone broker child created above.
        unsafe {
            libc::kill(pid, libc::SIGKILL);
        }
        wait_for_broker_process(pid);
        return Err(error).context("process broker exec/authentication handshake failed");
    }
    Ok(broker)
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
