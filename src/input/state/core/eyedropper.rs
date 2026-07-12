use super::InputState;

/// UI-facing lifecycle for the modal screen-color eyedropper.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub enum EyedropperUiState {
    #[default]
    Inactive,
    PendingCapture {
        auto_froze: bool,
    },
    Active {
        hover: Option<(f64, f64)>,
        auto_froze: bool,
    },
}

impl EyedropperUiState {
    pub fn is_active(self) -> bool {
        matches!(self, Self::Active { .. })
    }

    pub fn is_pending(self) -> bool {
        matches!(self, Self::PendingCapture { .. })
    }

    pub fn hover(self) -> Option<(f64, f64)> {
        match self {
            Self::Active { hover, .. } => hover,
            Self::Inactive | Self::PendingCapture { .. } => None,
        }
    }

    fn auto_froze(self) -> bool {
        match self {
            Self::PendingCapture { auto_froze } | Self::Active { auto_froze, .. } => auto_froze,
            Self::Inactive => false,
        }
    }
}

#[allow(dead_code)] // Backend lifecycle methods are consumed only by the binary target.
impl InputState {
    pub(crate) fn request_eyedropper_toggle(&mut self) {
        self.pending_eyedropper_toggle = true;
    }

    pub(crate) fn take_pending_eyedropper_toggle(&mut self) -> bool {
        std::mem::take(&mut self.pending_eyedropper_toggle)
    }

    pub fn eyedropper_state(&self) -> EyedropperUiState {
        self.eyedropper_ui_state
    }

    pub fn eyedropper_is_active(&self) -> bool {
        self.eyedropper_ui_state.is_active()
    }

    pub(crate) fn prepare_for_eyedropper(&mut self) {
        self.cancel_active_interaction();
        if self.show_help {
            self.toggle_help_overlay();
        }
        if self.command_palette_open {
            self.toggle_command_palette();
        }
        self.tour_active = false;
        self.close_radial_menu();
        self.close_context_menu();
        self.close_properties_panel();
        self.close_board_picker();
        if self.is_color_picker_popup_open() {
            self.close_color_picker_popup(true);
        }
    }

    pub(crate) fn set_eyedropper_pending_capture(&mut self) {
        self.eyedropper_ui_state = EyedropperUiState::PendingCapture { auto_froze: true };
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
    }

    pub(crate) fn activate_eyedropper(&mut self, auto_froze: bool) {
        self.eyedropper_ui_state = EyedropperUiState::Active {
            hover: None,
            auto_froze,
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

    /// Leave eyedropper mode and return whether it created frozen mode itself.
    pub(crate) fn cancel_eyedropper(&mut self) -> bool {
        let auto_froze = self.eyedropper_ui_state.auto_froze();
        if !matches!(self.eyedropper_ui_state, EyedropperUiState::Inactive) {
            self.eyedropper_ui_state = EyedropperUiState::Inactive;
            self.dirty_tracker.mark_full();
            self.needs_redraw = true;
        }
        auto_froze
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::state::test_support::make_test_input_state;

    #[test]
    fn cancel_reports_auto_frozen_ownership() {
        let mut state = make_test_input_state();
        state.set_eyedropper_pending_capture();
        state.activate_eyedropper(true);

        assert!(state.cancel_eyedropper());
        assert_eq!(state.eyedropper_state(), EyedropperUiState::Inactive);
    }
}
