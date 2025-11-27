//! Screenshot capture functionality for wayscriber.
//!
//! This module provides screenshot capture capabilities including:
//! - Full screen capture
//! - Active window capture
//! - Selection-based capture
//! - Clipboard integration
//! - File saving with configurable formats

pub mod clipboard;
pub mod file;
#[cfg(feature = "portal")]
pub mod portal;
pub mod sources;
pub mod types;

mod dependencies;
mod manager;
mod pipeline;
#[cfg(test)]
mod tests;

pub use manager::CaptureManager;
#[allow(unused_imports)]
pub use types::{
    CaptureDestination, CaptureError, CaptureOutcome, CaptureResult, CaptureStatus, CaptureType,
};
