use tokio::task;

use crate::capture::types::CaptureError;

/// Capture the entire Wayland scene using `grim`.
pub async fn capture_full_screen_hyprland() -> Result<Vec<u8>, CaptureError> {
    task::spawn_blocking(|| -> Result<Vec<u8>, CaptureError> {
        use std::process::{Command, Stdio};

        log::debug!("Capturing full screen via grim");

        let monitor_arg = hyprland_focused_monitor_name()?;

        let output = {
            let mut cmd = Command::new("grim");
            if let Some(name) = &monitor_arg {
                cmd.args(["-o", name]);
            }
            cmd.arg("-").stdout(Stdio::piped()).output().map_err(|e| {
                CaptureError::ImageError(format!("Failed to run grim for full screen: {}", e))
            })?
        };

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(CaptureError::ImageError(format!(
                "grim full screen capture failed: {}",
                stderr.trim()
            )));
        }

        if output.stdout.is_empty() {
            return Err(CaptureError::ImageError(
                "grim returned empty screenshot for full screen capture".into(),
            ));
        }

        Ok(output.stdout)
    })
    .await
    .map_err(|e| {
        CaptureError::ImageError(format!("Full screen capture task failed to join: {}", e))
    })?
}

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

        let monitor_name = json.get("monitor").and_then(|v| v.as_str());
        let monitor_info = monitor_name
            .and_then(|name| hyprland_monitor_info(name).transpose())
            .transpose()?;

        let (geom_str, use_monitor) = if let Some(info) = monitor_info {
            let gx = (x - info.origin_x as f64).round() as i32;
            let gy = (y - info.origin_y as f64).round() as i32;
            let geom = format!(
                "{},{} {}x{}",
                gx,
                gy,
                width.round() as u32,
                height.round() as u32
            );
            (geom, Some(info.name))
        } else {
            let geom = format!(
                "{},{} {}x{}",
                x.round() as i32,
                y.round() as i32,
                width.round() as u32,
                height.round() as u32
            );
            (geom, None)
        };

        log::debug!(
            "Capturing active window via grim: {} (monitor {:?})",
            geom_str,
            use_monitor
        );
        let grim_output = {
            let mut cmd = Command::new("grim");
            if let Some(name) = use_monitor {
                cmd.args(["-o", &name]);
            }
            cmd.args(["-g", &geom_str, "-"])
                .stdout(Stdio::piped())
                .output()
                .map_err(|e| CaptureError::ImageError(format!("Failed to run grim: {}", e)))?
        };

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
            if output.status.code() == Some(1) {
                log::info!("Selection capture cancelled by user (slurp exit code 1)");
                return Err(CaptureError::Cancelled("Selection cancelled".into()));
            }
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

struct MonitorInfo {
    name: String,
    origin_x: i32,
    origin_y: i32,
}

fn hyprland_focused_monitor_name() -> Result<Option<String>, CaptureError> {
    use serde_json::Value;
    use std::process::{Command, Stdio};

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
        let focused = monitor
            .get("focused")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        if focused {
            if let Some(name) = monitor.get("name").and_then(|v| v.as_str()) {
                return Ok(Some(name.to_string()));
            }
        }
    }

    Ok(None)
}

fn hyprland_monitor_info(name: &str) -> Result<Option<MonitorInfo>, CaptureError> {
    use serde_json::Value;
    use std::process::{Command, Stdio};

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
        let name_match = monitor
            .get("name")
            .and_then(|v| v.as_str())
            .map(|n| n == name)
            .unwrap_or(false);
        if !name_match {
            continue;
        }
        let origin_x = monitor.get("x").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
        let origin_y = monitor.get("y").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
        return Ok(Some(MonitorInfo {
            name: name.to_string(),
            origin_x,
            origin_y,
        }));
    }

    Ok(None)
}
