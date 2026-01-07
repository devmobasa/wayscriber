//! Library exports for reusing wayscriber subsystems.
//!
//! Exposes configuration data structures alongside the supporting modules they
//! rely on so that external tools (e.g. GUI configurators) can share validation
//! logic and serialization code with the main binary.

pub mod capture;
pub mod config;
pub mod draw;
pub mod input;
pub mod paths;
pub mod session;
pub mod time_utils;
pub mod toolbar_icons;
pub mod ui;
pub mod util;

pub use config::Config;
