//! Pre-lock subprocess broker for daemon and active-overlay runtime helpers.
//!
//! The client owns the authenticated control socket, the execed server owns all
//! runtime child creation/reaping, and the transport enforces bounded packets
//! plus sealed descriptors for larger payloads.

mod bootstrap;
mod client;
mod error;
mod execution;
mod manifest;
mod server;
mod transport;
mod wire;

#[cfg(test)]
mod tests;

pub(crate) use client::{BrokerChild, ProcessBroker, current, start_for_runtime};
pub(crate) use error::{BrokerErrorKind, error_kind};
pub(crate) use server::run_internal_broker_if_requested;
pub(crate) use wire::{BrokerOutput, HelperKind, HelperLifetime, STDOUT_CAP_EXCEEDED};

/// Largest payload accepted by retained clipboard publication.
pub(crate) const fn max_publish_bytes() -> usize {
    wire::MAX_OUTPUT_BYTES
}
