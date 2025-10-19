//! Library exports for reusing hyprmarker subsystems.
//!
//! Exposes configuration data structures alongside the supporting modules they
//! rely on so that external tools (e.g. GUI configurators) can share validation
//! logic and serialization code with the main binary.

pub mod config;
pub mod draw;
pub mod input;
pub mod util;

pub use config::Config;
