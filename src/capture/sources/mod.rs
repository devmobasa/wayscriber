use crate::capture::types::{CaptureError, CaptureType};

mod hyprland;
mod portal;
pub(crate) mod reader;

pub async fn capture_image(capture_type: CaptureType) -> Result<Vec<u8>, CaptureError> {
    match capture_type {
        CaptureType::ActiveWindow => match hyprland::capture_active_window_hyprland().await {
            Ok(data) => Ok(data),
            Err(e) => {
                log::warn!(
                    "Active window capture via Hyprland failed: {}. Falling back to portal.",
                    e
                );
                portal::capture_via_portal_bytes(CaptureType::ActiveWindow).await
            }
        },
        CaptureType::Selection { .. } => match hyprland::capture_selection_hyprland().await {
            Ok(data) => Ok(data),
            Err(e) => {
                log::warn!(
                    "Selection capture via Hyprland failed: {}. Falling back to portal.",
                    e
                );
                portal::capture_via_portal_bytes(CaptureType::Selection {
                    x: 0,
                    y: 0,
                    width: 0,
                    height: 0,
                })
                .await
            }
        },
        other => portal::capture_via_portal_bytes(other).await,
    }
}
