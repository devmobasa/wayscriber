use std::io;
use std::os::fd::{AsRawFd, OwnedFd};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

#[cfg(not(test))]
use std::env;
#[cfg(not(test))]
use std::ffi::CString;
#[cfg(not(test))]
use std::os::unix::ffi::OsStrExt;
#[cfg(not(test))]
use std::ptr;
use std::thread;
#[cfg(test)]
use std::thread::Builder;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
#[cfg(not(test))]
use rustix::io as rustix_io;
use rustix::io::Errno;
use rustix::net::{self, AddressFamily, SocketFlags, SocketType};
#[cfg(not(test))]
use rustix::process::Signal;
use rustix::process::{self as rustix_process, Pid, WaitOptions};

use crate::daemon::protocol_v2::ProtocolToken;
#[cfg(not(test))]
use crate::env_vars;

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
    let token = ProtocolToken::generate()?.to_string();
    let thread_token = token.clone();
    let thread = Builder::new()
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
    let token = ProtocolToken::generate()
        .context("failed to generate broker authentication token")?
        .to_string();
    let exe = env::current_exe().context("failed to resolve broker executable")?;
    let exe = CString::new(exe.as_os_str().as_bytes())?;
    let argv = [exe.as_ptr(), ptr::null()];
    let mut environment = env::vars_os()
        .filter(|(name, _)| {
            name != BROKER_FD_ENV
                && name != BROKER_TOKEN_ENV
                && name != BROKER_SHUTDOWN_FD_ENV
                && name != env_vars::DAEMON_WATCHDOG_FD_ENV
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
    envp.push(ptr::null());

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
        // SAFETY: the clone gave this child a private copy of the parent's
        // memory with only the calling thread, so `exe`, the `environment`
        // strings, and the null-terminated `argv` and `envp` arrays built
        // above are still valid here. `child_fd` and `shutdown_fd` are open
        // descriptors inherited from the parent. Only raw syscalls run, so no
        // lock held by another parent thread is touched, and the block never
        // returns: execve replaces the image or exit_group/_exit end the child.
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
        if let Some(broker_pid) = Pid::from_raw(pid) {
            let _ = rustix_process::kill_process(broker_pid, Signal::KILL);
        }
        wait_for_broker_process(pid);
        return Err(error).context("process broker exec/authentication handshake failed");
    }
    Ok(broker)
}

#[cfg(not(test))]
fn duplicate_for_exec(descriptor: &OwnedFd) -> Result<OwnedFd> {
    rustix_io::fcntl_dupfd_cloexec(descriptor, 5)
        .map_err(io::Error::from)
        .context("failed to stage broker descriptor")
}

fn socket_pair(label: &str) -> Result<(OwnedFd, OwnedFd)> {
    net::socketpair(
        AddressFamily::UNIX,
        SocketType::SEQPACKET,
        SocketFlags::CLOEXEC,
        None,
    )
    .map_err(io::Error::from)
    .with_context(|| format!("failed to create {label} socketpair"))
}

/// Whether the broker process exits within `grace`, reaping it if it does.
pub(super) fn broker_exits_within(child_pid: libc::pid_t, grace: Duration) -> bool {
    if child_pid <= 0 {
        return true;
    }

    let Some(pid) = Pid::from_raw(child_pid) else {
        return true;
    };
    let deadline = Instant::now() + grace;
    loop {
        match rustix_process::waitpid(Some(pid), WaitOptions::NOHANG) {
            Ok(Some(_)) => return true,
            Ok(None) => {}
            Err(Errno::INTR) => continue,
            // Already reaped: nothing is left to wait for.
            Err(_) => return true,
        }
        if Instant::now() >= deadline {
            return false;
        }
        thread::sleep(Duration::from_millis(10));
    }
}

pub(super) fn wait_for_broker_process(child_pid: libc::pid_t) {
    if child_pid <= 0 {
        return;
    }
    let Some(pid) = Pid::from_raw(child_pid) else {
        return;
    };
    while matches!(
        rustix_process::waitpid(Some(pid), WaitOptions::empty()),
        Err(Errno::INTR)
    ) {}
}
