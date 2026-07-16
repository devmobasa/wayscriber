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
mod desktop_backdrop;
mod manager;
mod pipeline;
#[cfg(test)]
mod tests;

pub use manager::{CaptureManager, CapturePoll, CaptureRequestId, CaptureSubmitError};
#[allow(unused_imports)]
pub(crate) use pipeline::CaptureRequest;
#[allow(unused_imports)]
pub use types::{
    CaptureDestination, CaptureError, CaptureOutcome, CaptureResult, CaptureStatus, CaptureType,
    DesktopBackdropCaptureRequest, DesktopBackdropCaptureResult, DesktopBackdropGeometry,
    DesktopBackdropOutputGeometry, DocumentDeliveryRequest, ImageDeliveryRequest,
    ImageFormatMetadata, ImageOperationKind, RenderedDocument, RenderedImage,
};
