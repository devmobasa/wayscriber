//! Clipboard helpers for color hex values.

use super::{HexCopyOutcome, WaylandState};
use crate::backend::wayland::RuntimeOperationPoll;
use crate::clipboard_text::{ClipboardTextError, read_clipboard_text_via_command};
use crate::draw::Color;
use crate::input::state::{HexPasteTarget, Toast, ToastPriority};
use crate::input::state::{color_to_hex, parse_hex_color};
use std::time::Duration;

impl WaylandState {
    /// Copies the color captured when the request was made as hex.
    pub(in crate::backend::wayland) fn handle_copy_hex_color(&mut self, color: Color) {
        let hex = color_to_hex(color);
        log::info!("Hex copy requested: {}", hex);
        self.suppress_focus_exit_for(Duration::from_millis(1500));

        if let Err(err) = self.clipboard.queue_hex_copy(hex) {
            log::warn!("Failed to start hex clipboard copy: {err}");
            self.input_state.push_toast(
                ToastPriority::Info,
                "color_picker",
                Toast::warning("Failed to copy to clipboard"),
            );
        }
    }

    pub(in crate::backend::wayland) fn poll_hex_copy_completion(&mut self) {
        match self.clipboard.poll_hex_copy() {
            RuntimeOperationPoll::Idle | RuntimeOperationPoll::Pending { .. } => {}
            RuntimeOperationPoll::Ready {
                context: hex,
                outcome: HexCopyOutcome::Copied,
                ..
            } => {
                self.input_state.push_toast(
                    ToastPriority::Info,
                    "color_picker",
                    Toast::info(format!("Copied {hex}")),
                );
            }
            RuntimeOperationPoll::Ready {
                context: hex,
                outcome: HexCopyOutcome::Failed,
                ..
            } => {
                log::warn!("wl-copy failed for hex copy {hex}");
                self.input_state.push_toast(
                    ToastPriority::Info,
                    "color_picker",
                    Toast::warning(HexCopyOutcome::Failed.message()),
                );
            }
            RuntimeOperationPoll::ProducerFailed {
                context: hex,
                reason,
                ..
            } => {
                log::error!("Hex copy producer failed for {hex}: {reason}");
                self.input_state.push_toast(
                    ToastPriority::Info,
                    "color_picker",
                    Toast::warning("Failed to copy to clipboard"),
                );
            }
            RuntimeOperationPoll::Disconnected { context: hex, .. } => {
                log::error!("Hex copy producer disconnected for {hex}");
                self.input_state.push_toast(
                    ToastPriority::Info,
                    "color_picker",
                    Toast::warning("Failed to copy to clipboard"),
                );
            }
        }
        self.start_pending_hex_copy_if_idle();
    }

    fn start_pending_hex_copy_if_idle(&mut self) {
        if let Err(err) = self.clipboard.submit_pending_hex_copy_if_idle() {
            log::warn!("Failed to start pending hex clipboard copy: {err}");
            self.input_state.push_toast(
                ToastPriority::Info,
                "color_picker",
                Toast::warning("Failed to copy to clipboard"),
            );
        }
    }

    /// Pastes a hex color from the clipboard.
    pub(in crate::backend::wayland) fn handle_paste_hex_color(&mut self, target: HexPasteTarget) {
        if !self.input_state.hex_paste_target_is_current(target) {
            log::debug!("Discarding stale color-picker hex paste request");
            return;
        }
        log::info!("Hex paste requested");
        self.suppress_focus_exit_for(Duration::from_millis(1500));
        let clipboard = match std::panic::catch_unwind(read_clipboard_text_via_command) {
            Ok(Ok(text)) => text,
            Ok(Err(ClipboardTextError::Empty)) => {
                self.input_state.push_toast(
                    ToastPriority::Info,
                    "color_picker",
                    Toast::warning("Clipboard empty"),
                );
                return;
            }
            Ok(Err(ClipboardTextError::Other(err))) => {
                log::warn!("wl-paste failed for hex paste: {}", err);
                self.input_state.push_toast(
                    ToastPriority::Info,
                    "color_picker",
                    Toast::warning("Failed to paste from clipboard"),
                );
                return;
            }
            Err(_) => {
                log::error!("Hex paste panicked");
                self.input_state.push_toast(
                    ToastPriority::Info,
                    "color_picker",
                    Toast::warning("Failed to paste from clipboard"),
                );
                return;
            }
        };

        if let Some(color) = parse_hex_color(clipboard.trim()) {
            match target {
                HexPasteTarget::ActiveTool => {
                    let _ = self.input_state.apply_color_from_ui(color);
                }
                HexPasteTarget::ColorPickerPopup { generation } => {
                    if !self
                        .input_state
                        .color_picker_popup_generation_is_current(generation)
                    {
                        log::debug!("Discarding stale color-picker hex paste completion");
                        return;
                    }
                    self.input_state.color_picker_popup_set_color(color);
                }
            }
            let hex = color_to_hex(color);
            self.input_state.push_toast(
                ToastPriority::Info,
                "color_picker",
                Toast::info(format!("Pasted {}", hex)),
            );
        } else {
            self.input_state.push_toast(
                ToastPriority::Info,
                "color_picker",
                Toast::warning(format!(
                    "Invalid hex: {}",
                    clipboard.chars().take(20).collect::<String>()
                )),
            );
        }
    }
}
