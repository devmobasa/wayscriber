//! Clipboard integration for copying screenshots.

use super::types::CaptureError;
use std::process::{Command, Stdio};
use wl_clipboard_rs::copy::{MimeType, Options, ServeRequests, Source};

/// Copy image data to the Wayland clipboard.
///
/// Attempts to use wl-copy first, falls back to wl-clipboard-rs
/// if the command path fails.
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

    copy_to_clipboard_with(image_data, copy_via_command, copy_via_library)
}

fn copy_to_clipboard_with<F, G>(
    image_data: &[u8],
    copy_cmd: F,
    copy_lib: G,
) -> Result<(), CaptureError>
where
    F: FnOnce(&[u8]) -> Result<(), CaptureError>,
    G: FnOnce(&[u8]) -> Result<(), CaptureError>,
{
    // Prefer wl-copy CLI; fall back to wl-clipboard-rs if needed.
    match copy_cmd(image_data) {
        Ok(()) => {
            log::info!("Successfully copied to clipboard via wl-copy command");
            Ok(())
        }
        Err(cmd_err) => {
            log::warn!(
                "wl-copy command path failed ({}). Falling back to wl-clipboard-rs",
                cmd_err
            );
            match copy_lib(image_data) {
                Ok(()) => {
                    log::info!("Successfully copied to clipboard via wl-clipboard-rs fallback");
                    Ok(())
                }
                Err(lib_err) => {
                    let combined = format!(
                        "wl-copy failed: {} ; wl-clipboard-rs failed: {}",
                        cmd_err, lib_err
                    );
                    Err(CaptureError::ClipboardError(combined))
                }
            }
        }
    }
}

/// Copy to clipboard using wl-clipboard-rs library.
fn copy_via_library(image_data: &[u8]) -> Result<(), CaptureError> {
    let mut opts = Options::new();
    // Serve requests until clipboard ownership changes.
    opts.serve_requests(ServeRequests::Unlimited);

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
    fn copy_to_clipboard_prefers_command_success() {
        let cmd_calls = Rc::new(Cell::new(0));
        let lib_calls = Rc::new(Cell::new(0));
        let cmd_calls_handle = cmd_calls.clone();
        let lib_calls_handle = lib_calls.clone();

        let result = copy_to_clipboard_with(
            b"data",
            move |_| {
                cmd_calls_handle.set(cmd_calls_handle.get() + 1);
                Ok(())
            },
            move |_| {
                lib_calls_handle.set(lib_calls_handle.get() + 1);
                Ok(())
            },
        );

        assert!(result.is_ok());
        assert_eq!(cmd_calls.get(), 1);
        assert_eq!(lib_calls.get(), 0);
    }

    #[test]
    fn copy_to_clipboard_falls_back_when_command_fails() {
        let cmd_calls = Rc::new(Cell::new(0));
        let lib_calls = Rc::new(Cell::new(0));
        let cmd_calls_handle = cmd_calls.clone();
        let lib_calls_handle = lib_calls.clone();

        let result = copy_to_clipboard_with(
            b"data",
            move |_| {
                cmd_calls_handle.set(cmd_calls_handle.get() + 1);
                Err(CaptureError::ClipboardError("cmd failed".to_string()))
            },
            move |_| {
                lib_calls_handle.set(lib_calls_handle.get() + 1);
                Ok(())
            },
        );

        assert!(result.is_ok());
        assert_eq!(cmd_calls.get(), 1);
        assert_eq!(lib_calls.get(), 1);
    }

    #[test]
    fn copy_to_clipboard_combines_errors() {
        let result = copy_to_clipboard_with(
            b"data",
            |_| Err(CaptureError::ClipboardError("cmd failed".to_string())),
            |_| Err(CaptureError::ClipboardError("lib failed".to_string())),
        )
        .expect_err("expected error");

        match result {
            CaptureError::ClipboardError(msg) => {
                assert!(msg.contains("wl-copy failed"));
                assert!(msg.contains("wl-clipboard-rs failed"));
                assert!(msg.contains("cmd failed"));
                assert!(msg.contains("lib failed"));
            }
            other => panic!("unexpected error variant: {other:?}"),
        }
    }
}
