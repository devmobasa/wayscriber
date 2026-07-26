//! Pre-lock subprocess broker for daemon and active-overlay runtime helpers.
//!
//! The client owns the authenticated control socket, the execed server owns all
//! runtime child creation/reaping, and the transport enforces bounded packets
//! plus sealed descriptors for larger payloads.

mod bootstrap;
mod client;
mod execution;
mod file_reader;
mod manifest;
mod server;
mod transport;
mod wire;

#[cfg(test)]
mod tests;

#[cfg(test)]
pub(crate) use client::start_for_runtime;
pub(crate) use client::{
    BrokerChild, PreparedProcessBroker, ProcessBrokerHandle, ProcessBrokerOwner,
    prepare_for_runtime,
};
pub(crate) use server::run_internal_broker_if_requested;
pub(crate) use wire::{BrokerFileRead, BrokerOutput, HelperKind, HelperLifetime};
