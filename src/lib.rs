//! Library exports for reusing wayscriber subsystems.
//!
//! Formerly known as **hyprmarker** prior to the v0.5.0 rename.
//!
//! Exposes configuration data structures alongside the supporting modules they
//! rely on so that external tools (e.g. GUI configurators) can share validation
//! logic and serialization code with the main binary.

pub mod config;
pub mod draw;
pub mod input;
pub mod legacy;
pub mod session;
pub mod ui;
pub mod util;

pub use config::Config;
