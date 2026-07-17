//! Clipboard helpers for color hex values.

use super::WaylandState;
use crate::input::state::UiToastKind;
use crate::input::state::{color_to_hex, parse_hex_color};
use std::ffi::OsStr;
use std::time::Duration;

impl WaylandState {
    /// Copies the current color as hex to the clipboard.
    pub(in crate::backend::wayland) fn handle_copy_hex_color(&mut self) {
        let color = self
            .input_state
            .color_for_tool(self.input_state.active_tool());
        let hex = color_to_hex(color);
        log::info!("Hex copy requested: {}", hex);
        self.suppress_focus_exit_for(Duration::from_millis(1500));

        let copied = match std::panic::catch_unwind(|| match copy_hex_via_command(&hex) {
            Ok(()) => true,
            Err(err) => {
                log::warn!("wl-copy failed for hex copy: {}", err);
                false
            }
        }) {
            Ok(result) => result,
            Err(_) => {
                log::error!("Hex copy panicked");
                false
            }
        };

        if copied {
            self.input_state
                .set_ui_toast(UiToastKind::Info, format!("Copied {}", hex));
        } else {
            self.input_state
                .set_ui_toast(UiToastKind::Warning, "Failed to copy to clipboard");
        }
    }

    /// Pastes a hex color from the clipboard.
    pub(in crate::backend::wayland) fn handle_paste_hex_color(&mut self) {
        log::info!("Hex paste requested");
        self.suppress_focus_exit_for(Duration::from_millis(1500));
        let clipboard = match std::panic::catch_unwind(read_clipboard_text_via_command) {
            Ok(Ok(text)) => text,
            Ok(Err(ClipboardTextError::Empty)) => {
                self.input_state
                    .set_ui_toast(UiToastKind::Warning, "Clipboard empty");
                return;
            }
            Ok(Err(ClipboardTextError::Other(err))) => {
                log::warn!("wl-paste failed for hex paste: {}", err);
                self.input_state
                    .set_ui_toast(UiToastKind::Warning, "Failed to paste from clipboard");
                return;
            }
            Err(_) => {
                log::error!("Hex paste panicked");
                self.input_state
                    .set_ui_toast(UiToastKind::Warning, "Failed to paste from clipboard");
                return;
            }
        };

        if let Some(color) = parse_hex_color(clipboard.trim()) {
            let _ = self.input_state.apply_color_from_ui(color);
            let hex = color_to_hex(color);
            self.input_state
                .set_ui_toast(UiToastKind::Info, format!("Pasted {}", hex));
        } else {
            self.input_state.set_ui_toast(
                UiToastKind::Warning,
                format!(
                    "Invalid hex: {}",
                    clipboard.chars().take(20).collect::<String>()
                ),
            );
        }
    }
}

enum ClipboardTextError {
    Empty,
    Other(String),
}

fn copy_hex_via_command(hex: &str) -> Result<(), String> {
    let output = crate::process_broker::current()
        .and_then(|broker| {
            broker.publish(
                crate::process_broker::HelperKind::WlCopy,
                OsStr::new("wl-copy"),
                [OsStr::new("--type"), OsStr::new("text/plain;charset=utf-8")],
                hex.as_bytes().to_vec(),
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

fn read_clipboard_text_via_command() -> Result<String, ClipboardTextError> {
    let output = crate::process_broker::current()
        .and_then(|broker| {
            broker.run(
                crate::process_broker::HelperKind::WlPaste,
                OsStr::new("wl-paste"),
                [OsStr::new("--no-newline")],
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
