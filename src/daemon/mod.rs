//! Daemon mode implementation: background service with toggle activation

mod core;
mod global_shortcuts;
mod icons;
mod overlay;
pub(crate) mod setup;
mod tray;
mod types;

#[cfg(test)]
mod tests;

pub use core::Daemon;
pub use types::AlreadyRunningError;
