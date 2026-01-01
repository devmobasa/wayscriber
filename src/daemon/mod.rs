//! Daemon mode implementation: background service with toggle activation

mod core;
mod icons;
mod overlay;
mod tray;
mod types;

#[cfg(all(test, feature = "tray"))]
mod tests;

pub use core::Daemon;
pub use types::AlreadyRunningError;
