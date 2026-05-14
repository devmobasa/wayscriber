//! Data types for screenshot capture functionality.

use std::fmt;
use std::path::PathBuf;

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
    Cancelled(String),
}

/// Where the captured image should be delivered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureDestination {
    #[allow(dead_code)] // Will be used by upcoming clipboard-only actions
    ClipboardOnly,
    FileOnly,
    ClipboardAndFile,
}

/// Errors that can occur during screenshot capture.
#[derive(Debug)]
pub enum CaptureError {
    #[allow(dead_code)] // Will be used in Phase 2 for capability checks
    PortalUnavailable,

    #[cfg_attr(not(feature = "portal"), allow(dead_code))]
    PermissionDenied,

    #[cfg(feature = "dbus")]
    DBusError(zbus::Error),

    SaveError(std::io::Error),

    ClipboardError(String),

    ImageError(String),

    InvalidResponse(String),

    Cancelled(String),
}

impl fmt::Display for CaptureError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PortalUnavailable => write!(f, "xdg-desktop-portal is not available"),
            Self::PermissionDenied => write!(f, "Screenshot permission denied by user"),
            #[cfg(feature = "dbus")]
            Self::DBusError(err) => write!(f, "D-Bus communication error: {err}"),
            Self::SaveError(err) => write!(f, "Failed to save screenshot: {err}"),
            Self::ClipboardError(err) => write!(f, "Clipboard operation failed: {err}"),
            Self::ImageError(err) => write!(f, "Image processing error: {err}"),
            Self::InvalidResponse(err) => write!(f, "Portal returned invalid response: {err}"),
            Self::Cancelled(reason) => write!(f, "Capture cancelled: {reason}"),
        }
    }
}

impl std::error::Error for CaptureError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            #[cfg(feature = "dbus")]
            Self::DBusError(err) => Some(err),
            Self::SaveError(err) => Some(err),
            _ => None,
        }
    }
}

#[cfg(feature = "dbus")]
impl From<zbus::Error> for CaptureError {
    fn from(value: zbus::Error) -> Self {
        Self::DBusError(value)
    }
}

impl From<std::io::Error> for CaptureError {
    fn from(value: std::io::Error) -> Self {
        Self::SaveError(value)
    }
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
    /// Capture was cancelled by the user.
    Cancelled(String),
}
