//! Test conveniences that read state private to `input::state::core`.
//! Production drains the ordered input effect stream instead.

use super::base::{
    InputEffect, InputEffectKind, InputState, KeybindingEditRequest, TextClipboardRequest,
    TextPasteTarget,
};
use crate::draw::TextMeasurer;
use crate::input::boards::PendingBoardRuntimeUiAction;

impl InputState {
    /// Test convenience for inspecting shortcut edits. Production drains the
    /// ordered InputEffect stream and forwards edits to the config worker.
    pub(crate) fn take_pending_keybinding_edits(&mut self) -> Vec<KeybindingEditRequest> {
        self.input_effects
            .drain_all(InputEffectKind::KeybindingEdit)
            .into_iter()
            .filter_map(|effect| match effect {
                InputEffect::KeybindingEdit(request) => Some(request),
                _ => None,
            })
            .collect()
    }

    /// Test convenience for inspecting board actions in the InputEffect stream.
    pub(crate) fn take_pending_board_runtime_ui_actions(
        &mut self,
    ) -> Vec<PendingBoardRuntimeUiAction> {
        self.input_effects
            .drain_all(InputEffectKind::BoardRuntimeUi)
            .into_iter()
            .filter_map(|effect| match effect {
                InputEffect::BoardRuntimeUi(action) => Some(action),
                _ => None,
            })
            .collect()
    }

    pub(crate) fn take_pending_eyedropper_toggle(&mut self) -> bool {
        self.input_effects
            .drain_one(InputEffectKind::EyedropperToggle)
            .is_some()
    }

    pub(crate) fn take_pending_ocr_request(&mut self) -> bool {
        match self.input_effects.drain_one(InputEffectKind::OcrPass) {
            Some(InputEffect::OcrPass { requested, .. }) => requested,
            Some(effect) => unreachable!("OCR drain returned {effect:?}"),
            None => false,
        }
    }

    pub(crate) fn take_pending_text_copy(&mut self) -> Option<TextClipboardRequest> {
        match self.input_effects.drain_one(InputEffectKind::TextCopy) {
            Some(InputEffect::TextCopy(request)) => Some(request),
            _ => None,
        }
    }

    pub(crate) fn take_pending_text_paste(&mut self) -> Option<TextPasteTarget> {
        match self.input_effects.drain_one(InputEffectKind::TextPaste) {
            Some(InputEffect::TextPaste(target)) => Some(target),
            _ => None,
        }
    }

    pub(crate) fn board_appearance_click(&mut self, x: i32, y: i32) -> bool {
        let measurer = TextMeasurer::default();
        self.board_appearance_click_with_measurer(&measurer, x, y)
    }
}
