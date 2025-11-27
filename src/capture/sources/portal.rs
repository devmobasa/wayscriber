#![cfg(feature = "portal")]
use crate::capture::{
    portal,
    types::{CaptureError, CaptureType},
};

use super::reader::read_image_from_uri;

/// Capture using xdg-desktop-portal and return image bytes without blocking the Tokio runtime.
pub async fn capture_via_portal_bytes(capture_type: CaptureType) -> Result<Vec<u8>, CaptureError> {
    let uri = portal::capture_via_portal(capture_type).await?;
    log::info!("Portal returned URI: {}", uri);

    tokio::task::spawn_blocking(move || read_image_from_uri(&uri))
        .await
        .map_err(|e| CaptureError::ImageError(format!("Portal reader task failed: {}", e)))?
}
