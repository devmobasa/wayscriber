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
pub mod portal;
pub mod types;

pub use types::{
    CaptureDestination, CaptureError, CaptureOutcome, CaptureResult, CaptureStatus, CaptureType,
};

use async_trait::async_trait;
use file::{FileSaveConfig, save_screenshot};
use std::path::PathBuf;
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
    last_result: Arc<Mutex<Option<CaptureOutcome>>>,
}

/// A request to perform a capture operation.
struct CaptureRequest {
    capture_type: CaptureType,
    destination: CaptureDestination,
    save_config: Option<FileSaveConfig>,
}

impl std::fmt::Debug for CaptureRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CaptureRequest")
            .field("capture_type", &self.capture_type)
            .field("destination", &self.destination)
            .field(
                "save_config",
                &self
                    .save_config
                    .as_ref()
                    .map(|cfg| cfg.filename_template.clone()),
            )
            .finish()
    }
}

/// Abstraction over how image data is captured for the different capture types.
#[async_trait]
pub trait CaptureSource: Send + Sync {
    async fn capture(&self, capture_type: CaptureType) -> Result<Vec<u8>, CaptureError>;
}

/// Abstraction over file saving for captured screenshots.
pub trait CaptureFileSaver: Send + Sync {
    fn save(&self, image_data: &[u8], config: &FileSaveConfig) -> Result<PathBuf, CaptureError>;
}

/// Abstraction over copying screenshots to the clipboard.
pub trait CaptureClipboard: Send + Sync {
    fn copy(&self, image_data: &[u8]) -> Result<(), CaptureError>;
}

/// Bundle of dependencies used by the capture pipeline. Each component can be mocked in tests.
#[derive(Clone)]
pub struct CaptureDependencies {
    pub source: Arc<dyn CaptureSource>,
    pub saver: Arc<dyn CaptureFileSaver>,
    pub clipboard: Arc<dyn CaptureClipboard>,
}

impl Default for CaptureDependencies {
    fn default() -> Self {
        Self {
            source: Arc::new(DefaultCaptureSource),
            saver: Arc::new(DefaultFileSaver),
            clipboard: Arc::new(DefaultClipboard),
        }
    }
}

struct DefaultCaptureSource;
struct DefaultFileSaver;
struct DefaultClipboard;

#[async_trait]
impl CaptureSource for DefaultCaptureSource {
    async fn capture(&self, capture_type: CaptureType) -> Result<Vec<u8>, CaptureError> {
        match capture_type {
            CaptureType::ActiveWindow => match capture_active_window_hyprland().await {
                Ok(data) => Ok(data),
                Err(e) => {
                    log::warn!(
                        "Active window capture via Hyprland failed: {}. Falling back to portal.",
                        e
                    );
                    capture_via_portal_bytes(CaptureType::ActiveWindow).await
                }
            },
            CaptureType::Selection { .. } => match capture_selection_hyprland().await {
                Ok(data) => Ok(data),
                Err(e) => {
                    log::warn!(
                        "Selection capture via Hyprland failed: {}. Falling back to portal.",
                        e
                    );
                    capture_via_portal_bytes(CaptureType::Selection {
                        x: 0,
                        y: 0,
                        width: 0,
                        height: 0,
                    })
                    .await
                }
            },
            other => capture_via_portal_bytes(other).await,
        }
    }
}

impl CaptureFileSaver for DefaultFileSaver {
    fn save(&self, image_data: &[u8], config: &FileSaveConfig) -> Result<PathBuf, CaptureError> {
        save_screenshot(image_data, config)
    }
}

impl CaptureClipboard for DefaultClipboard {
    fn copy(&self, image_data: &[u8]) -> Result<(), CaptureError> {
        clipboard::copy_to_clipboard(image_data)
    }
}

impl CaptureManager {
    /// Create a new capture manager.
    ///
    /// This spawns a background task that handles async portal operations.
    ///
    /// # Arguments
    /// * `runtime_handle` - Tokio runtime handle for spawning async tasks
    pub fn new(runtime_handle: &tokio::runtime::Handle) -> Self {
        Self::with_dependencies(runtime_handle, CaptureDependencies::default())
    }

    /// Create a capture manager with custom dependencies (useful for testing).
    pub fn with_dependencies(
        runtime_handle: &tokio::runtime::Handle,
        dependencies: CaptureDependencies,
    ) -> Self {
        let (request_tx, mut request_rx) = mpsc::unbounded_channel::<CaptureRequest>();
        let status = Arc::new(Mutex::new(CaptureStatus::Idle));
        let last_result = Arc::new(Mutex::new(None));
        let dependencies = Arc::new(dependencies);

        let status_clone = status.clone();
        let result_clone = last_result.clone();
        let deps_clone = dependencies.clone();

        // Spawn background task to handle capture requests
        runtime_handle.spawn(async move {
            while let Some(request) = request_rx.recv().await {
                log::debug!("Processing capture request: {:?}", request.capture_type);

                // Update status
                *status_clone.lock().await = CaptureStatus::AwaitingPermission;

                // Perform capture
                match perform_capture(request, deps_clone.clone()).await {
                    Ok(result) => {
                        log::info!("Capture successful: {:?}", result.saved_path);
                        *status_clone.lock().await = CaptureStatus::Success;
                        *result_clone.lock().await = Some(CaptureOutcome::Success(result));
                    }
                    Err(e) => {
                        let error_message = e.to_string();
                        log::error!("Capture failed: {}", error_message);
                        *status_clone.lock().await = CaptureStatus::Failed(error_message.clone());
                        *result_clone.lock().await = Some(CaptureOutcome::Failed(error_message));
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
        destination: CaptureDestination,
        save_config: Option<FileSaveConfig>,
    ) -> Result<(), CaptureError> {
        let request = CaptureRequest {
            capture_type,
            destination,
            save_config,
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
    pub async fn take_result(&self) -> Option<CaptureOutcome> {
        self.last_result.lock().await.take()
    }

    /// Try to get the result without waiting (non-blocking).
    #[allow(dead_code)] // Will be used in Phase 2 for status UI
    pub fn try_take_result(&self) -> Option<CaptureOutcome> {
        self.last_result.try_lock().ok().and_then(|mut r| r.take())
    }

    /// Reset status to idle.
    #[allow(dead_code)] // Will be used in Phase 2 for status UI
    pub async fn reset(&self) {
        *self.status.lock().await = CaptureStatus::Idle;
    }
}

async fn perform_capture(
    request: CaptureRequest,
    dependencies: Arc<CaptureDependencies>,
) -> Result<CaptureResult, CaptureError> {
    log::info!("Starting capture: {:?}", request.capture_type);

    // Step 1: Capture image bytes (prefer compositor-specific path where possible)
    let image_data = dependencies.source.capture(request.capture_type).await?;

    log::info!("Obtained screenshot data ({} bytes)", image_data.len());

    log::debug!(
        "Captured screenshot data size: {} bytes (capture_type={:?})",
        image_data.len(),
        request.capture_type
    );

    // Step 3: Save to file (if requested)
    let saved_path = match request.destination {
        CaptureDestination::FileOnly | CaptureDestination::ClipboardAndFile => {
            if let Some(save_config) = request.save_config.as_ref() {
                if !save_config.save_directory.as_os_str().is_empty() {
                    Some(dependencies.saver.save(&image_data, save_config)?)
                } else {
                    None
                }
            } else {
                None
            }
        }
        CaptureDestination::ClipboardOnly => None,
    };

    // Step 4: Copy to clipboard (if requested)
    let copied_to_clipboard = match request.destination {
        CaptureDestination::ClipboardOnly | CaptureDestination::ClipboardAndFile => {
            log::info!("Attempting to copy {} bytes to clipboard", image_data.len());
            match dependencies.clipboard.copy(&image_data) {
                Ok(()) => {
                    log::info!("Successfully copied to clipboard");
                    true
                }
                Err(e) => {
                    log::error!("Failed to copy to clipboard: {}", e);
                    false
                }
            }
        }
        CaptureDestination::FileOnly => {
            log::debug!("Clipboard copy not requested for this capture");
            false
        }
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
    use std::thread;
    use std::time::Duration;

    // Parse URL to handle percent-encoding (spaces → %20, unicode, etc.)
    let url = url::Url::parse(uri)
        .map_err(|e| CaptureError::InvalidResponse(format!("Invalid file URI '{}': {}", uri, e)))?;

    // Convert to file path (handles percent-decoding automatically)
    let path = url.to_file_path().map_err(|_| {
        CaptureError::InvalidResponse(format!("Cannot convert URI to path: {}", uri))
    })?;

    log::debug!("Reading screenshot from: {}", path.display());

    // Wait briefly for portal to flush the file to disk (some portals write asynchronously)
    const MAX_ATTEMPTS: usize = 60; // up to 3 seconds total
    const ATTEMPT_DELAY_MS: u64 = 50;

    let mut data = Vec::new();
    for attempt in 0..MAX_ATTEMPTS {
        match fs::read(&path) {
            Ok(bytes) if !bytes.is_empty() => {
                data = bytes;
                break;
            }
            Ok(_) => {
                log::trace!(
                    "Portal screenshot file {} still empty (attempt {}/{})",
                    path.display(),
                    attempt + 1,
                    MAX_ATTEMPTS
                );
            }
            Err(e) => {
                log::trace!(
                    "Portal screenshot file {} not ready yet (attempt {}/{}): {}",
                    path.display(),
                    attempt + 1,
                    MAX_ATTEMPTS,
                    e
                );
            }
        }

        if attempt + 1 == MAX_ATTEMPTS {
            return Err(CaptureError::ImageError(format!(
                "Portal screenshot file {} not ready after {} attempts",
                path.display(),
                MAX_ATTEMPTS
            )));
        }

        thread::sleep(Duration::from_millis(ATTEMPT_DELAY_MS));
    }

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

/// Capture using xdg-desktop-portal and return image bytes.
async fn capture_via_portal_bytes(capture_type: CaptureType) -> Result<Vec<u8>, CaptureError> {
    let uri = portal::capture_via_portal(capture_type).await?;
    log::info!("Portal returned URI: {}", uri);
    read_image_from_uri(&uri)
}

/// Capture the currently focused Hyprland window using `hyprctl` + `grim`.
async fn capture_active_window_hyprland() -> Result<Vec<u8>, CaptureError> {
    tokio::task::spawn_blocking(|| -> Result<Vec<u8>, CaptureError> {
        use serde_json::Value;
        use std::process::{Command, Stdio};

        // Query Hyprland for the active window geometry
        let output = Command::new("hyprctl")
            .args(["activewindow", "-j"])
            .stdout(Stdio::piped())
            .output()
            .map_err(|e| {
                CaptureError::ImageError(format!("Failed to run hyprctl activewindow: {}", e))
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(CaptureError::ImageError(format!(
                "hyprctl activewindow failed: {}",
                stderr.trim()
            )));
        }

        let json: Value = serde_json::from_slice(&output.stdout).map_err(|e| {
            CaptureError::InvalidResponse(format!("Failed to parse hyprctl output: {}", e))
        })?;

        let at = json.get("at").and_then(|v| v.as_array()).ok_or_else(|| {
            CaptureError::InvalidResponse("Missing 'at' in hyprctl output".into())
        })?;
        let size = json.get("size").and_then(|v| v.as_array()).ok_or_else(|| {
            CaptureError::InvalidResponse("Missing 'size' in hyprctl output".into())
        })?;

        let (x, y) = (
            at.first()
                .and_then(|v| v.as_f64())
                .ok_or_else(|| CaptureError::InvalidResponse("Invalid 'at[0]' value".into()))?,
            at.get(1)
                .and_then(|v| v.as_f64())
                .ok_or_else(|| CaptureError::InvalidResponse("Invalid 'at[1]' value".into()))?,
        );
        let (width, height) = (
            size.first()
                .and_then(|v| v.as_f64())
                .ok_or_else(|| CaptureError::InvalidResponse("Invalid 'size[0]' value".into()))?,
            size.get(1)
                .and_then(|v| v.as_f64())
                .ok_or_else(|| CaptureError::InvalidResponse("Invalid 'size[1]' value".into()))?,
        );

        if width <= 0.0 || height <= 0.0 {
            return Err(CaptureError::InvalidResponse(
                "Active window has non-positive dimensions".into(),
            ));
        }

        let geometry = format!(
            "{},{} {}x{}",
            x.round() as i32,
            y.round() as i32,
            width.round() as u32,
            height.round() as u32
        );

        log::debug!("Capturing active window via grim: {}", geometry);
        let grim_output = Command::new("grim")
            .args(["-g", &geometry, "-"])
            .stdout(Stdio::piped())
            .output()
            .map_err(|e| CaptureError::ImageError(format!("Failed to run grim: {}", e)))?;

        if !grim_output.status.success() {
            let stderr = String::from_utf8_lossy(&grim_output.stderr);
            return Err(CaptureError::ImageError(format!(
                "grim failed: {}",
                stderr.trim()
            )));
        }

        if grim_output.stdout.is_empty() {
            return Err(CaptureError::ImageError(
                "grim returned empty screenshot".into(),
            ));
        }

        Ok(grim_output.stdout)
    })
    .await
    .map_err(|e| CaptureError::ImageError(format!("Hyprland capture task failed to join: {}", e)))?
}

/// Capture a user-selected region using `slurp` + `grim` (Hyprland/wlroots fast path).
async fn capture_selection_hyprland() -> Result<Vec<u8>, CaptureError> {
    tokio::task::spawn_blocking(|| -> Result<Vec<u8>, CaptureError> {
        use std::process::{Command, Stdio};

        // `slurp` outputs geometry in the format "x,y widthxheight"
        let output = Command::new("slurp")
            .args(["-f", "%x,%y %wx%h"])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .map_err(|e| {
                CaptureError::ImageError(format!("Failed to run slurp for region selection: {}", e))
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(CaptureError::ImageError(format!(
                "slurp selection cancelled or failed: {}",
                stderr.trim()
            )));
        }

        let geometry = String::from_utf8(output.stdout)
            .map_err(|e| CaptureError::ImageError(format!("Invalid slurp output: {}", e)))?;
        let geometry = geometry.trim();

        if geometry.is_empty() {
            return Err(CaptureError::ImageError(
                "slurp returned empty geometry".to_string(),
            ));
        }

        log::debug!("Capturing selection via grim: {}", geometry);
        let grim_output = Command::new("grim")
            .args(["-g", geometry, "-"])
            .stdout(Stdio::piped())
            .output()
            .map_err(|e| CaptureError::ImageError(format!("Failed to run grim: {}", e)))?;

        if !grim_output.status.success() {
            let stderr = String::from_utf8_lossy(&grim_output.stderr);
            return Err(CaptureError::ImageError(format!(
                "grim selection capture failed: {}",
                stderr.trim()
            )));
        }

        if grim_output.stdout.is_empty() {
            return Err(CaptureError::ImageError(
                "grim returned empty selection screenshot".into(),
            ));
        }

        Ok(grim_output.stdout)
    })
    .await
    .map_err(|e| CaptureError::ImageError(format!("Selection capture task failed: {}", e)))?
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
    use std::fs;
    use std::sync::{Arc, Mutex};
    use tempfile::TempDir;
    use tokio::time::{Duration, sleep};

    #[derive(Clone)]
    struct MockSource {
        data: Vec<u8>,
        error: Arc<Mutex<Option<CaptureError>>>,
        captured_types: Arc<Mutex<Vec<CaptureType>>>,
    }

    #[async_trait]
    impl CaptureSource for MockSource {
        async fn capture(&self, capture_type: CaptureType) -> Result<Vec<u8>, CaptureError> {
            self.captured_types.lock().unwrap().push(capture_type);
            if let Some(err) = self.error.lock().unwrap().take() {
                Err(err)
            } else {
                Ok(self.data.clone())
            }
        }
    }

    #[derive(Clone)]
    struct MockSaver {
        pub should_fail: bool,
        pub path: PathBuf,
        pub calls: Arc<Mutex<usize>>,
    }

    impl CaptureFileSaver for MockSaver {
        fn save(
            &self,
            _image_data: &[u8],
            _config: &FileSaveConfig,
        ) -> Result<PathBuf, CaptureError> {
            *self.calls.lock().unwrap() += 1;
            if self.should_fail {
                Err(CaptureError::SaveError(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "save failed",
                )))
            } else {
                Ok(self.path.clone())
            }
        }
    }

    #[derive(Clone)]
    struct MockClipboard {
        pub should_fail: bool,
        pub calls: Arc<Mutex<usize>>,
    }

    impl CaptureClipboard for MockClipboard {
        fn copy(&self, _image_data: &[u8]) -> Result<(), CaptureError> {
            *self.calls.lock().unwrap() += 1;
            if self.should_fail {
                Err(CaptureError::ClipboardError(
                    "clipboard failure".to_string(),
                ))
            } else {
                Ok(())
            }
        }
    }

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

    #[tokio::test]
    async fn test_perform_capture_clipboard_only_success() {
        let source = MockSource {
            data: vec![1, 2, 3],
            error: Arc::new(Mutex::new(None)),
            captured_types: Arc::new(Mutex::new(Vec::new())),
        };
        let saver = MockSaver {
            should_fail: false,
            path: PathBuf::from("unused.png"),
            calls: Arc::new(Mutex::new(0)),
        };
        let saver_handle = saver.clone();
        let clipboard = MockClipboard {
            should_fail: false,
            calls: Arc::new(Mutex::new(0)),
        };
        let clipboard_handle = clipboard.clone();
        let deps = CaptureDependencies {
            source: Arc::new(source),
            saver: Arc::new(saver),
            clipboard: Arc::new(clipboard),
        };
        let request = CaptureRequest {
            capture_type: CaptureType::FullScreen,
            destination: CaptureDestination::ClipboardOnly,
            save_config: None,
        };

        let result = perform_capture(request, Arc::new(deps.clone()))
            .await
            .unwrap();
        assert!(result.saved_path.is_none());
        assert!(result.copied_to_clipboard);
        assert_eq!(*clipboard_handle.calls.lock().unwrap(), 1);
        assert_eq!(*saver_handle.calls.lock().unwrap(), 0);
    }

    #[tokio::test]
    async fn test_perform_capture_file_only_success() {
        let source = MockSource {
            data: vec![4, 5, 6],
            error: Arc::new(Mutex::new(None)),
            captured_types: Arc::new(Mutex::new(Vec::new())),
        };
        let saver = MockSaver {
            should_fail: false,
            path: PathBuf::from("/tmp/test.png"),
            calls: Arc::new(Mutex::new(0)),
        };
        let saver_handle = saver.clone();
        let clipboard = MockClipboard {
            should_fail: false,
            calls: Arc::new(Mutex::new(0)),
        };
        let clipboard_handle = clipboard.clone();
        let deps = CaptureDependencies {
            source: Arc::new(source),
            saver: Arc::new(saver),
            clipboard: Arc::new(clipboard),
        };
        let request = CaptureRequest {
            capture_type: CaptureType::FullScreen,
            destination: CaptureDestination::FileOnly,
            save_config: Some(FileSaveConfig::default()),
        };

        let result = perform_capture(request, Arc::new(deps.clone()))
            .await
            .unwrap();
        assert!(result.saved_path.is_some());
        assert!(!result.copied_to_clipboard);
        assert_eq!(*saver_handle.calls.lock().unwrap(), 1);
        assert_eq!(*clipboard_handle.calls.lock().unwrap(), 0);
    }

    #[tokio::test]
    async fn test_perform_capture_clipboard_failure() {
        let source = MockSource {
            data: vec![7, 8, 9],
            error: Arc::new(Mutex::new(None)),
            captured_types: Arc::new(Mutex::new(Vec::new())),
        };
        let saver = MockSaver {
            should_fail: false,
            path: PathBuf::from("/tmp/a.png"),
            calls: Arc::new(Mutex::new(0)),
        };
        let clipboard = MockClipboard {
            should_fail: true,
            calls: Arc::new(Mutex::new(0)),
        };
        let clipboard_handle = clipboard.clone();
        let deps = CaptureDependencies {
            source: Arc::new(source),
            saver: Arc::new(saver),
            clipboard: Arc::new(clipboard),
        };
        let request = CaptureRequest {
            capture_type: CaptureType::FullScreen,
            destination: CaptureDestination::ClipboardOnly,
            save_config: None,
        };

        let result = perform_capture(request, Arc::new(deps.clone()))
            .await
            .unwrap();
        assert!(!result.copied_to_clipboard);
        assert_eq!(*clipboard_handle.calls.lock().unwrap(), 1);
    }

    #[tokio::test]
    async fn test_perform_capture_save_failure() {
        let source = MockSource {
            data: vec![10, 11, 12],
            error: Arc::new(Mutex::new(None)),
            captured_types: Arc::new(Mutex::new(Vec::new())),
        };
        let saver = MockSaver {
            should_fail: true,
            path: PathBuf::from("/tmp/should_fail.png"),
            calls: Arc::new(Mutex::new(0)),
        };
        let saver_handle = saver.clone();
        let clipboard = MockClipboard {
            should_fail: false,
            calls: Arc::new(Mutex::new(0)),
        };
        let deps = CaptureDependencies {
            source: Arc::new(source),
            saver: Arc::new(saver),
            clipboard: Arc::new(clipboard),
        };
        let request = CaptureRequest {
            capture_type: CaptureType::FullScreen,
            destination: CaptureDestination::FileOnly,
            save_config: Some(FileSaveConfig::default()),
        };

        let err = perform_capture(request, Arc::new(deps.clone()))
            .await
            .unwrap_err();
        match err {
            CaptureError::SaveError(_) => {}
            other => panic!("expected SaveError, got {:?}", other),
        }
        assert_eq!(*saver_handle.calls.lock().unwrap(), 1);
    }

    #[tokio::test]
    async fn test_capture_manager_with_dependencies() {
        let clipboard_calls = Arc::new(Mutex::new(0));
        let source = MockSource {
            data: vec![13, 14, 15],
            error: Arc::new(Mutex::new(None)),
            captured_types: Arc::new(Mutex::new(Vec::new())),
        };
        let saver = MockSaver {
            should_fail: false,
            path: PathBuf::from("/tmp/manager.png"),
            calls: Arc::new(Mutex::new(0)),
        };
        let clipboard = MockClipboard {
            should_fail: false,
            calls: clipboard_calls.clone(),
        };
        let deps = CaptureDependencies {
            source: Arc::new(source),
            saver: Arc::new(saver),
            clipboard: Arc::new(clipboard),
        };
        let manager =
            CaptureManager::with_dependencies(&tokio::runtime::Handle::current(), deps.clone());

        manager
            .request_capture(
                CaptureType::FullScreen,
                CaptureDestination::ClipboardOnly,
                None,
            )
            .unwrap();

        // Wait for background thread to finish
        let mut outcome = None;
        for _ in 0..10 {
            if let Some(result) = manager.try_take_result() {
                outcome = Some(result);
                break;
            }
            sleep(Duration::from_millis(20)).await;
        }

        match outcome {
            Some(CaptureOutcome::Success(result)) => {
                assert!(result.saved_path.is_none());
                assert!(result.copied_to_clipboard);
            }
            other => panic!("Expected success outcome, got {:?}", other),
        }
        assert_eq!(*clipboard_calls.lock().unwrap(), 1);
        assert_eq!(manager.get_status().await, CaptureStatus::Success);
    }

    #[tokio::test]
    async fn test_perform_capture_clipboard_and_file_success() {
        let source = MockSource {
            data: vec![21, 22, 23],
            error: Arc::new(Mutex::new(None)),
            captured_types: Arc::new(Mutex::new(Vec::new())),
        };
        let saver = MockSaver {
            should_fail: false,
            path: PathBuf::from("/tmp/combined.png"),
            calls: Arc::new(Mutex::new(0)),
        };
        let clipboard = MockClipboard {
            should_fail: false,
            calls: Arc::new(Mutex::new(0)),
        };
        let deps = CaptureDependencies {
            source: Arc::new(source),
            saver: Arc::new(saver.clone()),
            clipboard: Arc::new(clipboard.clone()),
        };
        let request = CaptureRequest {
            capture_type: CaptureType::FullScreen,
            destination: CaptureDestination::ClipboardAndFile,
            save_config: Some(FileSaveConfig::default()),
        };

        let result = perform_capture(request, Arc::new(deps)).await.unwrap();
        assert!(result.saved_path.is_some());
        assert!(result.copied_to_clipboard);
        assert_eq!(*saver.calls.lock().unwrap(), 1);
        assert_eq!(*clipboard.calls.lock().unwrap(), 1);
    }

    #[test]
    fn test_read_image_from_uri_reads_and_removes_file() {
        let temp = TempDir::new().unwrap();
        let file_path = temp.path().join("capture file.png");
        fs::write(&file_path, b"portal-bytes").unwrap();
        let uri = url::Url::from_file_path(&file_path)
            .unwrap()
            .to_string();

        let data = read_image_from_uri(&uri).expect("read succeeds");
        assert_eq!(data, b"portal-bytes");
        assert!(
            !file_path.exists(),
            "read_image_from_uri should delete the portal temp file"
        );
    }
}
