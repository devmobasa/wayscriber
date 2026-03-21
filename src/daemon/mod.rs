//! Daemon mode implementation: background service with toggle activation

mod control;
mod core;
mod global_shortcuts;
mod icons;
mod overlay;
pub(crate) mod setup;
mod tray;
mod types;

#[cfg(test)]
mod tests;

pub(crate) use control::{
    DaemonToggleRequest, clear_daemon_pid_file, clear_daemon_toggle_request_file,
    generate_daemon_instance_token, send_daemon_toggle_request, take_daemon_toggle_requests,
    write_daemon_pid_file,
};
pub use core::Daemon;
pub use types::AlreadyRunningError;
