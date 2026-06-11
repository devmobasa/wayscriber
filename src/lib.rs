//! Library exports for reusing wayscriber subsystems.
//!
//! Exposes configuration data structures alongside the supporting modules they
//! rely on so that external tools (e.g. GUI configurators) can share validation
//! logic and serialization code with the main binary.

pub(crate) mod base64;
pub mod build_info;
pub mod canvas_export;
pub mod capture;
pub mod config;
pub mod draw;
pub mod env_vars;
#[cfg(feature = "portal")]
pub(crate) mod file_uri;
pub(crate) mod image_decode;
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
pub mod toolbar_icons;
pub mod ui;
pub(crate) mod ui_text;
pub mod util;
#[cfg(feature = "portal")]
pub(crate) mod zbus_stream;

pub use config::Config;
