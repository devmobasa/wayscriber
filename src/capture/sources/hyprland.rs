use tokio::task;

use crate::capture::types::CaptureError;
use crate::process_broker::{HelperKind, current};
use std::ffi::OsStr;
use std::time::Duration;

// Large, noisy multi-monitor PNGs can exceed the former 16 MiB transport cap.
// Keep capture bounded while allowing several uncompressed 8K-sized frames.
const CAPTURE_OUTPUT_CAP: usize = 256 * 1024 * 1024;

fn grim_geometry_arguments(geometry: &str) -> [&str; 3] {
    ["-g", geometry, "-"]
}

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
            HelperKind::HyprctlActiveWindow,
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
        let geometry = active_window_geometry(&json)?;

        log::debug!("Capturing active window via grim: {}", geometry);
        let arguments = grim_geometry_arguments(&geometry);
        let grim_output = run_helper(
            HelperKind::Grim,
            "grim",
            &arguments,
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
        let arguments = grim_geometry_arguments(geometry);
        let grim_output = run_helper(
            HelperKind::Grim,
            "grim",
            &arguments,
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

/// Build the `grim -g` region for the window described by `hyprctl activewindow -j`.
///
/// Hyprland reports `at` and `size` in layout (logical) coordinates, the same
/// space `grim -g` reads, so they pass through unscaled like the `slurp` path.
fn active_window_geometry(json: &serde_json::Value) -> Result<String, CaptureError> {
    let at = json
        .get("at")
        .and_then(|v| v.as_array())
        .ok_or_else(|| CaptureError::InvalidResponse("Missing 'at' in hyprctl output".into()))?;
    let size = json
        .get("size")
        .and_then(|v| v.as_array())
        .ok_or_else(|| CaptureError::InvalidResponse("Missing 'size' in hyprctl output".into()))?;

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

    Ok(format!(
        "{},{} {}x{}",
        x.round() as i32,
        y.round() as i32,
        width.round() as u32,
        height.round() as u32
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn active_window_geometry_keeps_layout_coordinates_on_a_scaled_monitor() {
        // Hyprland reports `at` and `size` in layout coordinates, the same
        // space `grim -g` reads, so a window on a scale-2 monitor is captured
        // at its reported position and size.
        let window = serde_json::json!({
            "at": [100, 100],
            "size": [800, 600],
            "monitor": 1,
        });

        let geometry = active_window_geometry(&window).unwrap();

        assert_eq!(geometry, "100,100 800x600");
    }

    #[test]
    fn grim_geometry_arguments_are_explicit() {
        assert_eq!(
            grim_geometry_arguments("12,34 800x600"),
            ["-g", "12,34 800x600", "-"]
        );
    }
}
