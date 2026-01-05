use crate::input::state::UiToastKind;

use super::*;

impl WaylandState {
    pub(in crate::backend::wayland) fn apply_onboarding_hints(&mut self) {
        if self.onboarding.state().toolbar_hint_shown {
            return;
        }
        if !self.surface.is_configured() || self.overlay_suppressed() {
            return;
        }
        if self.input_state.presenter_mode || self.input_state.show_help {
            return;
        }
        if self.input_state.toolbar_visible() || self.input_state.ui_toast.is_some() {
            return;
        }

        self.input_state
            .set_ui_toast(UiToastKind::Info, "Toolbars hidden. Press F2/F9 to show.");
        self.onboarding.state_mut().toolbar_hint_shown = true;
        self.onboarding.save();
    }
}
