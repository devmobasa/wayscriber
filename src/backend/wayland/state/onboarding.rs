use crate::config::keybindings::Action;
use crate::input::state::UiToastKind;
use crate::onboarding::DEFERRED_HINT_REPEAT_MAX;

use super::*;

mod first_run;

impl WaylandState {
    pub(in crate::backend::wayland) fn apply_onboarding_hints(&mut self) {
        // Show capability warning toast first if applicable.
        self.apply_capability_toast();
        self.apply_first_run_progress();
        self.apply_contextual_feature_hints();
        self.apply_toolbar_visibility_hint();
    }

    fn apply_contextual_feature_hints(&mut self) {
        if !self.surface.is_configured() || self.overlay_suppressed() {
            return;
        }
        if self.input_state.presenter_mode
            || self.input_state.show_help
            || self.input_state.command_palette_open
            || self.input_state.tour_active
        {
            return;
        }
        if self.input_state.ui_toast.is_some() {
            return;
        }

        let mut changed = false;
        let mut hint_kind: Option<&'static str> = None;
        {
            let state = self.onboarding.state_mut();
            if !state.first_run_completed {
                return;
            }
            if state.sessions_seen >= 2
                && !state.used_help_overlay
                && !state.hint_help_shown
                && state.hint_help_count < DEFERRED_HINT_REPEAT_MAX
            {
                state.hint_help_shown = true;
                state.hint_help_count = state.hint_help_count.saturating_add(1);
                changed = true;
                hint_kind = Some("help");
            } else if state.sessions_seen >= 3
                && !state.used_command_palette
                && !state.hint_palette_shown
                && state.hint_palette_count < DEFERRED_HINT_REPEAT_MAX
            {
                state.hint_palette_shown = true;
                state.hint_palette_count = state.hint_palette_count.saturating_add(1);
                changed = true;
                hint_kind = Some("palette");
            } else if state.sessions_seen >= 2
                && !state.used_radial_menu
                && !state.used_context_menu_right_click
                && !state.used_context_menu_keyboard
                && !state.hint_quick_access_shown
                && state.hint_quick_access_count < DEFERRED_HINT_REPEAT_MAX
            {
                state.hint_quick_access_shown = true;
                state.hint_quick_access_count = state.hint_quick_access_count.saturating_add(1);
                changed = true;
                hint_kind = Some("quick_access");
            }
        }

        if changed {
            self.onboarding.save();
        }
        if let Some(kind) = hint_kind {
            let message = match kind {
                "help" => format!(
                    "Press {} for all shortcuts.",
                    self.shortcut_label(Action::ToggleHelp, "Help")
                ),
                "palette" => format!(
                    "Press {} to search actions.",
                    self.shortcut_label(Action::ToggleCommandPalette, "Command Palette")
                ),
                _ => {
                    let context = self.shortcut_label_opt(Action::OpenContextMenu);
                    let radial = self.shortcut_label_opt(Action::ToggleRadialMenu);
                    match (context, radial) {
                        (Some(c), Some(r)) => format!("Try quick access: {c} or {r}."),
                        (Some(c), None) => format!("Try quick access: {c}."),
                        (None, Some(r)) => format!("Try quick access: {r}."),
                        (None, None) => {
                            "Quick-access menus are available from toolbar actions.".to_string()
                        }
                    }
                }
            };
            self.input_state.set_ui_toast(UiToastKind::Info, message);
        }
    }

    fn apply_toolbar_visibility_hint(&mut self) {
        if self.onboarding.state().toolbar_hint_shown {
            return;
        }
        if !self.surface.is_configured() || self.overlay_suppressed() {
            return;
        }
        if self.input_state.presenter_mode || self.input_state.show_help {
            return;
        }
        if self.onboarding.state().first_run_active() {
            return;
        }
        if self.input_state.toolbar_visible() || self.input_state.ui_toast.is_some() {
            return;
        }

        let toolbar_binding = self.shortcut_label(Action::ToggleToolbar, "Toggle toolbar");
        self.input_state.set_ui_toast_with_action(
            UiToastKind::Info,
            "Toolbars hidden",
            format!("Show ({toolbar_binding})"),
            Action::ToggleToolbar,
        );
        self.onboarding.state_mut().toolbar_hint_shown = true;
        self.onboarding.save();
    }

    fn shortcut_label(&self, action: Action, fallback: &str) -> String {
        self.shortcut_label_opt(action)
            .unwrap_or_else(|| fallback.to_string())
    }

    fn shortcut_label_opt(&self, action: Action) -> Option<String> {
        self.input_state.shortcut_for_action(action)
    }

    /// Show a one-time toast warning about limited compositor features.
    fn apply_capability_toast(&mut self) {
        if self.input_state.capability_toast_shown {
            return;
        }
        if !self.config.ui.show_capabilities_warning {
            return;
        }
        if !self.surface.is_configured() {
            return;
        }
        // Don't interrupt other toasts.
        if self.input_state.ui_toast.is_some() {
            return;
        }

        let caps = &self.input_state.compositor_capabilities;
        if caps.all_available() {
            // No limitations to report.
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

#[cfg(test)]
mod tests;
