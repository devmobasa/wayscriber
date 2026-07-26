use std::{future::Future, path::PathBuf, pin::Pin};

use crate::capture::{
    clipboard,
    file::{self, FileSaveConfig},
    sources,
    types::{CaptureError, CaptureType},
};

/// Abstraction over how image data is captured for the different capture types.
pub trait CaptureSource: Send {
    fn capture(&mut self, capture_type: CaptureType) -> CaptureFuture<'_>;
}

pub type CaptureFuture<'a> =
    Pin<Box<dyn Future<Output = Result<Vec<u8>, CaptureError>> + Send + 'a>>;

pub type CaptureSaveFuture<'a> =
    Pin<Box<dyn Future<Output = Result<PathBuf, CaptureError>> + Send + 'a>>;

/// Abstraction over file saving for captured screenshots.
pub trait CaptureFileSaver: Send {
    fn save(&mut self, image_data: Vec<u8>, config: FileSaveConfig) -> CaptureSaveFuture<'_>;
}

pub type CaptureClipboardFuture<'a> =
    Pin<Box<dyn Future<Output = Result<(), CaptureError>> + Send + 'a>>;

/// Abstraction over copying screenshots to the clipboard.
pub trait CaptureClipboard: Send {
    fn copy(&mut self, image_data: Vec<u8>) -> CaptureClipboardFuture<'_>;
}

/// Bundle of dependencies used by the capture pipeline. Each component can be mocked in tests.
pub struct CaptureDependencies {
    pub source: Box<dyn CaptureSource>,
    pub saver: Box<dyn CaptureFileSaver>,
    pub clipboard: Box<dyn CaptureClipboard>,
}

impl CaptureDependencies {
    pub(crate) fn production(process_broker: crate::process_broker::ProcessBrokerHandle) -> Self {
        Self {
            source: Box::new(DefaultCaptureSource {
                process_broker: process_broker.clone(),
            }),
            saver: Box::new(DefaultFileSaver),
            clipboard: Box::new(DefaultClipboard { process_broker }),
        }
    }
}

struct DefaultCaptureSource {
    process_broker: crate::process_broker::ProcessBrokerHandle,
}
struct DefaultFileSaver;
struct DefaultClipboard {
    process_broker: crate::process_broker::ProcessBrokerHandle,
}

impl CaptureSource for DefaultCaptureSource {
    fn capture(&mut self, capture_type: CaptureType) -> CaptureFuture<'_> {
        let process_broker = self.process_broker.clone();
        Box::pin(async move { sources::capture_image(capture_type, process_broker).await })
    }
}

impl CaptureFileSaver for DefaultFileSaver {
    fn save(&mut self, image_data: Vec<u8>, config: FileSaveConfig) -> CaptureSaveFuture<'_> {
        Box::pin(async move {
            tokio::task::spawn_blocking(move || file::save_screenshot(&image_data, &config))
                .await
                .map_err(|err| CaptureError::ImageError(format!("Save task failed: {err}")))?
        })
    }
}

impl CaptureClipboard for DefaultClipboard {
    fn copy(&mut self, image_data: Vec<u8>) -> CaptureClipboardFuture<'_> {
        let process_broker = self.process_broker.clone();
        Box::pin(async move {
            tokio::task::spawn_blocking(move || {
                clipboard::copy_to_clipboard(&process_broker, &image_data)
            })
            .await
            .map_err(|err| CaptureError::ClipboardError(format!("Clipboard task failed: {err}")))?
        })
    }
}

#[cfg(test)]
impl Default for CaptureDependencies {
    fn default() -> Self {
        Self {
            source: Box::new(UnusedCaptureSource),
            saver: Box::new(DefaultFileSaver),
            clipboard: Box::new(UnusedClipboard),
        }
    }
}

#[cfg(test)]
struct UnusedCaptureSource;

#[cfg(test)]
impl CaptureSource for UnusedCaptureSource {
    fn capture(&mut self, _capture_type: CaptureType) -> CaptureFuture<'_> {
        Box::pin(async {
            Err(CaptureError::ImageError(
                "unused capture test dependency was invoked".to_string(),
            ))
        })
    }
}

#[cfg(test)]
struct UnusedClipboard;

#[cfg(test)]
impl CaptureClipboard for UnusedClipboard {
    fn copy(&mut self, _image_data: Vec<u8>) -> CaptureClipboardFuture<'_> {
        Box::pin(async {
            Err(CaptureError::ClipboardError(
                "unused clipboard test dependency was invoked".to_string(),
            ))
        })
    }
}
