use crate::backend::wayland::state::WaylandState;
use crate::config::{RadialMenuMouseBinding, keybindings::Action};
use crate::draw::DirtyFullReason;
use crate::input::state::{Toast, ToastPriority};
use crate::input::{DrawingState, Key, state::PendingOnboardingUsage};
use crate::onboarding::{FirstRunStep, OnboardingState};
use crate::ui::OnboardingCardAction;

/// What the step machine needs to know about the live overlay.
#[derive(Debug, Clone, Copy)]
pub(super) struct FirstRunEnvironment {
    pub(super) context_enabled: bool,
    pub(super) radial_binding: RadialMenuMouseBinding,
    pub(super) radial_available: bool,
    pub(super) context_keyboard_available: bool,
    pub(super) toolbar_visible: bool,
}

/// What one pass of the step machine did.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct FirstRunAdvance {
    pub(super) changed: bool,
    pub(super) completed: bool,
}

impl WaylandState {
    /// Keyboard equivalents of the card buttons: Enter acknowledges the
    /// toolbar-and-exit step, and Y / N answer the background-mode step.
    /// Plain keys only, and only while nothing is being drawn or typed, so
    /// the card never steals a chord or a letter from text entry.
    pub(in crate::backend::wayland) fn try_handle_first_run_card_key(&mut self, key: Key) -> bool {
        if !self.first_run_onboarding_card_visible() {
            return false;
        }
        let state = self.preferences.onboarding().state();
        let Some(step) = state.active_step.filter(|_| state.first_run_active()) else {
            return false;
        };
        let modifiers = self.input_state.modifiers;
        let plain_key = !(modifiers.ctrl || modifiers.alt)
            && matches!(self.input_state.state, DrawingState::Idle);
        let Some(action) = first_run_card_key_action(step, key, plain_key) else {
            return false;
        };

        self.run_onboarding_card_action(action);
        true
    }

    /// Records the background-mode answer from the final tour step.
    pub(super) fn answer_first_run_background_mode(&mut self, enable: bool) {
        if !background_mode_prompt_active(
            self.preferences.onboarding().state(),
            self.first_run_onboarding_card_visible(),
        ) {
            return;
        }

        if enable {
            match crate::daemon::setup::setup_background_mode() {
                Ok(summary) => {
                    mark_background_mode_prompt(
                        self.preferences.onboarding_mut().state_mut(),
                        true,
                    );
                    self.save_onboarding_state();
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
                    mark_background_mode_prompt(
                        self.preferences.onboarding_mut().state_mut(),
                        false,
                    );
                    self.save_onboarding_state();
                    self.input_state.push_toast(ToastPriority::Critical, "onboarding.first_run", Toast::error(format!(
                            "Background mode setup failed: {err}. You can set this up later in Background Mode settings."
                        )));
                }
            }
        } else {
            mark_background_mode_prompt(self.preferences.onboarding_mut().state_mut(), false);
            self.save_onboarding_state();
            self.input_state.push_toast(
                ToastPriority::Info,
                "onboarding.first_run",
                Toast::info("Tour complete. Set up background mode any time in the configurator."),
            );
        }

        self.mark_first_run_card_dirty();
    }

    /// The toolbar-and-exit step has nothing to practice, so it waits for an
    /// explicit "Got it".
    pub(super) fn acknowledge_first_run_toolbar_exit(&mut self) {
        let state = self.preferences.onboarding_mut().state_mut();
        if state.active_step != Some(FirstRunStep::ToolbarExit) || state.first_run_toolbar_exit_seen
        {
            return;
        }
        state.first_run_toolbar_exit_seen = true;
        self.save_onboarding_state();
        self.mark_first_run_card_dirty();
    }

    pub(in crate::backend::wayland) fn try_skip_first_run_onboarding(&mut self) -> bool {
        if !first_run_skip_allowed(
            self.preferences.onboarding().state().first_run_active(),
            self.first_run_onboarding_card_visible(),
        ) {
            return false;
        }
        let state = self.preferences.onboarding_mut().state_mut();
        state.first_run_skipped = true;
        state.first_run_completed = true;
        state.active_step = None;
        state.quick_access_requires_toolbar = false;
        self.save_onboarding_state();
        self.input_state.push_toast(
            ToastPriority::Info,
            "onboarding.first_run",
            Toast::info("Onboarding skipped."),
        );
        self.mark_first_run_card_dirty();
        true
    }

    fn mark_first_run_card_dirty(&mut self) {
        self.input_state
            .dirty_tracker
            .mark_full_for(DirtyFullReason::FirstRunOnboarding);
        self.input_state.needs_redraw = true;
    }

    pub(super) fn first_run_onboarding_card_visible(&self) -> bool {
        if !super::automatic_onboarding_allowed(
            self.config.ui.show_onboarding_hints,
            self.preferences.onboarding().persistence_available(),
        ) || !self.surface.is_configured()
            || self.suppression.suppressed()
        {
            return false;
        }
        !first_run_card_hidden_by_ui_state(
            self.input_state.presenter_mode_active(),
            self.input_state.command_palette.is_open(),
            self.input_state.help_overlay.is_visible(),
            self.input_state.is_radial_menu_open(),
            self.input_state.is_context_menu_open(),
            self.input_state.tour.is_active(),
            self.zoom.is_engaged(),
        )
    }

    pub(super) fn apply_first_run_progress(&mut self) {
        let usage = std::mem::take(&mut self.input_state.pending_onboarding_usage);
        let environment = FirstRunEnvironment {
            context_enabled: self.input_state.context_menu_enabled(),
            radial_binding: self.input_state.radial_menu.mouse_binding(),
            radial_available: self.shortcut_label_opt(Action::ToggleRadialMenu).is_some(),
            context_keyboard_available: self.shortcut_label_opt(Action::OpenContextMenu).is_some(),
            toolbar_visible: self.input_state.toolbar_visible(),
        };

        let mut changed = false;
        let mut first_run_ui_changed = false;
        let advance = {
            let state = self.preferences.onboarding_mut().state_mut();
            let first_run_active = state.first_run_active();

            if apply_persisted_usage_signals(state, &usage) {
                changed = true;
                first_run_ui_changed |= first_run_active;
            }
            if first_run_active && apply_first_run_usage(state, &usage) {
                changed = true;
                first_run_ui_changed = true;
            }

            advance_first_run_steps(state, environment)
        };
        changed |= advance.changed;
        first_run_ui_changed |= advance.changed;

        self.finish_first_run_progress(changed, first_run_ui_changed, advance.completed);
    }

    fn finish_first_run_progress(
        &mut self,
        changed: bool,
        first_run_ui_changed: bool,
        completed_now: bool,
    ) {
        if changed {
            self.save_onboarding_state();
        }
        if first_run_ui_changed {
            self.mark_first_run_card_dirty();
        }
        if completed_now && !self.input_state.has_active_toast() {
            self.input_state.push_toast(
                ToastPriority::Info,
                "onboarding.first_run",
                Toast::info("Nice work. Onboarding complete."),
            );
        }
    }
}

/// Folds this tick's teaching signals into the tour's checklist state.
fn apply_first_run_usage(state: &mut OnboardingState, usage: &PendingOnboardingUsage) -> bool {
    let mut changed = false;

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
    if usage.used_color_change && !state.first_color_done {
        state.first_color_done = true;
        changed = true;
    }
    if usage.used_thickness_change && !state.first_thickness_done {
        state.first_thickness_done = true;
        changed = true;
    }

    changed
}

/// Moves the tour forward past every step whose goal is already met: draw
/// and undo, toolbar and exit, color and thickness, quick access, find
/// anything, then background mode last. Pure so the whole order is testable.
pub(super) fn advance_first_run_steps(
    state: &mut OnboardingState,
    environment: FirstRunEnvironment,
) -> FirstRunAdvance {
    let mut advance = FirstRunAdvance::default();

    if !state.first_run_active() {
        if state.active_step.is_some() || state.quick_access_requires_toolbar {
            state.active_step = None;
            state.quick_access_requires_toolbar = false;
            advance.changed = true;
        }
        return advance;
    }
    if state.active_step.is_none() {
        state.active_step = Some(FirstRunStep::FIRST);
        advance.changed = true;
    }

    while let Some(step) = state.active_step {
        let next = match step {
            // Retired steps resume where their teaching moved to.
            FirstRunStep::WaitDraw => FirstRunStep::DrawUndo,
            FirstRunStep::RadialFlick => FirstRunStep::Reference,
            FirstRunStep::DrawUndo => {
                if !(state.first_stroke_done && state.first_undo_done) {
                    break;
                }
                FirstRunStep::ToolbarExit
            }
            FirstRunStep::ToolbarExit => {
                if !state.first_run_toolbar_exit_seen {
                    break;
                }
                FirstRunStep::ColorThickness
            }
            FirstRunStep::ColorThickness => {
                if !color_thickness_completed(state) {
                    break;
                }
                state.quick_access_requires_toolbar = !environment.toolbar_visible;
                FirstRunStep::QuickAccess
            }
            FirstRunStep::QuickAccess => {
                if !quick_access_completed(
                    state,
                    environment.context_enabled,
                    environment.radial_binding,
                    environment.radial_available,
                    environment.context_keyboard_available,
                    environment.toolbar_visible,
                ) {
                    break;
                }
                state.quick_access_requires_toolbar = false;
                FirstRunStep::Reference
            }
            FirstRunStep::Reference => {
                if !(state.used_help_overlay && state.used_command_palette) {
                    break;
                }
                FirstRunStep::BackgroundModeSetup
            }
            FirstRunStep::BackgroundModeSetup => {
                if !state.first_run_background_mode_prompted {
                    break;
                }
                state.first_run_completed = true;
                state.first_run_skipped = false;
                state.active_step = None;
                state.quick_access_requires_toolbar = false;
                advance.changed = true;
                advance.completed = true;
                break;
            }
        };
        state.active_step = Some(next);
        advance.changed = true;
    }

    advance
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
    if usage.used_board_picker && !state.used_board_picker {
        state.used_board_picker = true;
        changed = true;
    }
    if usage.used_zoom_control && !state.used_zoom_control {
        state.used_zoom_control = true;
        changed = true;
    }
    if usage.used_canvas_popover && !state.used_canvas_popover {
        state.used_canvas_popover = true;
        changed = true;
    }

    changed
}

pub(super) fn background_mode_prompt_active(state: &OnboardingState, card_visible: bool) -> bool {
    state.first_run_active()
        && card_visible
        && state.active_step == Some(FirstRunStep::BackgroundModeSetup)
}

/// The card action a plain key triggers on `step`, if any.
pub(super) fn first_run_card_key_action(
    step: FirstRunStep,
    key: Key,
    plain_key: bool,
) -> Option<OnboardingCardAction> {
    if !plain_key {
        return None;
    }
    match (step, key) {
        (FirstRunStep::ToolbarExit, Key::Return) => Some(OnboardingCardAction::Continue),
        (FirstRunStep::BackgroundModeSetup, Key::Char(ch)) => match ch.to_ascii_lowercase() {
            'y' => Some(OnboardingCardAction::SetUpBackgroundMode),
            'n' => Some(OnboardingCardAction::SkipBackgroundMode),
            _ => None,
        },
        _ => None,
    }
}

fn mark_background_mode_prompt(state: &mut OnboardingState, enabled: bool) {
    state.first_run_background_mode_prompted = true;
    state.first_run_background_mode_enabled = enabled;
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
