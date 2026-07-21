//! Clipboard helpers for color hex values.

use super::{ClipboardOperationController, WaylandState};
use crate::backend::wayland::clipboard::ClipboardPoll;
use crate::draw::Color;
use crate::input::state::{HexPasteTarget, Toast, ToastPriority};
use crate::input::state::{color_to_hex, parse_hex_color};
use std::ffi::OsStr;
use std::time::Duration;

impl WaylandState {
    /// Copies the color captured when the request was made as hex.
    pub(in crate::backend::wayland) fn handle_copy_hex_color(&mut self, color: Color) {
        let hex = color_to_hex(color);
        log::info!("Hex copy requested: {}", hex);
        self.suppress_focus_exit_for(Duration::from_millis(1500));

        if let Err(err) = queue_latest_hex_copy(
            &mut self.clipboard_hex_copy,
            &mut self.pending_hex_copy,
            hex,
            copy_hex_via_command,
        ) {
            log::warn!("Failed to start hex clipboard copy: {err}");
            self.input_state.push_toast(
                ToastPriority::Info,
                "color_picker",
                Toast::warning("Failed to copy to clipboard"),
            );
        }
    }

    pub(in crate::backend::wayland) fn poll_hex_copy_completion(&mut self) {
        match self.clipboard_hex_copy.poll() {
            ClipboardPoll::Idle | ClipboardPoll::Pending { .. } => {}
            ClipboardPoll::Ready {
                context: hex,
                outcome: Ok(()),
                ..
            } => {
                self.input_state.push_toast(
                    ToastPriority::Info,
                    "color_picker",
                    Toast::info(format!("Copied {hex}")),
                );
            }
            ClipboardPoll::Ready {
                context: hex,
                outcome: Err(err),
                ..
            } => {
                log::warn!("wl-copy failed for hex copy {hex}: {err}");
                self.input_state.push_toast(
                    ToastPriority::Info,
                    "color_picker",
                    Toast::warning("Failed to copy to clipboard"),
                );
            }
            ClipboardPoll::ProducerFailed {
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
            ClipboardPoll::Disconnected { context: hex, .. } => {
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
        if let Err(err) = submit_pending_hex_copy_if_idle(
            &mut self.clipboard_hex_copy,
            &mut self.pending_hex_copy,
            copy_hex_via_command,
        ) {
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

fn start_hex_copy(
    controller: &mut ClipboardOperationController<String, Result<(), String>>,
    hex: String,
    operation: impl FnOnce(&str) -> Result<(), String> + Send + 'static,
) -> Result<(), String> {
    let worker_hex = hex.clone();
    controller
        .try_submit(hex, "wayscriber-hex-copy", move || operation(&worker_hex))
        .map(drop)
        .map_err(|failure| failure.into_parts().0.to_string())
}

fn queue_latest_hex_copy(
    controller: &mut ClipboardOperationController<String, Result<(), String>>,
    pending: &mut Option<String>,
    hex: String,
    operation: impl FnOnce(&str) -> Result<(), String> + Send + 'static,
) -> Result<(), String> {
    *pending = Some(hex);
    submit_pending_hex_copy_if_idle(controller, pending, operation)
}

fn submit_pending_hex_copy_if_idle(
    controller: &mut ClipboardOperationController<String, Result<(), String>>,
    pending: &mut Option<String>,
    operation: impl FnOnce(&str) -> Result<(), String> + Send + 'static,
) -> Result<(), String> {
    if controller.is_active() {
        return Ok(());
    }
    let Some(hex) = pending.take() else {
        return Ok(());
    };
    start_hex_copy(controller, hex, operation)
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

#[cfg(test)]
mod tests {
    use std::sync::mpsc;
    use std::time::Duration;

    use super::*;
    use crate::backend::wayland::RuntimeWakeSource;
    use crate::backend::wayland::clipboard::ClipboardOperationIdSource;

    #[test]
    fn hex_copy_submission_stays_off_the_event_thread_until_completion() {
        let wake = RuntimeWakeSource::new().unwrap();
        let mut controller =
            ClipboardOperationController::new(ClipboardOperationIdSource::new(), wake.handle());
        let (started_tx, started_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();

        start_hex_copy(&mut controller, "#123456".to_string(), move |hex| {
            assert_eq!(hex, "#123456");
            started_tx.send(()).unwrap();
            release_rx
                .recv_timeout(Duration::from_secs(1))
                .map_err(|error| error.to_string())?;
            Ok(())
        })
        .unwrap();

        started_rx.recv_timeout(Duration::from_secs(1)).unwrap();
        assert!(matches!(controller.poll(), ClipboardPoll::Pending { .. }));
        release_tx.send(()).unwrap();
        assert!(
            wake.wait_readable(Some(Duration::from_secs(1))).unwrap(),
            "hex copy completion did not wake the event loop"
        );
        assert!(matches!(
            controller.poll(),
            ClipboardPoll::Ready {
                context,
                outcome: Ok(()),
                ..
            } if context == "#123456"
        ));
    }

    #[test]
    fn active_hex_copy_retains_only_the_newest_pending_request() {
        let wake = RuntimeWakeSource::new().unwrap();
        let mut controller =
            ClipboardOperationController::new(ClipboardOperationIdSource::new(), wake.handle());
        let mut pending = None;
        let (first_started_tx, first_started_rx) = mpsc::channel();
        let (first_release_tx, first_release_rx) = mpsc::channel();

        queue_latest_hex_copy(
            &mut controller,
            &mut pending,
            "#111111".to_string(),
            move |hex| {
                assert_eq!(hex, "#111111");
                first_started_tx.send(()).unwrap();
                first_release_rx
                    .recv_timeout(Duration::from_secs(1))
                    .map_err(|error| error.to_string())?;
                Ok(())
            },
        )
        .unwrap();
        first_started_rx
            .recv_timeout(Duration::from_secs(1))
            .unwrap();

        queue_latest_hex_copy(
            &mut controller,
            &mut pending,
            "#222222".to_string(),
            |_| -> Result<(), String> { panic!("busy submission must not run") },
        )
        .unwrap();
        queue_latest_hex_copy(
            &mut controller,
            &mut pending,
            "#333333".to_string(),
            |_| -> Result<(), String> { panic!("busy submission must not run") },
        )
        .unwrap();
        assert_eq!(pending.as_deref(), Some("#333333"));
        assert!(matches!(controller.poll(), ClipboardPoll::Pending { .. }));

        first_release_tx.send(()).unwrap();
        assert!(wake.wait_readable(Some(Duration::from_secs(1))).unwrap());
        assert!(matches!(
            controller.poll(),
            ClipboardPoll::Ready {
                context,
                outcome: Ok(()),
                ..
            } if context == "#111111"
        ));

        let (newest_started_tx, newest_started_rx) = mpsc::channel();
        let (newest_release_tx, newest_release_rx) = mpsc::channel();
        submit_pending_hex_copy_if_idle(&mut controller, &mut pending, move |hex| {
            newest_started_tx.send(hex.to_string()).unwrap();
            newest_release_rx
                .recv_timeout(Duration::from_secs(1))
                .map_err(|error| error.to_string())?;
            Ok(())
        })
        .unwrap();
        assert_eq!(pending, None);
        assert_eq!(
            newest_started_rx
                .recv_timeout(Duration::from_secs(1))
                .unwrap(),
            "#333333"
        );

        newest_release_tx.send(()).unwrap();
        assert!(wake.wait_readable(Some(Duration::from_secs(1))).unwrap());
        assert!(matches!(
            controller.poll(),
            ClipboardPoll::Ready {
                context,
                outcome: Ok(()),
                ..
            } if context == "#333333"
        ));
    }
}
