use std::collections::VecDeque;
use std::ffi::{OsStr, OsString};
use std::os::fd::{AsRawFd, OwnedFd, RawFd};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock, TryLockError, Weak};
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};

use super::execution::supports_retained_publication;
use super::transport::{
    GRACEFUL_SHUTDOWN_BYTE, decode_blob, encode_blob, recv_packet, send_packet, set_socket_timeout,
};
use super::wire::{
    BlobWire, BrokerOperation, BrokerOutcome, BrokerOutput, BrokerRequest, BrokerResponse,
    HelperKind, HelperLifetime, MAX_INPUT_BYTES, MAX_OUTPUT_BYTES, MAX_PACKET_BYTES, OsWire,
    OutputMode,
};

#[derive(Debug)]
pub(super) struct BrokerInner {
    pub(super) socket: OwnedFd,
    pub(super) shutdown: OwnedFd,
    pub(super) token: String,
    pub(super) child_pid: libc::pid_t,
    pub(super) exchange_lock: Mutex<()>,
    pub(super) healthy: AtomicBool,
    #[cfg(test)]
    pub(super) test_thread: Mutex<Option<std::thread::JoinHandle<()>>>,
}

#[derive(Clone, Debug)]
pub(crate) struct ProcessBroker {
    pub(super) inner: Arc<BrokerInner>,
}

#[derive(Debug)]
pub(crate) struct BrokerChild {
    broker: ProcessBroker,
    handle: String,
    pid: u32,
}

#[derive(Debug)]
pub(crate) struct ProcessBrokerGuard {
    broker: ProcessBroker,
}

#[derive(Debug, Clone, Copy)]
struct RunOptions {
    timeout: Duration,
    output_cap: usize,
    output_mode: OutputMode,
}

/// Failure text used when a caller declines to wait for the transport.
/// Callers match on it to offer "try again" rather than a generic failure.
pub(crate) const BROKER_BUSY: &str = "process broker is busy running another helper";

/// How a caller wants the single-socket transport acquired.
///
/// One socket serializes every exchange, and a `Run` can hold it for as long as
/// its helper lives — thirty seconds for screen text recognition, two minutes
/// for an interactive selector.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExchangeWait {
    /// Queue until the transport is free. Everything on a worker thread, the
    /// daemon, and startup: those requests have to happen, and waiting for them
    /// costs nothing the user can feel.
    Queue,
    /// Take the transport only if it is free right now.
    ///
    /// For requests issued from the Wayland callback thread, where any wait at
    /// all stalls input and redraw dispatch. A contended launch is reported so
    /// the user can retry, which is strictly better than the UI locking up for
    /// the length of somebody else's helper.
    Immediate,
}

/// The transport policy and watchdog a spawn request carries, kept together so
/// the private spawn path stays inside the argument-count lint.
#[derive(Debug, Clone, Copy)]
struct SpawnOptions {
    watchdog: Option<RawFd>,
    wait: ExchangeWait,
}

static ACTIVE_BROKER: OnceLock<Mutex<Weak<BrokerInner>>> = OnceLock::new();

fn active_slot() -> &'static Mutex<Weak<BrokerInner>> {
    ACTIVE_BROKER.get_or_init(|| Mutex::new(Weak::new()))
}

pub(crate) fn start_for_runtime() -> Result<ProcessBrokerGuard> {
    let broker = super::bootstrap::start()?;
    *active_slot()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = Arc::downgrade(&broker.inner);
    Ok(ProcessBrokerGuard { broker })
}

pub(crate) fn current() -> Result<ProcessBroker> {
    let inner = active_slot()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .upgrade()
        .ok_or_else(|| anyhow!("runtime process broker is not active"))?;
    Ok(ProcessBroker { inner })
}

impl Drop for ProcessBrokerGuard {
    fn drop(&mut self) {
        self.broker.inner.healthy.store(false, Ordering::Release);
        if signal_shutdown(self.broker.inner.shutdown.as_raw_fd()).is_err()
            && self.broker.inner.child_pid > 0
        {
            // SAFETY: child_pid is the broker process owned by this guard.
            unsafe {
                libc::kill(self.broker.inner.child_pid, libc::SIGKILL);
            }
        }
        #[cfg(test)]
        if let Some(thread) = self
            .broker
            .inner
            .test_thread
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .take()
        {
            let _ = thread.join();
        } else {
            super::bootstrap::wait_for_broker_process(self.broker.inner.child_pid);
        }
        #[cfg(not(test))]
        super::bootstrap::wait_for_broker_process(self.broker.inner.child_pid);

        let mut slot = active_slot()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if slot
            .upgrade()
            .is_some_and(|active| Arc::ptr_eq(&active, &self.broker.inner))
        {
            *slot = Weak::new();
        }
    }
}

fn signal_shutdown(descriptor: RawFd) -> std::io::Result<()> {
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
        if written < 0 && std::io::Error::last_os_error().kind() == std::io::ErrorKind::Interrupted
        {
            continue;
        }
        return Err(if written < 0 {
            std::io::Error::last_os_error()
        } else {
            std::io::Error::new(std::io::ErrorKind::WriteZero, "short broker shutdown write")
        });
    }
}

#[cfg(test)]
impl ProcessBrokerGuard {
    pub(crate) fn broker(&self) -> &ProcessBroker {
        &self.broker
    }
}

impl ProcessBroker {
    fn lock_exchange(&self, wait: ExchangeWait) -> Result<MutexGuard<'_, ()>> {
        match wait {
            ExchangeWait::Queue => Ok(self
                .inner
                .exchange_lock
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())),
            // One attempt, never a timed retry: sleeping here would be the very
            // stall this policy exists to avoid.
            ExchangeWait::Immediate => match self.inner.exchange_lock.try_lock() {
                Ok(guard) => Ok(guard),
                Err(TryLockError::Poisoned(poisoned)) => Ok(poisoned.into_inner()),
                Err(TryLockError::WouldBlock) => bail!(BROKER_BUSY),
            },
        }
    }

    pub(super) fn request(&self, operation: BrokerOperation) -> Result<BrokerOutcome> {
        let (outcome, descriptors) =
            self.request_with_descriptors(operation, &[], ExchangeWait::Queue)?;
        if !descriptors.is_empty() {
            bail!("broker returned unexpected descriptors");
        }
        Ok(outcome)
    }

    fn request_with_descriptors(
        &self,
        operation: BrokerOperation,
        descriptors: &[RawFd],
        wait: ExchangeWait,
    ) -> Result<(BrokerOutcome, Vec<OwnedFd>)> {
        let request_id = crate::daemon::protocol_v2::ProtocolId::generate()?.to_string();
        let exchange_timeout = broker_exchange_timeout(&operation);
        let packet = serde_json::to_vec(&BrokerRequest {
            token: self.inner.token.clone(),
            request_id: request_id.clone(),
            operation,
        })?;
        if packet.len() > MAX_PACKET_BYTES {
            bail!("broker request exceeds packet cap");
        }
        let _guard = self.lock_exchange(wait)?;
        if !self.inner.healthy.load(Ordering::Acquire) {
            bail!("process broker transport is no longer usable");
        }
        let exchange = (|| -> Result<(BrokerResponse, Vec<OwnedFd>)> {
            set_socket_timeout(self.inner.socket.as_raw_fd(), exchange_timeout)?;
            send_packet(self.inner.socket.as_raw_fd(), &packet, descriptors)?;
            let (packet, descriptors) = recv_packet(self.inner.socket.as_raw_fd())?;
            let response: BrokerResponse = serde_json::from_slice(&packet)?;
            if response.request_id != request_id {
                bail!("broker response identity mismatch");
            }
            Ok((response, descriptors))
        })();
        let (response, descriptors) = match exchange {
            Ok(response) => response,
            Err(error) => {
                self.inner.healthy.store(false, Ordering::Release);
                return Err(error).context("process broker exchange failed");
            }
        };
        if let BrokerOutcome::Error { message } = response.outcome {
            bail!("process broker rejected request: {message}");
        }
        Ok((response.outcome, descriptors))
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
        let request_descriptors = input_descriptor
            .as_ref()
            .map(|descriptor| vec![descriptor.as_raw_fd()])
            .unwrap_or_default();
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
            &request_descriptors,
            ExchangeWait::Queue,
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
        let request_descriptors = input_descriptor
            .as_ref()
            .map(|descriptor| vec![descriptor.as_raw_fd()])
            .unwrap_or_default();
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
            &request_descriptors,
            ExchangeWait::Queue,
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
        self.spawn_inner(
            kind,
            lifetime,
            program,
            arguments,
            environment,
            SpawnOptions {
                watchdog: None,
                wait: ExchangeWait::Queue,
            },
        )
    }

    /// Spawn without waiting for the transport, failing with [`BROKER_BUSY`]
    /// when another exchange holds it.
    ///
    /// For the Wayland callback thread only. Everything else — the daemon's
    /// overlay spawn, the tray, startup detach — must keep queueing: those
    /// requests have to happen, and failing them behind an unrelated helper
    /// would turn a wait into a spurious error and a retry backoff.
    pub(crate) fn try_spawn<I, S>(
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
        self.spawn_inner(
            kind,
            lifetime,
            program,
            arguments,
            environment,
            SpawnOptions {
                watchdog: None,
                wait: ExchangeWait::Immediate,
            },
        )
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
            SpawnOptions {
                watchdog: Some(watchdog),
                wait: ExchangeWait::Queue,
            },
        )
    }

    fn spawn_inner<I, S>(
        &self,
        kind: HelperKind,
        lifetime: HelperLifetime,
        program: &OsStr,
        arguments: I,
        environment: Vec<(OsString, Option<OsString>)>,
        options: SpawnOptions,
    ) -> Result<BrokerChild>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let operation = BrokerOperation::Spawn {
            kind,
            lifetime,
            watchdog: options.watchdog.is_some(),
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
            self.request_with_descriptors(operation, options.watchdog.as_slice(), options.wait)?;
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

fn broker_exchange_timeout(operation: &BrokerOperation) -> Duration {
    match operation {
        BrokerOperation::Run { timeout_ms, .. } | BrokerOperation::Publish { timeout_ms, .. } => {
            Duration::from_millis(*timeout_ms)
                .min(Duration::from_secs(120))
                .saturating_add(Duration::from_secs(5))
        }
        BrokerOperation::Spawn { .. } => Duration::from_secs(10),
        BrokerOperation::Signal { .. }
        | BrokerOperation::TryWait { .. }
        | BrokerOperation::KillWait { .. } => Duration::from_secs(5),
        BrokerOperation::Ping => Duration::from_secs(2),
    }
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
