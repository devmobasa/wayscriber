use crate::config::{RadialMenuMouseBinding, keybindings::Action};
use crate::input::state::UiToastKind;
use crate::onboarding::{FirstRunStep, OnboardingState};
use crate::ui::{OnboardingCard, OnboardingChecklistItem};

use super::*;

impl WaylandState {
    pub(in crate::backend::wayland) fn apply_onboarding_hints(&mut self) {
        // Show capability warning toast first if applicable.
        self.apply_capability_toast();
        self.apply_first_run_progress();
        self.apply_contextual_feature_hints();
        self.apply_toolbar_visibility_hint();
    }

    pub(in crate::backend::wayland) fn try_skip_first_run_onboarding(&mut self) -> bool {
        if !first_run_skip_allowed(
            self.onboarding.state().first_run_active(),
            self.first_run_onboarding_card_visible(),
        ) {
            return false;
        }
        let state = self.onboarding.state_mut();
        state.first_run_skipped = true;
        state.first_run_completed = true;
        state.active_step = None;
        state.quick_access_requires_toolbar = false;
        self.onboarding.save();
        self.input_state
            .set_ui_toast(UiToastKind::Info, "Onboarding skipped.");
        true
    }

    pub(in crate::backend::wayland) fn first_run_onboarding_card(&self) -> Option<OnboardingCard> {
        if !self.first_run_onboarding_card_visible() {
            return None;
        }

        let state = self.onboarding.state();
        if !state.first_run_active() {
            return None;
        }
        let step = state.active_step?;
        let footer = "Shift+Escape to skip".to_string();

        let card = match step {
            FirstRunStep::WaitDraw => OnboardingCard {
                eyebrow: "First-run onboarding".to_string(),
                title: "Draw one mark".to_string(),
                body: "Make one quick stroke to start. This onboarding stays out of your way."
                    .to_string(),
                items: vec![OnboardingChecklistItem {
                    label: "Draw a stroke".to_string(),
                    done: state.first_stroke_done,
                }],
                footer,
            },
            FirstRunStep::DrawUndo => OnboardingCard {
                eyebrow: "First-run onboarding".to_string(),
                title: "Try Undo".to_string(),
                body: "You can always revert mistakes. Draw, then undo once.".to_string(),
                items: vec![
                    OnboardingChecklistItem {
                        label: "Draw a stroke".to_string(),
                        done: state.first_stroke_done,
                    },
                    OnboardingChecklistItem {
                        label: format!("Undo once ({})", self.shortcut_label(Action::Undo, "Undo")),
                        done: state.first_undo_done,
                    },
                ],
                footer,
            },
            FirstRunStep::QuickAccess => {
                let items = self.quick_access_checklist_items(state);
                OnboardingCard {
                    eyebrow: "First-run onboarding".to_string(),
                    title: "Quick access at cursor".to_string(),
                    body:
                        "Open quick actions near the pointer. This is faster than hunting buttons."
                            .to_string(),
                    items,
                    footer,
                }
            }
            FirstRunStep::Reference => OnboardingCard {
                eyebrow: "First-run onboarding".to_string(),
                title: "Find anything fast".to_string(),
                body: "Use help for full shortcuts and command palette for searchable actions."
                    .to_string(),
                items: vec![
                    OnboardingChecklistItem {
                        label: format!(
                            "Open Help ({})",
                            self.shortcut_label(Action::ToggleHelp, "Help")
                        ),
                        done: state.used_help_overlay,
                    },
                    OnboardingChecklistItem {
                        label: format!(
                            "Open Command Palette ({})",
                            self.shortcut_label(Action::ToggleCommandPalette, "Command Palette")
                        ),
                        done: state.used_command_palette,
                    },
                ],
                footer,
            },
        };

        Some(card)
    }

    fn first_run_onboarding_card_visible(&self) -> bool {
        if !self.surface.is_configured() || self.overlay_suppressed() {
            return false;
        }
        !first_run_card_hidden_by_ui_state(
            self.input_state.presenter_mode,
            self.input_state.command_palette_open,
            self.input_state.show_help,
            self.input_state.is_radial_menu_open(),
            self.input_state.is_context_menu_open(),
            self.input_state.tour_active,
            self.zoom.is_engaged(),
        )
    }

    fn apply_first_run_progress(&mut self) {
        let usage = std::mem::take(&mut self.input_state.pending_onboarding_usage);
        let context_enabled = self.input_state.context_menu_enabled();
        let radial_binding = self.input_state.radial_menu_mouse_binding;
        let radial_available = self.shortcut_label_opt(Action::ToggleRadialMenu).is_some();
        let context_keyboard_available = self.shortcut_label_opt(Action::OpenContextMenu).is_some();
        let toolbar_visible = self.input_state.toolbar_visible();

        let mut changed = false;
        let mut completed_now = false;

        {
            let state = self.onboarding.state_mut();

            if usage.first_stroke_done && !state.first_stroke_done {
                state.first_stroke_done = true;
                changed = true;
            }
            if usage.first_undo_done && !state.first_undo_done {
                state.first_undo_done = true;
                changed = true;
            }
            if usage.used_toolbar_toggle && !state.used_toolbar_toggle {
                state.used_toolbar_toggle = true;
                changed = true;
            }
            if usage.used_radial_menu && !state.used_radial_menu {
                state.used_radial_menu = true;
                changed = true;
            }
            if usage.used_context_menu_right_click && !state.used_context_menu_right_click {
                state.used_context_menu_right_click = true;
                changed = true;
            }
            if usage.used_context_menu_keyboard && !state.used_context_menu_keyboard {
                state.used_context_menu_keyboard = true;
                changed = true;
            }
            if usage.used_help_overlay && !state.used_help_overlay {
                state.used_help_overlay = true;
                changed = true;
            }
            if usage.used_command_palette && !state.used_command_palette {
                state.used_command_palette = true;
                changed = true;
            }

            if !state.first_run_active() {
                if state.active_step.is_some() || state.quick_access_requires_toolbar {
                    state.active_step = None;
                    state.quick_access_requires_toolbar = false;
                    changed = true;
                }
            } else if state.active_step.is_none() {
                state.active_step = Some(FirstRunStep::WaitDraw);
                changed = true;
            }

            loop {
                let Some(step) = state.active_step else {
                    break;
                };
                match step {
                    FirstRunStep::WaitDraw => {
                        if !state.first_stroke_done {
                            break;
                        }
                        state.active_step = Some(FirstRunStep::DrawUndo);
                        changed = true;
                    }
                    FirstRunStep::DrawUndo => {
                        if !state.first_undo_done {
                            break;
                        }
                        state.active_step = Some(FirstRunStep::QuickAccess);
                        state.quick_access_requires_toolbar = !toolbar_visible;
                        changed = true;
                    }
                    FirstRunStep::QuickAccess => {
                        if !quick_access_completed(
                            state,
                            context_enabled,
                            radial_binding,
                            radial_available,
                            context_keyboard_available,
                            toolbar_visible,
                        ) {
                            break;
                        }
                        state.active_step = Some(FirstRunStep::Reference);
                        state.quick_access_requires_toolbar = false;
                        changed = true;
                    }
                    FirstRunStep::Reference => {
                        if !(state.used_help_overlay && state.used_command_palette) {
                            break;
                        }
                        state.first_run_completed = true;
                        state.first_run_skipped = false;
                        state.active_step = None;
                        state.quick_access_requires_toolbar = false;
                        changed = true;
                        completed_now = true;
                        break;
                    }
                }
            }
        }

        if changed {
            self.onboarding.save();
            self.input_state.dirty_tracker.mark_full();
            self.input_state.needs_redraw = true;
        }
        if completed_now && self.input_state.ui_toast.is_none() {
            self.input_state
                .set_ui_toast(UiToastKind::Info, "Onboarding complete.");
        }
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
            if state.sessions_seen >= 2 && !state.used_help_overlay && !state.hint_help_shown {
                state.hint_help_shown = true;
                changed = true;
                hint_kind = Some("help");
            } else if state.sessions_seen >= 3
                && !state.used_command_palette
                && !state.hint_palette_shown
            {
                state.hint_palette_shown = true;
                changed = true;
                hint_kind = Some("palette");
            } else if state.sessions_seen >= 2
                && !state.used_radial_menu
                && !state.used_context_menu_right_click
                && !state.used_context_menu_keyboard
                && !state.hint_quick_access_shown
            {
                state.hint_quick_access_shown = true;
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

    fn quick_access_checklist_items(
        &self,
        state: &OnboardingState,
    ) -> Vec<OnboardingChecklistItem> {
        let context_enabled = self.input_state.context_menu_enabled();
        let radial_binding = self.input_state.radial_menu_mouse_binding;
        let radial_label = self.shortcut_label_opt(Action::ToggleRadialMenu);
        let radial_available = radial_label.is_some();
        let context_keyboard = self.shortcut_label_opt(Action::OpenContextMenu);
        let mut items = Vec::new();

        if context_enabled {
            if matches!(radial_binding, RadialMenuMouseBinding::Right) && radial_available {
                if let Some(label) = radial_label {
                    items.push(OnboardingChecklistItem {
                        label: format!("Open radial menu ({label})"),
                        done: state.used_radial_menu,
                    });
                }
                if let Some(label) = context_keyboard {
                    items.push(OnboardingChecklistItem {
                        label: format!("Open context menu ({label})"),
                        done: state.used_context_menu_keyboard,
                    });
                } else {
                    items.push(OnboardingChecklistItem {
                        label: "Context menu keyboard shortcut not configured".to_string(),
                        done: true,
                    });
                }
            } else {
                items.push(OnboardingChecklistItem {
                    label: "Open context menu (Right Click)".to_string(),
                    done: state.used_context_menu_right_click,
                });
                if let Some(label) = radial_label {
                    items.push(OnboardingChecklistItem {
                        label: format!("Open radial menu ({label})"),
                        done: state.used_radial_menu,
                    });
                }
            }
        } else if let Some(label) = radial_label {
            items.push(OnboardingChecklistItem {
                label: format!("Open radial menu ({label})"),
                done: state.used_radial_menu,
            });
        } else {
            items.push(OnboardingChecklistItem {
                label: "Quick-access menus disabled in config".to_string(),
                done: true,
            });
        }

        if state.quick_access_requires_toolbar {
            items.push(OnboardingChecklistItem {
                label: format!(
                    "Show toolbars ({})",
                    self.shortcut_label(Action::ToggleToolbar, "Toggle toolbar")
                ),
                done: self.input_state.toolbar_visible() || state.used_toolbar_toggle,
            });
        }

        items
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

fn quick_access_context_required(
    context_enabled: bool,
    radial_binding: RadialMenuMouseBinding,
    radial_available: bool,
    context_keyboard_available: bool,
) -> bool {
    if !context_enabled {
        return false;
    }
    if matches!(radial_binding, RadialMenuMouseBinding::Right) && radial_available {
        return context_keyboard_available;
    }
    true
}

fn quick_access_context_done(
    state: &OnboardingState,
    context_enabled: bool,
    radial_binding: RadialMenuMouseBinding,
    radial_available: bool,
    context_keyboard_available: bool,
) -> bool {
    if !quick_access_context_required(
        context_enabled,
        radial_binding,
        radial_available,
        context_keyboard_available,
    ) {
        return true;
    }
    if matches!(radial_binding, RadialMenuMouseBinding::Right) && radial_available {
        state.used_context_menu_keyboard
    } else {
        state.used_context_menu_right_click
    }
}

fn quick_access_completed(
    state: &OnboardingState,
    context_enabled: bool,
    radial_binding: RadialMenuMouseBinding,
    radial_available: bool,
    context_keyboard_available: bool,
    toolbar_visible: bool,
) -> bool {
    let mut done = true;
    if radial_available {
        done &= state.used_radial_menu;
    }
    done &= quick_access_context_done(
        state,
        context_enabled,
        radial_binding,
        radial_available,
        context_keyboard_available,
    );

    if state.quick_access_requires_toolbar {
        done &= toolbar_visible || state.used_toolbar_toggle;
    }

    done
}

fn first_run_skip_allowed(first_run_active: bool, card_visible: bool) -> bool {
    first_run_active && card_visible
}

fn first_run_card_hidden_by_ui_state(
    presenter_mode: bool,
    command_palette_open: bool,
    show_help: bool,
    radial_menu_open: bool,
    context_menu_open: bool,
    tour_active: bool,
    zoom_engaged: bool,
) -> bool {
    presenter_mode
        || command_palette_open
        || show_help
        || radial_menu_open
        || context_menu_open
        || tour_active
        || zoom_engaged
}

#[cfg(test)]
mod tests {
    use super::{first_run_card_hidden_by_ui_state, first_run_skip_allowed};

    #[test]
    fn first_run_skip_requires_active_onboarding_and_visible_card() {
        assert!(first_run_skip_allowed(true, true));
        assert!(!first_run_skip_allowed(true, false));
        assert!(!first_run_skip_allowed(false, true));
        assert!(!first_run_skip_allowed(false, false));
    }

    #[test]
    fn first_run_card_hides_for_each_modal_state() {
        let modal_cases = [
            (true, false, false, false, false, false, false), // presenter
            (false, true, false, false, false, false, false), // palette
            (false, false, true, false, false, false, false), // help
            (false, false, false, true, false, false, false), // radial
            (false, false, false, false, true, false, false), // context menu
            (false, false, false, false, false, true, false), // tour
            (false, false, false, false, false, false, true), // zoom
        ];

        for case in modal_cases {
            assert!(
                first_run_card_hidden_by_ui_state(
                    case.0, case.1, case.2, case.3, case.4, case.5, case.6
                ),
                "expected modal case to hide onboarding card"
            );
        }
    }

    #[test]
    fn first_run_card_remains_visible_without_modal_states() {
        assert!(!first_run_card_hidden_by_ui_state(
            false, false, false, false, false, false, false
        ));
    }
}
