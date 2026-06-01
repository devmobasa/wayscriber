//! Data types for screenshot capture functionality.

use std::path::PathBuf;
use std::{fmt, sync::Arc};

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DesktopBackdropGeometry {
    pub logical_x: i32,
    pub logical_y: i32,
    pub logical_width: u32,
    pub logical_height: u32,
    pub scale: i32,
    pub physical_width: Option<u32>,
    pub physical_height: Option<u32>,
    pub crop_x: Option<u32>,
    pub crop_y: Option<u32>,
}

impl DesktopBackdropGeometry {
    pub fn from_outputs(
        active: DesktopBackdropOutputGeometry,
        outputs: &[DesktopBackdropOutputGeometry],
        scale: i32,
    ) -> Option<Self> {
        Some(Self {
            logical_x: active.logical_x,
            logical_y: active.logical_y,
            logical_width: active.logical_width,
            logical_height: active.logical_height,
            scale,
            physical_width: Some(active.physical_width),
            physical_height: Some(active.physical_height),
            crop_x: Some(physical_axis_origin(
                active.logical_x,
                Axis::Horizontal,
                outputs,
            )?),
            crop_y: Some(physical_axis_origin(
                active.logical_y,
                Axis::Vertical,
                outputs,
            )?),
        })
    }

    pub fn physical_size(self) -> Option<(u32, u32)> {
        if let (Some(width), Some(height)) = (self.physical_width, self.physical_height)
            && width > 0
            && height > 0
        {
            return Some((width, height));
        }

        let scale = u32::try_from(self.scale).ok()?;
        Some((
            self.logical_width.checked_mul(scale)?,
            self.logical_height.checked_mul(scale)?,
        ))
    }

    pub fn physical_origin(self) -> Option<(u32, u32)> {
        Some((self.crop_x?, self.crop_y?))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DesktopBackdropOutputGeometry {
    pub logical_x: i32,
    pub logical_y: i32,
    pub logical_width: u32,
    pub logical_height: u32,
    pub physical_width: u32,
    pub physical_height: u32,
}

#[derive(Debug, Clone, Copy)]
enum Axis {
    Horizontal,
    Vertical,
}

fn physical_axis_origin(
    target_start: i32,
    axis: Axis,
    outputs: &[DesktopBackdropOutputGeometry],
) -> Option<u32> {
    if outputs.is_empty() {
        return None;
    }

    let target_start = i64::from(target_start);
    let spans = outputs
        .iter()
        .map(|output| axis_span(*output, axis))
        .collect::<Option<Vec<_>>>()?;
    let min_start = spans.iter().map(|span| span.logical_start).min()?;
    if target_start < min_start {
        return None;
    }
    if target_start == min_start {
        return Some(0);
    }

    let mut boundaries = Vec::with_capacity(spans.len() * 2 + 2);
    boundaries.push(min_start);
    boundaries.push(target_start);
    for span in &spans {
        if span.logical_start > min_start && span.logical_start < target_start {
            boundaries.push(span.logical_start);
        }
        if span.logical_end > min_start && span.logical_end < target_start {
            boundaries.push(span.logical_end);
        }
    }
    boundaries.sort_unstable();
    boundaries.dedup();

    let mut physical_origin = 0.0f64;
    for pair in boundaries.windows(2) {
        let start = pair[0];
        let end = pair[1];
        if end <= start {
            continue;
        }
        let segment = spans
            .iter()
            .filter(|span| span.logical_start <= start && span.logical_end >= end)
            .map(|span| {
                (end - start) as f64 * span.physical_length as f64 / span.logical_length() as f64
            })
            .reduce(f64::max)?;
        physical_origin += segment;
    }

    if !physical_origin.is_finite() || physical_origin < 0.0 || physical_origin > u32::MAX as f64 {
        return None;
    }
    Some(physical_origin.round() as u32)
}

#[derive(Debug, Clone, Copy)]
struct AxisSpan {
    logical_start: i64,
    logical_end: i64,
    physical_length: u32,
}

impl AxisSpan {
    fn logical_length(self) -> i64 {
        self.logical_end - self.logical_start
    }
}

fn axis_span(output: DesktopBackdropOutputGeometry, axis: Axis) -> Option<AxisSpan> {
    let (logical_start, logical_size, physical_length) = match axis {
        Axis::Horizontal => (
            output.logical_x,
            output.logical_width,
            output.physical_width,
        ),
        Axis::Vertical => (
            output.logical_y,
            output.logical_height,
            output.physical_height,
        ),
    };
    if logical_size == 0 || physical_length == 0 {
        return None;
    }

    let logical_start = i64::from(logical_start);
    let logical_end = logical_start.checked_add(i64::from(logical_size))?;
    if logical_end <= logical_start {
        return None;
    }

    Some(AxisSpan {
        logical_start,
        logical_end,
        physical_length,
    })
}

#[derive(Debug, Clone)]
pub struct DesktopBackdropCaptureRequest {
    pub logical_width: u32,
    pub logical_height: u32,
    pub scale: i32,
    pub geometry: Option<DesktopBackdropGeometry>,
    pub operation: ImageOperationKind,
}

#[derive(Debug, Clone)]
pub struct DesktopBackdropCaptureResult {
    pub data: Arc<[u8]>,
    pub width: i32,
    pub height: i32,
    pub stride: i32,
    pub logical_to_image_scale_x: f64,
    pub logical_to_image_scale_y: f64,
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
    DesktopBackdropSuccess(DesktopBackdropCaptureResult),
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
