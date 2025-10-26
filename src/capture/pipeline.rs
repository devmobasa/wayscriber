use std::{fmt, sync::Arc};

use crate::capture::{
    dependencies::CaptureDependencies,
    file::FileSaveConfig,
    types::{CaptureDestination, CaptureError, CaptureResult, CaptureType},
};

#[derive(Clone)]
pub(crate) struct CaptureRequest {
    pub(crate) capture_type: CaptureType,
    pub(crate) destination: CaptureDestination,
    pub(crate) save_config: Option<FileSaveConfig>,
}

impl fmt::Debug for CaptureRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
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

pub(crate) async fn perform_capture(
    request: CaptureRequest,
    dependencies: Arc<CaptureDependencies>,
) -> Result<CaptureResult, CaptureError> {
    log::info!("Starting capture: {:?}", request.capture_type);

    // Step 1: Capture image bytes (prefer compositor-specific path where possible)
    let image_data = match dependencies.source.capture(request.capture_type).await {
        Ok(data) => data,
        Err(CaptureError::Cancelled(reason)) => {
            log::info!("Capture cancelled: {}", reason);
            return Err(CaptureError::Cancelled(reason));
        }
        Err(err) => return Err(err),
    };

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
