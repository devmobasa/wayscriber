//! Clipboard integration for copying screenshots.

use super::{dependencies::CaptureClipboard, types::CaptureError};
use crate::process_broker::max_publish_bytes;
use std::ffi::OsStr;
use std::time::Duration;

const COPY_TIMEOUT: Duration = Duration::from_secs(10);
const VERIFY_TIMEOUT: Duration = Duration::from_secs(5);
const PERSIST_ATTEMPTS: usize = 2;
const MAX_VERIFY_OUTPUT_BYTES: usize = 64 * 1024 * 1024;

pub(super) struct BrokerClipboard;

impl CaptureClipboard for BrokerClipboard {
    fn copy(&self, image_data: &[u8]) -> Result<(), CaptureError> {
        copy_via_command(image_data).map_err(CaptureError::ClipboardError)
    }

    fn verify(&self, expected: &[u8], output_cap: usize) -> Result<bool, CaptureError> {
        read_back_via_command(output_cap)
            .map(|read_back| read_back == expected)
            .map_err(CaptureError::ClipboardError)
    }
}

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

    copy_to_clipboard_with(image_data, &BrokerClipboard, max_publish_bytes())
}

pub(super) fn copy_to_clipboard_with<C>(
    image_data: &[u8],
    commands: &C,
    publish_cap: usize,
) -> Result<(), CaptureError>
where
    C: CaptureClipboard + ?Sized,
{
    copy_to_clipboard_with_limits(image_data, commands, publish_cap, MAX_VERIFY_OUTPUT_BYTES)
}

fn copy_to_clipboard_with_limits<C>(
    image_data: &[u8],
    commands: &C,
    publish_cap: usize,
    verify_output_cap: usize,
) -> Result<(), CaptureError>
where
    C: CaptureClipboard + ?Sized,
{
    if image_data.len() > publish_cap {
        return Err(persistence_error(format!(
            "PNG payload is {} bytes, exceeding the {publish_cap}-byte clipboard publication limit",
            image_data.len()
        )));
    }

    let mut last_error = "clipboard verification did not match".to_string();
    for attempt in 0..PERSIST_ATTEMPTS {
        if let Err(error) = commands.copy(image_data) {
            last_error = clipboard_error_detail(error);
            log::warn!(
                "Clipboard publication attempt {}/{} failed: {}",
                attempt + 1,
                PERSIST_ATTEMPTS,
                last_error
            );
            continue;
        }

        if image_data.len() >= verify_output_cap {
            log::info!(
                "Published {} clipboard bytes; skipped read-back above the {}-byte verification threshold",
                image_data.len(),
                verify_output_cap
            );
            return Ok(());
        }

        match commands.verify(image_data, image_data.len() + 1) {
            Ok(true) => {
                log::info!("Successfully published and verified the clipboard image");
                return Ok(());
            }
            Ok(false) => {
                last_error = "clipboard verification did not match".to_string();
            }
            Err(error) => {
                last_error = clipboard_error_detail(error);
            }
        }

        log::warn!(
            "Clipboard verification attempt {}/{} failed: {}",
            attempt + 1,
            PERSIST_ATTEMPTS,
            last_error
        );
    }

    Err(persistence_error(last_error))
}

/// Copy to the clipboard through the brokered `wl-copy` helper.
fn copy_via_command(image_data: &[u8]) -> Result<(), String> {
    let output = crate::process_broker::current()
        .and_then(|broker| {
            broker.publish(
                crate::process_broker::HelperKind::WlCopy,
                OsStr::new("wl-copy"),
                [OsStr::new("--type"), OsStr::new("image/png")],
                image_data.to_vec(),
                COPY_TIMEOUT,
            )
        })
        .map_err(|error| format!("Failed to run wl-copy through the process broker: {error}"))?;
    if output.timed_out {
        return Err("wl-copy timed out".to_string());
    }
    if output.status != 0 {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return if stderr.is_empty() {
            Err("wl-copy exited unsuccessfully".to_string())
        } else {
            Err(format!("wl-copy exited unsuccessfully: {stderr}"))
        };
    }
    log::debug!("wl-copy command completed successfully");
    Ok(())
}

/// Read the published image back through brokered `wl-paste`.
fn read_back_via_command(output_cap: usize) -> Result<Vec<u8>, String> {
    let output = crate::process_broker::current()
        .and_then(|broker| {
            broker.run(
                crate::process_broker::HelperKind::WlPaste,
                OsStr::new("wl-paste"),
                clipboard_image_read_args(),
                Vec::new(),
                VERIFY_TIMEOUT,
                output_cap,
            )
        })
        .map_err(|error| format!("Failed to run wl-paste through the process broker: {error}"))?;
    if output.timed_out {
        return Err("wl-paste timed out".to_string());
    }
    if output.status != 0 {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return if stderr.is_empty() {
            Err("wl-paste exited unsuccessfully".to_string())
        } else {
            Err(format!("wl-paste exited unsuccessfully: {stderr}"))
        };
    }
    Ok(output.stdout)
}

fn clipboard_image_read_args() -> [&'static OsStr; 3] {
    [
        OsStr::new("--no-newline"),
        OsStr::new("--type"),
        OsStr::new("image/png"),
    ]
}

fn persistence_error(error: impl std::fmt::Display) -> CaptureError {
    CaptureError::ClipboardError(format!("Could not persist clipboard: {error}"))
}

fn clipboard_error_detail(error: CaptureError) -> String {
    match error {
        CaptureError::ClipboardError(message) => message,
        error => error.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;
    use std::sync::Mutex;

    #[derive(Default)]
    struct FakeClipboardCommands {
        copies: Mutex<Vec<Vec<u8>>>,
        copy_results: Mutex<VecDeque<Result<(), CaptureError>>>,
        verify_expected: Mutex<Vec<Vec<u8>>>,
        verify_caps: Mutex<Vec<usize>>,
        verify_results: Mutex<VecDeque<Result<bool, CaptureError>>>,
    }

    impl FakeClipboardCommands {
        fn with_results(
            copy_results: impl IntoIterator<Item = Result<(), CaptureError>>,
            verify_results: impl IntoIterator<Item = Result<bool, CaptureError>>,
        ) -> Self {
            Self {
                copy_results: Mutex::new(copy_results.into_iter().collect()),
                verify_results: Mutex::new(verify_results.into_iter().collect()),
                ..Self::default()
            }
        }
    }

    impl CaptureClipboard for FakeClipboardCommands {
        fn copy(&self, image_data: &[u8]) -> Result<(), CaptureError> {
            self.copies.lock().unwrap().push(image_data.to_vec());
            self.copy_results
                .lock()
                .unwrap()
                .pop_front()
                .expect("unexpected wl-copy call")
        }

        fn verify(&self, expected: &[u8], output_cap: usize) -> Result<bool, CaptureError> {
            self.verify_expected.lock().unwrap().push(expected.to_vec());
            self.verify_caps.lock().unwrap().push(output_cap);
            self.verify_results
                .lock()
                .unwrap()
                .pop_front()
                .expect("unexpected wl-paste call")
        }
    }

    #[test]
    fn verified_clipboard_accepts_an_exact_read_back() {
        let commands = FakeClipboardCommands::with_results([Ok(())], [Ok(true)]);

        copy_to_clipboard_with(b"data", &commands, max_publish_bytes()).unwrap();

        assert_eq!(&*commands.copies.lock().unwrap(), &[b"data".to_vec()]);
        assert_eq!(
            &*commands.verify_expected.lock().unwrap(),
            &[b"data".to_vec()]
        );
        assert_eq!(&*commands.verify_caps.lock().unwrap(), &[b"data".len() + 1]);
    }

    #[test]
    fn verified_clipboard_republishes_after_a_mismatch() {
        let commands = FakeClipboardCommands::with_results([Ok(()), Ok(())], [Ok(false), Ok(true)]);

        copy_to_clipboard_with(b"data", &commands, max_publish_bytes()).unwrap();

        assert_eq!(commands.copies.lock().unwrap().len(), 2);
        assert_eq!(&*commands.verify_caps.lock().unwrap(), &[5, 5]);
    }

    #[test]
    fn verified_clipboard_retries_the_complete_transaction_after_a_write_failure() {
        let commands = FakeClipboardCommands::with_results(
            [
                Err(CaptureError::ClipboardError("wl-copy failed".to_string())),
                Ok(()),
            ],
            [Ok(true)],
        );

        copy_to_clipboard_with(b"data", &commands, max_publish_bytes()).unwrap();

        assert_eq!(commands.copies.lock().unwrap().len(), 2);
        assert_eq!(&*commands.verify_caps.lock().unwrap(), &[5]);
    }

    #[test]
    fn verified_clipboard_reports_two_mismatches() {
        let commands =
            FakeClipboardCommands::with_results([Ok(()), Ok(())], [Ok(false), Ok(false)]);

        let error = copy_to_clipboard_with(b"data", &commands, max_publish_bytes())
            .expect_err("two mismatches must fail");

        assert_eq!(commands.copies.lock().unwrap().len(), 2);
        assert!(
            matches!(error, CaptureError::ClipboardError(message) if message == "Could not persist clipboard: clipboard verification did not match")
        );
    }

    #[test]
    fn verified_clipboard_retries_a_timeout_then_reports_it() {
        let commands = FakeClipboardCommands::with_results(
            [Ok(()), Ok(())],
            [
                Err(CaptureError::ClipboardError(
                    "wl-paste timed out".to_string(),
                )),
                Err(CaptureError::ClipboardError(
                    "wl-paste timed out".to_string(),
                )),
            ],
        );

        let error = copy_to_clipboard_with(b"data", &commands, max_publish_bytes())
            .expect_err("two verification timeouts must fail");

        assert_eq!(commands.copies.lock().unwrap().len(), 2);
        assert!(
            matches!(error, CaptureError::ClipboardError(message) if message == "Could not persist clipboard: wl-paste timed out")
        );
    }

    #[test]
    fn verified_clipboard_rejects_payloads_above_the_publication_cap() {
        let commands = FakeClipboardCommands::default();

        let error = copy_to_clipboard_with_limits(b"four", &commands, 3, 64)
            .expect_err("oversized clipboard payload must fail");

        assert!(commands.copies.lock().unwrap().is_empty());
        assert!(commands.verify_caps.lock().unwrap().is_empty());
        assert!(
            matches!(error, CaptureError::ClipboardError(message) if message.contains("Could not persist clipboard") && message.contains("3-byte"))
        );
    }

    #[test]
    fn verified_clipboard_skips_read_back_above_the_verification_threshold() {
        let commands = FakeClipboardCommands::with_results([Ok(())], []);

        copy_to_clipboard_with_limits(b"data", &commands, 8, 4).unwrap();

        assert_eq!(&*commands.copies.lock().unwrap(), &[b"data".to_vec()]);
        assert!(commands.verify_caps.lock().unwrap().is_empty());
    }

    #[test]
    fn image_read_back_requests_the_exact_png_mime_without_a_newline() {
        assert_eq!(
            clipboard_image_read_args(),
            [
                OsStr::new("--no-newline"),
                OsStr::new("--type"),
                OsStr::new("image/png"),
            ]
        );
    }

    #[test]
    fn publication_cap_retains_the_large_capture_limit() {
        assert_eq!(max_publish_bytes(), 256 * 1024 * 1024);
    }
}
