//! Screen color picker with multiple backend support.
//!
//! Tries in order:
//! 1. xdg-desktop-portal PickColor (GNOME, KDE with proper portal)
//! 2. hyprpicker (Hyprland)
//! 3. grim + slurp (wlroots compositors)
//! 4. Shows helpful error message

use super::{ColorPickOutcome, OverlaySuppression, WaylandState};
use crate::input::state::UiToastKind;
use std::io::Cursor;
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
                        self.input_state
                            .apply_picked_color(color.0, color.1, color.2);
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

    /// Handles the color pick action by trying multiple backends.
    pub(in crate::backend::wayland) fn handle_color_pick(&mut self) {
        log::info!("Color pick requested");

        if self.overlay_blocks_event_loop() {
            self.input_state.set_ui_toast(
                UiToastKind::Warning,
                "Finish current capture/zoom before picking a color",
            );
            return;
        }

        if self.pending_color_pick_result.is_some() {
            self.input_state
                .set_ui_toast(UiToastKind::Info, "Color picker already active");
            return;
        }

        self.enter_overlay_suppression(OverlaySuppression::ColorPick);

        // Spawn thread to try color picking methods
        let (tx, rx) = std::sync::mpsc::channel();

        std::thread::spawn(move || {
            let result = pick_color_with_fallbacks();
            let _ = tx.send(result);
        });

        // Store the receiver to check for completion in the event loop
        self.pending_color_pick_result = Some(rx);
    }

    /// Check if a pending color pick has completed and apply the result.
    pub(in crate::backend::wayland) fn check_pending_color_pick(&mut self) {
        let Some(rx) = self.pending_color_pick_result.take() else {
            return;
        };

        match rx.try_recv() {
            Ok(outcome) => {
                self.input_state.ignore_exit_until =
                    Some(std::time::Instant::now() + std::time::Duration::from_millis(500));
                self.exit_overlay_suppression(OverlaySuppression::ColorPick);
                match outcome {
                    ColorPickOutcome::Success(r, g, b) => {
                        log::info!("Color picked: ({:.3}, {:.3}, {:.3})", r, g, b);
                        self.input_state.apply_picked_color(r, g, b);

                        // Show a toast with the picked color
                        let hex = format!(
                            "#{:02X}{:02X}{:02X}",
                            (r * 255.0).round() as u8,
                            (g * 255.0).round() as u8,
                            (b * 255.0).round() as u8
                        );
                        self.input_state
                            .set_ui_toast(UiToastKind::Info, format!("Picked {}", hex));

                        // Mark toolbar dirty so color preview updates
                        self.toolbar.mark_dirty();
                        // Save the picked color to preferences
                        self.save_drawing_preferences();
                    }
                    ColorPickOutcome::Cancelled => {
                        log::info!("Color picker cancelled by user");
                        // Don't show a toast for user cancellation
                    }
                    ColorPickOutcome::NotAvailable(msg) => {
                        log::warn!("Color picker not available: {}", msg);
                        self.input_state.set_ui_toast(
                            UiToastKind::Warning,
                            "Install hyprpicker for color picking",
                        );
                    }
                    ColorPickOutcome::Failed(msg) => {
                        log::error!("Color picker failed: {}", msg);
                        self.input_state
                            .set_ui_toast(UiToastKind::Warning, "Color pick failed");
                    }
                }
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {
                // Still waiting, put it back
                self.pending_color_pick_result = Some(rx);
            }
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                log::error!("Color picker thread disconnected unexpectedly");
                self.input_state.ignore_exit_until =
                    Some(std::time::Instant::now() + std::time::Duration::from_millis(500));
                self.exit_overlay_suppression(OverlaySuppression::ColorPick);
                self.input_state
                    .set_ui_toast(UiToastKind::Warning, "Color pick failed");
            }
        }
    }
}

/// Try multiple color picker methods in order of preference.
fn pick_color_with_fallbacks() -> ColorPickOutcome {
    // Try portal first (works on GNOME, KDE)
    log::debug!("Trying xdg-desktop-portal color picker...");
    match try_portal_color_picker() {
        Ok(color) => return ColorPickOutcome::Success(color.0, color.1, color.2),
        Err(PickError::Cancelled) => return ColorPickOutcome::Cancelled,
        Err(e) => log::debug!("Portal color picker not available: {:?}", e),
    }

    // Try hyprpicker (Hyprland)
    log::debug!("Trying hyprpicker...");
    match try_hyprpicker() {
        Ok(color) => return ColorPickOutcome::Success(color.0, color.1, color.2),
        Err(PickError::Cancelled) => return ColorPickOutcome::Cancelled,
        Err(e) => log::debug!("hyprpicker not available: {:?}", e),
    }

    // Try grim with slurp for point selection (wlroots compositors)
    log::debug!("Trying grim...");
    match try_grim_picker() {
        Ok(color) => return ColorPickOutcome::Success(color.0, color.1, color.2),
        Err(PickError::Cancelled) => return ColorPickOutcome::Cancelled,
        Err(e) => log::debug!("grim not available: {:?}", e),
    }

    ColorPickOutcome::NotAvailable(
        "No color picker available. Install hyprpicker or ensure xdg-desktop-portal is configured."
            .to_string(),
    )
}

/// Internal error type for pick operations.
#[derive(Debug)]
#[allow(dead_code)] // String fields are for debugging
enum PickError {
    Cancelled,
    NotAvailable(String),
    Failed(String),
}

/// Try the xdg-desktop-portal PickColor method.
fn try_portal_color_picker() -> Result<(f64, f64, f64), PickError> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| PickError::Failed(format!("Failed to create runtime: {}", e)))?;

    match rt.block_on(crate::capture::portal::pick_color_via_portal()) {
        Ok(color) => Ok(color),
        Err(crate::capture::CaptureError::PermissionDenied) => Err(PickError::Cancelled),
        Err(e) => Err(PickError::NotAvailable(format!("Portal error: {}", e))),
    }
}

/// Try using hyprpicker (Hyprland's color picker).
fn try_hyprpicker() -> Result<(f64, f64, f64), PickError> {
    // Check if hyprpicker is available
    let output = Command::new("hyprpicker")
        .args(["--autocopy", "--format=rgb"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                PickError::NotAvailable("hyprpicker not found".to_string())
            } else {
                PickError::Failed(format!("Failed to run hyprpicker: {}", e))
            }
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("cancelled") || output.status.code() == Some(1) {
            return Err(PickError::Cancelled);
        }
        return Err(PickError::Failed(format!(
            "hyprpicker failed: {}",
            stderr.trim()
        )));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_rgb_output(&stdout)
}

/// Try using grim to capture a single pixel.
fn try_grim_picker() -> Result<(f64, f64, f64), PickError> {
    // First use slurp to get a point
    let slurp_output = Command::new("slurp")
        .args(["-p"]) // Point mode
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                PickError::NotAvailable("slurp not found".to_string())
            } else {
                PickError::Failed(format!("Failed to run slurp: {}", e))
            }
        })?;

    if !slurp_output.status.success() {
        return Err(PickError::Cancelled);
    }

    let geometry = String::from_utf8_lossy(&slurp_output.stdout)
        .trim()
        .to_string();

    // Use grim to capture that pixel
    let grim_output = Command::new("grim")
        .args(["-g", &format!("{} 1x1", geometry), "-"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                PickError::NotAvailable("grim not found".to_string())
            } else {
                PickError::Failed(format!("Failed to run grim: {}", e))
            }
        })?;

    if !grim_output.status.success() {
        let stderr = String::from_utf8_lossy(&grim_output.stderr);
        return Err(PickError::Failed(format!("grim failed: {}", stderr.trim())));
    }

    decode_png_pixel(&grim_output.stdout)
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

/// Parse RGB output like "rgb(255, 128, 64)" or "255 128 64".
fn parse_rgb_output(output: &str) -> Result<(f64, f64, f64), PickError> {
    let output = output.trim();

    // Try "rgb(r, g, b)" format
    if let Some(inner) = output
        .strip_prefix("rgb(")
        .and_then(|s| s.strip_suffix(')'))
    {
        let parts: Vec<&str> = inner.split(',').map(|s| s.trim()).collect();
        if parts.len() == 3 {
            let r: u8 = parts[0]
                .parse()
                .map_err(|_| PickError::Failed("Invalid R value".to_string()))?;
            let g: u8 = parts[1]
                .parse()
                .map_err(|_| PickError::Failed("Invalid G value".to_string()))?;
            let b: u8 = parts[2]
                .parse()
                .map_err(|_| PickError::Failed("Invalid B value".to_string()))?;
            return Ok((r as f64 / 255.0, g as f64 / 255.0, b as f64 / 255.0));
        }
    }

    // Try "r g b" format
    let parts: Vec<&str> = output.split_whitespace().collect();
    if parts.len() >= 3 {
        let r: u8 = parts[0]
            .parse()
            .map_err(|_| PickError::Failed("Invalid R value".to_string()))?;
        let g: u8 = parts[1]
            .parse()
            .map_err(|_| PickError::Failed("Invalid G value".to_string()))?;
        let b: u8 = parts[2]
            .parse()
            .map_err(|_| PickError::Failed("Invalid B value".to_string()))?;
        return Ok((r as f64 / 255.0, g as f64 / 255.0, b as f64 / 255.0));
    }

    Err(PickError::Failed(format!(
        "Could not parse color output: {}",
        output
    )))
}

/// Decode a single pixel from PNG data.
fn decode_png_pixel(bytes: &[u8]) -> Result<(f64, f64, f64), PickError> {
    let decoder = png::Decoder::new(Cursor::new(bytes));
    let mut reader = decoder
        .read_info()
        .map_err(|e| PickError::Failed(format!("PNG decode failed: {}", e)))?;
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader
        .next_frame(&mut buf)
        .map_err(|e| PickError::Failed(format!("PNG frame read failed: {}", e)))?;

    if info.width == 0 || info.height == 0 {
        return Err(PickError::Failed("PNG contained no pixels".into()));
    }

    match info.color_type {
        png::ColorType::Rgba | png::ColorType::Rgb => {
            let r = buf[0] as f64 / 255.0;
            let g = buf[1] as f64 / 255.0;
            let b = buf[2] as f64 / 255.0;
            Ok((r, g, b))
        }
        png::ColorType::Grayscale | png::ColorType::GrayscaleAlpha => {
            let gray = buf[0] as f64 / 255.0;
            Ok((gray, gray, gray))
        }
        other => Err(PickError::Failed(format!(
            "Unsupported PNG color type: {:?}",
            other
        ))),
    }
}
