use super::create_test_input_state;
use crate::config::{Action, TopDisplayMode};
use crate::input::state::PendingBackendAction;

#[test]
fn cycle_action_walks_full_micro_hidden_full_with_toasts() {
    let mut state = create_test_input_state();
    assert_eq!(state.top_display_state(), TopDisplayMode::Full);

    state.handle_action(Action::CycleToolbarDisplay);
    assert_eq!(state.top_display_state(), TopDisplayMode::Micro);
    assert!(
        state.toolbar_top_visible(),
        "micro keeps the surface mapped"
    );
    assert_eq!(
        state.ui_toast.as_ref().map(|toast| toast.message.as_str()),
        Some("Toolbar: micro")
    );
    assert_eq!(
        state.take_pending_backend_action(),
        Some(PendingBackendAction::PersistToolbarConfig),
        "keyboard cycle persists like the toolbar-event paths"
    );

    state.handle_action(Action::CycleToolbarDisplay);
    assert_eq!(state.top_display_state(), TopDisplayMode::Hidden);
    assert!(!state.toolbar_top_visible());
    assert!(
        state.toolbar_side_visible(),
        "the cycle only affects the top strip"
    );
    assert_eq!(
        state.ui_toast.as_ref().map(|toast| toast.message.as_str()),
        Some("Toolbar: hidden")
    );

    state.handle_action(Action::CycleToolbarDisplay);
    assert_eq!(state.top_display_state(), TopDisplayMode::Full);
    assert!(state.toolbar_top_visible());
    assert_eq!(
        state.ui_toast.as_ref().map(|toast| toast.message.as_str()),
        Some("Toolbar: full")
    );
}

#[test]
fn entering_micro_unminimizes_and_closes_top_menus() {
    let mut state = create_test_input_state();
    state.toolbar_top_minimized = true;
    state.toolbar_shapes_expanded = true;
    state.toolbar_top_overflow_open = true;

    state.handle_action(Action::CycleToolbarDisplay);
    assert_eq!(state.top_display_state(), TopDisplayMode::Micro);
    assert!(
        !state.toolbar_top_minimized,
        "micro and minimized are exclusive"
    );
    assert!(!state.toolbar_shapes_expanded);
    assert!(!state.toolbar_top_overflow_open);
}

#[test]
fn toggle_toolbar_show_restores_a_cycle_hidden_top_strip() {
    let mut state = create_test_input_state();
    state.handle_action(Action::CycleToolbarDisplay); // micro
    state.handle_action(Action::CycleToolbarDisplay); // hidden
    assert!(!state.toolbar_top_visible());

    state.handle_action(Action::ToggleToolbar); // hides side too
    state.handle_action(Action::ToggleToolbar); // shows everything
    assert!(state.toolbar_top_visible());
    assert_eq!(state.top_display_state(), TopDisplayMode::Full);
}

#[test]
fn micro_form_survives_a_visibility_toggle() {
    let mut state = create_test_input_state();
    state.handle_action(Action::CycleToolbarDisplay); // micro
    state.handle_action(Action::ToggleToolbar); // hide all
    state.handle_action(Action::ToggleToolbar); // show all
    assert_eq!(
        state.top_display_state(),
        TopDisplayMode::Micro,
        "the chip is a persisted form, like minimized"
    );
}

#[test]
fn presenter_mode_gates_the_cycle_like_toggle_toolbar() {
    let mut state = create_test_input_state();
    state.presenter_mode_config.hide_toolbars = true;
    state.toggle_presenter_mode();
    assert!(!state.toolbar_top_visible());

    state.handle_action(Action::CycleToolbarDisplay);
    assert!(
        !state.toolbar_top_visible(),
        "presenter mode owns toolbar visibility"
    );
    assert_eq!(state.toolbar_top_display_mode, TopDisplayMode::Full);
}

#[test]
fn presenter_mode_gates_the_micro_chip_event_like_the_cycle_action() {
    use crate::config::PresenterToolbarMode;
    use crate::ui::toolbar::ToolbarEvent;

    let mut state = create_test_input_state();
    state.presenter_mode_config.hide_toolbars = true;
    state.presenter_mode_config.toolbar_mode = PresenterToolbarMode::Micro;
    state.toggle_presenter_mode();
    assert_eq!(state.top_display_state(), TopDisplayMode::Micro);

    // Clicking the chip while presenter mode owns toolbar visibility is a
    // no-op. The false return also means the backend event path skips its
    // event-policy persistence, so `top_display_mode = "full"` is never
    // written to disk mid-presenter.
    assert!(
        !state.apply_toolbar_event(ToolbarEvent::SetTopDisplayMode(TopDisplayMode::Full)),
        "chip click must be ignored during presenter visibility ownership"
    );
    assert_eq!(state.top_display_state(), TopDisplayMode::Micro);

    // After presenter exit the chip works again.
    state.toggle_presenter_mode();
    assert!(!state.presenter_mode);
    state.handle_action(Action::CycleToolbarDisplay); // micro
    assert_eq!(state.top_display_state(), TopDisplayMode::Micro);
    assert!(state.apply_toolbar_event(ToolbarEvent::SetTopDisplayMode(TopDisplayMode::Full)));
    assert_eq!(state.top_display_state(), TopDisplayMode::Full);
}

#[test]
fn display_mode_init_sanitizes_hidden_to_full() {
    let mut state = create_test_input_state();
    state.init_toolbar_display_mode_from_config(TopDisplayMode::Micro);
    assert_eq!(state.toolbar_top_display_mode, TopDisplayMode::Micro);
    state.init_toolbar_display_mode_from_config(TopDisplayMode::Hidden);
    assert_eq!(state.toolbar_top_display_mode, TopDisplayMode::Full);
}

#[test]
fn micro_chip_event_restores_the_full_strip() {
    let mut state = create_test_input_state();
    state.handle_action(Action::CycleToolbarDisplay); // micro
    assert!(
        state.apply_toolbar_event(crate::ui::toolbar::ToolbarEvent::SetTopDisplayMode(
            TopDisplayMode::Full
        ))
    );
    assert_eq!(state.top_display_state(), TopDisplayMode::Full);
    // Idempotent: applying the current state reports no change.
    assert!(
        !state.apply_toolbar_event(crate::ui::toolbar::ToolbarEvent::SetTopDisplayMode(
            TopDisplayMode::Full
        ))
    );
}
