//! Clipboard helpers for color hex values.

use super::WaylandState;
use crate::input::state::UiToastKind;
use crate::input::state::{color_to_hex, parse_hex_color};
use std::io::{Read, Write};
use std::process::{Command, Stdio};
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
    let mut child = Command::new("wl-copy")
        .arg("--type")
        .arg("text/plain;charset=utf-8")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to spawn wl-copy: {}", e))?;

    if let Some(mut stdin) = child.stdin.take() {
        if let Err(err) = stdin.write_all(hex.as_bytes()) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(format!("Failed to write to wl-copy stdin: {}", err));
        }
    } else {
        let _ = child.kill();
        let _ = child.wait();
        return Err("wl-copy stdin unavailable".to_string());
    }

    match child.try_wait() {
        Ok(Some(status)) => {
            if !status.success() {
                let stderr = child
                    .stderr
                    .take()
                    .and_then(|mut err| {
                        let mut buf = Vec::new();
                        let _ = err.read_to_end(&mut buf);
                        if buf.is_empty() {
                            None
                        } else {
                            Some(String::from_utf8_lossy(&buf).trim().to_string())
                        }
                    })
                    .unwrap_or_default();
                if stderr.is_empty() {
                    return Err("wl-copy exited unsuccessfully".to_string());
                }
                return Err(format!("wl-copy exited unsuccessfully: {}", stderr));
            }
            Ok(())
        }
        Ok(None) => {
            std::thread::spawn(move || match child.wait() {
                Ok(status) => {
                    if !status.success() {
                        let stderr = child
                            .stderr
                            .take()
                            .and_then(|mut err| {
                                let mut buf = Vec::new();
                                let _ = err.read_to_end(&mut buf);
                                if buf.is_empty() {
                                    None
                                } else {
                                    Some(String::from_utf8_lossy(&buf).trim().to_string())
                                }
                            })
                            .unwrap_or_default();
                        if stderr.is_empty() {
                            log::warn!("wl-copy exited unsuccessfully");
                        } else {
                            log::warn!("wl-copy failed: {}", stderr);
                        }
                    }
                }
                Err(err) => {
                    log::warn!("Failed to wait for wl-copy: {}", err);
                }
            });
            Ok(())
        }
        Err(err) => Err(format!("Failed to poll wl-copy status: {}", err)),
    }
}

fn read_clipboard_text_via_command() -> Result<String, ClipboardTextError> {
    let output = Command::new("wl-paste")
        .arg("--no-newline")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|err| ClipboardTextError::Other(format!("Failed to spawn wl-paste: {}", err)))?;

    if output.status.success() {
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
