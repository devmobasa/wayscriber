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
pub(crate) fn copy_to_clipboard(
    process_broker: &crate::process_broker::ProcessBrokerHandle,
    image_data: &[u8],
) -> Result<(), CaptureError> {
    log::debug!(
        "Attempting to copy screenshot to clipboard ({} bytes)",
        image_data.len()
    );

    copy_to_clipboard_with(image_data, |image_data| {
        copy_via_command(process_broker, image_data)
    })
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
fn copy_via_command(
    process_broker: &crate::process_broker::ProcessBrokerHandle,
    image_data: &[u8],
) -> Result<(), CaptureError> {
    let output = process_broker
        .publish(
            crate::process_broker::HelperKind::WlCopy,
            OsStr::new("wl-copy"),
            [OsStr::new("--type"), OsStr::new("image/png")],
            image_data.to_vec(),
            Duration::from_secs(10),
        )
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

/// Check if clipboard functionality is available.
///
/// Tests if wl-copy command exists as a basic availability check.
#[allow(dead_code)] // Will be used in Phase 2 for capability checks
pub(crate) fn is_clipboard_available(
    process_broker: &crate::process_broker::ProcessBrokerHandle,
) -> bool {
    process_broker
        .run(
            crate::process_broker::HelperKind::WlCopy,
            OsStr::new("wl-copy"),
            [OsStr::new("--version")],
            Vec::new(),
            Duration::from_secs(2),
            4096,
        )
        .is_ok_and(|output| !output.timed_out && output.status == 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn copy_to_clipboard_uses_command_success() {
        let mut cmd_calls = 0;

        let result = copy_to_clipboard_with(b"data", |_| {
            cmd_calls += 1;
            Ok(())
        });

        assert!(result.is_ok());
        assert_eq!(cmd_calls, 1);
    }

    #[test]
    fn copy_to_clipboard_returns_command_error() {
        let mut cmd_calls = 0;

        let result = copy_to_clipboard_with(b"data", |_| {
            cmd_calls += 1;
            Err(CaptureError::ClipboardError("cmd failed".to_string()))
        });

        assert!(result.is_err());
        assert_eq!(cmd_calls, 1);
    }

    #[test]
    fn copy_to_clipboard_preserves_command_error() {
        let result = copy_to_clipboard_with(b"data", |_| {
            Err(CaptureError::ClipboardError("cmd failed".to_string()))
        })
        .expect_err("expected error");

        assert!(matches!(
            result,
            CaptureError::ClipboardError(message) if message.contains("cmd failed")
        ));
    }
}
