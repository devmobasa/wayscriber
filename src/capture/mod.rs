//! Screenshot capture functionality for hyprmarker.
//!
//! This module provides screenshot capture capabilities including:
//! - Full screen capture
//! - Active window capture
//! - Selection-based capture
//! - Clipboard integration
//! - File saving with configurable formats

pub mod clipboard;
pub mod file;
pub mod portal;
pub mod types;

pub use types::{CaptureError, CaptureResult, CaptureStatus, CaptureType};

use file::{FileSaveConfig, save_screenshot};
use std::sync::Arc;
use tokio::sync::{Mutex, mpsc};

/// Shared state for managing async capture operations.
///
/// This structure bridges the async portal world with the sync Wayland event loop.
#[derive(Clone)]
pub struct CaptureManager {
    /// Channel for sending capture requests.
    request_tx: mpsc::UnboundedSender<CaptureRequest>,
    /// Shared status of the current capture operation.
    status: Arc<Mutex<CaptureStatus>>,
    /// Shared result of the last capture (if any).
    #[allow(dead_code)] // Will be used in Phase 2 for status UI
    last_result: Arc<Mutex<Option<CaptureResult>>>,
}

/// A request to perform a capture operation.
struct CaptureRequest {
    capture_type: CaptureType,
    save_config: FileSaveConfig,
    copy_to_clipboard: bool,
}

impl CaptureManager {
    /// Create a new capture manager.
    ///
    /// This spawns a background task that handles async portal operations.
    ///
    /// # Arguments
    /// * `runtime_handle` - Tokio runtime handle for spawning async tasks
    pub fn new(runtime_handle: &tokio::runtime::Handle) -> Self {
        let (request_tx, mut request_rx) = mpsc::unbounded_channel::<CaptureRequest>();
        let status = Arc::new(Mutex::new(CaptureStatus::Idle));
        let last_result = Arc::new(Mutex::new(None));

        let status_clone = status.clone();
        let result_clone = last_result.clone();

        // Spawn background task to handle capture requests
        runtime_handle.spawn(async move {
            while let Some(request) = request_rx.recv().await {
                log::debug!("Processing capture request: {:?}", request.capture_type);

                // Update status
                *status_clone.lock().await = CaptureStatus::AwaitingPermission;

                // Perform capture
                match perform_capture(request).await {
                    Ok(result) => {
                        log::info!("Capture successful: {:?}", result.saved_path);
                        *status_clone.lock().await = CaptureStatus::Success;
                        *result_clone.lock().await = Some(result);
                    }
                    Err(e) => {
                        log::error!("Capture failed: {}", e);
                        *status_clone.lock().await = CaptureStatus::Failed(e.to_string());
                    }
                }
            }
        });

        Self {
            request_tx,
            status,
            last_result,
        }
    }

    /// Request a screenshot capture.
    ///
    /// This is non-blocking and returns immediately. The capture happens
    /// asynchronously in the background.
    ///
    /// # Arguments
    /// * `capture_type` - Type of capture to perform
    /// * `save_config` - File save configuration
    /// * `copy_to_clipboard` - Whether to copy to clipboard
    pub fn request_capture(
        &self,
        capture_type: CaptureType,
        save_config: FileSaveConfig,
        copy_to_clipboard: bool,
    ) -> Result<(), CaptureError> {
        let request = CaptureRequest {
            capture_type,
            save_config,
            copy_to_clipboard,
        };

        self.request_tx
            .send(request)
            .map_err(|_| CaptureError::ImageError("Capture manager not running".to_string()))?;

        Ok(())
    }

    /// Get the current capture status.
    #[allow(dead_code)] // Will be used in Phase 2 for status UI
    pub async fn get_status(&self) -> CaptureStatus {
        self.status.lock().await.clone()
    }

    /// Get the result of the last capture and clear it.
    #[allow(dead_code)] // Will be used in Phase 2 for status UI
    pub async fn take_result(&self) -> Option<CaptureResult> {
        self.last_result.lock().await.take()
    }

    /// Try to get the result without waiting (non-blocking).
    #[allow(dead_code)] // Will be used in Phase 2 for status UI
    pub fn try_take_result(&self) -> Option<CaptureResult> {
        self.last_result.try_lock().ok().and_then(|mut r| r.take())
    }

    /// Reset status to idle.
    #[allow(dead_code)] // Will be used in Phase 2 for status UI
    pub async fn reset(&self) {
        *self.status.lock().await = CaptureStatus::Idle;
    }
}

/// Perform the actual capture operation (async).
async fn perform_capture(request: CaptureRequest) -> Result<CaptureResult, CaptureError> {
    log::info!("Starting capture: {:?}", request.capture_type);

    // Step 1: Capture via portal (get file URI)
    let uri = portal::capture_via_portal(request.capture_type).await?;
    log::info!("Portal returned URI: {}", uri);

    // Step 2: Read image data from the file URI
    let image_data = read_image_from_uri(&uri)?;
    log::info!("Read {} bytes from screenshot", image_data.len());

    // Step 3: Save to file
    let saved_path = if !request.save_config.save_directory.as_os_str().is_empty() {
        Some(save_screenshot(&image_data, &request.save_config)?)
    } else {
        None
    };

    // Step 4: Copy to clipboard
    let copied_to_clipboard = if request.copy_to_clipboard {
        log::info!("Attempting to copy {} bytes to clipboard", image_data.len());
        match clipboard::copy_to_clipboard(&image_data) {
            Ok(()) => {
                log::info!("Successfully copied to clipboard");
                true
            }
            Err(e) => {
                log::error!("Failed to copy to clipboard: {}", e);
                false
            }
        }
    } else {
        log::debug!("Clipboard copy disabled in config");
        false
    };

    Ok(CaptureResult {
        image_data,
        saved_path,
        copied_to_clipboard,
    })
}

/// Read image data from a file:// URI.
///
/// This properly decodes percent-encoded URIs (spaces, non-ASCII characters, etc.)
/// and cleans up the temporary file after reading.
fn read_image_from_uri(uri: &str) -> Result<Vec<u8>, CaptureError> {
    use std::fs;

    // Parse URL to handle percent-encoding (spaces → %20, unicode, etc.)
    let url = url::Url::parse(uri)
        .map_err(|e| CaptureError::InvalidResponse(format!("Invalid file URI '{}': {}", uri, e)))?;

    // Convert to file path (handles percent-decoding automatically)
    let path = url.to_file_path().map_err(|_| {
        CaptureError::InvalidResponse(format!("Cannot convert URI to path: {}", uri))
    })?;

    log::debug!("Reading screenshot from: {}", path.display());

    // Read the file
    let data = fs::read(&path).map_err(|e| {
        CaptureError::ImageError(format!(
            "Failed to read screenshot file {}: {}",
            path.display(),
            e
        ))
    })?;

    log::info!(
        "Successfully read {} bytes from portal screenshot",
        data.len()
    );

    // Clean up portal temp file to prevent accumulation
    if let Err(e) = fs::remove_file(&path) {
        log::warn!(
            "Failed to remove portal temp file {}: {}",
            path.display(),
            e
        );
    } else {
        log::debug!("Removed portal temp file: {}", path.display());
    }

    Ok(data)
}

/// Create a placeholder PNG image for testing.
///
/// TODO: Remove this in Phase 2 when we read actual portal screenshots.
#[allow(dead_code)] // Used in tests
fn create_placeholder_image() -> Vec<u8> {
    use cairo::{Format, ImageSurface};

    // Create a small 100x100 red square as placeholder
    let surface = ImageSurface::create(Format::ARgb32, 100, 100).unwrap();
    let ctx = cairo::Context::new(&surface).unwrap();

    // Fill with red
    ctx.set_source_rgb(1.0, 0.0, 0.0);
    ctx.paint().unwrap();

    // Add text
    ctx.set_source_rgb(1.0, 1.0, 1.0);
    ctx.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Bold);
    ctx.set_font_size(20.0);
    ctx.move_to(10.0, 50.0);
    ctx.show_text("TEST").unwrap();

    // Export to PNG bytes
    let mut buffer = Vec::new();
    surface.write_to_png(&mut buffer).unwrap();
    buffer
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_placeholder_image() {
        let image = create_placeholder_image();
        assert!(!image.is_empty());
        // PNG signature
        assert_eq!(&image[0..8], &[137, 80, 78, 71, 13, 10, 26, 10]);
    }

    #[tokio::test]
    async fn test_capture_manager_creation() {
        // Use the existing tokio runtime from #[tokio::test]
        let manager = CaptureManager::new(&tokio::runtime::Handle::current());
        let status = manager.get_status().await;
        assert_eq!(status, CaptureStatus::Idle);
    }
}
