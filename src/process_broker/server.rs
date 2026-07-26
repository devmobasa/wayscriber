use std::collections::{BTreeMap, VecDeque};
use std::ffi::OsString;
use std::io;
use std::os::fd::{AsRawFd, OwnedFd, RawFd};
use std::os::unix::process::CommandExt;
use std::process::{ExitCode, Stdio};
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};

use super::execution::{
    OwnedProcess, kill_child_process_group, publish_bounded, run_bounded, status_code,
    supports_retained_publication, terminate_owned_children,
};
use super::transport::{
    decode_blob, encode_blob, recv_packet, send_packet, shutdown_requested,
    take_graceful_shutdown_signal,
};
use super::wire::ensure_admission_deadline;
use super::wire::{
    BROKER_FD, BROKER_FD_ENV, BROKER_SHUTDOWN_FD, BROKER_SHUTDOWN_FD_ENV, BROKER_TOKEN_ENV,
    BlobWire, BrokerFileReadWire, BrokerOperation, BrokerOutcome, BrokerRequest, BrokerResponse,
    BrokerWireResponse, HelperKind, HelperLifetime, MAX_INPUT_BYTES, MAX_OUTPUT_BYTES,
    MAX_OWNED_CHILDREN, MAX_PACKET_BYTES, OutputMode,
};

pub(crate) fn run_internal_broker_if_requested() -> Option<ExitCode> {
    if let Some(exit_code) = super::file_reader::run_if_requested() {
        return Some(exit_code);
    }
    let bootstrap = match classify_internal_broker(
        std::env::var_os(BROKER_FD_ENV),
        std::env::var_os(BROKER_SHUTDOWN_FD_ENV),
        std::env::var_os(BROKER_TOKEN_ENV),
    ) {
        InternalBrokerClassification::OrdinaryEntry => return None,
        InternalBrokerClassification::InvalidBootstrap => {
            return Some(ExitCode::from(126));
        }
        InternalBrokerClassification::InternalBroker(bootstrap) => bootstrap,
    };
    let BrokerBootstrap {
        fd,
        shutdown_fd,
        token,
    } = bootstrap;
    if fd != BROKER_FD
        || shutdown_fd != BROKER_SHUTDOWN_FD
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
    // Leave process environment untouched so the safe public entry remains
    // valid for embedding callers. Every broker-created helper strips these
    // private markers, and both validated descriptors are close-on-exec.
    Some(match broker_loop(fd, shutdown_fd, &token) {
        Ok(()) => ExitCode::SUCCESS,
        Err(_) => ExitCode::FAILURE,
    })
}

#[derive(Debug, PartialEq, Eq)]
enum InternalBrokerClassification {
    OrdinaryEntry,
    InternalBroker(BrokerBootstrap),
    InvalidBootstrap,
}

#[derive(Debug, PartialEq, Eq)]
struct BrokerBootstrap {
    fd: RawFd,
    shutdown_fd: RawFd,
    token: String,
}

fn classify_internal_broker(
    fd: Option<OsString>,
    shutdown_fd: Option<OsString>,
    token: Option<OsString>,
) -> InternalBrokerClassification {
    let (fd, shutdown_fd, token) = match (fd, shutdown_fd, token) {
        (None, None, None) => return InternalBrokerClassification::OrdinaryEntry,
        (Some(fd), Some(shutdown_fd), Some(token)) => (fd, shutdown_fd, token),
        _ => return InternalBrokerClassification::InvalidBootstrap,
    };

    let fd = match parse_descriptor(fd) {
        Some(fd) => fd,
        None => return InternalBrokerClassification::InvalidBootstrap,
    };
    let shutdown_fd = match parse_descriptor(shutdown_fd) {
        Some(shutdown_fd) => shutdown_fd,
        None => return InternalBrokerClassification::InvalidBootstrap,
    };
    let token = match token.into_string() {
        Ok(token) if canonical_lower_hex(&token, 64) => token,
        Ok(_) | Err(_) => return InternalBrokerClassification::InvalidBootstrap,
    };

    InternalBrokerClassification::InternalBroker(BrokerBootstrap {
        fd,
        shutdown_fd,
        token,
    })
}

fn parse_descriptor(value: OsString) -> Option<RawFd> {
    value.into_string().ok()?.parse().ok()
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
    broker_loop_with_admission(socket, shutdown_fd, token, |_| Ok(()))
}

fn broker_loop_with_admission(
    socket: RawFd,
    shutdown_fd: RawFd,
    token: &str,
    mut before_admission: impl FnMut(&BrokerOperation) -> Result<()>,
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
        let admission_deadline_monotonic_ns = request.admission_deadline_monotonic_ns;
        let operation = request.operation;
        let mut descriptors = VecDeque::from(descriptors);
        let wire_response = before_admission(&operation)
            .and_then(|()| ensure_admission_deadline(admission_deadline_monotonic_ns))
            .and_then(|()| {
                handle_operation(
                    operation,
                    &mut descriptors,
                    &mut ownership,
                    shutdown_fd,
                    admission_deadline_monotonic_ns,
                )
            })
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
        if descriptors[1].revents & libc::POLLIN != 0 && take_graceful_shutdown_signal(shutdown_fd)?
        {
            return Ok(BrokerWake::Shutdown);
        }
        if descriptors[0].revents & libc::POLLIN != 0 {
            return Ok(BrokerWake::Request);
        }
        if descriptors[0].revents & (libc::POLLHUP | libc::POLLERR | libc::POLLNVAL) != 0
            || descriptors[1].revents & (libc::POLLHUP | libc::POLLERR | libc::POLLNVAL) != 0
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

#[cfg(test)]
pub(super) fn run_loop_for_test_with_admission_gate(
    socket: RawFd,
    shutdown_fd: RawFd,
    token: &str,
    paused: std::sync::mpsc::SyncSender<()>,
    release: std::sync::mpsc::Receiver<()>,
) -> Result<()> {
    let mut paused = Some(paused);
    let mut release = Some(release);
    broker_loop_with_admission(socket, shutdown_fd, token, move |operation| {
        if matches!(operation, BrokerOperation::Ping) {
            return Ok(());
        }
        let Some(paused) = paused.take() else {
            return Ok(());
        };
        paused
            .send(())
            .context("test broker admission observer disconnected")?;
        let release = release
            .take()
            .ok_or_else(|| anyhow!("test broker admission release was already consumed"))?;
        release
            .recv()
            .context("test broker admission release disconnected")
    })
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
    shutdown_fd: RawFd,
    admission_deadline_monotonic_ns: u64,
) -> Result<BrokerWireResponse> {
    ensure_operation_admitted(admission_deadline_monotonic_ns, shutdown_fd)?;
    match operation {
        BrokerOperation::Ping => {
            reject_descriptors(descriptors)?;
            Ok(wire_outcome(BrokerOutcome::Acknowledged))
        }
        BrokerOperation::ReadFile {
            path,
            timeout_ms,
            byte_limit,
        } => {
            reject_descriptors(descriptors)?;
            read_file(
                path,
                timeout_ms,
                byte_limit,
                shutdown_fd,
                admission_deadline_monotonic_ns,
            )
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
            ensure_operation_admitted(admission_deadline_monotonic_ns, shutdown_fd)?;
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
            ensure_operation_admitted(admission_deadline_monotonic_ns, shutdown_fd)?;
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
            admission_deadline_monotonic_ns,
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
            ensure_operation_admitted(admission_deadline_monotonic_ns, shutdown_fd)?;
            // SAFETY: the broker retains the exact unreaped child handle.
            if unsafe { libc::kill(child.id() as i32, signal) } != 0 {
                return Err(io::Error::last_os_error()).context("broker child signal failed");
            }
            Ok(wire_outcome(BrokerOutcome::Acknowledged))
        }
        BrokerOperation::TryWait { handle } => {
            reject_descriptors(descriptors)?;
            ensure_operation_admitted(admission_deadline_monotonic_ns, shutdown_fd)?;
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
            ensure_operation_admitted(admission_deadline_monotonic_ns, shutdown_fd)?;
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

fn read_file(
    path: super::wire::OsWire,
    timeout_ms: u64,
    byte_limit: usize,
    shutdown_fd: RawFd,
    admission_deadline_monotonic_ns: u64,
) -> Result<BrokerWireResponse> {
    if byte_limit == 0 || byte_limit > MAX_OUTPUT_BYTES {
        bail!("broker file-read byte limit is outside transport bounds");
    }
    let path = std::path::PathBuf::from(path.into_os());
    if !path.is_absolute() {
        bail!("broker file-read path must be absolute");
    }
    ensure_operation_admitted(admission_deadline_monotonic_ns, shutdown_fd)?;
    let child_output_cap = if byte_limit < 4096 { 4096 } else { byte_limit };
    let output = run_bounded(
        super::file_reader::command(&path, byte_limit)?,
        Vec::new(),
        Duration::from_millis(timeout_ms).min(Duration::from_secs(120)),
        child_output_cap,
        OutputMode::Complete,
        shutdown_fd,
    )?;
    let status = if output.timed_out {
        BrokerFileReadWire::TimedOut
    } else {
        match status_code(output.status) {
            status if status == i32::from(super::file_reader::EXIT_READY) => {
                if output.stdout.is_empty() {
                    BrokerFileReadWire::ReadFailed {
                        reason: "internal file reader returned an empty ready payload".to_string(),
                    }
                } else if output.stdout.len() > byte_limit {
                    BrokerFileReadWire::TooLarge
                } else {
                    BrokerFileReadWire::Ready
                }
            }
            status if status == i32::from(super::file_reader::EXIT_EMPTY) => {
                BrokerFileReadWire::Empty
            }
            status if status == i32::from(super::file_reader::EXIT_TOO_LARGE) => {
                BrokerFileReadWire::TooLarge
            }
            status if status == i32::from(super::file_reader::EXIT_NOT_REGULAR) => {
                BrokerFileReadWire::NotRegular
            }
            _ => {
                let reason = String::from_utf8_lossy(&output.stderr).trim().to_owned();
                BrokerFileReadWire::ReadFailed {
                    reason: if reason.is_empty() {
                        "internal file reader failed without a diagnostic".to_string()
                    } else {
                        truncate_reason(&reason, 2048)
                    },
                }
            }
        }
    };
    let bytes = if matches!(&status, BrokerFileReadWire::Ready) {
        output.stdout
    } else {
        Vec::new()
    };
    let (bytes, descriptor) = encode_blob(bytes, byte_limit)?;
    Ok(BrokerWireResponse {
        outcome: BrokerOutcome::FileRead {
            result: status,
            bytes,
        },
        descriptors: descriptor.into_iter().collect(),
    })
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
    admission_deadline_monotonic_ns: u64,
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
    let initial_detach = matches!(kind, HelperKind::InitialDetach);
    if !initial_detach {
        command.process_group(0);
    }
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let handle = loop {
        let candidate = crate::daemon::protocol_v2::ProtocolId::generate()?.to_string();
        if !children.contains_key(&candidate) {
            break candidate;
        }
    };
    ensure_operation_admitted(admission_deadline_monotonic_ns, shutdown_fd)?;
    let child = command.spawn().context("broker helper spawn failed")?;
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
            children.insert(
                handle.clone(),
                child
                    .into_child()
                    .context("broker helper ownership disappeared before registration")?,
            );
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

fn ensure_operation_admitted(
    admission_deadline_monotonic_ns: u64,
    shutdown_fd: RawFd,
) -> Result<()> {
    ensure_admission_deadline(admission_deadline_monotonic_ns)?;
    if shutdown_requested(shutdown_fd)? {
        bail!("broker operation cancelled during shutdown");
    }
    Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::fd::FromRawFd;
    use std::os::unix::ffi::OsStringExt;

    fn canonical_test_token() -> OsString {
        OsString::from("a".repeat(64))
    }

    #[test]
    fn broker_classifier_reserves_only_a_complete_bootstrap() {
        assert_eq!(
            classify_internal_broker(None, None, None),
            InternalBrokerClassification::OrdinaryEntry
        );

        let descriptor = Some(OsString::from("3"));
        let shutdown = Some(OsString::from("4"));
        let token = Some(canonical_test_token());
        let partial_markers = [
            (descriptor.clone(), None, None),
            (None, shutdown.clone(), None),
            (None, None, token.clone()),
            (descriptor.clone(), shutdown.clone(), None),
            (descriptor.clone(), None, token.clone()),
            (None, shutdown, token),
        ];

        for (descriptor, shutdown, token) in partial_markers {
            assert_eq!(
                classify_internal_broker(descriptor, shutdown, token),
                InternalBrokerClassification::InvalidBootstrap
            );
        }
    }

    #[test]
    fn broker_classifier_rejects_malformed_descriptors() {
        let invalid_cases = [
            (OsString::from("not-a-number"), OsString::from("4")),
            (OsString::from("3"), OsString::from("not-a-number")),
            (OsString::from_vec(vec![0xff]), OsString::from("4")),
            (OsString::from("3"), OsString::from_vec(vec![0xff])),
        ];

        for (descriptor, shutdown) in invalid_cases {
            assert_eq!(
                classify_internal_broker(
                    Some(descriptor),
                    Some(shutdown),
                    Some(canonical_test_token()),
                ),
                InternalBrokerClassification::InvalidBootstrap
            );
        }
    }

    #[test]
    fn broker_classifier_rejects_malformed_tokens() {
        let invalid_tokens = [
            OsString::from("a".repeat(63)),
            OsString::from("A".repeat(64)),
            OsString::from("g".repeat(64)),
            OsString::from_vec(vec![0xff; 64]),
        ];

        for token in invalid_tokens {
            assert_eq!(
                classify_internal_broker(
                    Some(OsString::from("3")),
                    Some(OsString::from("4")),
                    Some(token),
                ),
                InternalBrokerClassification::InvalidBootstrap
            );
        }
    }

    #[test]
    fn broker_classifier_leaves_descriptor_identity_to_the_process_adapter() {
        assert_eq!(
            classify_internal_broker(
                Some(OsString::from("9")),
                Some(OsString::from("10")),
                Some(canonical_test_token()),
            ),
            InternalBrokerClassification::InternalBroker(BrokerBootstrap {
                fd: 9,
                shutdown_fd: 10,
                token: "a".repeat(64),
            })
        );
    }

    fn test_socket_pair() -> (OwnedFd, OwnedFd) {
        let mut sockets = [0; 2];
        // SAFETY: sockets has room for both returned descriptors.
        assert_eq!(
            unsafe {
                libc::socketpair(
                    libc::AF_UNIX,
                    libc::SOCK_SEQPACKET | libc::SOCK_CLOEXEC,
                    0,
                    sockets.as_mut_ptr(),
                )
            },
            0
        );
        // SAFETY: socketpair returned two fresh owned descriptors.
        unsafe {
            (
                OwnedFd::from_raw_fd(sockets[0]),
                OwnedFd::from_raw_fd(sockets[1]),
            )
        }
    }

    #[test]
    fn shutdown_channel_hangup_is_not_a_graceful_shutdown() -> Result<(), &'static str> {
        let (_control_writer, control_reader) = test_socket_pair();
        let (shutdown_writer, shutdown_reader) = test_socket_pair();
        drop(shutdown_writer);

        let Err(error) = wait_for_request(control_reader.as_raw_fd(), shutdown_reader.as_raw_fd())
        else {
            return Err("hangup must unwind through destructive broker cleanup");
        };

        assert_eq!(error.kind(), io::ErrorKind::BrokenPipe);
        Ok(())
    }

    #[test]
    fn graceful_shutdown_packet_is_accepted_before_peer_hangup() {
        let (_control_writer, control_reader) = test_socket_pair();
        let (shutdown_writer, shutdown_reader) = test_socket_pair();
        let signal = [super::super::transport::GRACEFUL_SHUTDOWN_BYTE];
        // SAFETY: both the descriptor and one-byte buffer are valid.
        assert_eq!(
            unsafe {
                libc::send(
                    shutdown_writer.as_raw_fd(),
                    signal.as_ptr().cast(),
                    signal.len(),
                    libc::MSG_NOSIGNAL,
                )
            },
            1
        );
        drop(shutdown_writer);

        assert!(matches!(
            wait_for_request(control_reader.as_raw_fd(), shutdown_reader.as_raw_fd()),
            Ok(BrokerWake::Shutdown)
        ));
    }

    #[test]
    fn malformed_shutdown_packet_is_not_a_graceful_shutdown() -> Result<(), &'static str> {
        let (_control_writer, control_reader) = test_socket_pair();
        let (shutdown_writer, shutdown_reader) = test_socket_pair();
        let invalid = [2_u8];
        // SAFETY: both the descriptor and one-byte buffer are valid.
        assert_eq!(
            unsafe {
                libc::send(
                    shutdown_writer.as_raw_fd(),
                    invalid.as_ptr().cast(),
                    invalid.len(),
                    libc::MSG_NOSIGNAL,
                )
            },
            1
        );

        let Err(error) = wait_for_request(control_reader.as_raw_fd(), shutdown_reader.as_raw_fd())
        else {
            return Err("invalid packet must not authorize ownership release");
        };

        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        Ok(())
    }

    #[test]
    fn oversized_shutdown_packet_is_not_a_graceful_shutdown() -> Result<(), &'static str> {
        let (_control_writer, control_reader) = test_socket_pair();
        let (shutdown_writer, shutdown_reader) = test_socket_pair();
        let invalid = [super::super::transport::GRACEFUL_SHUTDOWN_BYTE; 2];
        // SAFETY: both the descriptor and two-byte buffer are valid.
        assert_eq!(
            unsafe {
                libc::send(
                    shutdown_writer.as_raw_fd(),
                    invalid.as_ptr().cast(),
                    invalid.len(),
                    libc::MSG_NOSIGNAL,
                )
            },
            2
        );

        let Err(error) = wait_for_request(control_reader.as_raw_fd(), shutdown_reader.as_raw_fd())
        else {
            return Err("oversized packet must not authorize ownership release");
        };

        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        Ok(())
    }

    #[test]
    fn abnormal_ownership_drop_kills_a_retained_provider() {
        let mut command = std::process::Command::new("sleep");
        command.arg("30").process_group(0);
        let child = command
            .spawn()
            .expect("test fixture can launch its retained sleep provider");
        let pid = i32::try_from(child.id()).expect("test child PID fits libc pid_t");
        let ownership = BrokerOwnership {
            children: BTreeMap::new(),
            retained_publication: Some(child),
        };

        drop(ownership);

        // SAFETY: signal zero only probes the test-owned provider after teardown.
        assert_ne!(unsafe { libc::kill(pid, 0) }, 0);
    }
}
