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

        let (mut x, mut y) = (
            at.first()
                .and_then(|v| v.as_f64())
                .ok_or_else(|| CaptureError::InvalidResponse("Invalid 'at[0]' value".into()))?,
            at.get(1)
                .and_then(|v| v.as_f64())
                .ok_or_else(|| CaptureError::InvalidResponse("Invalid 'at[1]' value".into()))?,
        );
        let (mut width, mut height) = (
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

        let monitor_id = json.get("monitor").and_then(|v| v.as_i64());
        let monitor_name = json.get("monitor").and_then(|v| v.as_str());

        if let Some(scale) = hyprland_monitor_scale(monitor_id, monitor_name)? {
            if (scale - 1.0).abs() > f64::EPSILON {
                log::debug!(
                    "Applying monitor scale {:.2} to active window capture",
                    scale
                );
                x *= scale;
                y *= scale;
                width *= scale;
                height *= scale;
            }
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

fn hyprland_monitor_scale(
    monitor_id: Option<i64>,
    monitor_name: Option<&str>,
) -> Result<Option<f64>, CaptureError> {
    use serde_json::Value;
    use std::process::{Command, Stdio};

    if monitor_id.is_none() && monitor_name.is_none() {
        return Ok(None);
    }

    let output = Command::new("hyprctl")
        .args(["monitors", "-j"])
        .stdout(Stdio::piped())
        .output()
        .map_err(|e| CaptureError::ImageError(format!("Failed to run hyprctl monitors: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(CaptureError::ImageError(format!(
            "hyprctl monitors failed: {}",
            stderr.trim()
        )));
    }

    let monitors: Value = serde_json::from_slice(&output.stdout).map_err(|e| {
        CaptureError::InvalidResponse(format!("Failed to parse hyprctl monitors output: {}", e))
    })?;

    let list = monitors.as_array().ok_or_else(|| {
        CaptureError::InvalidResponse("hyprctl monitors did not return an array".into())
    })?;

    for monitor in list {
        let id_match = monitor_id
            .and_then(|target| {
                monitor
                    .get("id")
                    .and_then(|v| v.as_i64())
                    .map(|id| id == target)
            })
            .unwrap_or(false);
        let name_match = monitor_name
            .and_then(|target| {
                monitor
                    .get("name")
                    .and_then(|v| v.as_str())
                    .map(|name| name == target)
            })
            .unwrap_or(false);

        if id_match || name_match {
            if let Some(scale) = monitor.get("scale").and_then(|v| v.as_f64()) {
                return Ok(Some(scale));
            } else {
                return Ok(Some(1.0));
            }
        }
    }

    Ok(None)
}
