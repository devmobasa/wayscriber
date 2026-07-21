use crate::backend::wayland::state::WaylandState;
use crate::config::{RadialMenuMouseBinding, ToolbarRebindModifier, keybindings::Action};
use crate::draw::DirtyFullReason;
use crate::input::state::{Toast, ToastPriority};
use crate::input::{Key, state::PendingOnboardingUsage};
use crate::onboarding::{FirstRunStep, OnboardingState};
use crate::ui::{OnboardingCard, OnboardingChecklistItem};

impl WaylandState {
    pub(in crate::backend::wayland) fn try_handle_first_run_background_mode_choice(
        &mut self,
        key: Key,
    ) -> bool {
        if !background_mode_prompt_active(
            self.onboarding.state(),
            self.first_run_onboarding_card_visible(),
        ) {
            return false;
        }

        let Some(enable_background_mode) = background_mode_prompt_choice(key) else {
            return false;
        };

        if enable_background_mode {
            match crate::daemon::setup::setup_background_mode() {
                Ok(summary) => {
                    mark_background_mode_prompt(self.onboarding.state_mut(), true);
                    self.onboarding.save();
                    self.input_state.push_toast(
                        ToastPriority::Info,
                        "onboarding.first_run",
                        Toast::info(format!(
                            "Background mode enabled. Service file: {}",
                            summary.service_path.display()
                        )),
                    );
                }
                Err(err) => {
                    mark_background_mode_prompt(self.onboarding.state_mut(), false);
                    self.onboarding.save();
                    self.input_state.push_toast(ToastPriority::Critical, "onboarding.first_run", Toast::error(format!(
                            "Background mode setup failed: {err}. You can set this up later in Background Mode settings."
                        )));
                }
            }
        } else {
            mark_background_mode_prompt(self.onboarding.state_mut(), false);
            self.onboarding.save();
            self.input_state.push_toast(
                ToastPriority::Info,
                "onboarding.first_run",
                Toast::info("Skipped background mode setup for now."),
            );
        }

        self.input_state
            .dirty_tracker
            .mark_full_for(DirtyFullReason::FirstRunOnboarding);
        self.input_state.needs_redraw = true;
        true
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
        self.input_state.push_toast(
            ToastPriority::Info,
            "onboarding.first_run",
            Toast::info("Onboarding skipped."),
        );
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
        let eyebrow = first_run_step_eyebrow(step);
        let footer = "Shift+Escape to skip".to_string();

        let card = match step {
            FirstRunStep::BackgroundModeSetup => OnboardingCard {
                eyebrow: eyebrow.to_string(),
                title: "Enable background mode?".to_string(),
                body: "Keeps Wayscriber ready in the background for quick overlay access."
                    .to_string(),
                items: Vec::new(),
                footer: "Y = set up now   •   N = skip   •   Shift+Escape = skip onboarding"
                    .to_string(),
            },
            FirstRunStep::WaitDraw => OnboardingCard {
                eyebrow: eyebrow.to_string(),
                title: "Draw one mark".to_string(),
                body: "Draw one quick stroke anywhere on the canvas.".to_string(),
                items: vec![OnboardingChecklistItem {
                    label: "Draw a stroke".to_string(),
                    done: state.first_stroke_done,
                }],
                footer,
            },
            FirstRunStep::DrawUndo => OnboardingCard {
                eyebrow: eyebrow.to_string(),
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
            FirstRunStep::ColorThickness => {
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
                OnboardingCard {
                    eyebrow: eyebrow.to_string(),
                    title: "Color and thickness".to_string(),
                    body: "Recolor and resize your strokes without leaving the canvas.".to_string(),
                    items: vec![
                        OnboardingChecklistItem {
                            label: color_label,
                            done: state.first_color_done,
                        },
                        OnboardingChecklistItem {
                            label: thickness_label,
                            done: state.first_thickness_done,
                        },
                    ],
                    footer,
                }
            }
            FirstRunStep::QuickAccess => {
                let items = self.quick_access_checklist_items(state);
                OnboardingCard {
                    eyebrow: eyebrow.to_string(),
                    title: "Quick access at cursor".to_string(),
                    body: "Open quick actions near the pointer.".to_string(),
                    items,
                    footer,
                }
            }
            FirstRunStep::RadialFlick => {
                let radial_label = self.shortcut_label_opt(Action::ToggleRadialMenu);
                let radial_available = radial_label.is_some();
                let item_label = match radial_label {
                    Some(label) => format!("Flick-commit from the radial menu ({label})"),
                    None => "Radial menu disabled in config".to_string(),
                };
                OnboardingCard {
                    eyebrow: eyebrow.to_string(),
                    title: "Flick to commit".to_string(),
                    body: "Open the radial menu, then press-flick-release toward a slice: a \
                           quick flick commits the tool before the menu even paints (about \
                           220 ms)."
                        .to_string(),
                    items: vec![OnboardingChecklistItem {
                        label: item_label,
                        done: state.radial_flick_done || !radial_available,
                    }],
                    footer,
                }
            }
            FirstRunStep::Reference => OnboardingCard {
                eyebrow: eyebrow.to_string(),
                title: "Find and customize anything".to_string(),
                body: "Palette controls can edit, unbind, or reset shortcuts.".to_string(),
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
                footer: shortcut_rebind_footer(self.config.ui.toolbar.rebind_modifier),
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

    pub(super) fn apply_first_run_progress(&mut self) {
        let usage = std::mem::take(&mut self.input_state.pending_onboarding_usage);
        let context_enabled = self.input_state.context_menu_enabled();
        let radial_binding = self.input_state.radial_menu_mouse_binding;
        let radial_available = self.shortcut_label_opt(Action::ToggleRadialMenu).is_some();
        let context_keyboard_available = self.shortcut_label_opt(Action::OpenContextMenu).is_some();
        let toolbar_visible = self.input_state.toolbar_visible();

        let mut changed = false;
        let mut first_run_ui_changed = false;
        let mut completed_now = false;

        {
            let state = self.onboarding.state_mut();
            let first_run_active = state.first_run_active();

            if apply_persisted_usage_signals(state, &usage) {
                changed = true;
                first_run_ui_changed |= first_run_active;
            }

            if first_run_active {
                if usage.first_stroke_done && !state.first_stroke_done {
                    state.first_stroke_done = true;
                    changed = true;
                    first_run_ui_changed = true;
                }
                if usage.first_undo_done && !state.first_undo_done {
                    state.first_undo_done = true;
                    changed = true;
                    first_run_ui_changed = true;
                }
                if usage.used_toolbar_toggle && !state.used_toolbar_toggle {
                    state.used_toolbar_toggle = true;
                    changed = true;
                    first_run_ui_changed = true;
                }
                if usage.used_color_change && !state.first_color_done {
                    state.first_color_done = true;
                    changed = true;
                    first_run_ui_changed = true;
                }
                if usage.used_thickness_change && !state.first_thickness_done {
                    state.first_thickness_done = true;
                    changed = true;
                    first_run_ui_changed = true;
                }
                if usage.used_radial_flick && !state.radial_flick_done {
                    state.radial_flick_done = true;
                    changed = true;
                    first_run_ui_changed = true;
                }
            }

            if !first_run_active {
                if state.active_step.is_some() || state.quick_access_requires_toolbar {
                    state.active_step = None;
                    state.quick_access_requires_toolbar = false;
                    changed = true;
                    first_run_ui_changed = true;
                }
            } else if state.active_step.is_none() {
                state.active_step = Some(FirstRunStep::BackgroundModeSetup);
                changed = true;
                first_run_ui_changed = true;
            }

            while let Some(step) = state.active_step {
                match step {
                    FirstRunStep::BackgroundModeSetup => {
                        if !state.first_run_background_mode_prompted {
                            break;
                        }
                        state.active_step = Some(FirstRunStep::WaitDraw);
                        changed = true;
                        first_run_ui_changed = true;
                    }
                    FirstRunStep::WaitDraw => {
                        if !state.first_stroke_done {
                            break;
                        }
                        state.active_step = Some(FirstRunStep::DrawUndo);
                        changed = true;
                        first_run_ui_changed = true;
                    }
                    FirstRunStep::DrawUndo => {
                        if !state.first_undo_done {
                            break;
                        }
                        state.active_step = Some(FirstRunStep::ColorThickness);
                        changed = true;
                        first_run_ui_changed = true;
                    }
                    FirstRunStep::ColorThickness => {
                        if !color_thickness_completed(state) {
                            break;
                        }
                        state.active_step = Some(FirstRunStep::QuickAccess);
                        state.quick_access_requires_toolbar = !toolbar_visible;
                        changed = true;
                        first_run_ui_changed = true;
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
                        state.active_step = Some(FirstRunStep::RadialFlick);
                        state.quick_access_requires_toolbar = false;
                        changed = true;
                        first_run_ui_changed = true;
                    }
                    FirstRunStep::RadialFlick => {
                        if !radial_flick_completed(state, radial_available) {
                            break;
                        }
                        state.active_step = Some(FirstRunStep::Reference);
                        changed = true;
                        first_run_ui_changed = true;
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
                        first_run_ui_changed = true;
                        completed_now = true;
                        break;
                    }
                }
            }
        }

        if changed {
            self.onboarding.save();
        }
        if first_run_ui_changed {
            self.input_state
                .dirty_tracker
                .mark_full_for(DirtyFullReason::FirstRunOnboarding);
            self.input_state.needs_redraw = true;
        }
        if completed_now && self.input_state.ui_toast.is_none() {
            self.input_state.push_toast(
                ToastPriority::Info,
                "onboarding.first_run",
                Toast::info("Nice work. Onboarding complete."),
            );
        }
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

/// The colors/thickness teaching step completes once the user has both changed
/// a color and adjusted stroke thickness.
pub(super) fn color_thickness_completed(state: &OnboardingState) -> bool {
    state.first_color_done && state.first_thickness_done
}

/// The radial-flick teaching step completes on a flick commit, or is waived
/// when the radial menu is unbound so users without it are never stuck.
pub(super) fn radial_flick_completed(state: &OnboardingState, radial_available: bool) -> bool {
    state.radial_flick_done || !radial_available
}

pub(super) fn quick_access_completed(
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

pub(super) fn apply_persisted_usage_signals(
    state: &mut OnboardingState,
    usage: &PendingOnboardingUsage,
) -> bool {
    let mut changed = false;

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

    changed
}

pub(super) fn background_mode_prompt_active(state: &OnboardingState, card_visible: bool) -> bool {
    state.first_run_active()
        && card_visible
        && state.active_step == Some(FirstRunStep::BackgroundModeSetup)
}

pub(super) fn background_mode_prompt_choice(key: Key) -> Option<bool> {
    let Key::Char(ch) = key else {
        return None;
    };

    match ch.to_ascii_lowercase() {
        'y' => Some(true),
        'n' => Some(false),
        _ => None,
    }
}

fn mark_background_mode_prompt(state: &mut OnboardingState, enabled: bool) {
    state.first_run_background_mode_prompted = true;
    state.first_run_background_mode_enabled = enabled;
}

pub(super) fn first_run_step_eyebrow(step: FirstRunStep) -> &'static str {
    match step {
        FirstRunStep::BackgroundModeSetup => "Step 1 / 7",
        FirstRunStep::WaitDraw => "Step 2 / 7",
        FirstRunStep::DrawUndo => "Step 3 / 7",
        FirstRunStep::ColorThickness => "Step 4 / 7",
        FirstRunStep::QuickAccess => "Step 5 / 7",
        FirstRunStep::RadialFlick => "Step 6 / 7",
        FirstRunStep::Reference => "Step 7 / 7",
    }
}

pub(super) fn shortcut_rebind_footer(modifier: ToolbarRebindModifier) -> String {
    match modifier.click_label() {
        None => "Toolbar shortcut-click editing disabled • Shift+Escape to skip".to_string(),
        Some(click) => {
            format!("{click} a bindable toolbar control to rebind • Shift+Escape to skip")
        }
    }
}

pub(super) fn first_run_skip_allowed(first_run_active: bool, card_visible: bool) -> bool {
    first_run_active && card_visible
}

pub(super) fn first_run_card_hidden_by_ui_state(
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
