//! Private process broker client and server shared by the public app and its
//! companion executable. This crate must remain free of graphical dependencies.

#[cfg(all(feature = "test-support", not(debug_assertions)))]
compile_error!("broker test-support is only for debug test builds");

mod bootstrap;
mod client;
mod execution;
mod identity;
mod manifest;
mod server;
mod transport;
mod trusted_url;
mod wire;

#[cfg(test)]
mod tests;

pub use client::{BROKER_BUSY, BrokerChild, ProcessBroker, current, start_for_runtime};
pub use identity::{BootClock, BootDeadline, ProtocolId, ProtocolToken};
pub use trusted_url::is_trusted_url;
pub use wire::{BrokerOutput, HelperKind, HelperLifetime, STDOUT_CAP_EXCEEDED};

pub const CONFIGURATOR_ENV: &str = "WAYSCRIBER_CONFIGURATOR";
pub const DAEMON_WATCHDOG_FD_ENV: &str = "WAYSCRIBER_INTERNAL_DAEMON_WATCHDOG_FD";
pub const BROKER_PROTOCOL_GENERATION: u32 = 1;
pub const BROKER_COHORT: &str = env!("WAYSCRIBER_BROKER_COHORT");

pub const fn max_publish_bytes() -> usize {
    wire::MAX_OUTPUT_BYTES
}

pub fn run_broker_from_env() -> std::process::ExitCode {
    server::run_internal_broker_if_requested().unwrap_or(std::process::ExitCode::from(2))
}
