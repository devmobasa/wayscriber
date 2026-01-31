use crate::capture::types::{CaptureError, CaptureType};

pub(crate) mod frozen;
mod hyprland;
#[cfg(feature = "portal")]
pub mod portal;
pub(crate) mod reader;

pub async fn capture_image(capture_type: CaptureType) -> Result<Vec<u8>, CaptureError> {
    match capture_type {
        CaptureType::FullScreen => match hyprland::capture_full_screen_hyprland().await {
            Ok(data) => Ok(data),
            Err(CaptureError::Cancelled(reason)) => Err(CaptureError::Cancelled(reason)),
            Err(e) => {
                let primary = e.to_string();
                log::warn!(
                    "Full screen capture via Hyprland failed: {}. Falling back to portal.",
                    primary
                );
                match portal_fallback(CaptureType::FullScreen).await {
                    Ok(data) => Ok(data),
                    Err(portal_err) => Err(CaptureError::ImageError(format!(
                        "Hyprland capture failed: {primary}. Portal fallback failed: {portal_err}"
                    ))),
                }
            }
        },
        CaptureType::ActiveWindow => match hyprland::capture_active_window_hyprland().await {
            Ok(data) => Ok(data),
            Err(CaptureError::Cancelled(reason)) => Err(CaptureError::Cancelled(reason)),
            Err(e) => {
                let primary = e.to_string();
                log::warn!(
                    "Active window capture via Hyprland failed: {}. Falling back to portal.",
                    primary
                );
                match portal_fallback(CaptureType::ActiveWindow).await {
                    Ok(data) => Ok(data),
                    Err(portal_err) => Err(CaptureError::ImageError(format!(
                        "Hyprland capture failed: {primary}. Portal fallback failed: {portal_err}"
                    ))),
                }
            }
        },
        CaptureType::Selection { .. } => match hyprland::capture_selection_hyprland().await {
            Ok(data) => Ok(data),
            Err(CaptureError::Cancelled(reason)) => Err(CaptureError::Cancelled(reason)),
            Err(e) => {
                let primary = e.to_string();
                log::warn!(
                    "Selection capture via Hyprland failed: {}. Falling back to portal.",
                    primary
                );
                match portal_fallback(CaptureType::Selection {
                    x: 0,
                    y: 0,
                    width: 0,
                    height: 0,
                })
                .await
                {
                    Ok(data) => Ok(data),
                    Err(portal_err) => Err(CaptureError::ImageError(format!(
                        "Hyprland capture failed: {primary}. Portal fallback failed: {portal_err}"
                    ))),
                }
            }
        },
    }
}

async fn portal_fallback(capture_type: CaptureType) -> Result<Vec<u8>, CaptureError> {
    #[cfg(feature = "portal")]
    {
        portal::capture_via_portal_bytes(capture_type).await
    }
    #[cfg(not(feature = "portal"))]
    {
        let _ = capture_type;
        Err(CaptureError::PortalUnavailable)
    }
}
