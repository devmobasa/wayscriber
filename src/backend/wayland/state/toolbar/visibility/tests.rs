use super::*;
use crate::backend::wayland::state::focus::FocusState;

fn separate_toolbar_policy_input() -> KeyboardInteractivityPolicyInput {
    KeyboardInteractivityPolicyInput {
        keyboard_release_requested: false,
        main_layer_focus_acquiring: false,
        layer_shell_available: true,
        separate_toolbar_visible: true,
        inline_toolbars_active: false,
        canvas_modal_active: false,
    }
}

#[test]
fn pending_main_layer_focus_acquisition_requests_exclusive_keyboard() {
    let input = KeyboardInteractivityPolicyInput {
        main_layer_focus_acquiring: true,
        ..separate_toolbar_policy_input()
    };

    assert_eq!(
        keyboard_interactivity_for(input),
        KeyboardInteractivity::Exclusive
    );
}

#[test]
fn keyboard_interactivity_policy_preserves_release_toolbar_and_fallback_precedence() {
    let cases = [
        (
            "release overrides acquisition",
            KeyboardInteractivityPolicyInput {
                keyboard_release_requested: true,
                main_layer_focus_acquiring: true,
                ..separate_toolbar_policy_input()
            },
            KeyboardInteractivity::None,
        ),
        (
            "release overrides steady state",
            KeyboardInteractivityPolicyInput {
                keyboard_release_requested: true,
                ..separate_toolbar_policy_input()
            },
            KeyboardInteractivity::None,
        ),
        (
            "separate toolbar uses on-demand after acquisition",
            separate_toolbar_policy_input(),
            KeyboardInteractivity::OnDemand,
        ),
        (
            "hidden toolbar keeps exclusive after acquisition",
            KeyboardInteractivityPolicyInput {
                separate_toolbar_visible: false,
                ..separate_toolbar_policy_input()
            },
            KeyboardInteractivity::Exclusive,
        ),
        (
            "hidden toolbar keeps exclusive during acquisition",
            KeyboardInteractivityPolicyInput {
                main_layer_focus_acquiring: true,
                separate_toolbar_visible: false,
                ..separate_toolbar_policy_input()
            },
            KeyboardInteractivity::Exclusive,
        ),
        (
            "xdg fallback keeps exclusive after acquisition",
            KeyboardInteractivityPolicyInput {
                layer_shell_available: false,
                ..separate_toolbar_policy_input()
            },
            KeyboardInteractivity::Exclusive,
        ),
        (
            "xdg fallback keeps exclusive during acquisition",
            KeyboardInteractivityPolicyInput {
                main_layer_focus_acquiring: true,
                layer_shell_available: false,
                ..separate_toolbar_policy_input()
            },
            KeyboardInteractivity::Exclusive,
        ),
        (
            "inline toolbar keeps exclusive after acquisition",
            KeyboardInteractivityPolicyInput {
                inline_toolbars_active: true,
                ..separate_toolbar_policy_input()
            },
            KeyboardInteractivity::Exclusive,
        ),
        (
            "inline toolbar keeps exclusive during acquisition",
            KeyboardInteractivityPolicyInput {
                main_layer_focus_acquiring: true,
                inline_toolbars_active: true,
                ..separate_toolbar_policy_input()
            },
            KeyboardInteractivity::Exclusive,
        ),
        (
            "canvas modal keeps exclusive after acquisition",
            KeyboardInteractivityPolicyInput {
                canvas_modal_active: true,
                ..separate_toolbar_policy_input()
            },
            KeyboardInteractivity::Exclusive,
        ),
        (
            "canvas modal keeps exclusive during acquisition",
            KeyboardInteractivityPolicyInput {
                main_layer_focus_acquiring: true,
                canvas_modal_active: true,
                ..separate_toolbar_policy_input()
            },
            KeyboardInteractivity::Exclusive,
        ),
    ];

    for (name, input, expected) in cases {
        assert_eq!(keyboard_interactivity_for(input), expected, "{name}");
    }
}

#[test]
fn superseded_enter_policy_sequence_retries_acquisition_before_steady_state() {
    let mut input = KeyboardInteractivityPolicyInput {
        main_layer_focus_acquiring: true,
        ..separate_toolbar_policy_input()
    };

    assert_eq!(
        keyboard_interactivity_for(input),
        KeyboardInteractivity::Exclusive
    );

    input.keyboard_release_requested = true;
    assert_eq!(
        keyboard_interactivity_for(input),
        KeyboardInteractivity::None
    );

    input.keyboard_release_requested = false;
    assert_eq!(
        keyboard_interactivity_for(input),
        KeyboardInteractivity::Exclusive
    );

    input.main_layer_focus_acquiring = false;
    assert_eq!(
        keyboard_interactivity_for(input),
        KeyboardInteractivity::OnDemand
    );
}

#[test]
fn acquired_release_restores_separate_toolbar_on_demand_policy() {
    let mut input = separate_toolbar_policy_input();

    assert_eq!(
        keyboard_interactivity_for(input),
        KeyboardInteractivity::OnDemand
    );

    input.keyboard_release_requested = true;
    assert_eq!(
        keyboard_interactivity_for(input),
        KeyboardInteractivity::None
    );

    input.keyboard_release_requested = false;
    assert_eq!(
        keyboard_interactivity_for(input),
        KeyboardInteractivity::OnDemand
    );
}

#[test]
fn retained_suppression_follows_acquisition_and_steady_state_policy() {
    let mut input = KeyboardInteractivityPolicyInput {
        main_layer_focus_acquiring: true,
        ..separate_toolbar_policy_input()
    };

    assert_eq!(
        keyboard_interactivity_for(input),
        KeyboardInteractivity::Exclusive
    );

    input.main_layer_focus_acquiring = false;
    assert_eq!(
        keyboard_interactivity_for(input),
        KeyboardInteractivity::OnDemand
    );
}

#[test]
fn stale_enter_sequence_preserves_acquisition_until_a_valid_exclusive_enter() {
    let mut focus = FocusState::new(None);
    let mut input = KeyboardInteractivityPolicyInput {
        main_layer_focus_acquiring: focus.main_layer_acquiring(),
        ..separate_toolbar_policy_input()
    };
    let mut committed = keyboard_interactivity_for(input);
    assert_eq!(committed, KeyboardInteractivity::Exclusive);
    focus.set_keyboard_interactivity(Some(committed));

    input.keyboard_release_requested = true;
    committed = keyboard_interactivity_for(input);
    assert_eq!(committed, KeyboardInteractivity::None);
    focus.set_keyboard_interactivity(Some(committed));
    assert!(!focus.can_complete_main_layer_acquisition(true, input.keyboard_release_requested,));
    assert!(focus.main_layer_acquiring());

    input.keyboard_release_requested = false;
    assert!(!focus.can_complete_main_layer_acquisition(true, input.keyboard_release_requested,));
    assert!(focus.main_layer_acquiring());

    committed = keyboard_interactivity_for(input);
    assert_eq!(committed, KeyboardInteractivity::Exclusive);
    focus.set_keyboard_interactivity(Some(committed));
    assert!(focus.can_complete_main_layer_acquisition(true, input.keyboard_release_requested,));
    assert!(focus.complete_main_layer_acquisition());
    assert!(!focus.main_layer_acquiring());

    input.main_layer_focus_acquiring = focus.main_layer_acquiring();
    committed = keyboard_interactivity_for(input);
    assert_eq!(committed, KeyboardInteractivity::OnDemand);
}
