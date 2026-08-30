use super::InputState;
use crate::domain::Action;
use crate::input::Key;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EyedropperCaptureSource {
    Frozen,
    Zoom,
}

/// UI-facing lifecycle for the modal screen-color eyedropper.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub enum EyedropperUiState {
    #[default]
    Inactive,
    PendingCapture {
        source: EyedropperCaptureSource,
        owned_frozen_generation: Option<u64>,
    },
    Active {
        hover: Option<(f64, f64)>,
        owned_frozen_generation: Option<u64>,
    },
}

impl EyedropperUiState {
    pub fn is_active(self) -> bool {
        matches!(self, Self::Active { .. })
    }

    pub fn is_pending(self) -> bool {
        matches!(self, Self::PendingCapture { .. })
    }

    pub fn is_engaged(self) -> bool {
        self.is_active() || self.is_pending()
    }

    pub fn hover(self) -> Option<(f64, f64)> {
        match self {
            Self::Active { hover, .. } => hover,
            Self::Inactive | Self::PendingCapture { .. } => None,
        }
    }

    pub fn pending_source(self) -> Option<EyedropperCaptureSource> {
        match self {
            Self::PendingCapture { source, .. } => Some(source),
            Self::Inactive | Self::Active { .. } => None,
        }
    }

    pub(crate) fn owned_frozen_generation(self) -> Option<u64> {
        match self {
            Self::PendingCapture {
                owned_frozen_generation,
                ..
            }
            | Self::Active {
                owned_frozen_generation,
                ..
            } => owned_frozen_generation,
            Self::Inactive => None,
        }
    }
}

impl InputState {
    pub(crate) fn request_eyedropper_toggle(&mut self) {
        self.emit_input_effect(super::base::InputEffect::EyedropperToggle);
    }

    pub fn eyedropper_state(&self) -> EyedropperUiState {
        self.eyedropper_ui_state
    }

    pub fn eyedropper_is_active(&self) -> bool {
        self.eyedropper_ui_state.is_active()
    }

    pub fn eyedropper_is_engaged(&self) -> bool {
        self.eyedropper_ui_state.is_engaged()
    }

    pub(crate) fn action_for_key(&self, key: Key) -> Option<Action> {
        crate::input::state::interaction::action_for_key_binding(self, key)
            .ok()
            .flatten()
    }

    pub(crate) fn set_eyedropper_pending_capture(&mut self, source: EyedropperCaptureSource) {
        self.eyedropper_ui_state = EyedropperUiState::PendingCapture {
            source,
            owned_frozen_generation: None,
        };
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
    }

    pub(crate) fn activate_eyedropper(&mut self, owned_frozen_generation: Option<u64>) {
        // A capture can take long enough for another interaction to begin while
        // the eyedropper is pending. Entering the modal state must cancel it so
        // the eyedropper cannot swallow the matching release event.
        self.prepare_for_screen_modal();
        self.eyedropper_ui_state = EyedropperUiState::Active {
            hover: None,
            owned_frozen_generation,
        };
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
    }

    pub(crate) fn update_eyedropper_hover(&mut self, hover: (f64, f64)) {
        if let EyedropperUiState::Active { hover: current, .. } = &mut self.eyedropper_ui_state {
            *current = Some(hover);
            // The loupe moves with the pointer; full damage clears the old
            // position and draws the new one on every incremental buffer.
            self.dirty_tracker.mark_full();
            self.needs_redraw = true;
        }
    }

    /// Leave eyedropper mode and return the exact frozen generation it owns.
    pub(crate) fn cancel_eyedropper(&mut self) -> Option<u64> {
        let owned_frozen_generation = self.eyedropper_ui_state.owned_frozen_generation();
        if !matches!(self.eyedropper_ui_state, EyedropperUiState::Inactive) {
            self.eyedropper_ui_state = EyedropperUiState::Inactive;
            self.dirty_tracker.mark_full();
            self.needs_redraw = true;
        }
        owned_frozen_generation
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::state::test_support::make_test_input_state;
    use crate::input::{DrawingState, MouseButton};

    #[test]
    fn cancel_returns_the_exact_owned_frozen_generation() {
        let mut state = make_test_input_state();
        state.set_eyedropper_pending_capture(EyedropperCaptureSource::Frozen);
        state.activate_eyedropper(Some(17));

        assert_eq!(state.cancel_eyedropper(), Some(17));
        assert_eq!(state.eyedropper_state(), EyedropperUiState::Inactive);
    }

    #[test]
    fn waiting_for_zoom_does_not_claim_frozen_mode() {
        let mut state = make_test_input_state();
        state.set_eyedropper_pending_capture(EyedropperCaptureSource::Zoom);

        assert_eq!(state.cancel_eyedropper(), None);
        assert_eq!(state.eyedropper_state(), EyedropperUiState::Inactive);
    }

    #[test]
    fn activation_cancels_interaction_started_while_capture_was_pending() {
        let mut state = make_test_input_state();
        state.set_eyedropper_pending_capture(EyedropperCaptureSource::Frozen);
        state.on_mouse_press(MouseButton::Left, 10, 20);
        assert!(matches!(state.state, DrawingState::Drawing { .. }));

        state.activate_eyedropper(Some(1));

        assert!(matches!(state.state, DrawingState::Idle));
        assert!(state.active_drag_button.is_none());
        assert!(state.eyedropper_is_active());
    }

    /// The eyedropper swallows every key press it receives, so a key held from
    /// before it opened must not keep repeating into the canvas behind it.
    #[test]
    fn an_engaged_eyedropper_stops_canvas_key_repeat() {
        let mut state = make_test_input_state();
        assert!(!state.modal_blocks_canvas_key_repeat());

        state.set_eyedropper_pending_capture(EyedropperCaptureSource::Frozen);
        assert!(state.modal_blocks_canvas_key_repeat());

        state.activate_eyedropper(Some(1));
        assert!(state.modal_blocks_canvas_key_repeat());

        state.cancel_eyedropper();
        assert!(!state.modal_blocks_canvas_key_repeat());
    }

    #[test]
    fn default_i_key_resolves_to_screen_eyedropper() {
        let state = make_test_input_state();

        assert_eq!(
            state.action_for_key(Key::Char('i')),
            Some(Action::PickScreenColor)
        );
    }
}
