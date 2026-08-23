//! Idle-client negotiation and admission scheduling.

use std::os::fd::AsRawFd;
use std::sync::mpsc;
use std::thread;
use std::time::Instant;

use anyhow::Result;

use super::PinHost;
use super::event_loop::{ActiveDecode, IDLE_CLIENT_GRACE, Runtime};
use crate::pin::limits::MAX_PINS;
use crate::pin::transport::{PendingCreate, PinConnection, ReceivedPacket};
use crate::pin::{PinCreateError, PinCreateResponse, PinId, PinMemoryCharge, PinRefusal, image};

const MAX_IDLE_CLIENTS: usize = 16;

impl Runtime {
    pub(super) fn begin_decode(
        &mut self,
        connection: PinConnection,
        pending: PendingCreate,
        pin_id: PinId,
        host: &mut PinHost,
    ) -> Result<()> {
        let started_at = Instant::now();
        let request_id = pending.wire.request_id;
        log::debug!("Pin request {request_id} received; decode starting");
        let image_charge = match usize::try_from(pending.wire.png_length)
            .map_err(|_| PinRefusal::LimitExceeded)
            .and_then(|length| {
                PinMemoryCharge::for_image(length, pending.wire.width, pending.wire.height)
            }) {
            Ok(charge) => charge,
            Err(reason) => {
                Self::send_best_effort(
                    &connection,
                    PinCreateResponse::Refused { request_id, reason },
                );
                host.finish_create_transaction();
                return Ok(());
            }
        };
        if let Err(reason) = host.memory.try_reserve(image_charge) {
            Self::send_best_effort(
                &connection,
                PinCreateResponse::Refused { request_id, reason },
            );
            host.finish_create_transaction();
            return Ok(());
        }
        let (sender, receiver) = mpsc::sync_channel(1);
        let worker = match thread::Builder::new()
            .name("wayscriber-pin-decode".to_string())
            .spawn(move || {
                let result = pending
                    .read_png()
                    .map_err(|error| PinCreateError::Transport(error.to_string()))
                    .and_then(|(wire, bytes)| {
                        image::decode_png_bytes(bytes, wire.width, wire.height)
                            .map(|image| (wire, image))
                    });
                let _ = sender.send(result);
            }) {
            Ok(worker) => worker,
            Err(error) => {
                host.memory
                    .release(image_charge)
                    .map_err(|reason| anyhow::anyhow!(reason))?;
                Self::send_best_effort(
                    &connection,
                    PinCreateResponse::Failed {
                        request_id,
                        message: format!("failed to start pin decoder: {error}"),
                    },
                );
                host.finish_create_transaction();
                return Ok(());
            }
        };
        self.decoder = Some(ActiveDecode {
            pin_id,
            request_id,
            connection,
            receiver,
            worker,
            disconnected: false,
            image_charge,
            started_at,
        });
        Ok(())
    }

    pub(super) fn accept_clients(&mut self) -> Result<()> {
        while let Some(connection) = self.listener.accept()? {
            if self.clients.len() >= MAX_IDLE_CLIENTS {
                log::warn!("Dropping excess idle pin client");
                continue;
            }
            connection.set_nonblocking(true)?;
            self.idle_deadlines
                .insert(connection.as_raw_fd(), Instant::now() + IDLE_CLIENT_GRACE);
            self.clients.push(connection);
        }
        Ok(())
    }

    pub(super) fn process_client(&mut self, fd: i32, host: &mut PinHost) -> Result<()> {
        let Some(index) = self
            .clients
            .iter()
            .position(|client| client.as_raw_fd() == fd)
        else {
            return Ok(());
        };
        let packet = match self.clients[index].receive() {
            Ok(packet) => packet,
            Err(error) => {
                log::debug!("Dropping invalid or disconnected pin client: {error:#}");
                self.idle_deadlines.remove(&fd);
                self.clients.swap_remove(index);
                self.arm_after_terminal_idle_client(host);
                return Ok(());
            }
        };
        match packet {
            ReceivedPacket::Hello { version } => {
                if !matches!(self.clients[index].send_hello(version), Ok(true)) {
                    self.idle_deadlines.remove(&fd);
                    self.clients.swap_remove(index);
                    self.arm_after_terminal_idle_client(host);
                }
            }
            ReceivedPacket::Create(pending) => {
                self.idle_deadlines.remove(&fd);
                let connection = self.clients.swap_remove(index);
                if self.decoder.is_some() {
                    Self::send_best_effort(
                        &connection,
                        PinCreateResponse::Failed {
                            request_id: pending.wire.request_id,
                            message: "the pin host is already decoding another image".to_string(),
                        },
                    );
                    host.finish_create_transaction();
                } else if host.pins.len() >= MAX_PINS {
                    Self::send_best_effort(
                        &connection,
                        PinCreateResponse::Refused {
                            request_id: pending.wire.request_id,
                            reason: PinRefusal::TooManyPins,
                        },
                    );
                    host.finish_create_transaction();
                } else {
                    let pin_id = match self.ids.allocate() {
                        Ok(id) => id,
                        Err(reason) => {
                            Self::send_best_effort(
                                &connection,
                                PinCreateResponse::Refused {
                                    request_id: pending.wire.request_id,
                                    reason,
                                },
                            );
                            host.finish_create_transaction();
                            return Ok(());
                        }
                    };
                    self.begin_decode(connection, pending, pin_id, host)?;
                }
            }
        }
        Ok(())
    }

    pub(super) fn expire_idle_clients(&mut self, host: &mut PinHost) {
        let now = Instant::now();
        let expired: Vec<_> = self
            .idle_deadlines
            .iter()
            .filter_map(|(fd, deadline)| (now >= *deadline).then_some(*fd))
            .collect();
        for fd in &expired {
            self.idle_deadlines.remove(fd);
        }
        if !expired.is_empty() {
            self.clients
                .retain(|connection| !expired.contains(&connection.as_raw_fd()));
        }
        let grace_expired = now >= self.startup_deadline || !expired.is_empty();
        if idle_shutdown_eligible(
            host.pins.len(),
            self.decoder.is_some(),
            !self.pending_ready.is_empty(),
            self.clients.len(),
            grace_expired,
        ) {
            host.finish_create_transaction();
        }
    }

    fn arm_after_terminal_idle_client(&mut self, host: &mut PinHost) {
        if idle_shutdown_eligible(
            host.pins.len(),
            self.decoder.is_some(),
            !self.pending_ready.is_empty(),
            self.clients.len(),
            true,
        ) {
            host.finish_create_transaction();
        }
    }
}

fn idle_shutdown_eligible(
    pins: usize,
    decoder_active: bool,
    pending_ready: bool,
    idle_clients: usize,
    grace_expired: bool,
) -> bool {
    grace_expired && pins == 0 && !decoder_active && !pending_ready && idle_clients == 0
}

#[cfg(test)]
mod tests {
    use super::idle_shutdown_eligible;

    #[test]
    fn hello_disconnect_or_malformed_client_arms_empty_host() {
        assert!(idle_shutdown_eligible(0, false, false, 0, true));
    }

    #[test]
    fn idle_hello_arms_only_after_its_bounded_grace() {
        assert!(!idle_shutdown_eligible(0, false, false, 1, false));
        assert!(idle_shutdown_eligible(0, false, false, 0, true));
    }

    #[test]
    fn normal_hello_create_survives_while_decode_or_ready_is_active() {
        assert!(!idle_shutdown_eligible(0, true, false, 0, true));
        assert!(!idle_shutdown_eligible(0, false, true, 0, true));
        assert!(!idle_shutdown_eligible(1, false, false, 0, true));
    }
}
