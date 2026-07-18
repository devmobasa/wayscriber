use tokio::task;

use crate::capture::types::CaptureError;
use crate::process_broker::{HelperKind, current};
use std::ffi::OsStr;
use std::time::Duration;

// Large, noisy multi-monitor PNGs can exceed the former 16 MiB transport cap.
// Keep capture bounded while allowing several uncompressed 8K-sized frames.
const CAPTURE_OUTPUT_CAP: usize = 256 * 1024 * 1024;

fn run_helper(
    kind: HelperKind,
    program: &str,
    arguments: &[&str],
    timeout: Duration,
    output_cap: usize,
) -> Result<crate::process_broker::BrokerOutput, CaptureError> {
    current()
        .and_then(|broker| {
            broker.run(
                kind,
                OsStr::new(program),
                arguments.iter().map(OsStr::new),
                Vec::new(),
                timeout,
                output_cap,
            )
        })
        .map_err(|error| CaptureError::ImageError(format!("failed to run {program}: {error:#}")))
}

/// Capture the entire Wayland scene using `grim`.
pub async fn capture_full_screen_hyprland() -> Result<Vec<u8>, CaptureError> {
    task::spawn_blocking(|| -> Result<Vec<u8>, CaptureError> {
        log::debug!("Capturing full screen via grim");
        let output = run_helper(
            HelperKind::Grim,
            "grim",
            &["-"],
            Duration::from_secs(30),
            CAPTURE_OUTPUT_CAP,
        )?;

        if output.timed_out || output.status != 0 {
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

        // Query Hyprland for the active window geometry
        let output = run_helper(
            HelperKind::Hyprctl,
            "hyprctl",
            &["activewindow", "-j"],
            Duration::from_secs(5),
            2 * 1024 * 1024,
        )?;

        if output.timed_out || output.status != 0 {
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

        if let Some(scale) = hyprland_monitor_scale(monitor_id, monitor_name)?
            && (scale - 1.0).abs() > f64::EPSILON
        {
            log::debug!(
                "Applying monitor scale {:.2} to active window capture",
                scale
            );
            x *= scale;
            y *= scale;
            width *= scale;
            height *= scale;
        }

        let geometry = format!(
            "{},{} {}x{}",
            x.round() as i32,
            y.round() as i32,
            width.round() as u32,
            height.round() as u32
        );

        log::debug!("Capturing active window via grim: {}", geometry);
        let grim_output = run_helper(
            HelperKind::Grim,
            "grim",
            &["-g", &geometry, "-"],
            Duration::from_secs(30),
            CAPTURE_OUTPUT_CAP,
        )?;

        if grim_output.timed_out || grim_output.status != 0 {
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
        // `slurp` outputs geometry in the format "x,y widthxheight"
        let output = run_helper(
            HelperKind::Slurp,
            "slurp",
            &["-f", "%x,%y %wx%h"],
            Duration::from_secs(120),
            4096,
        )?;

        if output.timed_out || output.status != 0 {
            if output.status == 1 {
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
        let grim_output = run_helper(
            HelperKind::Grim,
            "grim",
            &["-g", geometry, "-"],
            Duration::from_secs(30),
            CAPTURE_OUTPUT_CAP,
        )?;

        if grim_output.timed_out || grim_output.status != 0 {
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

    if monitor_id.is_none() && monitor_name.is_none() {
        return Ok(None);
    }

    let output = run_helper(
        HelperKind::Hyprctl,
        "hyprctl",
        &["monitors", "-j"],
        Duration::from_secs(5),
        2 * 1024 * 1024,
    )?;

    if output.timed_out || output.status != 0 {
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
