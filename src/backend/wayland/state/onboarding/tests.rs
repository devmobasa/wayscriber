use super::first_run::{
    FirstRunAdvance, FirstRunEnvironment, advance_first_run_steps, apply_persisted_usage_signals,
    background_mode_prompt_active, color_thickness_completed, first_run_card_hidden_by_ui_state,
    first_run_card_key_action, first_run_skip_allowed, quick_access_completed,
};
use super::first_run_card::{first_run_step_eyebrow, toolbar_exit_body};
use super::{
    acknowledge_tip_command, automatic_onboarding_allowed, automatic_tip_toast,
    canvas_popover_hint_relevant, shortcut_coach_should_fire, status_bar_board_picker_entry,
};
use crate::config::RadialMenuMouseBinding;
use crate::domain::{Action, OnboardingTip};
use crate::input::state::{CompositorCapabilities, ToastCommand};
use crate::input::{Key, state::PendingOnboardingUsage};
use crate::onboarding::{DEFERRED_HINT_REPEAT_MAX, FirstRunStep, OnboardingState};
use crate::ui::OnboardingCardAction;
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
fn automatic_onboarding_requires_the_preference_and_durable_progress() {
    assert!(automatic_onboarding_allowed(true, true));
    assert!(!automatic_onboarding_allowed(false, true));
    assert!(!automatic_onboarding_allowed(true, false));
    assert!(!automatic_onboarding_allowed(false, false));
}

#[test]
fn automatic_tip_controls_acknowledge_the_exact_tip_before_optional_settings() {
    let toast = automatic_tip_toast("Try the board picker", OnboardingTip::StatusBar);
    let primary = toast.action.as_ref().expect("Got it action");
    let secondary = toast.secondary_action.as_ref().expect("settings action");

    assert_eq!(primary.label, "Got it");
    assert_eq!(
        primary.command,
        ToastCommand::AcknowledgeTip {
            tip: OnboardingTip::StatusBar,
            then: None,
        }
    );
    assert_eq!(secondary.label, "Tip settings…");
    assert_eq!(
        secondary.command,
        ToastCommand::AcknowledgeTip {
            tip: OnboardingTip::StatusBar,
            then: Some(Action::OpenConfiguratorOnboardingHints),
        }
    );
}

#[test]
fn tip_settings_navigation_survives_an_acknowledgement_write_failure() {
    let outcome = acknowledge_tip_command(
        Err(crate::onboarding::OnboardingSaveError::Unavailable),
        Some(Action::OpenConfiguratorOnboardingHints),
    );

    assert!(outcome.persistence_error.is_some());
    assert_eq!(
        outcome.follow_up,
        Some(Action::OpenConfiguratorOnboardingHints),
        "settings navigation must not depend on acknowledgement persistence"
    );
}

#[test]
fn first_run_eyebrow_counts_drawing_first_and_background_mode_last() {
    let steps = [
        (FirstRunStep::DrawUndo, "Step 1 / 6"),
        (FirstRunStep::WaitDraw, "Step 1 / 6"),
        (FirstRunStep::ToolbarExit, "Step 2 / 6"),
        (FirstRunStep::ColorThickness, "Step 3 / 6"),
        (FirstRunStep::QuickAccess, "Step 4 / 6"),
        (FirstRunStep::Reference, "Step 5 / 6"),
        (FirstRunStep::BackgroundModeSetup, "Step 6 / 6"),
    ];
    for (step, expected) in steps {
        assert_eq!(first_run_step_eyebrow(step, true), expected, "{step:?}");
    }

    // A tour that already answered the background prompt ends at step 5.
    assert_eq!(
        first_run_step_eyebrow(FirstRunStep::Reference, false),
        "Step 5 / 5"
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
    // First-run teaching fields absent from the old file default off.
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
fn canvas_hint_requires_reachable_full_top_strip() {
    let mut input = crate::input::state::test_support::make_test_input_state();
    input.test_set_toolbar_visibility_state(true, true, input.toolbar_top_pinned());
    input.test_set_toolbar_display_state(
        crate::config::TopDisplayMode::Full,
        input.toolbar_top_minimized(),
    );
    input.test_set_toolbar_display_state(input.toolbar_top_display_mode(), false);
    assert!(canvas_popover_hint_relevant(&input));

    input.test_set_toolbar_display_state(input.toolbar_top_display_mode(), true);
    assert!(
        !canvas_popover_hint_relevant(&input),
        "the minimized restore tab has no Canvas overflow entry"
    );
    input.test_set_toolbar_display_state(input.toolbar_top_display_mode(), false);
    input.test_set_toolbar_display_state(
        crate::config::TopDisplayMode::Micro,
        input.toolbar_top_minimized(),
    );
    assert!(!canvas_popover_hint_relevant(&input));
}

#[test]
fn status_bar_hint_requires_a_visible_board_picker_segment() {
    let mut input = crate::input::state::test_support::make_test_input_state();
    assert!(input.boards.create_board(), "test needs multiple boards");
    input.ui_visibility.show_status_bar = true;
    input.ui_visibility.status_bar_interactive = true;
    input.ui_visibility.show_status_board_badge = true;
    input.ui_visibility.show_status_page_badge = true;
    assert_eq!(status_bar_board_picker_entry(&input), Some("Board or Page"));

    input.ui_visibility.show_status_board_badge = false;
    input.ui_visibility.show_status_page_badge = false;
    assert_eq!(status_bar_board_picker_entry(&input), None);
}

#[test]
fn toolbar_exit_copy_names_the_live_bindings() {
    let body = toolbar_exit_body("Esc", Some("F9"));
    assert!(body.contains("Press Esc to leave the overlay"), "{body}");
    assert!(body.contains("F9 hides or shows it"), "{body}");

    let unbound = toolbar_exit_body("Ctrl+Q", None);
    assert!(unbound.contains("Press Ctrl+Q"), "{unbound}");
    assert!(!unbound.contains("hides or shows"), "{unbound}");
}

#[test]
fn card_keys_answer_only_their_own_step() {
    use FirstRunStep::{BackgroundModeSetup, DrawUndo, ToolbarExit};

    for (key, expected) in [
        (
            Key::Char('y'),
            Some(OnboardingCardAction::SetUpBackgroundMode),
        ),
        (
            Key::Char('Y'),
            Some(OnboardingCardAction::SetUpBackgroundMode),
        ),
        (
            Key::Char('n'),
            Some(OnboardingCardAction::SkipBackgroundMode),
        ),
        (
            Key::Char('N'),
            Some(OnboardingCardAction::SkipBackgroundMode),
        ),
        (Key::Char('x'), None),
        (Key::Escape, None),
        (Key::Return, None),
    ] {
        assert_eq!(
            first_run_card_key_action(BackgroundModeSetup, key, true),
            expected,
            "{key:?}"
        );
    }
    assert_eq!(
        first_run_card_key_action(ToolbarExit, Key::Return, true),
        Some(OnboardingCardAction::Continue)
    );

    // Y and N keep choosing a color and a sticky note on every other step.
    assert_eq!(
        first_run_card_key_action(DrawUndo, Key::Char('y'), true),
        None
    );
    assert_eq!(
        first_run_card_key_action(ToolbarExit, Key::Char('n'), true),
        None
    );
    assert_eq!(first_run_card_key_action(DrawUndo, Key::Return, true), None);
    // A chord or an in-progress gesture never answers the card.
    assert_eq!(
        first_run_card_key_action(BackgroundModeSetup, Key::Char('n'), false),
        None
    );
    assert_eq!(
        first_run_card_key_action(ToolbarExit, Key::Return, false),
        None
    );
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
    let mut input = crate::input::state::test_support::make_test_input_state();
    let caps = limited_caps();

    assert!(
        input.note_capability_toast(caps).is_some(),
        "limited capabilities warn on first evaluation"
    );
    assert_eq!(input.note_capability_toast(caps), None);
}

#[test]
fn capability_warning_returns_when_capabilities_change() {
    let mut input = crate::input::state::test_support::make_test_input_state();
    let caps = limited_caps();
    let first = input.note_capability_toast(caps);

    let mut changed = caps;
    changed.layer_shell = false;
    let second = input.note_capability_toast(changed);
    assert!(second.is_some(), "changed capability state warns again");
    assert_ne!(first, second, "the summary reflects the new state");
}

#[test]
fn capability_warning_skipped_when_everything_available() {
    let mut input = crate::input::state::test_support::make_test_input_state();
    let caps = CompositorCapabilities {
        layer_shell: true,
        screencopy: true,
        freeze_capture: true,
        pointer_constraints: true,
        ..CompositorCapabilities::default()
    };
    assert_eq!(input.note_capability_toast(caps), None);
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
        used_board_picker: true,
        used_zoom_control: true,
        used_canvas_popover: true,
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
    assert!(state.used_board_picker);
    assert!(state.used_zoom_control);
    assert!(state.used_canvas_popover);
}

fn tour_environment() -> FirstRunEnvironment {
    FirstRunEnvironment {
        context_enabled: true,
        radial_binding: RadialMenuMouseBinding::Middle,
        radial_available: true,
        context_keyboard_available: true,
        toolbar_visible: true,
    }
}

#[test]
fn the_tour_runs_value_first_and_asks_about_background_mode_last() {
    let environment = tour_environment();
    let mut state = OnboardingState::default();

    advance_first_run_steps(&mut state, environment);
    assert_eq!(state.active_step, Some(FirstRunStep::DrawUndo));

    state.first_stroke_done = true;
    advance_first_run_steps(&mut state, environment);
    assert_eq!(
        state.active_step,
        Some(FirstRunStep::DrawUndo),
        "drawing alone does not finish the step"
    );
    state.first_undo_done = true;
    advance_first_run_steps(&mut state, environment);
    assert_eq!(state.active_step, Some(FirstRunStep::ToolbarExit));

    // The toolbar-and-exit step waits for "Got it".
    advance_first_run_steps(&mut state, environment);
    assert_eq!(state.active_step, Some(FirstRunStep::ToolbarExit));
    state.first_run_toolbar_exit_seen = true;
    advance_first_run_steps(&mut state, environment);
    assert_eq!(state.active_step, Some(FirstRunStep::ColorThickness));

    state.first_color_done = true;
    state.first_thickness_done = true;
    advance_first_run_steps(&mut state, environment);
    assert_eq!(state.active_step, Some(FirstRunStep::QuickAccess));

    state.used_radial_menu = true;
    state.used_context_menu_right_click = true;
    advance_first_run_steps(&mut state, environment);
    assert_eq!(state.active_step, Some(FirstRunStep::Reference));

    state.used_help_overlay = true;
    state.used_command_palette = true;
    advance_first_run_steps(&mut state, environment);
    assert_eq!(state.active_step, Some(FirstRunStep::BackgroundModeSetup));
    assert!(state.first_run_active());

    state.first_run_background_mode_prompted = true;
    let advance = advance_first_run_steps(&mut state, environment);
    assert_eq!(
        advance,
        FirstRunAdvance {
            changed: true,
            completed: true
        }
    );
    assert!(state.first_run_completed);
    assert_eq!(state.active_step, None);
}

#[test]
fn an_answered_background_prompt_ends_the_tour_after_find_anything() {
    let mut state = OnboardingState {
        active_step: Some(FirstRunStep::Reference),
        first_run_background_mode_prompted: true,
        used_help_overlay: true,
        used_command_palette: true,
        ..OnboardingState::default()
    };

    let advance = advance_first_run_steps(&mut state, tour_environment());

    assert!(advance.completed);
    assert!(state.first_run_completed);
}

#[test]
fn retired_steps_resume_in_the_new_order() {
    let mut state = OnboardingState {
        active_step: Some(FirstRunStep::WaitDraw),
        ..OnboardingState::default()
    };
    advance_first_run_steps(&mut state, tour_environment());
    assert_eq!(state.active_step, Some(FirstRunStep::DrawUndo));

    state.active_step = Some(FirstRunStep::RadialFlick);
    advance_first_run_steps(&mut state, tour_environment());
    assert_eq!(state.active_step, Some(FirstRunStep::Reference));
}

#[test]
fn a_finished_tour_clears_any_leftover_step() {
    let mut state = OnboardingState {
        first_run_completed: true,
        active_step: Some(FirstRunStep::QuickAccess),
        quick_access_requires_toolbar: true,
        ..OnboardingState::default()
    };

    let advance = advance_first_run_steps(&mut state, tour_environment());

    assert!(advance.changed && !advance.completed);
    assert_eq!(state.active_step, None);
    assert!(!state.quick_access_requires_toolbar);
}
