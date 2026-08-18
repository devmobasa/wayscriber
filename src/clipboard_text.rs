//! Generic system text-clipboard commands (`wl-copy` / `wl-paste`).
//!
//! Blocking process-broker calls with no Wayland state of their own, shared by
//! the hex-color picker, the text editor, and screen text recognition. Every
//! caller runs them on a worker thread; none of them may run on the event loop.

use std::ffi::OsStr;
use std::time::Duration;

pub(crate) enum ClipboardTextError {
    Empty,
    Other(String),
}

pub(crate) fn copy_text_via_command(text: &str) -> Result<(), String> {
    let output = crate::process_broker::current()
        .and_then(|broker| {
            broker.publish(
                crate::process_broker::HelperKind::WlCopy,
                OsStr::new("wl-copy"),
                clipboard_text_copy_args(),
                text.as_bytes().to_vec(),
                Duration::from_secs(5),
            )
        })
        .map_err(|error| format!("Failed to run wl-copy: {error:#}"))?;
    if output.timed_out {
        return Err("wl-copy timed out".to_string());
    }
    if output.status != 0 {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        return if stderr.is_empty() {
            Err("wl-copy exited unsuccessfully".to_string())
        } else {
            Err(format!("wl-copy exited unsuccessfully: {stderr}"))
        };
    }
    Ok(())
}

fn clipboard_text_copy_args() -> [&'static OsStr; 2] {
    [OsStr::new("--type"), OsStr::new("text/plain;charset=utf-8")]
}

pub(crate) fn read_clipboard_text_via_command() -> Result<String, ClipboardTextError> {
    let output = crate::process_broker::current()
        .and_then(|broker| {
            broker.run(
                crate::process_broker::HelperKind::WlPaste,
                OsStr::new("wl-paste"),
                clipboard_text_read_args(),
                Vec::new(),
                Duration::from_secs(5),
                1024 * 1024,
            )
        })
        .map_err(|err| ClipboardTextError::Other(format!("Failed to run wl-paste: {err:#}")))?;

    if !output.timed_out && output.status == 0 {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        if stderr.to_ascii_lowercase().contains("nothing is copied")
            || stderr.to_ascii_lowercase().contains("clipboard is empty")
        {
            Err(ClipboardTextError::Empty)
        } else if stderr.is_empty() {
            Err(ClipboardTextError::Other(
                "wl-paste exited unsuccessfully".to_string(),
            ))
        } else {
            Err(ClipboardTextError::Other(format!(
                "wl-paste exited unsuccessfully: {}",
                stderr
            )))
        }
    }
}

fn clipboard_text_read_args() -> [&'static OsStr; 3] {
    [
        OsStr::new("--no-newline"),
        OsStr::new("--type"),
        OsStr::new("text"),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clipboard_text_read_requests_only_a_text_mime() {
        assert_eq!(
            clipboard_text_read_args(),
            [
                OsStr::new("--no-newline"),
                OsStr::new("--type"),
                OsStr::new("text"),
            ]
        );
    }

    #[test]
    fn clipboard_text_copy_publishes_one_explicit_utf8_mime() {
        assert_eq!(
            clipboard_text_copy_args(),
            [OsStr::new("--type"), OsStr::new("text/plain;charset=utf-8"),]
        );
    }
}
