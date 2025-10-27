use std::{path::PathBuf, sync::Arc};

use async_trait::async_trait;

use crate::capture::{
    clipboard,
    file::{self, FileSaveConfig},
    sources,
    types::{CaptureError, CaptureType},
};

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
        sources::capture_image(capture_type).await
    }
}

impl CaptureFileSaver for DefaultFileSaver {
    fn save(&self, image_data: &[u8], config: &FileSaveConfig) -> Result<PathBuf, CaptureError> {
        file::save_screenshot(image_data, config)
    }
}

impl CaptureClipboard for DefaultClipboard {
    fn copy(&self, image_data: &[u8]) -> Result<(), CaptureError> {
        clipboard::copy_to_clipboard(image_data)
    }
}
