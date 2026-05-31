//! Data types for screenshot capture functionality.

use std::fmt;
use std::path::PathBuf;

/// User-facing operation kind for image delivery and status labels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageOperationKind {
    Screenshot,
    CanvasExport,
    BoardPdfExport,
    AllBoardsPdfExport,
}

impl ImageOperationKind {
    pub fn success_title(self) -> &'static str {
        match self {
            Self::Screenshot => "Screenshot Captured",
            Self::CanvasExport => "Canvas exported",
            Self::BoardPdfExport => "Board exported",
            Self::AllBoardsPdfExport => "Boards exported",
        }
    }

    pub fn failure_title(self) -> &'static str {
        match self {
            Self::Screenshot => "Screenshot Failed",
            Self::CanvasExport => "Canvas export failed",
            Self::BoardPdfExport => "Board PDF export failed",
            Self::AllBoardsPdfExport => "All boards PDF export failed",
        }
    }

    pub fn clipboard_failure_title(self) -> &'static str {
        match self {
            Self::Screenshot => "Screenshot Clipboard Failed",
            Self::CanvasExport => "Canvas clipboard failed",
            Self::BoardPdfExport => "Board PDF clipboard failed",
            Self::AllBoardsPdfExport => "All boards PDF clipboard failed",
        }
    }

    pub fn fallback_toast(self) -> &'static str {
        match self {
            Self::Screenshot => "Clipboard failed",
            Self::CanvasExport => "Canvas clipboard failed",
            Self::BoardPdfExport => "Board PDF clipboard failed",
            Self::AllBoardsPdfExport => "All boards PDF clipboard failed",
        }
    }

    pub fn saved_log_label(self) -> &'static str {
        match self {
            Self::Screenshot => "Screenshot",
            Self::CanvasExport => "Canvas export",
            Self::BoardPdfExport => "Board PDF export",
            Self::AllBoardsPdfExport => "All boards PDF export",
        }
    }

    pub fn format_error(self, err: &CaptureError) -> String {
        match self {
            Self::Screenshot => err.to_string(),
            Self::CanvasExport => match err {
                CaptureError::SaveError(err) => {
                    format!("Failed to save canvas export: {err}")
                }
                CaptureError::ClipboardError(err) => {
                    format!("Canvas export clipboard operation failed: {err}")
                }
                CaptureError::ImageError(err) => format!("Canvas export failed: {err}"),
                CaptureError::Cancelled(reason) => format!("Canvas export cancelled: {reason}"),
                other => other.to_string(),
            },
            Self::BoardPdfExport => match err {
                CaptureError::SaveError(err) => {
                    format!("Failed to save board PDF export: {err}")
                }
                CaptureError::ClipboardError(err) => {
                    format!("Board PDF export clipboard operation failed: {err}")
                }
                CaptureError::ImageError(err) => format!("Board PDF export failed: {err}"),
                CaptureError::Cancelled(reason) => {
                    format!("Board PDF export cancelled: {reason}")
                }
                other => other.to_string(),
            },
            Self::AllBoardsPdfExport => match err {
                CaptureError::SaveError(err) => {
                    format!("Failed to save all boards PDF export: {err}")
                }
                CaptureError::ClipboardError(err) => {
                    format!("All boards PDF export clipboard operation failed: {err}")
                }
                CaptureError::ImageError(err) => {
                    format!("All boards PDF export failed: {err}")
                }
                CaptureError::Cancelled(reason) => {
                    format!("All boards PDF export cancelled: {reason}")
                }
                other => other.to_string(),
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageFormatMetadata {
    pub extension: String,
    pub mime_type: String,
}

impl ImageFormatMetadata {
    pub fn png() -> Self {
        Self {
            extension: "png".to_string(),
            mime_type: "image/png".to_string(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct RenderedImage {
    pub bytes: Vec<u8>,
    pub format: ImageFormatMetadata,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone)]
pub struct ImageDeliveryRequest {
    pub image: RenderedImage,
    pub destination: CaptureDestination,
    pub save_config: Option<crate::capture::file::FileSaveConfig>,
    pub operation: ImageOperationKind,
    pub fallback_format_override: Option<ImageFormatMetadata>,
}

#[derive(Debug, Clone)]
pub struct RenderedDocument {
    pub bytes: Vec<u8>,
    pub extension: String,
    pub mime_type: String,
}

#[derive(Debug, Clone)]
pub struct DocumentDeliveryRequest {
    pub document: RenderedDocument,
    pub destination: CaptureDestination,
    pub save_config: Option<crate::capture::file::FileSaveConfig>,
    pub operation: ImageOperationKind,
}

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
    pub operation: ImageOperationKind,
    pub fallback_format_override: Option<ImageFormatMetadata>,
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
    Failed {
        operation: ImageOperationKind,
        message: String,
    },
    Cancelled {
        operation: ImageOperationKind,
        reason: String,
    },
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
