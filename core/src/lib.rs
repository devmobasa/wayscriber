//! Shared wayscriber library modules for tools that do not need the overlay app crate.
//!
//! The modules are included from the main source tree to keep serialization,
//! validation, and utility behavior identical while allowing consumers like the
//! configurator to avoid the overlay-only dependency stack.

#![allow(dead_code, unused_imports)]

pub mod build_info;
pub mod config;
pub mod draw;
pub mod durable_io;
pub mod env_vars;
pub mod input;
mod label_format;
pub mod paths;
pub mod render_profiles;
pub mod runtime_capabilities;
pub mod session;
pub mod shortcut_hint;
pub mod systemd_user_service;
#[cfg(test)]
pub(crate) mod test_env;
#[cfg(test)]
pub(crate) mod test_temp;
pub mod time_utils;
pub mod util;

pub use config::Config;
