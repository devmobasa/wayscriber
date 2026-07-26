use std::collections::VecDeque;
use std::ffi::{OsStr, OsString};
use std::io;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, SyncSender, channel, sync_channel};
use std::thread::JoinHandle;
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};

use super::bootstrap::{BrokerBootstrap, BrokerServer};
use super::execution::supports_retained_publication;
use super::transport::{
    GRACEFUL_SHUTDOWN_BYTE, decode_blob, encode_blob, recv_packet, send_packet, set_socket_timeout,
};
use super::wire::{
    BlobWire, BrokerFileRead, BrokerFileReadWire, BrokerOperation, BrokerOutcome, BrokerOutput,
    BrokerRequest, BrokerResponse, HelperKind, HelperLifetime, MAX_INPUT_BYTES, MAX_OUTPUT_BYTES,
    MAX_PACKET_BYTES, OsWire, OutputMode,
};

/// The root-owned lifetime boundary for one pre-lock process broker.
///
/// The actor thread uniquely owns the authenticated request transport and the
/// broker child/reaping responsibility. This owner retains the independent
/// shutdown channel so dropping the root can preempt an operation that is
/// currently blocking in the broker process.
#[derive(Debug)]
pub(crate) struct ProcessBrokerOwner {
    commands: Sender<BrokerCommand>,
    shutdown: OwnedFd,
    actor_thread: ActorThread,
}

/// A broker subprocess prepared before the root installs its process-signal
/// mask, but not yet accompanied by any parent-side thread.
pub(crate) struct PreparedProcessBroker {
    bootstrap: Option<BrokerBootstrap>,
}

/// Cloneable capability for submitting synchronous work to one explicit broker owner.
#[derive(Clone, Debug)]
pub(crate) struct ProcessBrokerHandle {
    commands: Sender<BrokerCommand>,
}

#[derive(Debug)]
pub(crate) struct BrokerChild {
    broker: ProcessBrokerHandle,
    handle: String,
    pid: u32,
}

#[derive(Debug)]
enum ActorThread {
    Running(JoinHandle<()>),
    Joined,
}

enum BrokerCommand {
    Exchange {
        operation: BrokerOperation,
        descriptors: Vec<OwnedFd>,
        actor_execution_deadline_monotonic_ns: u64,
        admission_deadline_monotonic_ns: u64,
        reply: SyncSender<BrokerExchangeResult>,
    },
    Shutdown,
}

type BrokerExchangeResult = Result<(BrokerOutcome, Vec<OwnedFd>)>;

struct BrokerActor {
    socket: OwnedFd,
    shutdown: OwnedFd,
    token: String,
    server: BrokerServer,
    healthy: bool,
}

#[derive(Debug, Clone, Copy)]
struct RunOptions {
    timeout: Duration,
    output_cap: usize,
    output_mode: OutputMode,
}

#[cfg(test)]
pub(crate) fn start_for_runtime() -> Result<ProcessBrokerOwner> {
    prepare_for_runtime()?.activate()
}

#[cfg(test)]
pub(super) fn start_for_runtime_with_admission_gate()
-> Result<(ProcessBrokerOwner, super::bootstrap::BrokerAdmissionControl)> {
    let (bootstrap, control) = super::bootstrap::start_with_admission_gate()?;
    Ok((activate_prepared_broker(bootstrap)?, control))
}

/// Spawn the broker subprocess without creating an ordinary application
/// thread. The app uses this two-phase interface to install its signal mask
/// after the broker exec but before activating the parent actor thread.
pub(crate) fn prepare_for_runtime() -> Result<PreparedProcessBroker> {
    Ok(PreparedProcessBroker {
        bootstrap: Some(super::bootstrap::start()?),
    })
}

impl PreparedProcessBroker {
    pub(crate) fn activate(mut self) -> Result<ProcessBrokerOwner> {
        let bootstrap = match self.bootstrap.take() {
            Some(bootstrap) => bootstrap,
            None => {
                return Err(anyhow!(
                    "prepared process broker was already activated or stopped"
                ));
            }
        };
        activate_prepared_broker(bootstrap)
    }
}

impl Drop for PreparedProcessBroker {
    fn drop(&mut self) {
        if let Some(bootstrap) = self.bootstrap.take() {
            let _ = signal_shutdown(bootstrap.shutdown.as_raw_fd());
            bootstrap.server.wait();
        }
    }
}

fn activate_prepared_broker(bootstrap: BrokerBootstrap) -> Result<ProcessBrokerOwner> {
    let actor_shutdown = match bootstrap.shutdown.try_clone() {
        Ok(shutdown) => shutdown,
        Err(error) => {
            let _ = signal_shutdown(bootstrap.shutdown.as_raw_fd());
            bootstrap.server.wait();
            return Err(error).context("failed to duplicate broker fail-stop channel");
        }
    };
    let (commands, receiver) = channel();
    let (actor_start, actor_ready) = sync_channel::<BrokerActor>(1);
    let handle = ProcessBrokerHandle {
        commands: commands.clone(),
    };
    let actor_thread = match std::thread::Builder::new()
        .name("wayscriber-process-broker-owner".into())
        .spawn(move || {
            if let Ok(actor) = actor_ready.recv() {
                actor.run(receiver);
            }
        }) {
        Ok(thread) => thread,
        Err(error) => {
            let _ = signal_shutdown(bootstrap.shutdown.as_raw_fd());
            bootstrap.server.wait();
            return Err(error).context("failed to start process broker owner");
        }
    };
    let BrokerBootstrap {
        socket,
        shutdown,
        token,
        server,
    } = bootstrap;
    let actor = BrokerActor {
        socket,
        shutdown: actor_shutdown,
        token,
        server,
        healthy: true,
    };
    if let Err(error) = actor_start.send(actor) {
        let _ = signal_shutdown(shutdown.as_raw_fd());
        error.0.server.wait();
        let _ = actor_thread.join();
        return Err(anyhow!(
            "process broker owner stopped before accepting its transport"
        ));
    }
    let owner = ProcessBrokerOwner {
        commands,
        shutdown,
        actor_thread: ActorThread::Running(actor_thread),
    };
    if let Err(error) = handle.request(BrokerOperation::Ping) {
        drop(owner);
        return Err(error).context("process broker exec/authentication handshake failed");
    }
    Ok(owner)
}

impl ProcessBrokerOwner {
    pub(crate) fn handle(&self) -> ProcessBrokerHandle {
        ProcessBrokerHandle {
            commands: self.commands.clone(),
        }
    }

    #[cfg(test)]
    pub(crate) fn disconnect_shutdown_channel(&self) -> io::Result<()> {
        // SAFETY: this test-only fault injector operates on this owner's live
        // shutdown socket and does not transfer descriptor ownership.
        if unsafe { libc::shutdown(self.shutdown.as_raw_fd(), libc::SHUT_RDWR) } == 0 {
            Ok(())
        } else {
            Err(io::Error::last_os_error())
        }
    }
}

impl Drop for ProcessBrokerOwner {
    fn drop(&mut self) {
        let _ = signal_shutdown(self.shutdown.as_raw_fd());
        let _ = self.commands.send(BrokerCommand::Shutdown);
        if let ActorThread::Running(thread) =
            std::mem::replace(&mut self.actor_thread, ActorThread::Joined)
        {
            let _ = thread.join();
        }
    }
}

impl BrokerActor {
    fn run(mut self, receiver: Receiver<BrokerCommand>) {
        while let Ok(command) = receiver.recv() {
            match command {
                BrokerCommand::Exchange {
                    operation,
                    descriptors,
                    actor_execution_deadline_monotonic_ns,
                    admission_deadline_monotonic_ns,
                    reply,
                } => {
                    let result = self.exchange(
                        operation,
                        descriptors,
                        actor_execution_deadline_monotonic_ns,
                        admission_deadline_monotonic_ns,
                    );
                    let transport_failed = !self.healthy;
                    if reply.send(result).is_err() {
                        let _ = fail_stop_broker(self.shutdown.as_raw_fd());
                        break;
                    }
                    if transport_failed {
                        break;
                    }
                }
                BrokerCommand::Shutdown => break,
            }
        }
        self.server.wait();
    }

    fn exchange(
        &mut self,
        operation: BrokerOperation,
        descriptors: Vec<OwnedFd>,
        actor_execution_deadline_monotonic_ns: u64,
        admission_deadline_monotonic_ns: u64,
    ) -> BrokerExchangeResult {
        if !self.healthy {
            bail!("process broker transport is no longer usable");
        }
        let request_id = crate::daemon::protocol_v2::ProtocolId::generate()?.to_string();
        let exchange_timeout = broker_exchange_timeout_before(
            &operation,
            actor_execution_deadline_monotonic_ns,
            super::wire::monotonic_now_ns()?,
        )?;
        let packet = serde_json::to_vec(&BrokerRequest {
            token: self.token.clone(),
            request_id: request_id.clone(),
            admission_deadline_monotonic_ns,
            operation,
        })?;
        if packet.len() > MAX_PACKET_BYTES {
            bail!("broker request exceeds packet cap");
        }
        let descriptor_numbers = descriptors
            .iter()
            .map(AsRawFd::as_raw_fd)
            .collect::<Vec<_>>();
        let exchange = (|| -> Result<(BrokerResponse, Vec<OwnedFd>)> {
            set_socket_timeout(self.socket.as_raw_fd(), exchange_timeout)?;
            send_packet(self.socket.as_raw_fd(), &packet, &descriptor_numbers)?;
            let (packet, descriptors) = recv_packet(self.socket.as_raw_fd())?;
            let response: BrokerResponse = serde_json::from_slice(&packet)?;
            if response.request_id != request_id {
                bail!("broker response identity mismatch");
            }
            Ok((response, descriptors))
        })();
        let (response, descriptors) = match exchange {
            Ok(response) => response,
            Err(error) => {
                self.healthy = false;
                let _ = fail_stop_broker(self.shutdown.as_raw_fd());
                return Err(error).context("process broker exchange failed");
            }
        };
        if let BrokerOutcome::Error { message } = response.outcome {
            bail!("process broker rejected request: {message}");
        }
        Ok((response.outcome, descriptors))
    }
}

fn signal_shutdown(descriptor: RawFd) -> io::Result<()> {
    let byte = [GRACEFUL_SHUTDOWN_BYTE];
    loop {
        // SAFETY: descriptor is the broker shutdown socket and byte is readable.
        let written = unsafe {
            libc::send(
                descriptor,
                byte.as_ptr().cast(),
                byte.len(),
                libc::MSG_NOSIGNAL,
            )
        };
        if written == 1 {
            return Ok(());
        }
        if written < 0 && io::Error::last_os_error().kind() == io::ErrorKind::Interrupted {
            continue;
        }
        return Err(if written < 0 {
            io::Error::last_os_error()
        } else {
            io::Error::new(io::ErrorKind::WriteZero, "short broker shutdown write")
        });
    }
}

/// Makes broker ownership teardown destructive after an ambiguous operation.
///
/// Unlike the graceful byte used by root teardown, socket shutdown reaches the
/// server as abnormal peer loss. Its ownership guard therefore kills the
/// retained clipboard provider instead of handing it off to the compositor.
fn fail_stop_broker(descriptor: RawFd) -> io::Result<()> {
    // SAFETY: descriptor is this actor's duplicate of the broker shutdown
    // socket. shutdown changes socket state without transferring ownership.
    if unsafe { libc::shutdown(descriptor, libc::SHUT_RDWR) } == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

impl ProcessBrokerHandle {
    fn request(&self, operation: BrokerOperation) -> Result<BrokerOutcome> {
        let (outcome, descriptors) = self.request_with_descriptors(operation, Vec::new())?;
        if !descriptors.is_empty() {
            bail!("broker returned unexpected descriptors");
        }
        Ok(outcome)
    }

    fn request_with_descriptors(
        &self,
        operation: BrokerOperation,
        descriptors: Vec<OwnedFd>,
    ) -> BrokerExchangeResult {
        let timing = broker_request_timing(&operation)?;
        self.request_with_descriptors_and_admission_deadline(
            operation,
            descriptors,
            timing,
            timing.actor_execution_deadline_monotonic_ns,
        )
    }

    #[cfg(test)]
    pub(super) fn abandon_ping_reply_for_test(&self) -> Result<()> {
        let operation = BrokerOperation::Ping;
        let timing = broker_request_timing(&operation)?;
        let (reply, response) = sync_channel(1);
        drop(response);
        self.commands
            .send(BrokerCommand::Exchange {
                operation,
                descriptors: Vec::new(),
                actor_execution_deadline_monotonic_ns: timing.actor_execution_deadline_monotonic_ns,
                admission_deadline_monotonic_ns: timing.actor_execution_deadline_monotonic_ns,
                reply,
            })
            .map_err(|_| anyhow!("process broker owner is no longer running"))
    }

    fn request_with_descriptors_and_admission_deadline(
        &self,
        operation: BrokerOperation,
        descriptors: Vec<OwnedFd>,
        timing: BrokerRequestTiming,
        admission_deadline_monotonic_ns: u64,
    ) -> BrokerExchangeResult {
        let (reply, response) = sync_channel(1);
        self.commands
            .send(BrokerCommand::Exchange {
                operation,
                descriptors,
                actor_execution_deadline_monotonic_ns: timing.actor_execution_deadline_monotonic_ns,
                admission_deadline_monotonic_ns,
                reply,
            })
            .map_err(|_| anyhow!("process broker owner is no longer running"))?;
        receive_broker_response(response, timing.response_wait)
    }

    #[cfg(test)]
    pub(super) fn request_with_admission_deadline_for_test(
        &self,
        operation: BrokerOperation,
        admission_deadline_monotonic_ns: u64,
    ) -> Result<BrokerOutcome> {
        let timing = broker_request_timing(&operation)?;
        let (outcome, descriptors) = self.request_with_descriptors_and_admission_deadline(
            operation,
            Vec::new(),
            timing,
            admission_deadline_monotonic_ns,
        )?;
        if !descriptors.is_empty() {
            bail!("broker returned unexpected test descriptors");
        }
        Ok(outcome)
    }

    pub(crate) fn run<I, S>(
        &self,
        kind: HelperKind,
        program: &OsStr,
        arguments: I,
        input: Vec<u8>,
        timeout: Duration,
        output_cap: usize,
    ) -> Result<BrokerOutput>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        self.run_with_mode(
            kind,
            program,
            arguments,
            input,
            RunOptions {
                timeout,
                output_cap,
                output_mode: OutputMode::Complete,
            },
        )
    }

    pub(crate) fn read_regular_file(
        &self,
        path: &std::path::Path,
        byte_limit: usize,
        timeout: Duration,
    ) -> Result<BrokerFileRead> {
        if !path.is_absolute() {
            bail!("broker file read requires an absolute path");
        }
        if byte_limit == 0 || byte_limit > MAX_OUTPUT_BYTES {
            bail!("broker file read byte limit is outside transport bounds");
        }
        let (outcome, descriptors) = self.request_with_descriptors(
            BrokerOperation::ReadFile {
                path: OsWire::from_os(path.as_os_str())?,
                timeout_ms: u64::try_from(timeout.as_millis()).map_or(u64::MAX, |value| value),
                byte_limit,
            },
            Vec::new(),
        )?;
        let BrokerOutcome::FileRead { result, bytes } = outcome else {
            bail!("broker returned the wrong response kind for file read");
        };
        let mut descriptors = VecDeque::from(descriptors);
        let bytes = decode_blob(bytes, &mut descriptors, byte_limit)?;
        if !descriptors.is_empty() {
            bail!("broker returned unused file-read descriptors");
        }
        match result {
            BrokerFileReadWire::Ready if bytes.is_empty() => {
                bail!("broker returned an empty ready file-read payload")
            }
            BrokerFileReadWire::Ready => Ok(BrokerFileRead::Ready(bytes)),
            BrokerFileReadWire::Empty if bytes.is_empty() => Ok(BrokerFileRead::Empty),
            BrokerFileReadWire::TooLarge if bytes.is_empty() => {
                Ok(BrokerFileRead::TooLarge { limit: byte_limit })
            }
            BrokerFileReadWire::NotRegular if bytes.is_empty() => Ok(BrokerFileRead::NotRegular),
            BrokerFileReadWire::TimedOut if bytes.is_empty() => Ok(BrokerFileRead::TimedOut),
            BrokerFileReadWire::ReadFailed { reason } if bytes.is_empty() => {
                Ok(BrokerFileRead::ReadFailed { reason })
            }
            _ => bail!("broker returned file bytes for a non-ready outcome"),
        }
    }

    /// Reads a bounded stdout prefix. The broker restricts this mode to `wl-paste`.
    pub(crate) fn run_prefix<I, S>(
        &self,
        kind: HelperKind,
        program: &OsStr,
        arguments: I,
        input: Vec<u8>,
        timeout: Duration,
        output_cap: usize,
    ) -> Result<BrokerOutput>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        self.run_with_mode(
            kind,
            program,
            arguments,
            input,
            RunOptions {
                timeout,
                output_cap,
                output_mode: OutputMode::Prefix,
            },
        )
    }

    fn run_with_mode<I, S>(
        &self,
        kind: HelperKind,
        program: &OsStr,
        arguments: I,
        input: Vec<u8>,
        options: RunOptions,
    ) -> Result<BrokerOutput>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let arguments = arguments
            .into_iter()
            .map(|argument| OsWire::from_os(argument.as_ref()))
            .collect::<Result<Vec<_>>>()?;
        let (input, input_descriptor) = encode_blob(input, MAX_INPUT_BYTES)?;
        let descriptors = input_descriptor.into_iter().collect();
        let (outcome, descriptors) = self.request_with_descriptors(
            BrokerOperation::Run {
                kind,
                program: OsWire::from_os(program)?,
                arguments,
                environment: Vec::new(),
                input,
                timeout_ms: u64::try_from(options.timeout.as_millis()).unwrap_or(u64::MAX),
                output_cap: options.output_cap.min(MAX_OUTPUT_BYTES),
                output_mode: options.output_mode,
            },
            descriptors,
        )?;
        match outcome {
            BrokerOutcome::Output {
                status,
                stdout,
                stderr,
                timed_out,
                stdout_limit_reached,
            } => {
                let mut descriptors = VecDeque::from(descriptors);
                let stdout = decode_blob(stdout, &mut descriptors, MAX_OUTPUT_BYTES)?;
                let stderr = decode_blob(stderr, &mut descriptors, MAX_OUTPUT_BYTES)?;
                if !descriptors.is_empty() {
                    bail!("broker returned unused output descriptors");
                }
                Ok(BrokerOutput {
                    status,
                    stdout,
                    stderr,
                    timed_out,
                    stdout_limit_reached,
                })
            }
            _ => bail!("broker returned the wrong response kind for run"),
        }
    }

    pub(crate) fn publish<I, S>(
        &self,
        kind: HelperKind,
        program: &OsStr,
        arguments: I,
        input: Vec<u8>,
        timeout: Duration,
    ) -> Result<BrokerOutput>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        if !supports_retained_publication(kind) {
            bail!("only wl-copy supports retained broker publication");
        }
        let (input, input_descriptor) = encode_blob(input, super::manifest::input_cap(kind))?;
        let (outcome, descriptors) = self.request_with_descriptors(
            BrokerOperation::Publish {
                kind,
                program: OsWire::from_os(program)?,
                arguments: arguments
                    .into_iter()
                    .map(|argument| OsWire::from_os(argument.as_ref()))
                    .collect::<Result<Vec<_>>>()?,
                environment: Vec::new(),
                input,
                timeout_ms: u64::try_from(timeout.as_millis()).unwrap_or(u64::MAX),
            },
            input_descriptor.into_iter().collect(),
        )?;
        if !descriptors.is_empty() {
            bail!("broker returned unexpected publication descriptors");
        }
        match outcome {
            BrokerOutcome::Output {
                status,
                stdout: BlobWire::Inline { bytes: stdout },
                stderr: BlobWire::Inline { bytes: stderr },
                timed_out,
                stdout_limit_reached: false,
            } => Ok(BrokerOutput {
                status,
                stdout,
                stderr,
                timed_out,
                stdout_limit_reached: false,
            }),
            _ => bail!("broker returned the wrong response kind for publication"),
        }
    }

    #[cfg(test)]
    pub(crate) fn publication_wait_bound(timeout: Duration) -> Duration {
        broker_response_wait_for_exchange(
            timeout
                .min(Duration::from_secs(120))
                .saturating_add(BROKER_EXCHANGE_GRACE),
        )
    }

    pub(crate) fn spawn<I, S>(
        &self,
        kind: HelperKind,
        lifetime: HelperLifetime,
        program: &OsStr,
        arguments: I,
        environment: Vec<(OsString, Option<OsString>)>,
    ) -> Result<BrokerChild>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        self.spawn_inner(kind, lifetime, program, arguments, environment, None)
    }

    pub(crate) fn spawn_with_watchdog<I, S>(
        &self,
        kind: HelperKind,
        lifetime: HelperLifetime,
        program: &OsStr,
        arguments: I,
        environment: Vec<(OsString, Option<OsString>)>,
        watchdog: RawFd,
    ) -> Result<BrokerChild>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        self.spawn_inner(
            kind,
            lifetime,
            program,
            arguments,
            environment,
            Some(duplicate_descriptor(watchdog)?),
        )
    }

    fn spawn_inner<I, S>(
        &self,
        kind: HelperKind,
        lifetime: HelperLifetime,
        program: &OsStr,
        arguments: I,
        environment: Vec<(OsString, Option<OsString>)>,
        watchdog: Option<OwnedFd>,
    ) -> Result<BrokerChild>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let operation = BrokerOperation::Spawn {
            kind,
            lifetime,
            watchdog: watchdog.is_some(),
            program: OsWire::from_os(program)?,
            arguments: arguments
                .into_iter()
                .map(|argument| OsWire::from_os(argument.as_ref()))
                .collect::<Result<Vec<_>>>()?,
            environment: environment
                .into_iter()
                .map(|(name, value)| {
                    Ok((
                        OsWire::from_os(&name)?,
                        value.as_deref().map(OsWire::from_os).transpose()?,
                    ))
                })
                .collect::<Result<Vec<_>>>()?,
        };
        let (outcome, descriptors) =
            self.request_with_descriptors(operation, watchdog.into_iter().collect())?;
        if !descriptors.is_empty() {
            bail!("broker returned unexpected spawn descriptors");
        }
        match outcome {
            BrokerOutcome::Spawned { handle, pid } => Ok(BrokerChild {
                broker: self.clone(),
                handle,
                pid,
            }),
            _ => bail!("broker returned the wrong response kind for spawn"),
        }
    }
}

fn duplicate_descriptor(descriptor: RawFd) -> Result<OwnedFd> {
    // SAFETY: F_DUPFD_CLOEXEC duplicates the borrowed live descriptor so the
    // actor command owns its transfer lifetime independently of the caller.
    let duplicate = unsafe { libc::fcntl(descriptor, libc::F_DUPFD_CLOEXEC, 5) };
    if duplicate < 0 {
        return Err(io::Error::last_os_error()).context("failed to duplicate broker descriptor");
    }
    // SAFETY: fcntl returned a new owned descriptor.
    Ok(unsafe { OwnedFd::from_raw_fd(duplicate) })
}

fn broker_exchange_timeout(operation: &BrokerOperation) -> Duration {
    match operation {
        BrokerOperation::ReadFile { timeout_ms, .. }
        | BrokerOperation::Run { timeout_ms, .. }
        | BrokerOperation::Publish { timeout_ms, .. } => Duration::from_millis(*timeout_ms)
            .min(Duration::from_secs(120))
            .saturating_add(BROKER_EXCHANGE_GRACE),
        BrokerOperation::Spawn { .. } => Duration::from_secs(10),
        BrokerOperation::Signal { .. }
        | BrokerOperation::TryWait { .. }
        | BrokerOperation::KillWait { .. } => Duration::from_secs(5),
        BrokerOperation::Ping => Duration::from_secs(2),
    }
}

const BROKER_EXCHANGE_GRACE: Duration = Duration::from_secs(5);
const BROKER_COMMAND_QUEUE_GRACE: Duration = Duration::from_secs(5);
const BROKER_RESPONSE_DELIVERY_GRACE: Duration = Duration::from_secs(1);

#[derive(Clone, Copy)]
struct BrokerRequestTiming {
    actor_execution_deadline_monotonic_ns: u64,
    response_wait: Duration,
}

fn broker_request_timing(operation: &BrokerOperation) -> Result<BrokerRequestTiming> {
    let exchange_timeout = broker_exchange_timeout(operation);
    let execution_budget = exchange_timeout.saturating_add(BROKER_COMMAND_QUEUE_GRACE);
    Ok(BrokerRequestTiming {
        actor_execution_deadline_monotonic_ns: super::wire::admission_deadline_after(
            execution_budget,
        )?,
        response_wait: broker_response_wait_for_exchange(exchange_timeout),
    })
}

fn broker_response_wait_for_exchange(exchange_timeout: Duration) -> Duration {
    exchange_timeout
        .saturating_add(BROKER_COMMAND_QUEUE_GRACE)
        .saturating_add(BROKER_RESPONSE_DELIVERY_GRACE)
}

fn receive_broker_response(
    response: Receiver<BrokerExchangeResult>,
    wait: Duration,
) -> BrokerExchangeResult {
    match response.recv_timeout(wait) {
        Ok(result) => result,
        Err(RecvTimeoutError::Timeout) => {
            bail!("process broker owner response deadline expired")
        }
        Err(RecvTimeoutError::Disconnected) => {
            bail!("process broker owner stopped before replying")
        }
    }
}

fn broker_exchange_timeout_before(
    operation: &BrokerOperation,
    admission_deadline_monotonic_ns: u64,
    now_monotonic_ns: u64,
) -> Result<Duration> {
    let exchange_timeout = broker_exchange_timeout(operation);
    let remaining_ns = admission_deadline_monotonic_ns.saturating_sub(now_monotonic_ns);
    if u128::from(remaining_ns) < exchange_timeout.as_nanos() {
        bail!("process broker request expired before execution");
    }
    Ok(exchange_timeout)
}

impl BrokerChild {
    pub(crate) fn id(&self) -> u32 {
        self.pid
    }

    pub(crate) fn signal(&self, signal: i32) -> Result<()> {
        match self.broker.request(BrokerOperation::Signal {
            handle: self.handle.clone(),
            signal,
        })? {
            BrokerOutcome::Acknowledged => Ok(()),
            _ => bail!("broker returned the wrong response kind for signal"),
        }
    }

    pub(crate) fn try_wait(&self) -> Result<Option<i32>> {
        match self.broker.request(BrokerOperation::TryWait {
            handle: self.handle.clone(),
        })? {
            BrokerOutcome::Running => Ok(None),
            BrokerOutcome::Exited { status } => Ok(Some(status)),
            _ => bail!("broker returned the wrong response kind for try-wait"),
        }
    }

    pub(crate) fn kill_wait(&self) -> Result<i32> {
        match self.broker.request(BrokerOperation::KillWait {
            handle: self.handle.clone(),
        })? {
            BrokerOutcome::Exited { status } => Ok(status),
            _ => bail!("broker returned the wrong response kind for kill-wait"),
        }
    }
}

#[cfg(test)]
mod timing_tests {
    use super::*;

    #[test]
    fn reply_wait_and_publication_settlement_are_explicitly_bounded() {
        assert_eq!(
            ProcessBrokerHandle::publication_wait_bound(Duration::from_secs(5)),
            Duration::from_secs(16)
        );
    }

    #[test]
    fn reply_wait_timeout_is_enforced_by_the_channel_boundary() {
        let (_reply, response) = sync_channel(1);

        let error = receive_broker_response(response, Duration::ZERO)
            .expect_err("fixture keeps the reply sender alive without sending a result");

        assert!(error.to_string().contains("response deadline expired"));
    }

    #[test]
    fn actor_rejects_a_queued_request_without_enough_exchange_time() {
        let now = 10_000_000_000;
        let deadline = now + 1_000_000_000;

        let error = broker_exchange_timeout_before(&BrokerOperation::Ping, deadline, now)
            .expect_err("fixture leaves less than the ping exchange bound");

        assert!(error.to_string().contains("expired before execution"));
    }

    #[test]
    fn actor_accepts_work_only_when_the_full_exchange_fits() {
        let now = 10_000_000_000;
        let deadline = now + 2_000_000_000;

        assert_eq!(
            broker_exchange_timeout_before(&BrokerOperation::Ping, deadline, now)
                .expect("fixture provides the complete ping exchange bound"),
            Duration::from_secs(2)
        );
    }
}
