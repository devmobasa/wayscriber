use crate::capture::{
    portal,
    types::{CaptureError, CaptureType},
};

use super::reader::read_image_from_uri;

/// Capture using xdg-desktop-portal and return image bytes.
pub async fn capture_via_portal_bytes(capture_type: CaptureType) -> Result<Vec<u8>, CaptureError> {
    let uri = portal::capture_via_portal(capture_type).await?;
    log::info!("Portal returned URI: {}", uri);
    read_image_from_uri(&uri)
}
