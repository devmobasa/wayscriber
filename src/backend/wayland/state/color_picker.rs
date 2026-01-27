//! Clipboard helpers for color hex values.

use super::WaylandState;
use crate::input::state::UiToastKind;
use crate::input::state::{color_to_hex, parse_hex_color};
use std::io::{Read, Write};
use std::process::{Command, Stdio};

impl WaylandState {
    /// Copies the current color as hex to the clipboard.
    pub(in crate::backend::wayland) fn handle_copy_hex_color(&mut self) {
        let color = self.input_state.current_color;
        let hex = color_to_hex(color);

        let mut child = match Command::new("wl-copy")
            .arg("--type")
            .arg("text/plain;charset=utf-8")
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
        {
            Ok(child) => child,
            Err(e) => {
                log::warn!("Failed to copy hex color: {}", e);
                self.input_state
                    .set_ui_toast(UiToastKind::Warning, "Failed to copy (install wl-copy)");
                return;
            }
        };

        if let Some(mut stdin) = child.stdin.take() {
            if let Err(err) = stdin.write_all(hex.as_bytes()) {
                log::warn!("Failed to write to wl-copy stdin: {}", err);
                let _ = child.kill();
                let _ = child.wait();
                self.input_state
                    .set_ui_toast(UiToastKind::Warning, "Failed to copy to clipboard");
                return;
            }
        } else {
            log::warn!("wl-copy stdin unavailable for hex copy");
            let _ = child.kill();
            let _ = child.wait();
            self.input_state
                .set_ui_toast(UiToastKind::Warning, "Failed to copy to clipboard");
            return;
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
                    log::warn!("wl-copy exited unsuccessfully: {}", stderr);
                    self.input_state
                        .set_ui_toast(UiToastKind::Warning, "Failed to copy to clipboard");
                    return;
                }
            }
            Ok(None) => {
                std::thread::spawn(move || match child.wait_with_output() {
                    Ok(output) => {
                        if !output.status.success() {
                            let stderr = String::from_utf8_lossy(&output.stderr);
                            log::warn!("wl-copy failed: {}", stderr.trim());
                        }
                    }
                    Err(err) => {
                        log::warn!("Failed to wait for wl-copy: {}", err);
                    }
                });
            }
            Err(err) => {
                log::warn!("Failed to poll wl-copy status: {}", err);
                self.input_state
                    .set_ui_toast(UiToastKind::Warning, "Failed to copy to clipboard");
                return;
            }
        }

        self.input_state
            .set_ui_toast(UiToastKind::Info, format!("Copied {}", hex));
    }

    /// Pastes a hex color from the clipboard.
    pub(in crate::backend::wayland) fn handle_paste_hex_color(&mut self) {
        // Use wl-paste for Wayland clipboard
        match Command::new("wl-paste")
            .arg("--no-newline")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
        {
            Ok(output) => {
                if output.status.success() {
                    let clipboard = String::from_utf8_lossy(&output.stdout);
                    if let Some(color) = parse_hex_color(clipboard.trim()) {
                        let _ = self.input_state.apply_color_from_ui(color);
                        let hex = color_to_hex(color);
                        self.input_state
                            .set_ui_toast(UiToastKind::Info, format!("Pasted {}", hex));
                        self.save_drawing_preferences();
                    } else {
                        self.input_state.set_ui_toast(
                            UiToastKind::Warning,
                            format!(
                                "Invalid hex: {}",
                                clipboard.chars().take(20).collect::<String>()
                            ),
                        );
                    }
                } else {
                    self.input_state
                        .set_ui_toast(UiToastKind::Warning, "Clipboard empty");
                }
            }
            Err(e) => {
                log::warn!("Failed to paste hex color: {}", e);
                self.input_state
                    .set_ui_toast(UiToastKind::Warning, "Failed to paste (install wl-paste)");
            }
        }
    }
}
