use super::create_test_input_state;
use crate::config::{Action, RadialMenuMouseBinding, TopDisplayMode};
use crate::input::state::core::{ContextMenuKind, MenuCommand};
use crate::input::state::{InputState, PendingBackendAction};
use std::collections::HashMap;

fn unbind_chrome_visibility_actions(state: &mut InputState) {
    let mut bindings = HashMap::new();
    bindings.insert(Action::ToggleToolbar, Vec::new());
    bindings.insert(Action::ToggleStatusBar, Vec::new());
    state.set_action_bindings(bindings);
}

fn hide_all_chrome(state: &mut InputState) {
    state.handle_action(Action::ToggleToolbar);
    state.handle_action(Action::ToggleStatusBar);
    assert!(!state.toolbar_visible());
    assert!(!state.show_status_bar);
}

#[test]
fn cycle_action_walks_full_micro_hidden_full_with_toasts() {
    let mut state = create_test_input_state();
    // The "cycle only affects the top strip" assertion below observes the
    // side palette: opt into the deprecated Panel escape hatch (the struct
    // default is Pill, which retires the side surface).
    state.init_toolbar_side_layout_from_config(crate::config::ToolbarSideLayout::Panel);
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
    // Two-press semantics are Panel-specific (the still-visible side
    // palette makes the first press a hide); the single-press Pill variant
    // is covered below.
    state.init_toolbar_side_layout_from_config(crate::config::ToolbarSideLayout::Panel);
    state.handle_action(Action::CycleToolbarDisplay); // micro
    state.handle_action(Action::CycleToolbarDisplay); // hidden
    assert!(!state.toolbar_top_visible());

    state.handle_action(Action::ToggleToolbar); // hides side too
    state.handle_action(Action::ToggleToolbar); // shows everything
    assert!(state.toolbar_top_visible());
    assert_eq!(state.top_display_state(), TopDisplayMode::Full);
}

#[test]
fn toggle_toolbar_restores_a_cycle_hidden_strip_under_pill_layout() {
    let mut state = create_test_input_state();
    // The shipping default: the side palette is retired by the pill layout,
    // so a cycle-hidden top strip leaves NO visible toolbar surface while
    // every raw visibility flag stays true. The raw-flag early return in
    // set_toolbar_visible used to swallow the restore in exactly this
    // state, leaving F9 (and everything else dispatching ToggleToolbar)
    // dead.
    state.init_toolbar_side_layout_from_config(crate::config::ToolbarSideLayout::Pill);
    state.handle_action(Action::CycleToolbarDisplay); // micro
    state.handle_action(Action::CycleToolbarDisplay); // hidden
    assert!(
        !state.toolbar_visible(),
        "cycle-hidden under pill must leave no visible surface"
    );

    // A single ToggleToolbar press must bring the strip back.
    state.handle_action(Action::ToggleToolbar);
    assert!(state.toolbar_visible());
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
fn hidden_cycle_toast_offers_a_show_action() {
    let mut state = create_test_input_state();
    state.handle_action(Action::CycleToolbarDisplay); // micro
    state.handle_action(Action::CycleToolbarDisplay); // hidden
    let toast = state.ui_toast.as_ref().expect("hidden toast");
    assert_eq!(toast.message, "Toolbar: hidden");
    let action = toast.action.as_ref().expect("show action chip");
    assert_eq!(action.label, "Show (F2)");
    // Another cycle press from Hidden always lands on Full — unlike
    // ToggleToolbar, which under the Panel escape hatch would hide a
    // still-visible side palette instead.
    assert_eq!(action.action, Action::CycleToolbarDisplay);
}

#[test]
fn hiding_the_last_chrome_surface_warns_with_recovery_bindings() {
    let mut state = create_test_input_state();
    // Pill default: F9 alone hides every toolbar surface. The status bar is
    // still up, so its hint chip covers recovery — no warning yet.
    state.handle_action(Action::ToggleToolbar);
    assert!(
        state.ui_toast.is_none(),
        "no warning while the status bar remains"
    );

    // Hiding the status bar too removes the last interactive chrome.
    state.handle_action(Action::ToggleStatusBar);
    let toast = state.ui_toast.as_ref().expect("all-chrome warning");
    assert!(
        toast.message.starts_with("All UI hidden"),
        "unexpected message: {}",
        toast.message
    );
    assert!(toast.message.contains("F9"), "names the toolbar binding");
    assert!(
        toast.message.contains("F12"),
        "names the status bar binding"
    );
    let action = toast.action.as_ref().expect("recovery action chip");
    assert_eq!(action.action, Action::ToggleToolbar);
}

#[test]
fn all_chrome_warning_fires_from_the_cycle_path_and_supersedes_its_toast() {
    let mut state = create_test_input_state();
    state.handle_action(Action::ToggleStatusBar);
    assert!(state.ui_toast.is_none(), "toolbar still up: no warning");

    state.handle_action(Action::CycleToolbarDisplay); // micro
    state.handle_action(Action::CycleToolbarDisplay); // hidden: last chrome
    let toast = state.ui_toast.as_ref().expect("toast");
    assert!(
        toast.message.starts_with("All UI hidden"),
        "the warning must supersede \"Toolbar: hidden\", got: {}",
        toast.message
    );
}

#[test]
fn unbound_chrome_warning_advertises_right_click_only_when_it_can_open_the_menu() {
    let mut available = create_test_input_state();
    unbind_chrome_visibility_actions(&mut available);
    hide_all_chrome(&mut available);
    assert_eq!(
        available
            .ui_toast
            .as_ref()
            .map(|toast| toast.message.as_str()),
        Some("All UI hidden — right-click to restore")
    );

    let mut disabled = create_test_input_state();
    unbind_chrome_visibility_actions(&mut disabled);
    disabled.set_context_menu_enabled(false);
    hide_all_chrome(&mut disabled);
    assert_eq!(
        disabled
            .ui_toast
            .as_ref()
            .map(|toast| toast.message.as_str()),
        Some("All UI hidden — select the recovery action")
    );
    assert_eq!(
        disabled
            .ui_toast
            .as_ref()
            .and_then(|toast| toast.action.as_ref())
            .map(|action| action.action),
        Some(Action::ToggleToolbar)
    );

    let mut zoomed = create_test_input_state();
    unbind_chrome_visibility_actions(&mut zoomed);
    zoomed.set_zoom_status(true, false, 2.0, (0.0, 0.0));
    hide_all_chrome(&mut zoomed);
    assert_eq!(
        zoomed.ui_toast.as_ref().map(|toast| toast.message.as_str()),
        Some("All UI hidden — select the recovery action")
    );
    assert_eq!(
        zoomed
            .ui_toast
            .as_ref()
            .and_then(|toast| toast.action.as_ref())
            .map(|action| action.action),
        Some(Action::ToggleToolbar)
    );

    let mut right_click_radial = create_test_input_state();
    unbind_chrome_visibility_actions(&mut right_click_radial);
    right_click_radial.radial_menu_mouse_binding = RadialMenuMouseBinding::Right;
    hide_all_chrome(&mut right_click_radial);
    assert_eq!(
        right_click_radial
            .ui_toast
            .as_ref()
            .map(|toast| toast.message.as_str()),
        Some("All UI hidden — select the recovery action")
    );
}

#[test]
fn all_chrome_warning_suppressed_while_presenting() {
    let mut state = create_test_input_state();
    state.presenter_mode_config.hide_toolbars = true;
    state.presenter_mode_config.hide_status_bar = false;
    state.presenter_mode_config.show_toast = false;
    state.toggle_presenter_mode();
    assert!(!state.toolbar_visible());

    // Hiding the status bar now leaves no chrome, but presenter mode hides
    // chrome by design and restores it on exit — no nag mid-presentation.
    state.handle_action(Action::ToggleStatusBar);
    assert!(!state.show_status_bar);
    assert!(
        state.ui_toast.is_none(),
        "presenter mode must not trigger the all-chrome warning"
    );
}

#[test]
fn all_chrome_warning_fires_when_presenter_mode_did_not_hide_any_chrome() {
    let mut state = create_test_input_state();
    state.presenter_mode_config.hide_toolbars = false;
    state.presenter_mode_config.hide_status_bar = false;
    state.presenter_mode_config.show_toast = false;
    state.toggle_presenter_mode();

    hide_all_chrome(&mut state);
    let toast = state.ui_toast.as_ref().expect("all-chrome warning");
    assert!(
        toast.message.starts_with("All UI hidden"),
        "presenter mode must not suppress recovery for user-hidden chrome"
    );
    assert_eq!(
        toast.action.as_ref().map(|action| action.action),
        Some(Action::ToggleToolbar)
    );
}

#[test]
fn presenter_owned_hidden_toolbar_falls_back_to_status_bar_recovery() {
    let mut state = create_test_input_state();
    state.handle_action(Action::ToggleToolbar);
    assert!(!state.toolbar_visible());

    state.presenter_mode_config.hide_toolbars = true;
    state.presenter_mode_config.hide_status_bar = false;
    state.presenter_mode_config.show_toast = false;
    state.toggle_presenter_mode();
    state.handle_action(Action::ToggleStatusBar);

    let toast = state.ui_toast.as_ref().expect("all-chrome warning");
    assert!(toast.message.starts_with("All UI hidden"));
    let action = toast.action.as_ref().expect("recovery action");
    assert_eq!(action.label, "Show status bar");
    assert_eq!(action.action, Action::ToggleStatusBar);
}

#[test]
fn context_menu_offers_recovery_entries_only_while_chrome_hidden() {
    let mut state = create_test_input_state();
    state.open_context_menu((0, 0), Vec::new(), ContextMenuKind::Canvas, None);
    let labels = |state: &InputState| -> Vec<String> {
        state
            .context_menu_entries()
            .iter()
            .map(|entry| entry.label.clone())
            .collect()
    };
    assert!(!labels(&state).iter().any(|label| label == "Show Toolbar"));
    assert!(
        !labels(&state)
            .iter()
            .any(|label| label == "Show Status Bar")
    );

    state.handle_action(Action::ToggleToolbar);
    state.handle_action(Action::ToggleStatusBar);
    assert!(labels(&state).iter().any(|label| label == "Show Toolbar"));
    assert!(
        labels(&state)
            .iter()
            .any(|label| label == "Show Status Bar")
    );

    // The shape menu shares the recovery entries: right-clicking over a
    // large shape must not lock the user out of the mouse-only way back.
    state.open_context_menu((0, 0), Vec::new(), ContextMenuKind::Shape, None);
    assert!(labels(&state).iter().any(|label| label == "Show Toolbar"));
    assert!(
        labels(&state)
            .iter()
            .any(|label| label == "Show Status Bar")
    );
    state.open_context_menu((0, 0), Vec::new(), ContextMenuKind::Canvas, None);

    // Activating the entries restores the chrome (each execution also
    // closes the menu, so reopen between and after).
    state.execute_menu_command(MenuCommand::ShowToolbar);
    assert!(state.toolbar_visible());
    state.open_context_menu((0, 0), Vec::new(), ContextMenuKind::Canvas, None);
    state.execute_menu_command(MenuCommand::ShowStatusBar);
    assert!(state.show_status_bar);
    state.open_context_menu((0, 0), Vec::new(), ContextMenuKind::Canvas, None);
    assert!(!labels(&state).iter().any(|label| label == "Show Toolbar"));
    assert!(
        !labels(&state)
            .iter()
            .any(|label| label == "Show Status Bar")
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
