use std::sync::Arc;

use tokio::sync::{Mutex, mpsc};

use crate::capture::{
    dependencies::CaptureDependencies,
    file::FileSaveConfig,
    pipeline::{CaptureRequest, perform_capture},
    types::{CaptureDestination, CaptureError, CaptureOutcome, CaptureStatus, CaptureType},
};

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
                    Err(CaptureError::Cancelled(reason)) => {
                        log::info!("Capture cancelled: {}", reason);
                        *status_clone.lock().await = CaptureStatus::Cancelled(reason.clone());
                        *result_clone.lock().await = Some(CaptureOutcome::Cancelled(reason));
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

#[cfg(test)]
impl CaptureManager {
    pub(crate) fn with_closed_channel_for_test() -> Self {
        let (tx, rx) = mpsc::unbounded_channel::<CaptureRequest>();
        drop(rx);
        Self {
            request_tx: tx,
            status: Arc::new(Mutex::new(CaptureStatus::Idle)),
            last_result: Arc::new(Mutex::new(None)),
        }
    }
}
