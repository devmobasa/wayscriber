use super::super::base::{InputState, OutputFocusAction, PresetAction, ZoomAction};
use crate::config::{Action, BoardsConfig};

impl InputState {
    /// Takes and clears any pending capture action.
    pub fn take_pending_capture_action(&mut self) -> Option<Action> {
        self.pending_capture_action.take()
    }

    /// Stores a capture action for retrieval by the backend.
    pub(crate) fn set_pending_capture_action(&mut self, action: Action) {
        self.pending_capture_action = Some(action);
    }

    /// Stores an output focus action for retrieval by the backend.
    pub(crate) fn request_output_focus_action(&mut self, action: OutputFocusAction) {
        self.pending_output_focus_action = Some(action);
    }

    /// Takes and clears any pending output focus action.
    pub fn take_pending_output_focus_action(&mut self) -> Option<OutputFocusAction> {
        self.pending_output_focus_action.take()
    }

    /// Stores a zoom action for retrieval by the backend.
    pub(crate) fn request_zoom_action(&mut self, action: ZoomAction) {
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

    /// Takes and clears any pending board config update.
    pub fn take_pending_board_config(&mut self) -> Option<BoardsConfig> {
        self.pending_board_config.take()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{BoardsConfig, KeybindingsConfig, PresenterModeConfig};
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
    fn pending_capture_action_is_taken_once() {
        let mut state = make_state();
        state.set_pending_capture_action(Action::CaptureFileFull);

        assert_eq!(
            state.take_pending_capture_action(),
            Some(Action::CaptureFileFull)
        );
        assert_eq!(state.take_pending_capture_action(), None);
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

    #[test]
    fn pending_board_config_is_taken_once() {
        let mut state = make_state();
        let config = BoardsConfig {
            default_board: "blackboard".to_string(),
            ..BoardsConfig::default()
        };
        state.pending_board_config = Some(config.clone());

        let taken = state.take_pending_board_config().expect("board config");
        assert_eq!(taken.default_board, "blackboard");
        assert_eq!(taken.items.len(), config.items.len());
        assert!(state.take_pending_board_config().is_none());
    }
}
