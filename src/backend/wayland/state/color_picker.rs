//! Clipboard helpers for color hex values.

use super::WaylandState;
use crate::input::state::UiToastKind;
use std::process::{Command, Stdio};

impl WaylandState {
    /// Copies the current color as hex to the clipboard.
    pub(in crate::backend::wayland) fn handle_copy_hex_color(&mut self) {
        let color = self.input_state.current_color;
        let hex = format!(
            "#{:02X}{:02X}{:02X}",
            (color.r * 255.0).round() as u8,
            (color.g * 255.0).round() as u8,
            (color.b * 255.0).round() as u8
        );

        // Use wl-copy for Wayland clipboard
        match Command::new("wl-copy")
            .arg(&hex)
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
        {
            Ok(mut child) => {
                // Don't wait for wl-copy to finish (it may stay running)
                std::thread::spawn(move || {
                    let _ = child.wait();
                });
                self.input_state
                    .set_ui_toast(UiToastKind::Info, format!("Copied {}", hex));
            }
            Err(e) => {
                log::warn!("Failed to copy hex color: {}", e);
                self.input_state
                    .set_ui_toast(UiToastKind::Warning, "Failed to copy (install wl-copy)");
            }
        }
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
                        let _ = self.input_state.apply_color_from_ui(crate::draw::Color {
                            r: color.0,
                            g: color.1,
                            b: color.2,
                            a: 1.0,
                        });
                        let hex = format!(
                            "#{:02X}{:02X}{:02X}",
                            (color.0 * 255.0).round() as u8,
                            (color.1 * 255.0).round() as u8,
                            (color.2 * 255.0).round() as u8
                        );
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

/// Parse a hex color string like "#FF8040" or "FF8040" to RGB values.
fn parse_hex_color(input: &str) -> Option<(f64, f64, f64)> {
    let hex = input.trim().trim_start_matches('#');

    // Support both 3-char (#RGB) and 6-char (#RRGGBB) formats
    let (r, g, b) = if hex.len() == 6 {
        let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
        let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
        let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
        (r, g, b)
    } else if hex.len() == 3 {
        let r = u8::from_str_radix(&hex[0..1], 16).ok()? * 17;
        let g = u8::from_str_radix(&hex[1..2], 16).ok()? * 17;
        let b = u8::from_str_radix(&hex[2..3], 16).ok()? * 17;
        (r, g, b)
    } else {
        return None;
    };

    Some((r as f64 / 255.0, g as f64 / 255.0, b as f64 / 255.0))
}
