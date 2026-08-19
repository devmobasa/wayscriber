//! Clipboard integration for copying screenshots.

use super::types::CaptureError;
use std::ffi::OsStr;
use std::time::Duration;

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

/// Copy to the clipboard through the brokered `wl-copy` helper.
fn copy_via_command(image_data: &[u8]) -> Result<(), CaptureError> {
    let output = crate::process_broker::current()
        .and_then(|broker| {
            broker.publish(
                crate::process_broker::HelperKind::WlCopy,
                OsStr::new("wl-copy"),
                [OsStr::new("--type"), OsStr::new("image/png")],
                image_data.to_vec(),
                Duration::from_secs(10),
            )
        })
        .map_err(|e| {
            CaptureError::ClipboardError(format!(
                "Failed to run wl-copy through the process broker: {}",
                e
            ))
        })?;
    if output.timed_out {
        return Err(CaptureError::ClipboardError(
            "wl-copy timed out".to_string(),
        ));
    }
    if output.status != 0 {
        return Err(CaptureError::ClipboardError(format!(
            "wl-copy exited unsuccessfully: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    log::debug!("wl-copy command completed successfully");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::rc::Rc;

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
