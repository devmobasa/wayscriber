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
            Err(e) => fall_back_to_portal("Full screen", e, CaptureType::FullScreen).await,
        },
        CaptureType::ActiveWindow => match hyprland::capture_active_window_hyprland().await {
            Ok(data) => Ok(data),
            Err(CaptureError::Cancelled(reason)) => Err(CaptureError::Cancelled(reason)),
            Err(e) => fall_back_to_portal("Active window", e, CaptureType::ActiveWindow).await,
        },
        CaptureType::Selection { .. } => match hyprland::capture_selection_hyprland().await {
            Ok(data) => Ok(data),
            Err(CaptureError::Cancelled(reason)) => Err(CaptureError::Cancelled(reason)),
            Err(e) => {
                let selection = CaptureType::Selection {
                    x: 0,
                    y: 0,
                    width: 0,
                    height: 0,
                };
                fall_back_to_portal("Selection", e, selection).await
            }
        },
    }
}

/// Retry through the portal after the compositor fast path failed.
///
/// Both errors are kept, so the user is told the more useful reason. A user
/// cancelling the portal dialog stays a cancellation, not a failure.
async fn fall_back_to_portal(
    label: &str,
    primary: CaptureError,
    capture_type: CaptureType,
) -> Result<Vec<u8>, CaptureError> {
    log::warn!("{label} capture via Hyprland failed: {primary}. Falling back to portal.");

    match portal_fallback(capture_type).await {
        Ok(data) => Ok(data),
        Err(CaptureError::Cancelled(reason)) => Err(CaptureError::Cancelled(reason)),
        Err(fallback) => Err(CaptureError::FallbackFailed {
            primary: Box::new(primary),
            fallback: Box::new(fallback),
        }),
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
