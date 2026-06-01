use std::{fmt, path::PathBuf, sync::Arc};

use crate::capture::{
    dependencies::{CaptureClipboard, CaptureDependencies, CaptureFileSaver},
    file::FileSaveConfig,
    types::{
        CaptureDestination, CaptureError, CaptureResult, CaptureType,
        DesktopBackdropCaptureRequest, DesktopBackdropCaptureResult, DocumentDeliveryRequest,
        ImageDeliveryRequest, ImageOperationKind,
    },
};
use tokio::task;

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

#[derive(Clone)]
pub(crate) enum CaptureManagerRequest {
    Capture(CaptureRequest),
    CaptureDesktopBackdrop(DesktopBackdropCaptureRequest),
    DeliverImage(ImageDeliveryRequest),
    DeliverDocument(DocumentDeliveryRequest),
}

impl CaptureManagerRequest {
    pub(crate) fn operation(&self) -> ImageOperationKind {
        match self {
            Self::Capture(_) => ImageOperationKind::Screenshot,
            Self::CaptureDesktopBackdrop(request) => request.operation,
            Self::DeliverImage(request) => request.operation,
            Self::DeliverDocument(request) => request.operation,
        }
    }
}

impl fmt::Debug for CaptureManagerRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Capture(request) => f.debug_tuple("Capture").field(request).finish(),
            Self::CaptureDesktopBackdrop(request) => f
                .debug_struct("CaptureDesktopBackdrop")
                .field("logical_width", &request.logical_width)
                .field("logical_height", &request.logical_height)
                .field("scale", &request.scale)
                .field("geometry", &request.geometry)
                .field("operation", &request.operation)
                .finish(),
            Self::DeliverImage(request) => f
                .debug_struct("DeliverImage")
                .field("destination", &request.destination)
                .field("operation", &request.operation)
                .field("width", &request.image.width)
                .field("height", &request.image.height)
                .field("format", &request.image.format)
                .finish(),
            Self::DeliverDocument(request) => f
                .debug_struct("DeliverDocument")
                .field("destination", &request.destination)
                .field("operation", &request.operation)
                .field("extension", &request.document.extension)
                .field("mime_type", &request.document.mime_type)
                .finish(),
        }
    }
}

pub(crate) enum CaptureManagerResult {
    Capture(CaptureResult),
    DesktopBackdrop(DesktopBackdropCaptureResult),
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
    let mut save_error = None;
    let saved_path = match request.destination {
        CaptureDestination::FileOnly => {
            if let Some(save_config) = request.save_config.clone() {
                if !save_config.save_directory.as_os_str().is_empty() {
                    Some(
                        save_bytes(
                            Arc::clone(&dependencies.saver),
                            image_data.clone(),
                            save_config,
                        )
                        .await?,
                    )
                } else {
                    None
                }
            } else {
                None
            }
        }
        CaptureDestination::ClipboardAndFile => {
            if let Some(save_config) = request.save_config.clone() {
                if !save_config.save_directory.as_os_str().is_empty() {
                    match save_bytes(
                        Arc::clone(&dependencies.saver),
                        image_data.clone(),
                        save_config,
                    )
                    .await
                    {
                        Ok(path) => Some(path),
                        Err(err) => {
                            log::warn!("Failed to save screenshot: {}", err);
                            save_error = Some(err);
                            None
                        }
                    }
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
            copy_to_clipboard(Arc::clone(&dependencies.clipboard), image_data.clone()).await
        }
        CaptureDestination::FileOnly => {
            log::debug!("Clipboard copy not requested for this capture");
            false
        }
    };

    if matches!(request.destination, CaptureDestination::ClipboardAndFile)
        && !copied_to_clipboard
        && let Some(save_error) = save_error
    {
        return Err(save_error);
    }

    Ok(CaptureResult {
        image_data,
        operation: ImageOperationKind::Screenshot,
        fallback_format_override: None,
        saved_path,
        copied_to_clipboard,
    })
}

pub(crate) async fn deliver_image(
    request: ImageDeliveryRequest,
    dependencies: Arc<CaptureDependencies>,
) -> Result<CaptureResult, CaptureError> {
    log::info!(
        "Starting image delivery: {:?} {}x{} {} bytes",
        request.operation,
        request.image.width,
        request.image.height,
        request.image.bytes.len()
    );

    let image_data = request.image.bytes;
    let save_config = request.save_config.map(|mut config| {
        config.format = request.image.format.extension.clone();
        config
    });

    let mut save_error = None;
    let saved_path = match request.destination {
        CaptureDestination::FileOnly => {
            if let Some(config) =
                save_config.filter(|config| !config.save_directory.as_os_str().is_empty())
            {
                Some(save_bytes(Arc::clone(&dependencies.saver), image_data.clone(), config).await?)
            } else {
                None
            }
        }
        CaptureDestination::ClipboardAndFile => {
            if let Some(config) =
                save_config.filter(|config| !config.save_directory.as_os_str().is_empty())
            {
                match save_bytes(Arc::clone(&dependencies.saver), image_data.clone(), config).await
                {
                    Ok(path) => Some(path),
                    Err(err) => {
                        log::warn!("Failed to save delivered image: {}", err);
                        save_error = Some(err);
                        None
                    }
                }
            } else {
                None
            }
        }
        CaptureDestination::ClipboardOnly => None,
    };

    let copied_to_clipboard = match request.destination {
        CaptureDestination::ClipboardOnly | CaptureDestination::ClipboardAndFile => {
            log::info!(
                "Attempting to copy delivered image {} bytes to clipboard",
                image_data.len()
            );
            copy_to_clipboard(Arc::clone(&dependencies.clipboard), image_data.clone()).await
        }
        CaptureDestination::FileOnly => false,
    };

    if matches!(request.destination, CaptureDestination::ClipboardAndFile)
        && !copied_to_clipboard
        && let Some(save_error) = save_error
    {
        return Err(save_error);
    }

    Ok(CaptureResult {
        image_data,
        operation: request.operation,
        fallback_format_override: request.fallback_format_override,
        saved_path,
        copied_to_clipboard,
    })
}

pub(crate) async fn deliver_document(
    request: DocumentDeliveryRequest,
    dependencies: Arc<CaptureDependencies>,
) -> Result<CaptureResult, CaptureError> {
    log::info!(
        "Starting document delivery: {:?} {} {} bytes",
        request.operation,
        request.document.mime_type,
        request.document.bytes.len()
    );

    if !matches!(request.destination, CaptureDestination::FileOnly) {
        return Err(CaptureError::ImageError(
            "PDF clipboard export is not supported yet".to_string(),
        ));
    }

    let Some(mut save_config) = request.save_config else {
        return Err(CaptureError::ImageError(
            "Board PDF export requires file save configuration".to_string(),
        ));
    };

    if save_config.save_directory.as_os_str().is_empty() {
        return Err(CaptureError::ImageError(
            "Board PDF export requires a save directory".to_string(),
        ));
    }

    save_config.format = request.document.extension.clone();
    let document_bytes = request.document.bytes;
    let saved_path = save_bytes(
        Arc::clone(&dependencies.saver),
        document_bytes.clone(),
        save_config,
    )
    .await?;

    Ok(CaptureResult {
        image_data: document_bytes,
        operation: request.operation,
        fallback_format_override: None,
        saved_path: Some(saved_path),
        copied_to_clipboard: false,
    })
}

async fn save_bytes(
    saver: Arc<dyn CaptureFileSaver>,
    bytes: Vec<u8>,
    config: FileSaveConfig,
) -> Result<PathBuf, CaptureError> {
    task::spawn_blocking(move || saver.save(&bytes, &config))
        .await
        .map_err(|e| CaptureError::ImageError(format!("Save task failed: {}", e)))?
}

async fn copy_to_clipboard(clipboard: Arc<dyn CaptureClipboard>, image_data: Vec<u8>) -> bool {
    match task::spawn_blocking(move || clipboard.copy(&image_data))
        .await
        .map_err(|e| CaptureError::ClipboardError(format!("Clipboard task failed: {}", e)))
    {
        Ok(Ok(())) => {
            log::info!("Successfully copied to clipboard");
            true
        }
        Ok(Err(e)) | Err(e) => {
            log::error!("Failed to copy to clipboard: {}", e);
            false
        }
    }
}
