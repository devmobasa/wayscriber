use smithay_client_toolkit::shell::wlr_layer::KeyboardInteractivity;

use super::super::{WaylandState, data::MainLayerFocusPhase};

#[derive(Debug, Clone, Copy)]
pub(in crate::backend::wayland::state) struct MainLayerEnterFacts {
    pub(in crate::backend::wayland::state) is_current_main_layer_surface: bool,
    pub(in crate::backend::wayland::state) phase: MainLayerFocusPhase,
    pub(in crate::backend::wayland::state) committed_keyboard_interactivity:
        Option<KeyboardInteractivity>,
    pub(in crate::backend::wayland::state) keyboard_release_requested: bool,
}

pub(in crate::backend::wayland::state) fn can_complete_main_layer_focus_acquisition(
    facts: MainLayerEnterFacts,
) -> bool {
    facts.is_current_main_layer_surface
        && facts.phase.is_acquiring()
        && facts.committed_keyboard_interactivity == Some(KeyboardInteractivity::Exclusive)
        && !facts.keyboard_release_requested
}

impl WaylandState {
    pub(in crate::backend::wayland) fn begin_main_layer_focus_acquisition(&mut self) {
        self.data.main_layer_focus_phase.begin();
    }

    pub(in crate::backend::wayland) fn main_layer_focus_acquiring(&self) -> bool {
        self.data.main_layer_focus_phase.is_acquiring()
    }

    pub(in crate::backend::wayland) fn try_complete_main_layer_focus_acquisition(
        &mut self,
        is_current_main_layer_surface: bool,
    ) -> bool {
        let facts = MainLayerEnterFacts {
            is_current_main_layer_surface,
            phase: self.data.main_layer_focus_phase,
            committed_keyboard_interactivity: self.current_keyboard_interactivity(),
            keyboard_release_requested: self.overlay_keyboard_passthrough_requested(),
        };
        if !can_complete_main_layer_focus_acquisition(facts) {
            return false;
        }
        self.data.main_layer_focus_phase.complete()
    }

    /// Retire every keyboard-owned transient when focus is lost.
    ///
    /// Both the compositor's keyboard-leave callback and layer-output surface
    /// recreation enter through this state lifecycle boundary.
    pub(in crate::backend::wayland) fn teardown_keyboard_focus(&mut self) {
        self.data.main_layer_focus_phase =
            self.data.main_layer_focus_phase.after_keyboard_teardown();
        self.set_keyboard_focus(false);
        self.set_overlay_ready(false);
        self.clear_toolbar_focus();
        self.input_state.clear_focus_owned_key_state();
        self.sync_region_square_modifier(false);
        self.clear_key_repeat();
        self.set_board_pan_key_held(false);
        self.stop_board_pan();
    }
}

#[cfg(test)]
mod tests {
    use smithay_client_toolkit::shell::wlr_layer::KeyboardInteractivity;

    use super::{
        MainLayerEnterFacts, MainLayerFocusPhase, can_complete_main_layer_focus_acquisition,
    };

    #[test]
    fn stale_main_enter_under_none_keeps_acquisition_pending() {
        let mut phase = MainLayerFocusPhase::default();

        assert!(!can_complete_main_layer_focus_acquisition(
            MainLayerEnterFacts {
                is_current_main_layer_surface: true,
                phase,
                committed_keyboard_interactivity: Some(KeyboardInteractivity::None),
                keyboard_release_requested: true,
            }
        ));
        assert!(phase.is_acquiring());

        assert!(can_complete_main_layer_focus_acquisition(
            MainLayerEnterFacts {
                is_current_main_layer_surface: true,
                phase,
                committed_keyboard_interactivity: Some(KeyboardInteractivity::Exclusive),
                keyboard_release_requested: false,
            }
        ));
        assert!(phase.complete());
        assert!(!phase.is_acquiring());
    }

    #[test]
    fn main_layer_enter_requires_current_surface_acquiring_exclusive_and_no_release() {
        let valid = MainLayerEnterFacts {
            is_current_main_layer_surface: true,
            phase: MainLayerFocusPhase::Acquiring,
            committed_keyboard_interactivity: Some(KeyboardInteractivity::Exclusive),
            keyboard_release_requested: false,
        };

        assert!(can_complete_main_layer_focus_acquisition(valid));
        assert!(!can_complete_main_layer_focus_acquisition(
            MainLayerEnterFacts {
                is_current_main_layer_surface: false,
                ..valid
            }
        ));
        assert!(!can_complete_main_layer_focus_acquisition(
            MainLayerEnterFacts {
                phase: MainLayerFocusPhase::Acquired,
                ..valid
            }
        ));
        assert!(!can_complete_main_layer_focus_acquisition(
            MainLayerEnterFacts {
                committed_keyboard_interactivity: Some(KeyboardInteractivity::OnDemand),
                ..valid
            }
        ));
        assert!(!can_complete_main_layer_focus_acquisition(
            MainLayerEnterFacts {
                committed_keyboard_interactivity: None,
                ..valid
            }
        ));
        assert!(!can_complete_main_layer_focus_acquisition(
            MainLayerEnterFacts {
                keyboard_release_requested: true,
                ..valid
            }
        ));
    }

    #[test]
    fn ordinary_keyboard_teardown_keeps_an_acquired_surface_out_of_acquisition() {
        let mut phase = MainLayerFocusPhase::default();
        assert!(phase.complete());

        phase = phase.after_keyboard_teardown();

        assert!(!phase.is_acquiring());
        assert!(!can_complete_main_layer_focus_acquisition(
            MainLayerEnterFacts {
                is_current_main_layer_surface: true,
                phase,
                committed_keyboard_interactivity: Some(KeyboardInteractivity::Exclusive),
                keyboard_release_requested: false,
            }
        ));
    }
}
