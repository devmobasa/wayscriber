//! Clipboard integration for copying screenshots.

use super::types::CaptureError;
use std::process::{Command, Stdio};

/// Copy image data to the Wayland clipboard.
///
/// Uses `wl-copy`, which is already a packaged/runtime dependency.
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

    copy_to_clipboard_with(image_data, copy_via_command)
}

fn copy_to_clipboard_with<F>(image_data: &[u8], copy_cmd: F) -> Result<(), CaptureError>
where
    F: FnOnce(&[u8]) -> Result<(), CaptureError>,
{
    copy_cmd(image_data)?;
    log::info!("Successfully copied to clipboard via wl-copy command");
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

    match child.try_wait() {
        Ok(Some(status)) => {
            if !status.success() {
                return Err(CaptureError::ClipboardError(
                    "wl-copy exited unsuccessfully".to_string(),
                ));
            }
            log::debug!("wl-copy command completed successfully");
            Ok(())
        }
        Ok(None) => {
            // Wait in the background so we don't block the capture pipeline.
            std::thread::spawn(move || match child.wait_with_output() {
                Ok(output) => {
                    if !output.status.success() {
                        let stderr = String::from_utf8_lossy(&output.stderr);
                        log::warn!("wl-copy failed: {}", stderr.trim());
                    } else {
                        log::debug!("wl-copy command completed successfully");
                    }
                }
                Err(err) => {
                    log::warn!("Failed to wait for wl-copy: {}", err);
                }
            });
            Ok(())
        }
        Err(err) => Err(CaptureError::ClipboardError(format!(
            "Failed to poll wl-copy status: {}",
            err
        ))),
    }
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
    use std::cell::Cell;
    use std::rc::Rc;

    #[test]
    fn test_is_clipboard_available() {
        // This test will pass or fail depending on system setup
        // Just ensure it doesn't panic
        let _available = is_clipboard_available();
    }

    #[test]
    fn copy_to_clipboard_uses_command_success() {
        let cmd_calls = Rc::new(Cell::new(0));
        let cmd_calls_handle = cmd_calls.clone();

        let result = copy_to_clipboard_with(b"data", move |_| {
            cmd_calls_handle.set(cmd_calls_handle.get() + 1);
            Ok(())
        });

        assert!(result.is_ok());
        assert_eq!(cmd_calls.get(), 1);
    }

    #[test]
    fn copy_to_clipboard_returns_command_error() {
        let cmd_calls = Rc::new(Cell::new(0));
        let cmd_calls_handle = cmd_calls.clone();

        let result = copy_to_clipboard_with(b"data", move |_| {
            cmd_calls_handle.set(cmd_calls_handle.get() + 1);
            Err(CaptureError::ClipboardError("cmd failed".to_string()))
        });

        assert!(result.is_err());
        assert_eq!(cmd_calls.get(), 1);
    }

    #[test]
    fn copy_to_clipboard_preserves_command_error() {
        let result = copy_to_clipboard_with(b"data", |_| {
            Err(CaptureError::ClipboardError("cmd failed".to_string()))
        })
        .expect_err("expected error");

        match result {
            CaptureError::ClipboardError(msg) => {
                assert!(msg.contains("cmd failed"));
            }
            other => panic!("unexpected error variant: {other:?}"),
        }
    }
}
