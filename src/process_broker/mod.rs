//! Main-crate forwarding surface for the private broker package.

#[cfg(not(test))]
pub(crate) use wayscriber_process_broker::{
    BROKER_BUSY, BrokerChild, BrokerOutput, HelperKind, HelperLifetime, ProcessBroker,
    STDOUT_CAP_EXCEEDED, current, max_publish_bytes, start_for_runtime,
};

// The existing test-thread broker tests compile the same source files inside
// the main crate, where their daemon fixture helpers are available. Production
// uses only the separate broker package above.
#[cfg(test)]
#[path = "../../broker/src/bootstrap.rs"]
mod bootstrap;
#[cfg(test)]
#[path = "../../broker/src/client.rs"]
mod client;
#[cfg(test)]
#[path = "../../broker/src/execution.rs"]
mod execution;
#[cfg(test)]
#[path = "../../broker/src/manifest.rs"]
mod manifest;
#[cfg(test)]
#[allow(dead_code)] // The in-process test broker never enters the exec bootstrap.
#[path = "../../broker/src/server.rs"]
mod server;
#[cfg(test)]
mod tests;
#[cfg(test)]
#[path = "../../broker/src/transport.rs"]
mod transport;
#[cfg(test)]
#[allow(dead_code)] // Exec-only descriptor names are unused by the test thread.
#[path = "../../broker/src/wire.rs"]
mod wire;

#[cfg(test)]
pub(crate) use client::{BROKER_BUSY, BrokerChild, ProcessBroker, current, start_for_runtime};
#[cfg(test)]
pub(crate) use wayscriber_process_broker::{BROKER_COHORT, BROKER_PROTOCOL_GENERATION};
#[cfg(test)]
pub(crate) use wire::{BrokerOutput, HelperKind, HelperLifetime, STDOUT_CAP_EXCEEDED};

#[cfg(test)]
pub(crate) const fn max_publish_bytes() -> usize {
    wire::MAX_OUTPUT_BYTES
}
