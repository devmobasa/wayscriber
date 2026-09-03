use super::create_test_input_state;
use crate::config::{Action, InputHudConfig};
use crate::input::state::{InputHudActiveSource, InputHudEntryKind, InputHudSettings};
use crate::input::{InputState, Key, Modifiers, MouseButton};
use crate::ui::toolbar::ToolbarEvent;

fn enabled_hud_state() -> InputState {
    let mut state = create_test_input_state();
    state.init_input_hud_from_config(InputHudSettings::from(&InputHudConfig {
        enabled: true,
        ..InputHudConfig::default()
    }));
    state
}

#[test]
fn toggle_input_hud_action_changes_state_and_redraws() {
    let mut state = create_test_input_state();
    assert!(!state.input_hud_enabled());

    state.handle_action(Action::ToggleInputHud);
    assert!(state.input_hud_enabled());
    assert!(state.needs_redraw);

    state.needs_redraw = false;
    state.handle_action(Action::ToggleInputHud);
    assert!(!state.input_hud_enabled());
    assert!(state.needs_redraw);
}

/// Enabling defers the source announcement to the backend: the effective
/// source is only known after `sync_input_monitor` reconciles the reader
/// thread, so the toggle itself must not claim one. Disabling is
/// source-independent and toasts immediately.
#[test]
fn toggle_input_hud_defers_the_source_announcement_to_the_backend() {
    let mut state = create_test_input_state();
    state.handle_action(Action::ToggleInputHud);
    assert!(
        state.active_toast().is_none(),
        "the enable path must not toast a source before reconciliation"
    );
    assert!(
        state.take_input_hud_source_announce(),
        "a runtime enable requests the post-reconciliation announcement"
    );
    assert!(
        !state.take_input_hud_source_announce(),
        "the announcement request is consumed by the take"
    );

    state.handle_action(Action::ToggleInputHud);
    let toast = state.active_toast().expect("disable toast");
    assert_eq!(toast.message, "Input HUD disabled");
    assert!(
        !state.take_input_hud_source_announce(),
        "disabling never leaves a stale announcement pending"
    );
}

/// Presenter mode owns the HUD while it forces it on, so the manual toggle is
/// swallowed exactly like `ToggleClickHighlight` is.
#[test]
fn presenter_mode_forces_input_hud_and_gates_the_manual_toggle() {
    let mut state = create_test_input_state();
    state.presenter_mode_config.enable_input_hud = true;

    assert!(!state.input_hud_enabled());
    state.toggle_presenter_mode();
    assert!(state.presenter_mode);
    assert!(state.input_hud_enabled());
    assert!(
        state.take_input_hud_source_announce(),
        "a presenter-forced enable announces its source too - system capture \
         starting is privacy-relevant regardless of what flipped the toggle"
    );

    state.handle_action(Action::ToggleInputHud);
    assert!(
        state.input_hud_enabled(),
        "presenter mode must swallow the manual toggle while it forces the HUD on"
    );

    state.toggle_presenter_mode();
    assert!(!state.presenter_mode);
    assert!(
        !state.input_hud_enabled(),
        "exiting presenter mode restores the pre-presenter value"
    );
}

/// Presenter mode leaves an already-enabled HUD on after exit.
#[test]
fn presenter_mode_restores_a_manually_enabled_input_hud() {
    let mut state = create_test_input_state();
    state.presenter_mode_config.enable_input_hud = true;
    state.handle_action(Action::ToggleInputHud);
    assert!(state.input_hud_enabled());

    state.toggle_presenter_mode();
    state.toggle_presenter_mode();
    assert!(state.input_hud_enabled());
}

#[test]
fn overlay_key_presses_populate_the_chip_row() {
    let mut state = enabled_hud_state();
    state.note_input_hud_key(Key::Char('z'), Modifiers::new());
    let labels: Vec<_> = state
        .input_hud_entries()
        .map(|entry| entry.label().to_string())
        .collect();
    assert_eq!(labels, vec!["Z".to_string()]);
    assert!(state.input_hud_visible());
}

#[test]
fn mouse_and_scroll_chips_carry_their_own_chrome() {
    let mut state = enabled_hud_state();
    state.note_input_hud_mouse("Click", Modifiers::new());
    state.note_input_hud_scroll(false, Modifiers::new());

    let kinds: Vec<_> = state
        .input_hud_entries()
        .map(|entry| entry.kind())
        .collect();
    assert_eq!(
        kinds,
        vec![InputHudEntryKind::Mouse, InputHudEntryKind::Scroll]
    );
}

/// While the system monitor owns reporting, the overlay hooks stay silent so a
/// press is never shown twice.
#[test]
fn system_source_suppresses_overlay_notes() {
    let mut state = enabled_hud_state();
    assert!(state.set_input_hud_source(InputHudActiveSource::System));

    state.note_input_hud_key(Key::Char('a'), Modifiers::new());
    state.note_input_hud_mouse("Click", Modifiers::new());
    state.note_input_hud_scroll(true, Modifiers::new());
    assert_eq!(state.input_hud_entries().len(), 0);

    state.note_input_hud_system_key("A".to_string(), false);
    assert_eq!(state.input_hud_entries().len(), 1);
}

/// Disabling the HUD clears the row, so re-enabling never resurrects chips from
/// an earlier session of the feature.
#[test]
fn disabling_the_hud_drops_its_chips() {
    let mut state = enabled_hud_state();
    state.note_input_hud_key(Key::Char('a'), Modifiers::new());
    assert!(state.input_hud_visible());

    state.handle_action(Action::ToggleInputHud);
    assert!(!state.input_hud_enabled());
    assert_eq!(state.input_hud_entries().len(), 0);

    state.handle_action(Action::ToggleInputHud);
    assert!(state.input_hud_enabled());
    assert!(!state.input_hud_visible());
}

/// The Settings checkbox drives the same runtime state as the action, and the
/// presenter gate applies to it too.
#[test]
fn toolbar_checkbox_toggles_the_input_hud() {
    let mut state = create_test_input_state();
    assert!(state.apply_toolbar_event(ToolbarEvent::ToggleInputHud(true)));
    assert!(state.input_hud_enabled());
    assert!(state.apply_toolbar_event(ToolbarEvent::ToggleInputHud(false)));
    assert!(!state.input_hud_enabled());

    state.presenter_mode_config.enable_input_hud = true;
    state.toggle_presenter_mode();
    assert!(state.input_hud_enabled());
    assert!(!state.apply_toolbar_event(ToolbarEvent::ToggleInputHud(false)));
    assert!(state.input_hud_enabled());
}

/// The HUD reports the physical press even when another subsystem consumes it,
/// which is what makes it useful while demoing wayscriber itself.
#[test]
fn drawing_presses_still_report_to_the_hud() {
    let mut state = enabled_hud_state();
    state.note_input_hud_mouse("Click", Modifiers::new());
    state.on_mouse_press(MouseButton::Left, 10, 10);
    state.on_mouse_release(MouseButton::Left, 20, 20);
    assert_eq!(state.input_hud_entries().len(), 1);
}
