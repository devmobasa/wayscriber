use std::collections::{BTreeMap, VecDeque};
use std::io;
use std::os::fd::{AsRawFd, OwnedFd, RawFd};
use std::os::unix::process::CommandExt;
use std::process::{ExitCode, Stdio};
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};

use super::execution::{
    ProcessGroupChild, kill_child_process_group, publish_bounded, run_bounded, status_code,
    supports_retained_publication, terminate_owned_children,
};
use super::transport::{decode_blob, encode_blob, recv_packet, send_packet, shutdown_requested};
use super::wire::{
    BROKER_FD, BROKER_FD_ENV, BROKER_SHUTDOWN_FD, BROKER_SHUTDOWN_FD_ENV, BROKER_TOKEN_ENV,
    BlobWire, BrokerOperation, BrokerOutcome, BrokerRequest, BrokerResponse, BrokerWireResponse,
    HelperKind, HelperLifetime, MAX_INPUT_BYTES, MAX_OUTPUT_BYTES, MAX_OWNED_CHILDREN,
    MAX_PACKET_BYTES, OutputMode,
};

pub(crate) fn run_internal_broker_if_requested() -> Option<ExitCode> {
    let fd = std::env::var(BROKER_FD_ENV).ok()?.parse::<RawFd>().ok()?;
    let shutdown_fd = std::env::var(BROKER_SHUTDOWN_FD_ENV)
        .ok()?
        .parse::<RawFd>()
        .ok()?;
    let token = std::env::var(BROKER_TOKEN_ENV).ok()?;
    if fd != BROKER_FD
        || shutdown_fd != BROKER_SHUTDOWN_FD
        || !canonical_lower_hex(&token, 64)
        || validate_broker_socket(fd).is_err()
        || validate_broker_socket(shutdown_fd).is_err()
    {
        return Some(ExitCode::from(126));
    }
    // Restore CLOEXEC before any runtime helper can inherit broker internals.
    for descriptor in [fd, shutdown_fd] {
        // SAFETY: both descriptors were validated as inherited broker channels.
        if unsafe { libc::fcntl(descriptor, libc::F_SETFD, libc::FD_CLOEXEC) } != 0 {
            return Some(ExitCode::from(126));
        }
    }
    // SAFETY: this entry runs before application worker threads start.
    unsafe {
        std::env::remove_var(BROKER_FD_ENV);
        std::env::remove_var(BROKER_SHUTDOWN_FD_ENV);
        std::env::remove_var(BROKER_TOKEN_ENV);
    }
    Some(match broker_loop(fd, shutdown_fd, &token) {
        Ok(()) => ExitCode::SUCCESS,
        Err(_) => ExitCode::FAILURE,
    })
}

fn validate_broker_socket(fd: RawFd) -> io::Result<()> {
    let mut socket_type = 0_i32;
    let mut length = std::mem::size_of::<i32>() as libc::socklen_t;
    // SAFETY: the output slots are valid for getsockopt.
    if unsafe {
        libc::getsockopt(
            fd,
            libc::SOL_SOCKET,
            libc::SO_TYPE,
            (&mut socket_type as *mut i32).cast(),
            &mut length,
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

fn broker_loop(socket: RawFd, shutdown_fd: RawFd, token: &str) -> Result<()> {
    let mut ownership = BrokerOwnership::default();
    loop {
        if wait_for_request(socket, shutdown_fd)? == BrokerWake::Shutdown {
            return Ok(());
        }
        let (packet, descriptors) = recv_packet(socket)?;
        let request: BrokerRequest = match serde_json::from_slice(&packet) {
            Ok(request) => request,
            Err(error) => {
                let response = BrokerResponse {
                    request_id: String::new(),
                    outcome: BrokerOutcome::Error {
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
            .map(AsRawFd::as_raw_fd)
            .collect::<Vec<_>>();
        send_packet(socket, &bytes, &response_descriptors)?;
        if shutdown_requested(shutdown_fd)? {
            return Ok(());
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum BrokerWake {
    Request,
    Shutdown,
}

fn wait_for_request(socket: RawFd, shutdown_fd: RawFd) -> io::Result<BrokerWake> {
    let mut descriptors = [
        libc::pollfd {
            fd: socket,
            events: libc::POLLIN,
            revents: 0,
        },
        libc::pollfd {
            fd: shutdown_fd,
            events: libc::POLLIN,
            revents: 0,
        },
    ];
    loop {
        // SAFETY: descriptors points to two initialized pollfd entries.
        let result = unsafe { libc::poll(descriptors.as_mut_ptr(), descriptors.len() as _, -1) };
        if result < 0 {
            let error = io::Error::last_os_error();
            if error.kind() == io::ErrorKind::Interrupted {
                continue;
            }
            return Err(error);
        }
        if descriptors[1].revents & (libc::POLLIN | libc::POLLHUP | libc::POLLERR) != 0 {
            return Ok(BrokerWake::Shutdown);
        }
        if descriptors[0].revents & libc::POLLIN != 0 {
            return Ok(BrokerWake::Request);
        }
        if descriptors[0].revents & (libc::POLLHUP | libc::POLLERR | libc::POLLNVAL) != 0
            || descriptors[1].revents & libc::POLLNVAL != 0
        {
            return Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "broker control channel became unusable",
            ));
        }
    }
}

#[cfg(test)]
pub(super) fn run_loop_for_test(socket: RawFd, shutdown_fd: RawFd, token: &str) -> Result<()> {
    broker_loop(socket, shutdown_fd, token)
}

#[derive(Default)]
struct BrokerOwnership {
    children: BTreeMap<String, std::process::Child>,
    /// At most one regular clipboard provider is current for this runtime.
    retained_publication: Option<std::process::Child>,
}

impl BrokerOwnership {
    fn replace_retained_publication(&mut self, child: std::process::Child) {
        if let Some(mut previous) = self.retained_publication.replace(child) {
            kill_child_process_group(&mut previous);
            let _ = previous.wait();
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
    shutdown_fd: RawFd,
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
                super::manifest::command(program, arguments, environment),
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
            let output = publish_bounded(
                super::manifest::command(program, arguments, environment),
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
            if !matches!(
                signal,
                libc::SIGUSR1 | libc::SIGUSR2 | libc::SIGTERM | libc::SIGKILL
            ) {
                bail!("signal is not allowed by broker manifest");
            }
            let child = ownership
                .children
                .get(&handle)
                .ok_or_else(|| anyhow!("unknown broker child handle"))?;
            // SAFETY: the broker retains the exact unreaped child handle.
            if unsafe { libc::kill(child.id() as i32, signal) } != 0 {
                return Err(io::Error::last_os_error()).context("broker child signal failed");
            }
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
    children: &mut BTreeMap<String, std::process::Child>,
    shutdown_fd: RawFd,
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
    if children.len() >= MAX_OWNED_CHILDREN {
        bail!("broker child capacity exhausted");
    }
    let mut command = super::manifest::command(program, arguments, environment);
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
            crate::env_vars::DAEMON_WATCHDOG_FD_ENV,
            descriptor.as_raw_fd().to_string(),
        );
        Some(descriptor)
    } else {
        reject_descriptors(descriptors)?;
        None
    };
    command
        .process_group(0)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let handle = loop {
        let candidate = crate::daemon::protocol_v2::ProtocolId::generate()?.to_string();
        if !children.contains_key(&candidate) {
            break candidate;
        }
    };
    if shutdown_requested(shutdown_fd)? {
        bail!("broker spawn cancelled during shutdown");
    }
    let child = ProcessGroupChild::new(command.spawn().context("broker helper spawn failed")?);
    drop(watchdog_descriptor);
    let pid = child.id();
    match lifetime {
        HelperLifetime::OwnedChild | HelperLifetime::OperationBound => {
            children.insert(handle.clone(), child.into_child());
        }
        HelperLifetime::DetachedAfterExec => {
            std::thread::Builder::new()
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
    // SAFETY: fcntl reads and updates descriptor-local flags.
    let current = unsafe { libc::fcntl(descriptor.as_raw_fd(), libc::F_GETFD) };
    if current < 0 {
        return Err(io::Error::last_os_error()).context("failed to read descriptor flags");
    }
    let updated = if enabled {
        current | libc::FD_CLOEXEC
    } else {
        current & !libc::FD_CLOEXEC
    };
    if unsafe { libc::fcntl(descriptor.as_raw_fd(), libc::F_SETFD, updated) } != 0 {
        return Err(io::Error::last_os_error()).context("failed to update descriptor flags");
    }
    Ok(())
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
