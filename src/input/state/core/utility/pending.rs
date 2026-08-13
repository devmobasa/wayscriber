use super::super::base::{
    ClipboardFingerprint, ClipboardPasteRequest, InputState, KeybindingEditRequest,
    OutputFocusAction, PendingBackendAction, PendingSelectionClipboardPublish,
    PendingToolbarPersistence, PresetAction, QuickColorEdit, SelectionPublishState, ZoomAction,
};
use crate::input::boards::PendingBoardRuntimeUiAction;

#[allow(dead_code)]
impl InputState {
    /// Takes and clears any pending backend output action.
    pub fn take_pending_backend_action(&mut self) -> Option<PendingBackendAction> {
        self.pending_backend_action.take()
    }

    /// Whether another backend output action is waiting to be drained.
    pub(crate) fn has_pending_backend_actions(&self) -> bool {
        self.pending_backend_action.is_some()
    }

    /// Stores backend output work for retrieval by the backend, with
    /// last-action semantics. Durable toolbar chrome changes do not use this
    /// slot (see [`Self::queue_toolbar_persistence`]) because last-action
    /// semantics would let a capture cost a toggle its persistence.
    pub(crate) fn set_pending_backend_action(&mut self, action: PendingBackendAction) {
        self.pending_backend_action = Some(action);
    }

    /// Queues a durable toolbar chrome change, oldest first.
    ///
    /// Coalesced per kind: the eventual write reads live state at drain time,
    /// so a second change of the same kind needs no second entry, and the
    /// FIRST entry's previous values remain the rollback baseline — they
    /// describe the state before the whole burst.
    pub(crate) fn queue_toolbar_persistence(&mut self, entry: PendingToolbarPersistence) {
        let already_queued = self
            .pending_toolbar_persistence
            .iter()
            .any(|queued| std::mem::discriminant(queued) == std::mem::discriminant(&entry));
        if already_queued {
            return;
        }
        self.pending_toolbar_persistence.push(entry);
    }

    /// Takes every due toolbar persistence entry, oldest first.
    ///
    /// A burst that lands exactly where it started (F9 pressed twice, a
    /// display cycle walked full circle) is dropped here: nothing durable
    /// changed, the write would be byte-identical to its own rollback, and
    /// the only observable effect of writing it would be a bad rollback.
    pub(crate) fn take_pending_toolbar_persistence(&mut self) -> Vec<PendingToolbarPersistence> {
        let mut entries = std::mem::take(&mut self.pending_toolbar_persistence);
        entries.retain(|entry| match *entry {
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
        });
        entries
    }

    /// Whether durable toolbar work is waiting to be drained.
    pub(crate) fn has_pending_toolbar_persistence(&self) -> bool {
        !self.pending_toolbar_persistence.is_empty()
    }

    /// Takes every shortcut edit recorded since the last drain, oldest first.
    ///
    /// The backend hands each one to the config-edit worker, which answers them
    /// in the same order. They are drained together rather than one per pass
    /// because two edits can be recorded from a single batch of input events,
    /// and the second must not cost the first its write or its toast.
    pub(crate) fn take_pending_keybinding_edits(&mut self) -> Vec<KeybindingEditRequest> {
        std::mem::take(&mut self.pending_keybinding_edits)
    }

    /// Stores an output focus action for retrieval by the backend.
    pub(crate) fn request_output_focus_action(&mut self, action: OutputFocusAction) {
        self.pending_output_focus_action = Some(action);
    }

    /// Takes and clears any pending output focus action.
    pub fn take_pending_output_focus_action(&mut self) -> Option<OutputFocusAction> {
        self.pending_output_focus_action.take()
    }

    /// Stores a user-requested zoom action for retrieval by the backend and
    /// records that the zoom controls have been used for onboarding guidance.
    pub(crate) fn request_zoom_action(&mut self, action: ZoomAction) {
        self.pending_onboarding_usage.used_zoom_control = true;
        self.pending_zoom_action = Some(action);
    }

    /// Takes and clears any pending zoom action.
    pub fn take_pending_zoom_action(&mut self) -> Option<ZoomAction> {
        self.pending_zoom_action.take()
    }

    /// Takes and clears any pending preset save/clear action.
    pub fn take_pending_preset_action(&mut self) -> Option<PresetAction> {
        self.pending_preset_action.take()
    }

    /// Takes and clears any accepted quick-color recolor awaiting its config
    /// write. The runtime palette already shows the new color.
    pub fn take_pending_quick_color_edit(&mut self) -> Option<QuickColorEdit> {
        self.pending_quick_color_edit.take()
    }

    pub(crate) fn take_pending_board_runtime_ui_actions(
        &mut self,
    ) -> Vec<PendingBoardRuntimeUiAction> {
        std::mem::take(&mut self.pending_board_runtime_ui)
    }

    pub(crate) fn take_pending_selection_clipboard_publish(
        &mut self,
    ) -> Option<PendingSelectionClipboardPublish> {
        self.pending_selection_clipboard_publish.take()
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
        self.pending_clipboard_paste_request.take()
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
    use crate::draw::{Color, FontDescriptor};
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
        state.pending_preset_action = Some(PresetAction::Clear { slot: 2 });

        assert!(matches!(
            state.take_pending_preset_action(),
            Some(PresetAction::Clear { slot: 2 })
        ));
        assert!(state.take_pending_preset_action().is_none());
    }
}
