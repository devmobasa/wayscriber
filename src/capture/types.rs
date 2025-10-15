//! Data types for screenshot capture functionality.

use std::path::PathBuf;
use thiserror::Error;

/// Type of screenshot capture to perform.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureType {
    /// Capture the entire screen/monitor.
    FullScreen,
    /// Capture the currently focused window.
    ActiveWindow,
    /// Capture a user-selected rectangular region.
    #[allow(dead_code)] // Will be used in Phase 2 for region selection
    Selection {
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    },
}

/// Result of a screenshot capture operation.
#[derive(Debug, Clone)]
pub struct CaptureResult {
    /// Raw image data (PNG format).
    #[allow(dead_code)] // Will be used in Phase 2 for annotation compositing
    pub image_data: Vec<u8>,
    /// Path where the image was saved (if saved).
    pub saved_path: Option<PathBuf>,
    /// Whether the image was copied to clipboard.
    #[allow(dead_code)] // Will be used in Phase 2 for status notifications
    pub copied_to_clipboard: bool,
}

/// Outcome of a capture request (success or failure).
#[derive(Debug, Clone)]
pub enum CaptureOutcome {
    Success(CaptureResult),
    Failed(String),
}

/// Errors that can occur during screenshot capture.
#[derive(Debug, Error)]
pub enum CaptureError {
    #[error("xdg-desktop-portal is not available")]
    #[allow(dead_code)] // Will be used in Phase 2 for capability checks
    PortalUnavailable,

    #[error("Screenshot permission denied by user")]
    PermissionDenied,

    #[error("D-Bus communication error: {0}")]
    DBusError(#[from] zbus::Error),

    #[error("Failed to save screenshot: {0}")]
    SaveError(#[from] std::io::Error),

    #[error("Clipboard operation failed: {0}")]
    ClipboardError(String),

    #[error("Image processing error: {0}")]
    ImageError(String),

    #[error("Portal returned invalid response: {0}")]
    InvalidResponse(String),
}

/// Status of an ongoing capture operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CaptureStatus {
    /// Capture is idle/not started.
    Idle,
    /// Waiting for user permission from portal.
    AwaitingPermission,
    /// Capture is in progress.
    #[allow(dead_code)] // Will be used in Phase 2 for progress UI
    InProgress,
    /// Capture completed successfully.
    Success,
    /// Capture failed.
    Failed(String),
}
