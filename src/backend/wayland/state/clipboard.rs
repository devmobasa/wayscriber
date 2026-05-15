//! Wayland-state glue for clipboard publish and paste requests.

use super::WaylandState;
use crate::backend::wayland::clipboard::{
    self, ClipboardPasteCompletion, ClipboardPasteResult, ClipboardPublishCompletion,
    FailedLocalSelectionProbe, PasteAction, TransferEffect, TransferPlan, TransferWarning,
    transfer,
};
use crate::input::state::{ClipboardPasteRequest, UiToastKind};
use std::sync::mpsc;
use std::time::Duration;

impl WaylandState {
    pub(in crate::backend::wayland) fn drain_clipboard_requests(&mut self) {
        if self.clipboard_publish_rx.is_none()
            && let Some(request) = self.input_state.take_pending_selection_clipboard_publish()
        {
            self.start_selection_clipboard_publish(request.generation, request.payload_json);
        }

        if let Some(request) = self.input_state.take_pending_clipboard_paste_request() {
            self.start_clipboard_paste(request);
        }
    }

    pub(in crate::backend::wayland) fn poll_clipboard_publish_completion(&mut self) {
        let Some(rx) = &self.clipboard_publish_rx else {
            return;
        };
        let Ok(completion) = rx.try_recv() else {
            return;
        };
        self.clipboard_publish_rx = None;
        self.apply_selection_clipboard_publish_completion(completion);
    }

    pub(in crate::backend::wayland) fn poll_clipboard_paste_completion(&mut self) {
        let Some(rx) = &self.clipboard_paste_rx else {
            return;
        };
        let Ok(completion) = rx.try_recv() else {
            return;
        };
        self.clipboard_paste_rx = None;
        self.apply_clipboard_paste_completion(completion);
    }

    fn start_selection_clipboard_publish(&mut self, generation: u64, payload_json: String) {
        self.suppress_focus_exit_for(Duration::from_millis(1500));
        let (tx, rx) = mpsc::channel();
        self.clipboard_publish_rx = Some(rx);
        std::thread::spawn(move || {
            let completion =
                transfer::resolve_selection_clipboard_publish(generation, payload_json);
            let _ = tx.send(completion);
        });
    }

    fn apply_selection_clipboard_publish_completion(
        &mut self,
        completion: ClipboardPublishCompletion,
    ) {
        let applied = self.input_state.complete_selection_clipboard_publish(
            completion.generation,
            completion.fingerprint,
            completion.copied,
        );
        if applied && let Some(warning) = completion.warning {
            self.set_transfer_warning_toast(warning);
        }
    }

    fn start_clipboard_paste(&mut self, request: ClipboardPasteRequest) {
        self.suppress_focus_exit_for(Duration::from_millis(1500));

        let pending_shapes = self.input_state.local_selection_shapes_for_pending_publish(
            request.local_selection_fallback_generation,
        );
        let failed_probe = self
            .input_state
            .failed_local_selection_probe_for_generation(
                request.local_selection_fallback_generation,
            )
            .map(|(generation, expected)| FailedLocalSelectionProbe {
                generation,
                expected,
            });
        let plan = transfer::plan_paste_start(request, pending_shapes, failed_probe);
        self.apply_paste_plan(plan);
    }

    fn apply_clipboard_paste_completion(&mut self, completion: ClipboardPasteCompletion) {
        let active_request_id = self.input_state.active_clipboard_paste_request_id();
        let private_payload = match &completion.result {
            ClipboardPasteResult::PrivateSelection(payload)
                if active_request_id == Some(completion.request.id) =>
            {
                let payload_matches_local = self
                    .input_state
                    .private_payload_matches_request_selection(&completion.request, payload);
                let same_instance = self.input_state.private_payload_is_same_instance(payload);
                let shapes = self
                    .input_state
                    .private_payload_shapes_for_request(&completion.request, payload.clone());
                Some(transfer::PrivateSelectionResolution {
                    payload_matches_local,
                    same_instance,
                    shapes,
                })
            }
            _ => None,
        };
        let plan = transfer::plan_paste_completion(completion, active_request_id, private_payload);
        self.apply_paste_plan(plan);
    }

    fn apply_paste_plan(&mut self, plan: TransferPlan<PasteAction>) {
        for effect in plan.effects {
            self.apply_transfer_effect(effect);
        }

        match plan.action {
            PasteAction::UseLocalShapes {
                request,
                shapes,
                warning,
            } => {
                let pasted = self
                    .input_state
                    .paste_clipboard_shapes_from_request(&request, shapes);
                self.input_state.finish_clipboard_paste_request(request.id);
                if pasted == 0 {
                    self.set_transfer_warning_toast(
                        warning.unwrap_or(TransferWarning::NothingPasted),
                    );
                    self.input_state.trigger_blocked_feedback();
                } else if let Some(warning) = warning {
                    self.set_transfer_info_toast(warning);
                }
            }
            PasteAction::ProbeSystemFingerprint {
                request,
                generation,
                expected,
            } => {
                let current = clipboard::clipboard_fingerprint();
                let local_shapes = self
                    .input_state
                    .local_selection_shapes_for_fallback(generation);
                let plan = transfer::plan_after_fingerprint_probe(
                    request,
                    generation,
                    expected,
                    current,
                    local_shapes,
                );
                self.apply_paste_plan(plan);
            }
            PasteAction::ReadSystemClipboard { request } => {
                self.start_system_clipboard_read(request);
            }
            PasteAction::StaleCompletion { request_id } => {
                log::debug!("Ignoring stale clipboard paste completion {}", request_id);
            }
            PasteAction::ApplyPrivateSelection { request, shapes } => {
                let pasted = self
                    .input_state
                    .paste_clipboard_shapes_from_request(&request, shapes);
                self.input_state.finish_clipboard_paste_request(request.id);
                if pasted == 0 {
                    self.set_transfer_warning_toast(TransferWarning::NoShapesPasted);
                    self.input_state.trigger_blocked_feedback();
                }
            }
            PasteAction::ApplyExternalImage { request, image } => {
                if !self
                    .input_state
                    .paste_external_image_from_request(&request, image)
                {
                    self.input_state.trigger_blocked_feedback();
                }
                self.input_state.finish_clipboard_paste_request(request.id);
            }
            PasteAction::TryFreshLocalFallbackOrWarn {
                request,
                missing_warning,
                fallback_message,
            } => {
                self.paste_fresh_local_fallback_or_warn(
                    &request,
                    missing_warning,
                    fallback_message,
                );
                self.input_state.finish_clipboard_paste_request(request.id);
            }
            PasteAction::ShowWarning {
                request,
                warning,
                block_feedback,
            } => {
                self.set_transfer_warning_toast(warning);
                if block_feedback {
                    self.input_state.trigger_blocked_feedback();
                }
                self.input_state.finish_clipboard_paste_request(request.id);
            }
        }
    }

    fn start_system_clipboard_read(&mut self, request: ClipboardPasteRequest) {
        let (tx, rx) = mpsc::channel();
        self.clipboard_paste_rx = Some(rx);
        std::thread::spawn(move || {
            let result = transfer::resolve_system_clipboard();
            let _ = tx.send(ClipboardPasteCompletion { request, result });
        });
    }

    fn apply_transfer_effect(&mut self, effect: TransferEffect) {
        match effect {
            TransferEffect::SupersedeLocalGeneration { generation } => self
                .input_state
                .mark_selection_clipboard_superseded_for_generation(Some(generation)),
        }
    }

    fn paste_fresh_local_fallback_or_warn(
        &mut self,
        request: &ClipboardPasteRequest,
        missing_warning: TransferWarning,
        fallback_message: &'static str,
    ) {
        if let Some(generation) = request.local_selection_fallback_generation
            && let Some(shapes) = self
                .input_state
                .local_selection_shapes_for_fallback(generation)
        {
            let pasted = self
                .input_state
                .paste_clipboard_shapes_from_request(request, shapes);
            if pasted > 0 {
                self.input_state
                    .set_ui_toast(UiToastKind::Info, fallback_message);
                return;
            }
        }

        self.set_transfer_warning_toast(missing_warning);
        self.input_state.trigger_blocked_feedback();
    }

    fn set_transfer_info_toast(&mut self, warning: TransferWarning) {
        let message = match warning {
            TransferWarning::ReadTimedOut => Some("Clipboard read timed out; pasted local copy."),
            TransferWarning::ClipboardUnavailable => {
                Some("System clipboard unavailable; pasted local copy.")
            }
            TransferWarning::ClipboardError => Some("Failed to read clipboard; pasted local copy."),
            _ => None,
        };
        if let Some(message) = message {
            self.input_state.set_ui_toast(UiToastKind::Info, message);
        }
    }

    fn set_transfer_warning_toast(&mut self, warning: TransferWarning) {
        match warning {
            TransferWarning::PublishSelectionTooLarge => self.input_state.set_ui_toast(
                UiToastKind::Warning,
                "Copied locally; selection is too large for system clipboard.",
            ),
            TransferWarning::PublishUnavailable => self.input_state.set_ui_toast(
                UiToastKind::Warning,
                "Copied locally; system clipboard unavailable.",
            ),
            TransferWarning::NothingPasted => self
                .input_state
                .set_ui_toast(UiToastKind::Warning, "Nothing pasted."),
            TransferWarning::NoShapesPasted => self
                .input_state
                .set_ui_toast(UiToastKind::Warning, "No shapes pasted."),
            TransferWarning::ClipboardEmpty => self
                .input_state
                .set_ui_toast(UiToastKind::Warning, "System clipboard is empty."),
            TransferWarning::UnsupportedContent => self.input_state.set_ui_toast(
                UiToastKind::Warning,
                "Clipboard content is not a supported image or Wayscriber selection.",
            ),
            TransferWarning::TooLarge { limit } => self.input_state.set_ui_toast(
                UiToastKind::Warning,
                format!(
                    "Clipboard data is too large to paste (limit {} MB).",
                    limit / 1024 / 1024
                ),
            ),
            TransferWarning::TooManyPixels {
                width,
                height,
                limit,
            } => self.input_state.set_ui_toast(
                UiToastKind::Warning,
                format!(
                    "Clipboard image is too large ({}x{}, limit {} pixels).",
                    width, height, limit
                ),
            ),
            TransferWarning::DecodeFailed => self.input_state.set_ui_toast(
                UiToastKind::Warning,
                "Clipboard image could not be decoded.",
            ),
            TransferWarning::ReadTimedOut => self
                .input_state
                .set_ui_toast(UiToastKind::Warning, "Clipboard read timed out."),
            TransferWarning::ClipboardUnavailable => self
                .input_state
                .set_ui_toast(UiToastKind::Warning, "System clipboard unavailable."),
            TransferWarning::ClipboardError => self
                .input_state
                .set_ui_toast(UiToastKind::Warning, "Failed to read clipboard."),
            TransferWarning::MalformedPrivateSelection => self.input_state.set_ui_toast(
                UiToastKind::Warning,
                "Wayscriber clipboard selection could not be read.",
            ),
        }
    }
}
