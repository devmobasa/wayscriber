use super::super::base::{
    ClipboardFingerprint, ClipboardPasteRequest, InputEffect, InputEffectDrain, InputEffectKind,
    InputState, KeybindingEditRequest, OutputFocusAction, PendingBackendAction,
    PendingSelectionClipboardPublish, PendingToolbarPersistence, PresetAction, QuickColorEdit,
    SelectionPublishState, ZoomAction,
};
use super::super::base::{TextClipboardRequest, TextPasteTarget};
use crate::draw::Color;
use crate::input::boards::PendingBoardRuntimeUiAction;
use crate::input::state::HexPasteTarget;

#[allow(dead_code)]
impl InputState {
    pub(crate) fn emit_input_effect(&mut self, effect: InputEffect) {
        self.input_effects.emit(effect);
    }

    pub(crate) fn drain_input_effects(&mut self, drain: InputEffectDrain) -> Vec<InputEffect> {
        let mut effects = self.input_effects.drain(drain);
        effects.retain(|effect| match effect {
            InputEffect::ToolbarPersistence(entry) => self.toolbar_persistence_is_due(*entry),
            _ => true,
        });
        effects
    }

    /// Records that a user action created or changed a magnified Spotlight, so
    /// the backend can resolve source availability and warn once for it.
    ///
    /// This deliberately does not travel through [`PendingBackendAction`]:
    /// that slot has last-action semantics, so an export or screenshot queued
    /// in the same batch of input events would silently cost this request its
    /// warning — the same reason durable toolbar chrome has its own queue.
    pub(crate) fn request_spotlight_magnifier_feedback(&mut self) {
        self.emit_input_effect(InputEffect::SpotlightMagnifierFeedback);
    }

    /// Takes the coalesced request to resolve Spotlight source availability.
    pub fn take_pending_spotlight_magnifier_feedback(&mut self) -> bool {
        self.input_effects
            .drain_one(InputEffectKind::SpotlightMagnifierFeedback)
            .is_some()
    }

    /// Takes and clears any pending backend output action.
    pub fn take_pending_backend_action(&mut self) -> Option<PendingBackendAction> {
        match self.input_effects.drain_one(InputEffectKind::Backend) {
            Some(InputEffect::Backend(action)) => Some(action),
            _ => None,
        }
    }

    /// Whether another backend output action is waiting to be drained.
    pub(crate) fn has_pending_backend_actions(&self) -> bool {
        self.input_effects.contains(InputEffectKind::Backend)
    }

    /// Stores backend output work for retrieval by the backend, with
    /// last-action semantics. Durable toolbar chrome changes do not use this
    /// slot (see [`Self::queue_toolbar_persistence`]) because last-action
    /// semantics would let a capture cost a toggle its persistence.
    pub(crate) fn set_pending_backend_action(&mut self, action: PendingBackendAction) {
        self.emit_input_effect(InputEffect::Backend(action));
    }

    /// Queues a durable toolbar chrome change, oldest first.
    ///
    /// Coalesced per kind: the eventual write reads live state at drain time,
    /// so a second change of the same kind needs no second entry, and the
    /// FIRST entry's previous values remain the rollback baseline — they
    /// describe the state before the whole burst.
    pub(crate) fn queue_toolbar_persistence(&mut self, entry: PendingToolbarPersistence) {
        self.emit_input_effect(InputEffect::ToolbarPersistence(entry));
    }

    /// Takes every due toolbar persistence entry, oldest first.
    ///
    /// A burst that lands exactly where it started (F9 pressed twice, a
    /// display cycle walked full circle) is dropped here: nothing durable
    /// changed, the write would be byte-identical to its own rollback, and
    /// the only observable effect of writing it would be a bad rollback.
    pub(crate) fn take_pending_toolbar_persistence(&mut self) -> Vec<PendingToolbarPersistence> {
        self.input_effects
            .drain_all(InputEffectKind::ToolbarPersistence)
            .into_iter()
            .filter_map(|effect| match effect {
                InputEffect::ToolbarPersistence(entry)
                    if self.toolbar_persistence_is_due(entry) =>
                {
                    Some(entry)
                }
                _ => None,
            })
            .collect()
    }

    fn toolbar_persistence_is_due(&self, entry: PendingToolbarPersistence) -> bool {
        match entry {
            PendingToolbarPersistence::DisplayMode { previous } => {
                previous != self.toolbar_top_display_mode
            }
            PendingToolbarPersistence::Visibility {
                previous_top_pinned,
            } => previous_top_pinned != self.toolbar_top_pinned,
            PendingToolbarPersistence::StatusBar { previous } => previous != self.show_status_bar,
            PendingToolbarPersistence::FloatingBadge { previous } => {
                previous != self.show_floating_badge
            }
            PendingToolbarPersistence::ZoomChip { previous } => previous != self.show_zoom_chip,
            PendingToolbarPersistence::InputHud { previous } => {
                previous != self.input_hud_enabled()
            }
            PendingToolbarPersistence::ClickHighlight {
                previous_enabled,
                previous_tool_ring,
            } => {
                previous_enabled != self.click_highlight_enabled()
                    || previous_tool_ring != self.highlight_tool_ring_enabled()
            }
        }
    }

    /// Whether durable toolbar work is waiting to be drained.
    pub(crate) fn has_pending_toolbar_persistence(&self) -> bool {
        self.input_effects
            .contains(InputEffectKind::ToolbarPersistence)
    }

    /// Takes every shortcut edit recorded since the last drain, oldest first.
    ///
    /// The backend hands each one to the config-edit worker, which answers them
    /// in the same order. They are drained together rather than one per pass
    /// because two edits can be recorded from a single batch of input events,
    /// and the second must not cost the first its write or its toast.
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

    /// Stores an output focus action for retrieval by the backend.
    pub(crate) fn request_output_focus_action(&mut self, action: OutputFocusAction) {
        self.emit_input_effect(InputEffect::OutputFocus(action));
    }

    /// Takes and clears any pending output focus action.
    pub fn take_pending_output_focus_action(&mut self) -> Option<OutputFocusAction> {
        match self.input_effects.drain_one(InputEffectKind::OutputFocus) {
            Some(InputEffect::OutputFocus(action)) => Some(action),
            _ => None,
        }
    }

    /// Stores a user-requested zoom action for retrieval by the backend and
    /// records that the zoom controls have been used for onboarding guidance.
    pub(crate) fn request_zoom_action(&mut self, action: ZoomAction) {
        self.pending_onboarding_usage.used_zoom_control = true;
        self.emit_input_effect(InputEffect::Zoom(action));
    }

    /// Takes and clears any pending zoom action.
    pub fn take_pending_zoom_action(&mut self) -> Option<ZoomAction> {
        match self.input_effects.drain_one(InputEffectKind::Zoom) {
            Some(InputEffect::Zoom(action)) => Some(action),
            _ => None,
        }
    }

    /// Takes and clears any pending preset save/clear action.
    pub fn take_pending_preset_action(&mut self) -> Option<PresetAction> {
        match self.input_effects.drain_one(InputEffectKind::Preset) {
            Some(InputEffect::Preset(action)) => Some(action),
            _ => None,
        }
    }

    /// Takes and clears any accepted quick-color recolor awaiting its config
    /// write. The runtime palette already shows the new color.
    pub fn take_pending_quick_color_edit(&mut self) -> Option<QuickColorEdit> {
        match self.input_effects.drain_one(InputEffectKind::QuickColor) {
            Some(InputEffect::QuickColor(edit)) => Some(edit),
            _ => None,
        }
    }

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

    pub(crate) fn take_pending_selection_clipboard_publish(
        &mut self,
    ) -> Option<PendingSelectionClipboardPublish> {
        match self
            .input_effects
            .drain_one(InputEffectKind::SelectionClipboardPublish)
        {
            Some(InputEffect::SelectionClipboardPublish(request)) => Some(request),
            _ => None,
        }
    }

    pub(crate) fn complete_selection_clipboard_publish(
        &mut self,
        generation: u64,
        fingerprint_at_failure: Option<ClipboardFingerprint>,
        succeeded: bool,
    ) -> bool {
        if generation != self.selection_clipboard_generation {
            return false;
        }
        self.selection_publish_state = if succeeded {
            SelectionPublishState::Published { generation }
        } else {
            SelectionPublishState::Failed {
                generation,
                clipboard_fingerprint_at_failure: fingerprint_at_failure,
            }
        };
        true
    }

    pub(crate) fn take_pending_clipboard_paste_request(&mut self) -> Option<ClipboardPasteRequest> {
        match self
            .input_effects
            .drain_one(InputEffectKind::ClipboardPaste)
        {
            Some(InputEffect::ClipboardPaste(request)) => Some(request),
            _ => None,
        }
    }

    pub(crate) fn clear_pending_text_pastes(&mut self) {
        self.input_effects
            .retain_kind(InputEffectKind::TextPaste, |_| false);
    }

    /// Returns and clears any pending frozen-mode toggle request.
    pub fn take_pending_frozen_toggle(&mut self) -> bool {
        self.input_effects
            .drain_one(InputEffectKind::FrozenToggle)
            .is_some()
    }

    pub(crate) fn pending_frozen_toggle(&self) -> bool {
        self.input_effects.contains(InputEffectKind::FrozenToggle)
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

    pub(crate) fn take_pending_copy_hex_request(&mut self) -> Option<Color> {
        match self.input_effects.drain_one(InputEffectKind::CopyHex) {
            Some(InputEffect::CopyHex(color)) => Some(color),
            _ => None,
        }
    }

    pub(crate) fn take_pending_paste_hex_request(&mut self) -> Option<HexPasteTarget> {
        match self.input_effects.drain_one(InputEffectKind::PasteHex) {
            Some(InputEffect::PasteHex(target)) => Some(target),
            _ => None,
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

    pub(crate) fn discard_pending_color_picker_paste(&mut self) {
        self.input_effects
            .retain_kind(InputEffectKind::PasteHex, |effect| {
                !matches!(
                    effect,
                    InputEffect::PasteHex(HexPasteTarget::ColorPickerPopup { .. })
                )
            });
    }

    pub(crate) fn replace_selection_clipboard_publish(
        &mut self,
        request: Option<PendingSelectionClipboardPublish>,
    ) {
        self.input_effects
            .retain_kind(InputEffectKind::SelectionClipboardPublish, |_| false);
        if let Some(request) = request {
            self.emit_input_effect(InputEffect::SelectionClipboardPublish(request));
        }
    }

    pub(crate) fn active_clipboard_paste_request_id(&self) -> Option<u64> {
        self.active_clipboard_paste_request_id
    }

    pub(crate) fn finish_clipboard_paste_request(&mut self, id: u64) {
        if self.active_clipboard_paste_request_id == Some(id) {
            self.active_clipboard_paste_request_id = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Action, BoardsConfig, KeybindingsConfig, PresenterModeConfig};
    use crate::draw::{BLACK, Color, FontDescriptor, WHITE};
    use crate::input::state::KeybindingEditOperation;
    use crate::input::state::core::base::{InputEffect, InputEffectDrain};
    use crate::input::{ClickHighlightSettings, EraserMode};

    fn make_state() -> InputState {
        let keybindings = KeybindingsConfig::default();
        let action_map = keybindings
            .build_action_map()
            .expect("default keybindings map");

        InputState::with_defaults(
            Color {
                r: 1.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
            4.0,
            4.0,
            EraserMode::Brush,
            0.32,
            false,
            32.0,
            FontDescriptor::default(),
            false,
            20.0,
            30.0,
            false,
            true,
            BoardsConfig::default(),
            action_map,
            usize::MAX,
            ClickHighlightSettings::disabled(),
            0,
            0,
            true,
            0,
            0,
            5,
            5,
            PresenterModeConfig::default(),
        )
    }

    #[test]
    fn pending_backend_action_is_taken_once() {
        let mut state = make_state();
        state.set_pending_backend_action(PendingBackendAction::Screenshot(Action::CaptureFileFull));

        assert_eq!(
            state.take_pending_backend_action(),
            Some(PendingBackendAction::Screenshot(Action::CaptureFileFull))
        );
        assert_eq!(state.take_pending_backend_action(), None);
    }

    #[test]
    fn pending_clear_saved_tool_state_action_is_taken_once() {
        let mut state = make_state();
        state.set_pending_backend_action(PendingBackendAction::ClearSavedToolState);

        assert_eq!(
            state.take_pending_backend_action(),
            Some(PendingBackendAction::ClearSavedToolState)
        );
        assert_eq!(state.take_pending_backend_action(), None);
    }

    #[test]
    fn pending_desktop_open_is_taken_without_requesting_early_exit() {
        let mut state = make_state();
        let request = crate::desktop_open::DesktopOpenRequest::CaptureFolder("/tmp/capture".into());
        state.set_pending_backend_action(PendingBackendAction::DesktopOpen(request.clone()));

        assert!(!state.should_exit);
        assert_eq!(
            state.take_pending_backend_action(),
            Some(PendingBackendAction::DesktopOpen(request))
        );
        assert_eq!(state.take_pending_backend_action(), None);
    }

    #[test]
    fn pending_output_focus_action_is_taken_once() {
        let mut state = make_state();
        state.request_output_focus_action(OutputFocusAction::Next);

        assert_eq!(
            state.take_pending_output_focus_action(),
            Some(OutputFocusAction::Next)
        );
        assert_eq!(state.take_pending_output_focus_action(), None);
    }

    #[test]
    fn pending_zoom_action_is_taken_once() {
        let mut state = make_state();
        state.request_zoom_action(ZoomAction::ToggleLock);

        assert_eq!(
            state.take_pending_zoom_action(),
            Some(ZoomAction::ToggleLock)
        );
        assert_eq!(state.take_pending_zoom_action(), None);
    }

    #[test]
    fn pending_preset_action_is_taken_once() {
        let mut state = make_state();
        state.emit_input_effect(InputEffect::Preset(PresetAction::Clear { slot: 2 }));

        assert!(matches!(
            state.take_pending_preset_action(),
            Some(PresetAction::Clear { slot: 2 })
        ));
        assert!(state.take_pending_preset_action().is_none());
    }

    #[test]
    fn runtime_effect_drain_orders_region_capture_before_freeze_and_keeps_last_wins_slots() {
        let mut state = make_state();
        state.set_pending_backend_action(PendingBackendAction::Screenshot(Action::CaptureFileFull));
        state.set_pending_backend_action(PendingBackendAction::Screenshot(
            Action::CaptureClipboardRegion,
        ));
        state.request_frozen_toggle();
        state.request_frozen_toggle();
        state.request_zoom_action(ZoomAction::In);
        state.request_zoom_action(ZoomAction::Reset);

        let effects = state.drain_input_effects(InputEffectDrain::Runtime);

        assert!(matches!(
            effects.as_slice(),
            [
                InputEffect::OcrPass {
                    requested: false,
                    dismissed_by_toolbar: false
                },
                InputEffect::Backend(PendingBackendAction::Screenshot(
                    Action::CaptureClipboardRegion
                )),
                InputEffect::FrozenPass {
                    user_requested: true
                },
                InputEffect::Zoom(ZoomAction::Reset),
            ]
        ));
        assert!(matches!(
            state
                .drain_input_effects(InputEffectDrain::Runtime)
                .as_slice(),
            [
                InputEffect::OcrPass {
                    requested: false,
                    dismissed_by_toolbar: false
                },
                InputEffect::FrozenPass {
                    user_requested: false
                }
            ]
        ));
    }

    #[test]
    fn runtime_effect_drain_keeps_non_region_backend_work_after_the_frozen_phase() {
        let mut state = make_state();
        state.set_pending_backend_action(PendingBackendAction::Screenshot(
            Action::CaptureClipboardFull,
        ));

        let effects = state.drain_input_effects(InputEffectDrain::Runtime);

        assert!(matches!(
            effects.as_slice(),
            [
                InputEffect::OcrPass {
                    requested: false,
                    dismissed_by_toolbar: false
                },
                InputEffect::FrozenPass {
                    user_requested: false
                },
                InputEffect::Backend(PendingBackendAction::Screenshot(
                    Action::CaptureClipboardFull
                )),
            ]
        ));
    }

    #[test]
    fn runtime_ocr_phase_clears_toolbar_dismissal_without_suppressing_a_later_request() {
        let mut state = make_state();
        state.note_ocr_cancelled_by_toolbar();

        let first_pass = state.drain_input_effects(InputEffectDrain::Runtime);
        assert!(matches!(
            first_pass.first(),
            Some(InputEffect::OcrPass {
                requested: false,
                dismissed_by_toolbar: true
            })
        ));

        state.request_copy_text_from_screen();
        let next_pass = state.drain_input_effects(InputEffectDrain::Runtime);
        assert!(matches!(
            next_pass.first(),
            Some(InputEffect::OcrPass {
                requested: true,
                dismissed_by_toolbar: false
            })
        ));
    }

    #[test]
    fn durable_effect_drain_keeps_edit_order_and_each_slots_latest_value() {
        let mut state = make_state();
        state.emit_input_effect(InputEffect::Preset(PresetAction::Clear { slot: 1 }));
        state.emit_input_effect(InputEffect::Preset(PresetAction::Clear { slot: 2 }));
        state.emit_input_effect(InputEffect::QuickColor(QuickColorEdit {
            index: 0,
            color: BLACK,
        }));
        state.emit_input_effect(InputEffect::QuickColor(QuickColorEdit {
            index: 3,
            color: WHITE,
        }));
        state.emit_input_effect(InputEffect::KeybindingEdit(KeybindingEditRequest {
            action: Action::Undo,
            operation: KeybindingEditOperation::Delete,
        }));
        state.emit_input_effect(InputEffect::KeybindingEdit(KeybindingEditRequest {
            action: Action::Redo,
            operation: KeybindingEditOperation::Reset,
        }));

        let effects = state.drain_input_effects(InputEffectDrain::DurableConfig);

        assert_eq!(effects.len(), 4);
        assert!(matches!(
            effects[0],
            InputEffect::Preset(PresetAction::Clear { slot: 2 })
        ));
        assert!(matches!(
            effects[1],
            InputEffect::QuickColor(QuickColorEdit { index: 3, color }) if color == WHITE
        ));
        assert!(matches!(
            effects[2],
            InputEffect::KeybindingEdit(KeybindingEditRequest {
                action: Action::Undo,
                operation: KeybindingEditOperation::Delete,
            })
        ));
        assert!(matches!(
            effects[3],
            InputEffect::KeybindingEdit(KeybindingEditRequest {
                action: Action::Redo,
                operation: KeybindingEditOperation::Reset,
            })
        ));
    }

    #[test]
    fn toolbar_effects_coalesce_per_kind_and_keep_the_first_rollback_baseline() {
        let mut state = make_state();
        state.show_status_bar = true;
        state.show_zoom_chip = true;
        state.queue_toolbar_persistence(PendingToolbarPersistence::StatusBar { previous: false });
        state.queue_toolbar_persistence(PendingToolbarPersistence::StatusBar { previous: true });
        state.queue_toolbar_persistence(PendingToolbarPersistence::ZoomChip { previous: false });

        assert_eq!(
            state.take_pending_toolbar_persistence(),
            vec![
                PendingToolbarPersistence::StatusBar { previous: false },
                PendingToolbarPersistence::ZoomChip { previous: false },
            ]
        );
    }
}
