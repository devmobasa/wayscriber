//! Copy and controls of each first-run card.
//!
//! The tour runs value first: draw and undo, the toolbar and the way out,
//! color and thickness, quick access, finding commands, and background mode
//! last. Every control on a card is a button with the keyboard equivalent
//! written beside it.

use crate::backend::wayland::state::WaylandState;
use crate::config::{RadialMenuMouseBinding, keybindings::Action};
use crate::onboarding::{FirstRunStep, OnboardingState};
use crate::ui::{
    OnboardingCard, OnboardingCardAction, OnboardingCardButton, OnboardingChecklistItem,
};

impl WaylandState {
    pub(in crate::backend::wayland) fn first_run_onboarding_card(&self) -> Option<OnboardingCard> {
        if !self.first_run_onboarding_card_visible() {
            return None;
        }

        let state = self.preferences.onboarding().state();
        if !state.first_run_active() {
            return None;
        }
        let step = state.active_step?;
        let eyebrow = first_run_step_eyebrow(step, !state.first_run_background_mode_prompted);

        let card = match step {
            FirstRunStep::WaitDraw | FirstRunStep::DrawUndo => OnboardingCard {
                eyebrow,
                title: "Draw, then undo".to_string(),
                body: "Drag anywhere on the screen to draw. Undo takes back the last change."
                    .to_string(),
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
                buttons: vec![skip_tour_button()],
                footer: String::new(),
            },
            FirstRunStep::ToolbarExit => OnboardingCard {
                eyebrow,
                title: "Toolbar and exit".to_string(),
                body: toolbar_exit_body(
                    &self.shortcut_label(Action::Exit, "Escape"),
                    self.shortcut_label_opt(Action::ToggleToolbar).as_deref(),
                ),
                items: Vec::new(),
                buttons: vec![continue_button(), skip_tour_button()],
                footer: String::new(),
            },
            FirstRunStep::ColorThickness => OnboardingCard {
                eyebrow,
                title: "Color and thickness".to_string(),
                body: "Recolor and resize your strokes without leaving the canvas.".to_string(),
                items: self.color_thickness_checklist_items(state),
                buttons: vec![skip_tour_button()],
                footer: String::new(),
            },
            FirstRunStep::QuickAccess => OnboardingCard {
                eyebrow,
                title: "Quick access at cursor".to_string(),
                body: "Open quick actions near the pointer.".to_string(),
                items: self.quick_access_checklist_items(state),
                buttons: vec![skip_tour_button()],
                footer: String::new(),
            },
            FirstRunStep::RadialFlick | FirstRunStep::Reference => OnboardingCard {
                eyebrow,
                title: "Find anything".to_string(),
                body: "Search every command, or see all shortcuts at once.".to_string(),
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
                buttons: vec![skip_tour_button()],
                footer: String::new(),
            },
            FirstRunStep::BackgroundModeSetup => OnboardingCard {
                eyebrow,
                title: "Keep Wayscriber ready?".to_string(),
                body: "Background mode keeps Wayscriber running, so your shortcut opens the \
                       overlay instantly. You can change this later in the configurator."
                    .to_string(),
                items: Vec::new(),
                buttons: background_mode_buttons(),
                footer: String::new(),
            },
        };

        Some(card)
    }

    /// Join the resolved shortcut labels for `actions` with `" / "`, skipping
    /// unbound actions. `None` when none resolve. Keeps onboarding copy free of
    /// hardcoded key strings.
    fn join_shortcut_labels(&self, actions: &[Action]) -> Option<String> {
        let labels: Vec<String> = actions
            .iter()
            .filter_map(|action| self.shortcut_label_opt(*action))
            .collect();
        (!labels.is_empty()).then(|| labels.join(" / "))
    }

    fn color_thickness_checklist_items(
        &self,
        state: &OnboardingState,
    ) -> Vec<OnboardingChecklistItem> {
        let color_label = match self.join_shortcut_labels(&[
            Action::SetColorRed,
            Action::SetColorGreen,
            Action::SetColorBlue,
            Action::SetColorYellow,
        ]) {
            Some(hint) => format!("Change color ({hint})"),
            None => "Change color".to_string(),
        };
        let thickness_label = match self
            .join_shortcut_labels(&[Action::IncreaseThickness, Action::DecreaseThickness])
        {
            Some(hint) => format!("Adjust thickness ({hint})"),
            None => "Adjust thickness".to_string(),
        };

        vec![
            OnboardingChecklistItem {
                label: color_label,
                done: state.first_color_done,
            },
            OnboardingChecklistItem {
                label: thickness_label,
                done: state.first_thickness_done,
            },
        ]
    }

    fn quick_access_checklist_items(
        &self,
        state: &OnboardingState,
    ) -> Vec<OnboardingChecklistItem> {
        let context_enabled = self.input_state.context_menu_enabled();
        let radial_binding = self.input_state.radial_menu.mouse_binding();
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
                    "Show toolbar ({})",
                    self.shortcut_label(Action::ToggleToolbar, "Toggle toolbar")
                ),
                done: self.input_state.toolbar_visible() || state.used_toolbar_toggle,
            });
        }

        items
    }
}

/// Keyboard equivalent of the Skip tour button.
const SKIP_TOUR_KEY: &str = "Shift+Esc";

fn skip_tour_button() -> OnboardingCardButton {
    OnboardingCardButton {
        label: "Skip tour".to_string(),
        key_hint: Some(SKIP_TOUR_KEY.to_string()),
        action: OnboardingCardAction::SkipTour,
        primary: false,
    }
}

fn continue_button() -> OnboardingCardButton {
    OnboardingCardButton {
        label: "Got it".to_string(),
        key_hint: Some("Enter".to_string()),
        action: OnboardingCardAction::Continue,
        primary: true,
    }
}

fn background_mode_buttons() -> Vec<OnboardingCardButton> {
    vec![
        OnboardingCardButton {
            label: "Set up".to_string(),
            key_hint: Some("Y".to_string()),
            action: OnboardingCardAction::SetUpBackgroundMode,
            primary: true,
        },
        OnboardingCardButton {
            label: "Not now".to_string(),
            key_hint: Some("N".to_string()),
            action: OnboardingCardAction::SkipBackgroundMode,
            primary: false,
        },
    ]
}

/// The toolbar-and-exit step: where the tools are, how to hide them, and how
/// to leave. Bindings come from the live keymap.
pub(super) fn toolbar_exit_body(exit: &str, toggle_toolbar: Option<&str>) -> String {
    match toggle_toolbar {
        Some(toggle) => format!(
            "Tools, colors, and Undo live in the toolbar; {toggle} hides or shows it. \
             Press {exit} to leave the overlay when you are done."
        ),
        None => format!(
            "Tools, colors, and Undo live in the toolbar. \
             Press {exit} to leave the overlay when you are done."
        ),
    }
}

/// "Step N / total". The background prompt is the last step and is skipped
/// once answered, so a tour that already answered it counts five steps.
pub(super) fn first_run_step_eyebrow(step: FirstRunStep, background_pending: bool) -> String {
    let number = match step {
        FirstRunStep::WaitDraw | FirstRunStep::DrawUndo => 1,
        FirstRunStep::ToolbarExit => 2,
        FirstRunStep::ColorThickness => 3,
        FirstRunStep::QuickAccess => 4,
        FirstRunStep::RadialFlick | FirstRunStep::Reference => 5,
        FirstRunStep::BackgroundModeSetup => 6,
    };
    let total = if background_pending || step == FirstRunStep::BackgroundModeSetup {
        6
    } else {
        5
    };
    format!("Step {number} / {total}")
}
