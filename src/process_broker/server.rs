use std::collections::{BTreeMap, VecDeque};
use std::os::fd::{AsFd, AsRawFd, BorrowedFd, OwnedFd, RawFd};
use std::os::unix::process::CommandExt;
use std::process::{Child, ExitCode, Stdio};
use std::thread::Builder;
use std::time::Duration;
use std::{env, io, mem};

use anyhow::{Context, Result, anyhow, bail};
use rustix::event::{self, PollFd, PollFlags};
use rustix::io::{self as rustix_io, Errno, FdFlags};
use rustix::process::{self as rustix_process, Pid, Signal};

use crate::daemon::protocol_v2::ProtocolId;
use crate::env_vars;

use super::execution::{
    OwnedProcess, kill_child_process_group, publish_bounded, run_bounded, status_code,
    supports_retained_publication, terminate_owned_children,
};
use super::transport::{
    decode_blob, encode_blob, recv_packet, send_packet, shutdown_requested,
    take_graceful_shutdown_signal,
};
use super::wire::{
    BROKER_FD, BROKER_FD_ENV, BROKER_SHUTDOWN_FD, BROKER_SHUTDOWN_FD_ENV, BROKER_TOKEN_ENV,
    BlobWire, BrokerOperation, BrokerOutcome, BrokerRequest, BrokerResponse, BrokerWireResponse,
    HelperKind, HelperLifetime, MAX_INPUT_BYTES, MAX_OUTPUT_BYTES, MAX_OWNED_CHILDREN,
    MAX_PACKET_BYTES, OutputMode,
};

pub(crate) fn run_internal_broker_if_requested() -> Option<ExitCode> {
    let fd = env::var(BROKER_FD_ENV).ok()?.parse::<RawFd>().ok()?;
    let shutdown_fd = env::var(BROKER_SHUTDOWN_FD_ENV)
        .ok()?
        .parse::<RawFd>()
        .ok()?;
    let token = env::var(BROKER_TOKEN_ENV).ok()?;
    if fd != BROKER_FD
        || shutdown_fd != BROKER_SHUTDOWN_FD
        || !canonical_lower_hex(&token, 64)
        || validate_broker_socket(fd).is_err()
        || validate_broker_socket(shutdown_fd).is_err()
    {
        return Some(ExitCode::from(126));
    }
    // SAFETY: both numbers were just checked to be open broker sockets, and
    // nothing in this process closes them before it exits.
    let (fd, shutdown_fd) = unsafe {
        (
            BorrowedFd::borrow_raw(fd),
            BorrowedFd::borrow_raw(shutdown_fd),
        )
    };
    // Restore CLOEXEC before any runtime helper can inherit broker internals.
    for descriptor in [fd, shutdown_fd] {
        if rustix_io::fcntl_setfd(descriptor, FdFlags::CLOEXEC).is_err() {
            return Some(ExitCode::from(126));
        }
    }
    // SAFETY: this entry runs before application worker threads start.
    unsafe {
        env::remove_var(BROKER_FD_ENV);
        env::remove_var(BROKER_SHUTDOWN_FD_ENV);
        env::remove_var(BROKER_TOKEN_ENV);
    }
    Some(match broker_loop(fd, shutdown_fd, &token) {
        Ok(()) => ExitCode::SUCCESS,
        Err(_) => ExitCode::FAILURE,
    })
}

fn validate_broker_socket(fd: RawFd) -> io::Result<()> {
    let mut socket_type = 0_i32;
    let mut length = mem::size_of::<i32>() as libc::socklen_t;
    // SAFETY: the output slots are valid for getsockopt, which also rejects a
    // number that is not an open descriptor. A borrowed descriptor cannot be
    // used here because it requires the number to be open already.
    if unsafe {
        libc::getsockopt(
            fd,
            libc::SOL_SOCKET,
            libc::SO_TYPE,
            (&raw mut socket_type).cast(),
            &raw mut length,
        )
    } != 0
    {
        return Err(io::Error::last_os_error());
    }
    if socket_type != libc::SOCK_SEQPACKET {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "broker descriptor is not SOCK_SEQPACKET",
        ));
    }
    Ok(())
}

fn broker_loop(
    socket: BorrowedFd<'static>,
    shutdown_fd: BorrowedFd<'static>,
    token: &str,
) -> Result<()> {
    let mut ownership = BrokerOwnership::default();
    loop {
        if wait_for_request(socket, shutdown_fd)? == BrokerWake::Shutdown {
            ownership.release_retained_publication();
            return Ok(());
        }
        let (packet, descriptors) = recv_packet(socket)?;
        let request: BrokerRequest = match serde_json::from_slice(&packet) {
            Ok(request) => request,
            Err(error) => {
                let response = BrokerResponse {
                    request_id: String::new(),
                    outcome: BrokerOutcome::Error {
                        kind: super::error::BrokerErrorKind::Rejected,
                        message: format!("malformed broker request: {error}"),
                    },
                };
                send_packet(socket, &serde_json::to_vec(&response)?, &[])?;
                continue;
            }
        };
        if request.token != token {
            bail!("broker authentication failed");
        }
        if !canonical_lower_hex(&request.request_id, 32) {
            bail!("broker request identity is not canonical");
        }
        let request_id = request.request_id;
        let mut descriptors = VecDeque::from(descriptors);
        let wire_response = handle_operation(
            request.operation,
            &mut descriptors,
            &mut ownership,
            shutdown_fd,
        )
        .unwrap_or_else(|error| BrokerWireResponse {
            outcome: BrokerOutcome::Error {
                kind: super::error::error_kind(&error).unwrap_or_default(),
                message: truncate_reason(&format!("{error:#}"), 2048),
            },
            descriptors: Vec::new(),
        });
        let response = BrokerResponse {
            request_id,
            outcome: wire_response.outcome,
        };
        let bytes = serde_json::to_vec(&response)?;
        if bytes.len() > MAX_PACKET_BYTES {
            bail!("broker response exceeded packet cap");
        }
        let response_descriptors = wire_response
            .descriptors
            .iter()
            .map(AsFd::as_fd)
            .collect::<Vec<_>>();
        send_packet(socket, &bytes, &response_descriptors)?;
        if take_graceful_shutdown_signal(shutdown_fd)? {
            ownership.release_retained_publication();
            return Ok(());
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum BrokerWake {
    Request,
    Shutdown,
}

fn wait_for_request(socket: BorrowedFd<'_>, shutdown_fd: BorrowedFd<'_>) -> io::Result<BrokerWake> {
    let mut descriptors = [
        PollFd::from_borrowed_fd(socket, PollFlags::IN),
        PollFd::from_borrowed_fd(shutdown_fd, PollFlags::IN),
    ];
    let unusable = PollFlags::HUP | PollFlags::ERR | PollFlags::NVAL;
    loop {
        match event::poll(&mut descriptors, None) {
            Ok(_) => {}
            Err(Errno::INTR) => continue,
            Err(errno) => return Err(errno.into()),
        }
        let socket_events = descriptors[0].revents();
        let shutdown_events = descriptors[1].revents();
        if shutdown_events.contains(PollFlags::IN) && take_graceful_shutdown_signal(shutdown_fd)? {
            return Ok(BrokerWake::Shutdown);
        }
        if socket_events.contains(PollFlags::IN) {
            return Ok(BrokerWake::Request);
        }
        if socket_events.intersects(unusable) || shutdown_events.intersects(unusable) {
            return Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "broker control channel became unusable",
            ));
        }
    }
}

#[cfg(test)]
pub(super) fn run_loop_for_test(socket: RawFd, shutdown_fd: RawFd, token: &str) -> Result<()> {
    // SAFETY: the test broker thread owns both descriptors and keeps them open
    // until this loop returns.
    let (socket, shutdown_fd) = unsafe {
        (
            BorrowedFd::borrow_raw(socket),
            BorrowedFd::borrow_raw(shutdown_fd),
        )
    };
    broker_loop(socket, shutdown_fd, token)
}

#[derive(Default)]
struct BrokerOwnership {
    children: BTreeMap<String, Child>,
    /// At most one regular clipboard provider is current for this runtime.
    retained_publication: Option<Child>,
}

impl BrokerOwnership {
    fn replace_retained_publication(&mut self, child: Child) {
        if let Some(mut previous) = self.retained_publication.replace(child) {
            kill_child_process_group(&mut previous);
            let _ = previous.wait();
        }
    }

    /// Hands a successful clipboard provider over to Wayland selection lifetime.
    ///
    /// The retained leader is already terminal, so reaping it cannot block. Its
    /// background provider remains in the process group until the compositor
    /// cancels or replaces the selection.
    fn release_retained_publication(&mut self) {
        if let Some(mut publication) = self.retained_publication.take() {
            let _ = publication.wait();
        }
    }
}

impl Drop for BrokerOwnership {
    fn drop(&mut self) {
        if let Some(mut publication) = self.retained_publication.take() {
            kill_child_process_group(&mut publication);
            let _ = publication.wait();
        }
        terminate_owned_children(&mut self.children);
    }
}

fn handle_operation(
    operation: BrokerOperation,
    descriptors: &mut VecDeque<OwnedFd>,
    ownership: &mut BrokerOwnership,
    shutdown_fd: BorrowedFd<'static>,
) -> Result<BrokerWireResponse> {
    if shutdown_requested(shutdown_fd)? {
        bail!("broker operation cancelled during shutdown");
    }
    match operation {
        BrokerOperation::Ping => {
            reject_descriptors(descriptors)?;
            Ok(wire_outcome(BrokerOutcome::Acknowledged))
        }
        BrokerOperation::Run {
            kind,
            program,
            arguments,
            environment,
            input,
            timeout_ms,
            output_cap,
            output_mode,
        } => {
            let input = decode_blob(input, descriptors, MAX_INPUT_BYTES)?;
            reject_descriptors(descriptors)?;
            super::manifest::validate(kind, &program, &arguments, &environment, &input)?;
            if output_mode == OutputMode::Prefix && !super::manifest::supports_prefix_output(kind) {
                bail!("prefix output is restricted to wl-paste");
            }
            let output = run_bounded(
                super::manifest::command(kind, program, arguments, environment),
                input,
                Duration::from_millis(timeout_ms).min(Duration::from_secs(120)),
                output_cap.min(MAX_OUTPUT_BYTES),
                output_mode,
                shutdown_fd,
            )?;
            let (stdout, stdout_descriptor) = encode_blob(output.stdout, MAX_OUTPUT_BYTES)?;
            let (stderr, stderr_descriptor) = encode_blob(output.stderr, MAX_OUTPUT_BYTES)?;
            Ok(BrokerWireResponse {
                outcome: BrokerOutcome::Output {
                    status: status_code(output.status),
                    stdout,
                    stderr,
                    timed_out: output.timed_out,
                    stdout_limit_reached: output.stdout_limit_reached,
                },
                descriptors: stdout_descriptor
                    .into_iter()
                    .chain(stderr_descriptor)
                    .collect(),
            })
        }
        BrokerOperation::Publish {
            kind,
            program,
            arguments,
            environment,
            input,
            timeout_ms,
        } => {
            if !supports_retained_publication(kind) {
                bail!("retained publication is restricted to wl-copy");
            }
            let input = decode_blob(input, descriptors, super::manifest::input_cap(kind))?;
            reject_descriptors(descriptors)?;
            super::manifest::validate(kind, &program, &arguments, &environment, &input)?;
            // Retained publication discards stdout/stderr structurally
            // (publish_bounded uses Stdio::null); there is no output cap to enforce.
            let output = publish_bounded(
                super::manifest::command(kind, program, arguments, environment),
                input,
                Duration::from_millis(timeout_ms).min(Duration::from_secs(120)),
                shutdown_fd,
            )?;
            if let Some(retained) = output.retained {
                ownership.replace_retained_publication(retained);
            }
            Ok(BrokerWireResponse {
                outcome: BrokerOutcome::Output {
                    status: output.status,
                    stdout: BlobWire::Inline { bytes: Vec::new() },
                    stderr: BlobWire::Inline { bytes: Vec::new() },
                    timed_out: output.timed_out,
                    stdout_limit_reached: false,
                },
                descriptors: Vec::new(),
            })
        }
        BrokerOperation::Spawn {
            kind,
            lifetime,
            watchdog,
            program,
            arguments,
            environment,
        } => spawn_helper(
            SpawnRequest {
                kind,
                lifetime,
                watchdog,
                program,
                arguments,
                environment,
            },
            descriptors,
            &mut ownership.children,
            shutdown_fd,
        ),
        BrokerOperation::Signal { handle, signal } => {
            reject_descriptors(descriptors)?;
            let signal = match signal {
                libc::SIGUSR1 => Signal::USR1,
                libc::SIGUSR2 => Signal::USR2,
                libc::SIGTERM => Signal::TERM,
                libc::SIGKILL => Signal::KILL,
                _ => bail!("signal is not allowed by broker manifest"),
            };
            let child = ownership
                .children
                .get(&handle)
                .ok_or_else(|| anyhow!("unknown broker child handle"))?;
            rustix_process::kill_process(Pid::from_child(child), signal)
                .map_err(io::Error::from)
                .context("broker child signal failed")?;
            Ok(wire_outcome(BrokerOutcome::Acknowledged))
        }
        BrokerOperation::TryWait { handle } => {
            reject_descriptors(descriptors)?;
            let child = ownership
                .children
                .get_mut(&handle)
                .ok_or_else(|| anyhow!("unknown broker child handle"))?;
            if let Some(status) = child.try_wait()? {
                ownership.children.remove(&handle);
                Ok(wire_outcome(BrokerOutcome::Exited {
                    status: status_code(status),
                }))
            } else {
                Ok(wire_outcome(BrokerOutcome::Running))
            }
        }
        BrokerOperation::KillWait { handle } => {
            reject_descriptors(descriptors)?;
            let mut child = ownership
                .children
                .remove(&handle)
                .ok_or_else(|| anyhow!("unknown broker child handle"))?;
            kill_child_process_group(&mut child);
            let status = child.wait()?;
            Ok(wire_outcome(BrokerOutcome::Exited {
                status: status_code(status),
            }))
        }
    }
}

struct SpawnRequest {
    kind: HelperKind,
    lifetime: HelperLifetime,
    watchdog: bool,
    program: super::wire::OsWire,
    arguments: Vec<super::wire::OsWire>,
    environment: Vec<(super::wire::OsWire, Option<super::wire::OsWire>)>,
}

fn spawn_helper(
    request: SpawnRequest,
    descriptors: &mut VecDeque<OwnedFd>,
    children: &mut BTreeMap<String, Child>,
    shutdown_fd: BorrowedFd<'static>,
) -> Result<BrokerWireResponse> {
    let SpawnRequest {
        kind,
        lifetime,
        watchdog,
        program,
        arguments,
        environment,
    } = request;
    super::manifest::validate(kind, &program, &arguments, &environment, &[])?;
    if matches!(kind, HelperKind::InitialDetach) && lifetime != HelperLifetime::DetachedAfterExec {
        bail!("initial detach helper must transfer ownership after exec");
    }
    if children.len() >= MAX_OWNED_CHILDREN {
        bail!("broker child capacity exhausted");
    }
    let mut command = super::manifest::command(kind, program, arguments, environment);
    let watchdog_descriptor = if watchdog {
        if !matches!(kind, HelperKind::Overlay) || lifetime != HelperLifetime::OwnedChild {
            #[cfg(test)]
            if !matches!(kind, HelperKind::TestSleep) {
                bail!("daemon watchdog is only valid for an owned overlay child");
            }
            #[cfg(not(test))]
            bail!("daemon watchdog is only valid for an owned overlay child");
        }
        if descriptors.len() != 1 {
            bail!("owned overlay spawn requires exactly one daemon watchdog");
        }
        let descriptor = descriptors
            .pop_front()
            .ok_or_else(|| anyhow!("checked watchdog descriptor disappeared"))?;
        set_cloexec(&descriptor, false)?;
        command.env(
            env_vars::DAEMON_WATCHDOG_FD_ENV,
            descriptor.as_raw_fd().to_string(),
        );
        Some(descriptor)
    } else {
        reject_descriptors(descriptors)?;
        None
    };
    let initial_detach = matches!(kind, HelperKind::InitialDetach);
    if !initial_detach {
        command.process_group(0);
    }
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let handle = loop {
        let candidate = ProtocolId::generate()?.to_string();
        if !children.contains_key(&candidate) {
            break candidate;
        }
    };
    if shutdown_requested(shutdown_fd)? {
        bail!("broker spawn cancelled during shutdown");
    }
    let child = command.spawn().map_err(super::error::BrokerError::spawn)?;
    let child = if initial_detach {
        // The execed overlay calls setsid(). It must not be a process-group leader
        // at that point or setsid() deterministically fails with EPERM.
        OwnedProcess::process(child)
    } else {
        OwnedProcess::process_group(child)
    };
    drop(watchdog_descriptor);
    let pid = child.id();
    match lifetime {
        HelperLifetime::OwnedChild | HelperLifetime::OperationBound => {
            children.insert(handle.clone(), child.into_child());
        }
        HelperLifetime::DetachedAfterExec => {
            Builder::new()
                .name(format!("wayscriber-detached-reaper-{pid}"))
                .spawn(move || {
                    let mut child = child;
                    let _ = child.wait();
                })
                .context("failed to start detached helper reaper")?;
        }
    }
    Ok(wire_outcome(BrokerOutcome::Spawned { handle, pid }))
}

fn reject_descriptors(descriptors: &VecDeque<OwnedFd>) -> Result<()> {
    if !descriptors.is_empty() {
        bail!("broker request included unexpected descriptors");
    }
    Ok(())
}

fn set_cloexec(descriptor: &OwnedFd, enabled: bool) -> Result<()> {
    let current = rustix_io::fcntl_getfd(descriptor)
        .map_err(io::Error::from)
        .context("failed to read descriptor flags")?;
    let updated = if enabled {
        current | FdFlags::CLOEXEC
    } else {
        current - FdFlags::CLOEXEC
    };
    rustix_io::fcntl_setfd(descriptor, updated)
        .map_err(io::Error::from)
        .context("failed to update descriptor flags")
}

fn wire_outcome(outcome: BrokerOutcome) -> BrokerWireResponse {
    BrokerWireResponse {
        outcome,
        descriptors: Vec::new(),
    }
}

fn truncate_reason(reason: &str, cap: usize) -> String {
    if reason.len() <= cap {
        return reason.to_owned();
    }
    let mut end = cap;
    while !reason.is_char_boundary(end) {
        end -= 1;
    }
    reason[..end].to_owned()
}

fn canonical_lower_hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

#[cfg(test)]
mod tests {
    use std::process::Command;

    use rustix::net::{self, AddressFamily, SendFlags, SocketFlags, SocketType};

    use super::*;

    fn test_socket_pair() -> (OwnedFd, OwnedFd) {
        net::socketpair(
            AddressFamily::UNIX,
            SocketType::SEQPACKET,
            SocketFlags::CLOEXEC,
            None,
        )
        .unwrap()
    }

    #[test]
    fn shutdown_channel_hangup_is_not_a_graceful_shutdown() {
        let (_control_writer, control_reader) = test_socket_pair();
        let (shutdown_writer, shutdown_reader) = test_socket_pair();
        drop(shutdown_writer);

        let Err(error) = wait_for_request(control_reader.as_fd(), shutdown_reader.as_fd()) else {
            panic!("hangup must unwind through destructive broker cleanup");
        };

        assert_eq!(error.kind(), io::ErrorKind::BrokenPipe);
    }

    #[test]
    fn graceful_shutdown_packet_is_accepted_before_peer_hangup() {
        let (_control_writer, control_reader) = test_socket_pair();
        let (shutdown_writer, shutdown_reader) = test_socket_pair();
        let signal = [super::super::transport::GRACEFUL_SHUTDOWN_BYTE];
        assert_eq!(
            net::send(&shutdown_writer, &signal, SendFlags::NOSIGNAL).unwrap(),
            1
        );
        drop(shutdown_writer);

        assert!(matches!(
            wait_for_request(control_reader.as_fd(), shutdown_reader.as_fd()),
            Ok(BrokerWake::Shutdown)
        ));
    }

    #[test]
    fn malformed_shutdown_packet_is_not_a_graceful_shutdown() {
        let (_control_writer, control_reader) = test_socket_pair();
        let (shutdown_writer, shutdown_reader) = test_socket_pair();
        let invalid = [2_u8];
        assert_eq!(
            net::send(&shutdown_writer, &invalid, SendFlags::NOSIGNAL).unwrap(),
            1
        );

        let Err(error) = wait_for_request(control_reader.as_fd(), shutdown_reader.as_fd()) else {
            panic!("invalid packet must not authorize ownership release");
        };

        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    }

    #[test]
    fn oversized_shutdown_packet_is_not_a_graceful_shutdown() {
        let (_control_writer, control_reader) = test_socket_pair();
        let (shutdown_writer, shutdown_reader) = test_socket_pair();
        let invalid = [super::super::transport::GRACEFUL_SHUTDOWN_BYTE; 2];
        assert_eq!(
            net::send(&shutdown_writer, &invalid, SendFlags::NOSIGNAL).unwrap(),
            2
        );

        let Err(error) = wait_for_request(control_reader.as_fd(), shutdown_reader.as_fd()) else {
            panic!("oversized packet must not authorize ownership release");
        };

        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    }

    #[test]
    fn abnormal_ownership_drop_kills_a_retained_provider() {
        let mut command = Command::new("sleep");
        command.arg("30").process_group(0);
        let child = command.spawn().unwrap();
        let pid = i32::try_from(child.id()).unwrap();
        let ownership = BrokerOwnership {
            children: BTreeMap::new(),
            retained_publication: Some(child),
        };

        drop(ownership);

        assert!(rustix_process::test_kill_process(Pid::from_raw(pid).unwrap()).is_err());
    }
}
