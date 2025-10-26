use tokio::task;

use crate::capture::types::CaptureError;

/// Capture the currently focused Hyprland window using `hyprctl` + `grim`.
pub async fn capture_active_window_hyprland() -> Result<Vec<u8>, CaptureError> {
    task::spawn_blocking(|| -> Result<Vec<u8>, CaptureError> {
        use serde_json::Value;
        use std::process::{Command, Stdio};

        // Query Hyprland for the active window geometry
        let output = Command::new("hyprctl")
            .args(["activewindow", "-j"])
            .stdout(Stdio::piped())
            .output()
            .map_err(|e| {
                CaptureError::ImageError(format!("Failed to run hyprctl activewindow: {}", e))
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(CaptureError::ImageError(format!(
                "hyprctl activewindow failed: {}",
                stderr.trim()
            )));
        }

        let json: Value = serde_json::from_slice(&output.stdout).map_err(|e| {
            CaptureError::InvalidResponse(format!("Failed to parse hyprctl output: {}", e))
        })?;

        let at = json.get("at").and_then(|v| v.as_array()).ok_or_else(|| {
            CaptureError::InvalidResponse("Missing 'at' in hyprctl output".into())
        })?;
        let size = json.get("size").and_then(|v| v.as_array()).ok_or_else(|| {
            CaptureError::InvalidResponse("Missing 'size' in hyprctl output".into())
        })?;

        let (x, y) = (
            at.first()
                .and_then(|v| v.as_f64())
                .ok_or_else(|| CaptureError::InvalidResponse("Invalid 'at[0]' value".into()))?,
            at.get(1)
                .and_then(|v| v.as_f64())
                .ok_or_else(|| CaptureError::InvalidResponse("Invalid 'at[1]' value".into()))?,
        );
        let (width, height) = (
            size.first()
                .and_then(|v| v.as_f64())
                .ok_or_else(|| CaptureError::InvalidResponse("Invalid 'size[0]' value".into()))?,
            size.get(1)
                .and_then(|v| v.as_f64())
                .ok_or_else(|| CaptureError::InvalidResponse("Invalid 'size[1]' value".into()))?,
        );

        if width <= 0.0 || height <= 0.0 {
            return Err(CaptureError::InvalidResponse(
                "Active window has non-positive dimensions".into(),
            ));
        }

        let geometry = format!(
            "{},{} {}x{}",
            x.round() as i32,
            y.round() as i32,
            width.round() as u32,
            height.round() as u32
        );

        log::debug!("Capturing active window via grim: {}", geometry);
        let grim_output = Command::new("grim")
            .args(["-g", &geometry, "-"])
            .stdout(Stdio::piped())
            .output()
            .map_err(|e| CaptureError::ImageError(format!("Failed to run grim: {}", e)))?;

        if !grim_output.status.success() {
            let stderr = String::from_utf8_lossy(&grim_output.stderr);
            return Err(CaptureError::ImageError(format!(
                "grim failed: {}",
                stderr.trim()
            )));
        }

        if grim_output.stdout.is_empty() {
            return Err(CaptureError::ImageError(
                "grim returned empty screenshot".into(),
            ));
        }

        Ok(grim_output.stdout)
    })
    .await
    .map_err(|e| CaptureError::ImageError(format!("Hyprland capture task failed to join: {}", e)))?
}

/// Capture a user-selected region using `slurp` + `grim` (Hyprland/wlroots fast path).
pub async fn capture_selection_hyprland() -> Result<Vec<u8>, CaptureError> {
    task::spawn_blocking(|| -> Result<Vec<u8>, CaptureError> {
        use std::process::{Command, Stdio};

        // `slurp` outputs geometry in the format "x,y widthxheight"
        let output = Command::new("slurp")
            .args(["-f", "%x,%y %wx%h"])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .map_err(|e| {
                CaptureError::ImageError(format!("Failed to run slurp for region selection: {}", e))
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(CaptureError::ImageError(format!(
                "slurp failed: {}",
                stderr.trim()
            )));
        }

        let geometry_output = String::from_utf8(output.stdout)
            .map_err(|e| CaptureError::InvalidResponse(format!("Invalid slurp output: {}", e)))?;

        let geometry = geometry_output.trim();
        if geometry.is_empty() {
            return Err(CaptureError::ImageError(
                "slurp returned empty geometry".into(),
            ));
        }

        log::debug!("Capturing region via grim: {}", geometry);
        let grim_output = Command::new("grim")
            .args(["-g", geometry, "-"])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .map_err(|e| CaptureError::ImageError(format!("Failed to run grim: {}", e)))?;

        if !grim_output.status.success() {
            let stderr = String::from_utf8_lossy(&grim_output.stderr);
            return Err(CaptureError::ImageError(format!(
                "grim failed: {}",
                stderr.trim()
            )));
        }

        if grim_output.stdout.is_empty() {
            return Err(CaptureError::ImageError(
                "grim returned empty screenshot".into(),
            ));
        }

        Ok(grim_output.stdout)
    })
    .await
    .map_err(|e| {
        CaptureError::ImageError(format!("Selection capture task failed to join: {}", e))
    })?
}
