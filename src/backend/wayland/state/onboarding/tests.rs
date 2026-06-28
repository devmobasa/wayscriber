use super::first_run::{
    background_mode_prompt_active, background_mode_prompt_choice,
    first_run_card_hidden_by_ui_state, first_run_skip_allowed, first_run_step_eyebrow,
    quick_access_completed,
};
use crate::config::RadialMenuMouseBinding;
use crate::input::Key;
use crate::onboarding::{FirstRunStep, OnboardingState};

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

#[test]
fn first_run_eyebrow_shows_progress() {
    assert_eq!(
        first_run_step_eyebrow(FirstRunStep::BackgroundModeSetup),
        "Step 1 / 5"
    );
    assert_eq!(first_run_step_eyebrow(FirstRunStep::WaitDraw), "Step 2 / 5");
    assert_eq!(first_run_step_eyebrow(FirstRunStep::DrawUndo), "Step 3 / 5");
    assert_eq!(
        first_run_step_eyebrow(FirstRunStep::QuickAccess),
        "Step 4 / 5"
    );
    assert_eq!(
        first_run_step_eyebrow(FirstRunStep::Reference),
        "Step 5 / 5"
    );
}

#[test]
fn background_mode_prompt_choice_accepts_yes_and_no_keys() {
    assert_eq!(background_mode_prompt_choice(Key::Char('y')), Some(true));
    assert_eq!(background_mode_prompt_choice(Key::Char('Y')), Some(true));
    assert_eq!(background_mode_prompt_choice(Key::Char('n')), Some(false));
    assert_eq!(background_mode_prompt_choice(Key::Char('N')), Some(false));
    assert_eq!(background_mode_prompt_choice(Key::Char('x')), None);
    assert_eq!(background_mode_prompt_choice(Key::Escape), None);
}

#[test]
fn background_mode_prompt_active_requires_step_and_visible_card() {
    let mut state = OnboardingState {
        active_step: Some(FirstRunStep::BackgroundModeSetup),
        ..OnboardingState::default()
    };
    assert!(background_mode_prompt_active(&state, true));
    assert!(!background_mode_prompt_active(&state, false));

    state.active_step = Some(FirstRunStep::WaitDraw);
    assert!(!background_mode_prompt_active(&state, true));

    state.active_step = Some(FirstRunStep::BackgroundModeSetup);
    state.first_run_completed = true;
    assert!(!background_mode_prompt_active(&state, true));
}

#[test]
fn quick_access_completes_when_radial_unavailable_and_context_disabled() {
    let state = OnboardingState::default();
    assert!(quick_access_completed(
        &state,
        false,
        RadialMenuMouseBinding::Middle,
        false,
        false,
        true,
    ));
}

#[test]
fn quick_access_waives_context_when_radial_uses_right_click_without_context_shortcut() {
    let state = OnboardingState {
        used_radial_menu: true,
        ..OnboardingState::default()
    };
    assert!(quick_access_completed(
        &state,
        true,
        RadialMenuMouseBinding::Right,
        true,
        false,
        true,
    ));
}

#[test]
fn quick_access_blocks_when_toolbar_required_and_still_hidden() {
    let mut state = OnboardingState {
        quick_access_requires_toolbar: true,
        ..OnboardingState::default()
    };
    assert!(!quick_access_completed(
        &state,
        false,
        RadialMenuMouseBinding::Middle,
        false,
        false,
        false,
    ));
    state.used_toolbar_toggle = true;
    assert!(quick_access_completed(
        &state,
        false,
        RadialMenuMouseBinding::Middle,
        false,
        false,
        false,
    ));
}
