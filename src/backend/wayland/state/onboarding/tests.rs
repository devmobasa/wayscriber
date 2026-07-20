use super::first_run::{
    apply_persisted_usage_signals, background_mode_prompt_active, background_mode_prompt_choice,
    color_thickness_completed, first_run_card_hidden_by_ui_state, first_run_skip_allowed,
    first_run_step_eyebrow, quick_access_completed, radial_flick_completed, shortcut_rebind_footer,
};
use super::{capability_toast_message, shortcut_coach_should_fire};
use crate::config::{RadialMenuMouseBinding, ToolbarRebindModifier};
use crate::input::state::CompositorCapabilities;
use crate::input::{Key, state::PendingOnboardingUsage};
use crate::onboarding::{DEFERRED_HINT_REPEAT_MAX, FirstRunStep, OnboardingState};
use std::time::{Duration, Instant};

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
        "Step 1 / 7"
    );
    assert_eq!(first_run_step_eyebrow(FirstRunStep::WaitDraw), "Step 2 / 7");
    assert_eq!(first_run_step_eyebrow(FirstRunStep::DrawUndo), "Step 3 / 7");
    assert_eq!(
        first_run_step_eyebrow(FirstRunStep::ColorThickness),
        "Step 4 / 7"
    );
    assert_eq!(
        first_run_step_eyebrow(FirstRunStep::QuickAccess),
        "Step 5 / 7"
    );
    assert_eq!(
        first_run_step_eyebrow(FirstRunStep::RadialFlick),
        "Step 6 / 7"
    );
    assert_eq!(
        first_run_step_eyebrow(FirstRunStep::Reference),
        "Step 7 / 7"
    );
}

#[test]
fn color_thickness_step_requires_both_color_and_thickness() {
    let mut state = OnboardingState::default();
    assert!(!color_thickness_completed(&state));

    state.first_color_done = true;
    assert!(
        !color_thickness_completed(&state),
        "color alone must not complete the step"
    );

    state.first_thickness_done = true;
    assert!(color_thickness_completed(&state));
}

#[test]
fn radial_flick_step_completes_on_flick_or_is_waived_when_unavailable() {
    let mut state = OnboardingState::default();

    // Radial available but no flick yet: still blocked.
    assert!(!radial_flick_completed(&state, true));

    // A flick commit completes it.
    state.radial_flick_done = true;
    assert!(radial_flick_completed(&state, true));

    // Without a flick, an unavailable radial menu waives the step.
    state.radial_flick_done = false;
    assert!(radial_flick_completed(&state, false));
}

#[test]
fn v3_onboarding_toml_loads_with_new_fields_defaulted() {
    // A pre-v4 file has none of the new first-run/coach fields. Serde defaults
    // must fill them in so the file still loads (backward compatible).
    let seed = "\
version = 3
welcome_shown = true
toolbar_hint_shown = true
first_run_completed = true
used_help_overlay = true
";
    let state: OnboardingState = toml::from_str(seed).expect("v3 file should still parse");

    assert!(state.welcome_shown);
    assert!(state.first_run_completed);
    assert!(state.used_help_overlay);
    // New F4 first-run teaching fields default off.
    assert!(!state.first_color_done);
    assert!(!state.first_thickness_done);
    assert!(!state.radial_flick_done);
    // New F5 coach bookkeeping defaults off/zero.
    assert!(!state.coach_hint_shown);
    assert_eq!(state.coach_hint_count, 0);
}

fn coach_now() -> Instant {
    Instant::now()
}

#[test]
fn shortcut_coach_fires_only_at_threshold() {
    let now = coach_now();
    // Below threshold: no fire.
    assert!(!shortcut_coach_should_fire(2, 0, false, 0, None, now));
    // At threshold with an idle history: fires.
    assert!(shortcut_coach_should_fire(3, 0, false, 0, None, now));
    assert!(shortcut_coach_should_fire(9, 0, false, 0, None, now));
}

#[test]
fn shortcut_coach_respects_cooldown() {
    let start = coach_now();
    // Just fired: still within cooldown -> suppressed.
    assert!(!shortcut_coach_should_fire(
        3,
        1,
        false,
        1,
        Some(start),
        start + Duration::from_secs(10)
    ));
    // After the cooldown elapses -> allowed again.
    assert!(shortcut_coach_should_fire(
        3,
        1,
        false,
        1,
        Some(start),
        start + Duration::from_secs(120)
    ));
}

#[test]
fn shortcut_coach_honors_per_session_cap() {
    let now = coach_now();
    // At the per-session cap of 2, no further coach hints this session.
    assert!(!shortcut_coach_should_fire(9, 2, false, 0, None, now));
    // One below the cap still fires.
    assert!(shortcut_coach_should_fire(9, 1, false, 0, None, now));
}

#[test]
fn shortcut_coach_suppressed_once_learned_or_capped() {
    let now = coach_now();
    // Learned (fully taught) -> permanently suppressed.
    assert!(!shortcut_coach_should_fire(9, 0, true, 0, None, now));
    // Across-session cap reached -> suppressed even if not flagged learned.
    assert!(!shortcut_coach_should_fire(
        9,
        0,
        false,
        DEFERRED_HINT_REPEAT_MAX,
        None,
        now
    ));
}

#[test]
fn shortcut_rebind_footer_uses_configured_modifier() {
    for (modifier, expected_chord) in [
        (ToolbarRebindModifier::CtrlShift, "Ctrl+Shift+click"),
        (ToolbarRebindModifier::CtrlAlt, "Ctrl+Alt+click"),
        (ToolbarRebindModifier::ShiftAlt, "Shift+Alt+click"),
        (ToolbarRebindModifier::CtrlShiftAlt, "Ctrl+Shift+Alt+click"),
    ] {
        let footer = shortcut_rebind_footer(modifier);
        assert!(footer.contains(expected_chord), "footer={footer:?}");
        assert!(footer.contains("bindable toolbar control to rebind"));
    }

    assert!(shortcut_rebind_footer(ToolbarRebindModifier::Disabled).contains("editing disabled"));
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

fn limited_caps() -> CompositorCapabilities {
    CompositorCapabilities {
        layer_shell: true,
        screencopy: true,
        freeze_capture: false,
        pointer_constraints: true,
        ..CompositorCapabilities::default()
    }
}

#[test]
fn capability_warning_shows_once_per_session() {
    let caps = limited_caps();

    let first = capability_toast_message(None, caps);
    assert!(
        first.is_some(),
        "limited capabilities warn on first evaluation"
    );

    // Same capability set again this session: no re-warning (#156).
    assert_eq!(capability_toast_message(Some(caps), caps), None);
}

#[test]
fn capability_warning_returns_when_capabilities_change() {
    let caps = limited_caps();
    let first = capability_toast_message(None, caps);

    let mut changed = caps;
    changed.layer_shell = false;
    let second = capability_toast_message(Some(caps), changed);
    assert!(second.is_some(), "changed capability state warns again");
    assert_ne!(first, second, "the summary reflects the new state");
}

#[test]
fn capability_warning_skipped_when_everything_available() {
    let caps = CompositorCapabilities {
        layer_shell: true,
        screencopy: true,
        freeze_capture: true,
        pointer_constraints: true,
        ..CompositorCapabilities::default()
    };
    assert_eq!(capability_toast_message(None, caps), None);
}

#[test]
fn persisted_usage_signals_apply_after_first_run_completion() {
    let mut state = OnboardingState {
        first_run_completed: true,
        first_run_skipped: true,
        ..OnboardingState::default()
    };
    let usage = PendingOnboardingUsage {
        first_stroke_done: true,
        first_undo_done: true,
        used_toolbar_toggle: true,
        used_radial_menu: true,
        used_context_menu_right_click: true,
        used_context_menu_keyboard: true,
        used_help_overlay: true,
        used_command_palette: true,
        ..PendingOnboardingUsage::default()
    };

    assert!(apply_persisted_usage_signals(&mut state, &usage));

    assert!(!state.first_stroke_done);
    assert!(!state.first_undo_done);
    assert!(!state.used_toolbar_toggle);
    assert!(state.used_radial_menu);
    assert!(state.used_context_menu_right_click);
    assert!(state.used_context_menu_keyboard);
    assert!(state.used_help_overlay);
    assert!(state.used_command_palette);
}
