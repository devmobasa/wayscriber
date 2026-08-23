//! Socket/Wayland/worker poll loop for the detached pin host.

use std::collections::{BTreeMap, HashMap};
use std::os::fd::{AsFd, AsRawFd};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};

use super::exit_state::{exit_shutdown_eligible, ready_delivery_allowed};
use super::{PinAdmission, PinCharge, PinHost};
use crate::pin::{
    PinCreateError, PinCreateResponse, PinId, PinIdSequence, PinMemoryCharge, geometry,
    protocol::PinCreateWire,
    transport::{HostLock, PinConnection, PinListener, PinRuntimePaths},
};

const HOST_OWNERSHIP_TIMEOUT: Duration = Duration::from_secs(2);
pub(super) const IDLE_CLIENT_GRACE: Duration = Duration::from_secs(3);
const SHUTDOWN_DRAIN_GRACE: Duration = Duration::from_millis(250);
const IDLE_POLL: i32 = 50;

pub(crate) fn run_host() -> Result<()> {
    let paths = PinRuntimePaths::secure_from_env().map_err(|error| anyhow::anyhow!(error))?;
    // Bounded acquisition rejects or serializes any existing host candidate;
    // this ownership guard is retained for the complete host lifetime.
    let lock = HostLock::acquire_for(&paths, HOST_OWNERSHIP_TIMEOUT)?;
    let listener = PinListener::bind(&paths, &lock)?;
    listener.set_nonblocking(true)?;

    let (connection, mut event_queue, mut host) = PinHost::connect()?;
    event_queue
        .roundtrip(&mut host)
        .context("initialize pin host outputs and seats")?;
    let mut runtime = Runtime {
        listener,
        _lock: lock,
        clients: Vec::new(),
        decoder: None,
        pending_ready: BTreeMap::new(),
        ids: PinIdSequence::default(),
        idle_deadlines: HashMap::new(),
        startup_deadline: Instant::now() + IDLE_CLIENT_GRACE,
        shutdown_not_before: None,
    };
    let qh = event_queue.handle();

    loop {
        event_queue.dispatch_pending(&mut host)?;
        runtime.expire_idle_clients(&mut host);
        host.poll_workers();
        runtime.poll_decoder(&mut host, &qh)?;
        host.render_dirty(&qh)?;
        event_queue
            .flush()
            .context("flush pin commits before Ready")?;
        runtime.send_ready(&mut host, true)?;
        if runtime.exit_eligible(&host) {
            runtime.clients.clear();
            break;
        }
        event_queue.flush()?;
        runtime.poll_sources(&connection, &mut event_queue, &mut host, &qh)?;
    }
    Ok(())
}

pub(super) struct Runtime {
    pub(super) listener: PinListener,
    // Retained until after listener/socket teardown; dropping it early permits
    // a second host to replace the rendezvous socket.
    _lock: HostLock,
    pub(super) clients: Vec<PinConnection>,
    pub(super) decoder: Option<ActiveDecode>,
    pub(super) pending_ready: BTreeMap<PinId, PendingReply>,
    pub(super) ids: PinIdSequence,
    pub(super) idle_deadlines: HashMap<i32, Instant>,
    pub(super) startup_deadline: Instant,
    shutdown_not_before: Option<Instant>,
}

pub(super) struct ActiveDecode {
    pub(super) pin_id: PinId,
    pub(super) request_id: crate::pin::PinRequestId,
    pub(super) connection: PinConnection,
    pub(super) receiver:
        mpsc::Receiver<Result<(PinCreateWire, crate::pin::PinImage), PinCreateError>>,
    pub(super) worker: thread::JoinHandle<()>,
    pub(super) disconnected: bool,
    pub(super) image_charge: PinMemoryCharge,
    pub(super) started_at: Instant,
}

pub(super) struct PendingReply {
    request_id: crate::pin::PinRequestId,
    connection: PinConnection,
}

impl Runtime {
    fn exit_eligible(&mut self, host: &PinHost) -> bool {
        if !host.should_exit {
            self.shutdown_not_before = None;
            return false;
        }
        if !exit_shutdown_eligible(
            true,
            self.decoder.is_some(),
            !self.pending_ready.is_empty(),
            self.clients.len(),
            true,
        ) {
            self.shutdown_not_before = None;
            return false;
        }
        let deadline = self
            .shutdown_not_before
            .get_or_insert_with(|| Instant::now() + SHUTDOWN_DRAIN_GRACE);
        exit_shutdown_eligible(true, false, false, 0, Instant::now() >= *deadline)
    }

    fn poll_sources(
        &mut self,
        connection: &wayland_client::Connection,
        event_queue: &mut wayland_client::EventQueue<PinHost>,
        host: &mut PinHost,
        qh: &wayland_client::QueueHandle<PinHost>,
    ) -> Result<()> {
        let guard = event_queue.prepare_read();
        let mut sources = Vec::new();
        sources.push((Source::Wayland, event_queue.as_fd().as_raw_fd()));
        sources.push((Source::Listener, self.listener.as_raw_fd()));
        for client in &self.clients {
            sources.push((Source::Client(client.as_raw_fd()), client.as_raw_fd()));
        }
        if let Some(decoder) = &self.decoder {
            sources.push((Source::DecoderClient, decoder.connection.as_raw_fd()));
        }
        for (id, pending) in &self.pending_ready {
            sources.push((Source::Pending(*id), pending.connection.as_raw_fd()));
        }
        let mut descriptors: Vec<_> = sources
            .iter()
            .map(|(_, fd)| libc::pollfd {
                fd: *fd,
                events: libc::POLLIN | libc::POLLERR | libc::POLLHUP,
                revents: 0,
            })
            .collect();
        // SAFETY: `descriptors` is a live contiguous pollfd array for this call.
        let result = unsafe {
            libc::poll(
                descriptors.as_mut_ptr(),
                libc::nfds_t::try_from(descriptors.len())?,
                IDLE_POLL,
            )
        };
        if result < 0 {
            let error = std::io::Error::last_os_error();
            if error.kind() != std::io::ErrorKind::Interrupted {
                return Err(error).context("poll pin host sources");
            }
        }

        let wayland_ready = descriptors
            .first()
            .is_some_and(|fd| fd.revents & (libc::POLLIN | libc::POLLERR | libc::POLLHUP) != 0);
        if wayland_ready {
            if let Some(guard) = guard {
                guard.read().context("read pin host Wayland events")?;
            }
        } else {
            drop(guard);
        }
        event_queue.dispatch_pending(host)?;

        let ready: Vec<_> = sources
            .into_iter()
            .zip(descriptors)
            .filter_map(|((source, _), descriptor)| {
                (descriptor.revents != 0).then_some((source, descriptor.revents))
            })
            .collect();
        for (source, events) in ready {
            match source {
                Source::Wayland => {}
                Source::Listener => self.accept_clients()?,
                Source::Client(fd) => self.process_client(fd, host)?,
                Source::DecoderClient => {
                    if events & (libc::POLLIN | libc::POLLERR | libc::POLLHUP) != 0
                        && let Some(decoder) = self.decoder.as_mut()
                    {
                        decoder.disconnected = true;
                    }
                }
                Source::Pending(id) => {
                    if events & (libc::POLLIN | libc::POLLERR | libc::POLLHUP) != 0 {
                        self.pending_ready.remove(&id);
                        host.close_pin(id);
                    }
                }
            }
        }
        connection.flush()?;
        let _ = qh;
        Ok(())
    }

    fn poll_decoder(
        &mut self,
        host: &mut PinHost,
        qh: &wayland_client::QueueHandle<PinHost>,
    ) -> Result<()> {
        let completion = match self
            .decoder
            .as_ref()
            .map(|decoder| decoder.receiver.try_recv())
        {
            Some(Ok(completion)) => completion,
            Some(Err(mpsc::TryRecvError::Empty)) | None => return Ok(()),
            Some(Err(mpsc::TryRecvError::Disconnected)) => {
                Err(PinCreateError::Host("pin decoder disconnected".to_string()))
            }
        };
        let decoder = self
            .decoder
            .take()
            .context("decoder existed for its completion")?;
        let decoded_at = Instant::now();
        let _ = decoder.worker.join();
        if decoder.disconnected {
            host.memory
                .release(decoder.image_charge)
                .map_err(|reason| anyhow::anyhow!(reason))?;
            host.finish_create_transaction();
            return Ok(());
        }
        let (wire, image) = match completion {
            Ok(decoded) => decoded,
            Err(error) => {
                let response = match error {
                    PinCreateError::Refused(reason) => PinCreateResponse::Refused {
                        request_id: decoder.request_id,
                        reason,
                    },
                    error => PinCreateResponse::Failed {
                        request_id: decoder.request_id,
                        message: error.to_string(),
                    },
                };
                host.memory
                    .release(decoder.image_charge)
                    .map_err(|reason| anyhow::anyhow!(reason))?;
                Self::send_best_effort(&decoder.connection, response);
                host.finish_create_transaction();
                return Ok(());
            }
        };
        let id = decoder.pin_id;
        let mut frame = match geometry::initial_frame(
            (image.width, image.height),
            wire.placement,
            &wire.output,
        ) {
            Ok(frame) => frame,
            Err(reason) => {
                host.memory
                    .release(decoder.image_charge)
                    .map_err(|reason| anyhow::anyhow!(reason))?;
                Self::send_best_effort(
                    &decoder.connection,
                    PinCreateResponse::Refused {
                        request_id: wire.request_id,
                        reason,
                    },
                );
                host.finish_create_transaction();
                return Ok(());
            }
        };
        frame = match super::output::fit_frame_to_surface_limit(
            frame,
            (image.width, image.height),
            wire.output.logical_size(),
            wire.output.scale,
        ) {
            Ok(frame) => frame,
            Err(reason) => {
                host.memory
                    .release(decoder.image_charge)
                    .map_err(|reason| anyhow::anyhow!(reason))?;
                Self::send_best_effort(
                    &decoder.connection,
                    PinCreateResponse::Refused {
                        request_id: wire.request_id,
                        reason,
                    },
                );
                host.finish_create_transaction();
                return Ok(());
            }
        };
        let (connector, scale, mapped) = if let Some(output) = host
            .outputs
            .by_connector(&wire.output.connector_name)
            .or_else(|| host.outputs.deterministic_first())
        {
            if output.connector_name != wire.output.connector_name
                || super::output::output_snapshot_changed(
                    wire.output.logical_size(),
                    wire.output.scale,
                    wire.output.transform,
                    output.logical_size,
                    output.scale.max(1) as u32,
                    output.transform,
                )
            {
                frame = match super::output::migrate_frame(
                    frame,
                    (image.width, image.height),
                    wire.output.logical_size(),
                    output.logical_size,
                    output.scale.max(1) as u32,
                ) {
                    Ok(frame) => frame,
                    Err(reason) => {
                        host.memory
                            .release(decoder.image_charge)
                            .map_err(|reason| anyhow::anyhow!(reason))?;
                        Self::send_best_effort(
                            &decoder.connection,
                            PinCreateResponse::Refused {
                                request_id: wire.request_id,
                                reason,
                            },
                        );
                        host.finish_create_transaction();
                        return Ok(());
                    }
                };
            }
            (
                output.connector_name.clone(),
                output.scale.max(1) as u32,
                true,
            )
        } else {
            (wire.output.connector_name.clone(), wire.output.scale, false)
        };
        let image_charge = decoder.image_charge;
        let surface_charge = if mapped {
            let surface_size =
                crate::pin::surface::surface_size(frame).context("pin chrome size overflow")?;
            PinMemoryCharge::for_surface(surface_size.0, surface_size.1, scale)
                .map_err(|error| anyhow::anyhow!(error))?
        } else {
            PinMemoryCharge::default()
        };
        let charge = PinCharge::new(image_charge, surface_charge);
        if let Err(reason) = host.memory.try_reserve(surface_charge) {
            host.memory
                .release(image_charge)
                .map_err(|reason| anyhow::anyhow!(reason))?;
            Self::send_best_effort(
                &decoder.connection,
                PinCreateResponse::Refused {
                    request_id: wire.request_id,
                    reason,
                },
            );
            host.finish_create_transaction();
            return Ok(());
        }
        let output_size = host
            .outputs
            .by_connector(&connector)
            .map_or(wire.output.logical_size(), |output| output.logical_size);
        let admission = PinAdmission::new(
            id,
            std::sync::Arc::new(image),
            connector,
            output_size,
            frame,
            charge,
        );
        if let Err(error) = host.insert_pin(admission, qh) {
            host.memory
                .release(surface_charge)
                .map_err(|reason| anyhow::anyhow!(reason))?;
            host.memory
                .release(image_charge)
                .map_err(|reason| anyhow::anyhow!(reason))?;
            Self::send_best_effort(
                &decoder.connection,
                PinCreateResponse::Failed {
                    request_id: wire.request_id,
                    message: error.to_string(),
                },
            );
            host.finish_create_transaction();
            return Ok(());
        }
        host.begin_timing(id, wire.request_id, decoder.started_at, decoded_at);
        log::debug!(
            "Pin {id} admitted: resident={} peak={}",
            host.memory.resident_bytes(),
            host.memory.peak_bytes()
        );
        host.should_exit = false;
        self.pending_ready.insert(
            id,
            PendingReply {
                request_id: wire.request_id,
                connection: decoder.connection,
            },
        );
        Ok(())
    }

    pub(super) fn send_best_effort(connection: &PinConnection, response: PinCreateResponse) {
        if let Err(error) = connection.send_response(response) {
            log::debug!("Pin client disconnected before terminal response: {error:#}");
        }
    }

    fn send_ready(&mut self, host: &mut PinHost, wayland_flushed: bool) -> Result<()> {
        anyhow::ensure!(
            ready_delivery_allowed(wayland_flushed),
            "pin Ready attempted before Wayland commit flush"
        );
        for id in host.take_newly_ready() {
            let Some(pending) = self.pending_ready.remove(&id) else {
                if !host.pins.get(&id).is_some_and(|pin| pin.ready_sent) {
                    host.close_pin(id);
                }
                continue;
            };
            if let Err(error) = pending.connection.send_response(PinCreateResponse::Ready {
                request_id: pending.request_id,
                pin_id: id,
            }) {
                host.close_pin(id);
                log::debug!("Pin client disconnected before Ready: {error:#}");
            } else if let Some(pin) = host.pins.get_mut(&id) {
                pin.ready_sent = true;
                host.finish_timing(id);
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy)]
enum Source {
    Wayland,
    Listener,
    Client(i32),
    DecoderClient,
    Pending(PinId),
}
