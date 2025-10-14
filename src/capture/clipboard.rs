//! Clipboard integration for copying screenshots.

use super::types::CaptureError;
use std::process::{Command, Stdio};
use wl_clipboard_rs::copy::{MimeType, Options, Source};

/// Copy image data to the Wayland clipboard.
///
/// Attempts to use wl-clipboard-rs library first, falls back to
/// wl-copy command if the library fails.
///
/// # Arguments
/// * `image_data` - Raw PNG image bytes
///
/// # Returns
/// Ok(()) if successful, error otherwise
pub fn copy_to_clipboard(image_data: &[u8]) -> Result<(), CaptureError> {
    log::debug!(
        "Attempting to copy screenshot to clipboard ({} bytes)",
        image_data.len()
    );

    // Try wl-clipboard-rs first
    match copy_via_library(image_data) {
        Ok(()) => {
            log::info!("Successfully copied to clipboard via wl-clipboard-rs");
            Ok(())
        }
        Err(e) => {
            log::warn!("wl-clipboard-rs failed: {}, trying wl-copy fallback", e);
            copy_via_command(image_data)
        }
    }
}

/// Copy to clipboard using wl-clipboard-rs library.
fn copy_via_library(image_data: &[u8]) -> Result<(), CaptureError> {
    use wl_clipboard_rs::copy::ServeRequests;

    let mut opts = Options::new();

    // Serve clipboard requests until paste or replacement
    // This keeps the clipboard data available after our process exits
    opts.serve_requests(ServeRequests::Only(1)); // Serve one paste then exit

    opts.copy(
        Source::Bytes(image_data.into()),
        MimeType::Specific("image/png".to_string()),
    )
    .map_err(|e| CaptureError::ClipboardError(format!("wl-clipboard-rs error: {}", e)))?;

    Ok(())
}

/// Copy to clipboard by shelling out to wl-copy command.
fn copy_via_command(image_data: &[u8]) -> Result<(), CaptureError> {
    use std::io::Write;

    let mut child = Command::new("wl-copy")
        .arg("--type")
        .arg("image/png")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| {
            CaptureError::ClipboardError(format!(
                "Failed to spawn wl-copy (is it installed?): {}",
                e
            ))
        })?;

    // Write image data to stdin
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(image_data).map_err(|e| {
            CaptureError::ClipboardError(format!("Failed to write to wl-copy stdin: {}", e))
        })?;
    }

    // Wait for completion
    let output = child
        .wait_with_output()
        .map_err(|e| CaptureError::ClipboardError(format!("Failed to wait for wl-copy: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(CaptureError::ClipboardError(format!(
            "wl-copy failed: {}",
            stderr
        )));
    }

    log::info!("Successfully copied to clipboard via wl-copy command");
    Ok(())
}

/// Check if clipboard functionality is available.
///
/// Tests if wl-copy command exists as a basic availability check.
#[allow(dead_code)] // Will be used in Phase 2 for capability checks
pub fn is_clipboard_available() -> bool {
    Command::new("wl-copy")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_clipboard_available() {
        // This test will pass or fail depending on system setup
        // Just ensure it doesn't panic
        let _available = is_clipboard_available();
    }
}
