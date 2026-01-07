use crate::config::keybindings::Action;
use crate::input::state::UiToastKind;

use super::*;

impl WaylandState {
    pub(in crate::backend::wayland) fn apply_onboarding_hints(&mut self) {
        // Show capability warning toast first if applicable
        self.apply_capability_toast();

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

        self.input_state.set_ui_toast_with_action(
            UiToastKind::Info,
            "Toolbars hidden",
            "Show (F2)",
            Action::ToggleToolbar,
        );
        self.onboarding.state_mut().toolbar_hint_shown = true;
        self.onboarding.save();
    }

    /// Show a one-time toast warning about limited compositor features.
    fn apply_capability_toast(&mut self) {
        if self.input_state.capability_toast_shown {
            return;
        }
        if !self.surface.is_configured() {
            return;
        }
        // Don't interrupt other toasts
        if self.input_state.ui_toast.is_some() {
            return;
        }

        let caps = &self.input_state.compositor_capabilities;
        if caps.all_available() {
            // No limitations to report
            self.input_state.capability_toast_shown = true;
            return;
        }

        if let Some(message) = caps.limitations_summary() {
            self.input_state
                .set_ui_toast(UiToastKind::Warning, &message);
            self.input_state.capability_toast_shown = true;
        }
    }
}
