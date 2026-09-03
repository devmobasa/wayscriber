use super::create_test_input_state;
use crate::config::{
    Action, RadialMenuMouseBinding, StatusBarItem, StatusBarStyle, StatusPosition, TopDisplayMode,
};
use crate::input::state::core::{ContextMenuKind, MenuCommand};
use crate::input::state::{
    InputState, PendingBackendAction, PendingToolbarPersistence, TopMenuState,
};
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
    assert!(!state.ui_visibility.show_status_bar);
}

fn refresh_status_hud_layout(state: &mut InputState) {
    state.update_status_hud_layout(
        StatusPosition::BottomLeft,
        &StatusBarStyle::default(),
        1280,
        720,
    );
}

#[test]
fn cycle_action_walks_full_micro_hidden_full_with_toasts() {
    let mut state = create_test_input_state();
    // Keep the status bar up so the hidden rung shows its routine toast
    // instead of the all-chrome-hidden recovery warning.
    refresh_status_hud_layout(&mut state);
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
        state.take_pending_toolbar_persistence(),
        vec![PendingToolbarPersistence::DisplayMode {
            previous: TopDisplayMode::Full,
        }],
        "keyboard cycle persists like the toolbar-event paths"
    );

    state.handle_action(Action::CycleToolbarDisplay);
    assert_eq!(state.top_display_state(), TopDisplayMode::Hidden);
    assert!(!state.toolbar_top_visible());
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
    for menu in [
        TopMenuState::ShapePicker,
        TopMenuState::TopOverflow,
        TopMenuState::CanvasPopover,
        TopMenuState::SessionPopover,
        TopMenuState::SettingsPopover,
    ] {
        let mut state = create_test_input_state();
        state.toolbar_top_minimized = true;
        state.toolbar_top_menu = menu;

        state.handle_action(Action::CycleToolbarDisplay);
        assert_eq!(state.top_display_state(), TopDisplayMode::Micro);
        assert!(
            !state.toolbar_top_minimized,
            "micro and minimized are exclusive"
        );
        assert_eq!(state.toolbar_top_menu, TopMenuState::Closed);
    }
}

#[test]
fn toggle_toolbar_show_restores_a_cycle_hidden_top_strip() {
    let mut state = create_test_input_state();
    // A cycle-hidden top strip leaves no visible toolbar surface while every
    // raw visibility flag stays true. The raw-flag early return in
    // set_toolbar_visible used to swallow the restore in exactly this state,
    // leaving F9 (and everything else dispatching ToggleToolbar) dead.
    state.handle_action(Action::CycleToolbarDisplay); // micro
    state.handle_action(Action::CycleToolbarDisplay); // hidden
    assert!(
        !state.toolbar_visible(),
        "a cycle-hidden strip must leave no visible surface"
    );

    // A single ToggleToolbar press must bring the strip back.
    state.handle_action(Action::ToggleToolbar);
    assert!(state.toolbar_visible());
    assert_eq!(state.top_display_state(), TopDisplayMode::Full);
}

#[test]
fn toggle_toolbar_drives_the_top_pin_and_queues_its_persistence() {
    let mut state = create_test_input_state();
    assert!(state.toolbar_visible());
    assert!(state.toolbar_top_pinned);

    // F9 hide: the durable form of the toggle unpins the strip, and the
    // pending action carries the pre-change pin for the preview's rollback.
    state.handle_action(Action::ToggleToolbar);
    assert!(!state.toolbar_visible());
    assert!(!state.toolbar_top_pinned);
    assert_eq!(
        state.take_pending_toolbar_persistence(),
        vec![PendingToolbarPersistence::Visibility {
            previous_top_pinned: true,
        }],
        "the keyboard toggle persists like the toolbar-event paths"
    );

    // F9 show: the pin comes back on, with the hidden state as rollback.
    state.handle_action(Action::ToggleToolbar);
    assert!(state.toolbar_visible());
    assert!(state.toolbar_top_pinned);
    assert_eq!(
        state.take_pending_toolbar_persistence(),
        vec![PendingToolbarPersistence::Visibility {
            previous_top_pinned: false,
        }]
    );
}

/// F9 is WYSIWYG, not a pin round-trip: the second press must restore exactly
/// the visible state the user was looking at.
#[test]
fn toggle_toolbar_resolves_pins_to_what_is_on_screen() {
    let mut state = create_test_input_state();
    assert!(state.toolbar_visible());

    state.handle_action(Action::ToggleToolbar); // off
    assert!(!state.toolbar_top_pinned);
    assert_eq!(
        state.take_pending_toolbar_persistence(),
        vec![PendingToolbarPersistence::Visibility {
            previous_top_pinned: true,
        }]
    );

    state.handle_action(Action::ToggleToolbar); // on
    assert!(state.toolbar_visible());
    assert!(
        state.toolbar_top_pinned,
        "showing pins the strip so the next start matches the screen"
    );
}

/// A cycle-hidden strip leaves no visible surface while the pin stays true,
/// so F9 there is a SHOW whose pin value
/// do not change. Nothing may be queued for persistence: the write would be
/// byte-identical, and its rollback would re-derive pin-true visibility for
/// a screen that was effectively hidden before the press. The unfold stays
/// runtime-only, exactly like F2's hidden rung.
#[test]
fn cycle_hidden_show_with_unchanged_pins_queues_no_persistence() {
    let mut state = create_test_input_state();
    state.handle_action(Action::CycleToolbarDisplay); // micro
    state.handle_action(Action::CycleToolbarDisplay); // hidden
    assert!(!state.toolbar_visible());
    assert!(state.toolbar_top_pinned);
    state.take_pending_toolbar_persistence(); // drain the cycle's display-mode write

    state.handle_action(Action::ToggleToolbar); // show: unfolds Hidden → Full
    assert!(state.toolbar_visible());
    assert_eq!(state.top_display_state(), TopDisplayMode::Full);
    assert!(state.toolbar_top_pinned);
    assert!(
        !state.has_pending_toolbar_persistence(),
        "a toggle that moves no pin queues nothing (the raw queue, not the \
         drain-time filter, is the contract here)"
    );
}

/// The hide twin: the strip is already unpinned via the pin button while
/// still visible, then F9. The hide moves no pin — the false pin already
/// persists the hidden restart state — so the toggle needs no
/// additional persistence and queues nothing.
#[test]
fn hide_with_an_already_unpinned_strip_queues_no_persistence() {
    let mut state = create_test_input_state();
    state.toolbar_top_pinned = false;
    assert!(state.toolbar_visible());

    state.handle_action(Action::ToggleToolbar); // hide
    assert!(!state.toolbar_visible());
    assert!(!state.toolbar_top_pinned);
    assert!(
        !state.has_pending_toolbar_persistence(),
        "a toggle that moves no pin queues nothing (the raw queue, not the \
         drain-time filter, is the contract here)"
    );
}

#[test]
fn presenter_swallowed_toggle_leaves_pins_and_persistence_untouched() {
    let mut state = create_test_input_state();
    state.presenter_mode_config.hide_toolbars = true;
    state.toggle_presenter_mode();
    assert!(!state.toolbar_visible());

    state.handle_action(Action::ToggleToolbar);
    assert!(state.toolbar_top_pinned);
    assert!(
        !state.has_pending_toolbar_persistence(),
        "a swallowed toggle must queue nothing (the raw queue, not the \
         drain-time filter, is the contract here)"
    );
}

/// Focus and presenter mode hide chrome implicitly and restore it on exit;
/// both stay run-only on purpose, so neither transition may reach the pin
/// flags or queue the visibility persistence the explicit toggle uses.
#[test]
fn focus_and_presenter_transitions_never_queue_pin_persistence() {
    let mut state = create_test_input_state();
    state.presenter_mode_config.hide_toolbars = true;

    for action in [
        Action::ToggleFocusMode,
        Action::ToggleFocusMode,
        Action::TogglePresenterMode,
        Action::TogglePresenterMode,
    ] {
        state.handle_action(action);
        assert!(
            state.toolbar_top_pinned,
            "{action:?} must not touch the pin overrides"
        );
        assert!(
            !state.has_pending_toolbar_persistence(),
            "{action:?} must not queue visibility persistence (the raw \
             queue, not the drain-time filter, is the contract here)"
        );
    }
}

/// F9 and F2 in one input batch, drained together: the queue keeps both,
/// oldest first. The old single backend-action slot would have kept only the
/// F2 write, silently costing the F9 press its persistence.
///
/// The strip sits on Micro first so the in-batch F2 moves the raw persisted
/// mode: pressed over the F9-hidden strip, the cycle's Hidden rung unfolds to
/// Full, and from a Full start that write would be a no-op the drain filter
/// (correctly) drops.
#[test]
fn a_toggle_and_a_cycle_in_one_batch_both_keep_their_persistence() {
    let mut state = create_test_input_state();
    assert!(state.toolbar_top_pinned);
    state.handle_action(Action::CycleToolbarDisplay); // micro
    state.take_pending_toolbar_persistence(); // drain the setup cycle's write

    state.handle_action(Action::ToggleToolbar); // F9 hide
    state.handle_action(Action::CycleToolbarDisplay); // F2: unfolds to full

    assert_eq!(
        state.take_pending_toolbar_persistence(),
        vec![
            PendingToolbarPersistence::Visibility {
                previous_top_pinned: true,
            },
            PendingToolbarPersistence::DisplayMode {
                previous: TopDisplayMode::Micro,
            },
        ],
        "both changes must survive the batch, oldest first"
    );
}

/// A capture and a visibility toggle in the same batch, in either order:
/// the capture rides the single backend-action slot, the toggle rides the
/// persistence queue, and neither may cost the other its delivery.
#[test]
fn visibility_persistence_survives_a_capture_request() {
    let mut state = create_test_input_state();

    state.handle_action(Action::ToggleToolbar); // F9 hide
    state.handle_action(Action::CaptureFileFull);
    assert_eq!(
        state.take_pending_backend_action(),
        Some(PendingBackendAction::Screenshot(Action::CaptureFileFull)),
        "the capture must survive the toggle"
    );
    assert_eq!(
        state.take_pending_toolbar_persistence(),
        vec![PendingToolbarPersistence::Visibility {
            previous_top_pinned: true,
        }],
        "the toggle must survive the capture"
    );

    // The reverse order: capture first, then the toggle (a show this time).
    state.handle_action(Action::CaptureFileFull);
    state.handle_action(Action::ToggleToolbar); // F9 show
    assert_eq!(
        state.take_pending_backend_action(),
        Some(PendingBackendAction::Screenshot(Action::CaptureFileFull)),
        "the capture must survive the toggle"
    );
    assert_eq!(
        state.take_pending_toolbar_persistence(),
        vec![PendingToolbarPersistence::Visibility {
            previous_top_pinned: false,
        }],
        "the toggle must survive the capture"
    );
}

/// F9 twice before a drain: one coalesced raw entry keeps the FIRST press's
/// pre-change pins (the burst's rollback baseline), and because the pins end
/// exactly where they started, the drain-time no-op filter drops it — the
/// write would be byte-identical to its own rollback.
#[test]
fn a_toggle_burst_coalesces_to_the_original_rollback_baseline() {
    let mut state = create_test_input_state();
    assert!(state.toolbar_top_pinned);

    state.handle_action(Action::ToggleToolbar); // hide
    state.handle_action(Action::ToggleToolbar); // show: pins back where they started
    assert!(state.toolbar_top_pinned);

    assert!(
        state.has_pending_toolbar_persistence(),
        "the burst coalesces to one raw entry, it is not dropped at queue time"
    );
    assert_eq!(
        state.take_pending_toolbar_persistence(),
        vec![],
        "a burst that lands where it started has nothing durable to write"
    );
}

/// The backend drains this queue once more at teardown; nothing on the input
/// side may drop a queued entry when the same batch also requests an exit.
#[test]
fn an_exit_request_does_not_clear_queued_toolbar_persistence() {
    let mut state = create_test_input_state();

    state.handle_action(Action::ToggleToolbar); // F9 hide
    state.handle_action(Action::Exit);
    assert!(state.should_exit, "the exit request must have landed");

    assert_eq!(
        state.take_pending_toolbar_persistence(),
        vec![PendingToolbarPersistence::Visibility {
            previous_top_pinned: true,
        }],
        "the toggle must still be waiting for the teardown drain"
    );
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
    refresh_status_hud_layout(&mut state);
    state.handle_action(Action::CycleToolbarDisplay); // micro
    state.handle_action(Action::CycleToolbarDisplay); // hidden
    let toast = state.ui_toast.as_ref().expect("hidden toast");
    assert_eq!(toast.message, "Toolbar: hidden");
    let action = toast.action.as_ref().expect("show action chip");
    assert_eq!(action.label, "Show (F2)");
    // Another cycle press from Hidden always lands on Full.
    assert_eq!(action.dispatch_action(), Some(Action::CycleToolbarDisplay));
}

#[test]
fn hiding_the_last_chrome_surface_warns_with_recovery_bindings() {
    let mut state = create_test_input_state();
    refresh_status_hud_layout(&mut state);
    // F9 alone hides every toolbar surface. The status bar is
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
    assert_eq!(action.dispatch_action(), Some(Action::ToggleToolbar));
}

#[test]
fn enabled_but_empty_status_bar_does_not_suppress_chrome_recovery_warning() {
    let mut state = create_test_input_state();
    refresh_status_hud_layout(&mut state);
    assert!(state.status_hud_layout().is_some());
    for item in StatusBarItem::ALL {
        state.set_status_bar_item_visible(item, false);
    }
    assert!(
        state.ui_visibility.show_status_bar,
        "the master preference remains enabled"
    );
    assert!(
        state.status_hud_layout().is_none(),
        "the final item change refreshes the cache before the next frame"
    );
    assert!(!state.status_hud_effectively_visible());

    assert!(state.set_status_bar_item_visible(StatusBarItem::About, true));
    assert!(
        state.status_hud_layout().is_some(),
        "enabling content refreshes an empty cache before the next frame"
    );
    assert!(
        state.status_hud_effectively_visible(),
        "policy sees the synchronously refreshed measured cache"
    );
    assert!(state.set_status_bar_item_visible(StatusBarItem::About, false));

    state.handle_action(Action::ToggleToolbar);

    let toast = state.ui_toast.as_ref().expect("all-chrome warning");
    assert!(toast.message.starts_with("All UI hidden"));
    assert_eq!(
        toast
            .action
            .as_ref()
            .and_then(|action| action.dispatch_action()),
        Some(Action::ToggleToolbar)
    );
}

/// An About-only HUD on an 80px-wide output is shed entirely by the width
/// budget (`shedding_the_last_optional_piece_does_not_leave_an_empty_pill`).
/// The policy predicate must agree immediately after the content toggle, so a
/// floating badge or all-chrome recovery warning is never suppressed for a
/// HUD the next frame will not draw.
#[test]
fn width_shed_content_never_reports_an_effectively_visible_hud() {
    let mut state = create_test_input_state();
    for item in StatusBarItem::ALL {
        state.set_status_bar_item_visible(item, false);
    }
    state.update_status_hud_layout(
        StatusPosition::BottomLeft,
        &StatusBarStyle::default(),
        80,
        60,
    );
    assert!(state.status_hud_layout().is_none());

    assert!(state.set_status_bar_item_visible(StatusBarItem::About, true));
    assert!(
        state.status_hud_layout().is_none(),
        "the narrow output sheds the About-only HUD entirely"
    );
    assert!(
        !state.status_hud_effectively_visible(),
        "policy must not report a HUD the width budget has shed"
    );
}

#[test]
fn toolbar_hint_prevents_a_false_all_chrome_warning_when_it_becomes_visible() {
    let mut state = create_test_input_state();
    for item in StatusBarItem::ALL {
        state.set_status_bar_item_visible(item, item == StatusBarItem::ToolbarHint);
    }
    refresh_status_hud_layout(&mut state);
    assert!(
        state.status_hud_layout().is_none(),
        "the hint is absent while a toolbar surface is visible"
    );

    state.handle_action(Action::ToggleToolbar);

    assert!(!state.toolbar_visible());
    assert!(
        state.status_hud_effectively_visible(),
        "effective visibility follows the newly eligible toolbar hint before redraw"
    );
    assert!(
        state.ui_toast.is_none(),
        "the visible recovery hint makes an all-chrome warning redundant"
    );
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
            .and_then(|action| action.dispatch_action()),
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
            .and_then(|action| action.dispatch_action()),
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
    assert!(!state.ui_visibility.show_status_bar);
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
        toast
            .action
            .as_ref()
            .and_then(|action| action.dispatch_action()),
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
    assert_eq!(action.dispatch_action(), Some(Action::ToggleStatusBar));
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
    assert!(state.ui_visibility.show_status_bar);
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

/// `ToolStateSnapshot` leaves chrome visibility out on purpose, so a toggle
/// that moves it has nothing for a save to carry. These actions are
/// session-INDEPENDENT, not run-only: F9 and F2 are durable through
/// `runtime-ui.toml`, they are just never part of the session file, so they
/// must never mark it dirty. Marking the session dirty anyway is not merely
/// redundant — a session that failed to restore is protected from
/// replacement by exactly "nothing persisted changed"
/// (`should_skip_save_for_protected_path`), so a false dirty is what lets
/// autosave clobber it.
#[test]
fn session_independent_chrome_actions_never_mark_the_session_dirty() {
    let mut state = create_test_input_state();
    // Chrome only: with the tool behavior left at its default, presenter mode
    // would also take the tool override, which *is* session content.
    state.presenter_mode_config.hide_status_bar = true;
    state.presenter_mode_config.hide_toolbars = true;
    state.presenter_mode_config.tool_behavior = crate::config::PresenterToolBehavior::Keep;
    state.presenter_mode_config.enable_click_highlight = false;
    state.presenter_mode_config.enable_input_hud = false;

    for action in [
        Action::ToggleStatusBar,
        Action::ToggleStatusBar,
        Action::ToggleFloatingBadge,
        Action::ToggleZoomChip,
        Action::ToggleToolbar,
        Action::CycleToolbarDisplay,
        Action::ToggleClickHighlight,
        Action::ToggleInputHud,
        // Enter, leave, and — with everything already hidden — the rescue arm
        // that shows every surface again.
        Action::ToggleFocusMode,
        Action::ToggleFocusMode,
        Action::TogglePresenterMode,
        Action::TogglePresenterMode,
    ] {
        state.handle_action(action);
        assert!(
            !state.is_session_dirty(),
            "{action:?} moves chrome the session file does not carry"
        );
    }

    hide_all_chrome(&mut state);
    state.handle_action(Action::ToggleFocusMode); // rescue arm
    assert!(state.ui_visibility.show_status_bar);
    assert!(
        !state.is_session_dirty(),
        "the rescue arm restores chrome only"
    );

    // Control: a change the snapshot does carry still marks the session dirty,
    // so the assertions above are about the toggles and not about a dirty flag
    // that stopped working.
    assert!(state.set_tool_override(Some(crate::input::Tool::Line)));
    assert!(state.is_session_dirty());
}

/// The toolbar-side half of the same audit: every event the policy classifies
/// as an authored preference changes the effective config for this run and
/// nothing in the session file, so none of them may mark the session dirty.
///
/// `SelectTool(Highlight)` and `ToggleAllHighlight` are deliberately absent:
/// both move the tool override, which the snapshot does persist.
#[test]
fn run_only_toolbar_preference_events_never_mark_the_session_dirty() {
    use crate::config::{ToolbarLayoutMode, ToolbarSectionFlag};
    use crate::ui::toolbar::ToolbarEvent;

    let mut state = create_test_input_state();
    let presets_item = ToolbarSectionFlag::Presets.item_id();

    for [first, second] in [
        [
            ToolbarEvent::ToggleIconMode(true),
            ToolbarEvent::ToggleIconMode(false),
        ],
        [
            ToolbarEvent::ToggleMoreColors(true),
            ToolbarEvent::ToggleMoreColors(false),
        ],
        [
            ToolbarEvent::ToggleActionsSection(false),
            ToolbarEvent::ToggleActionsSection(true),
        ],
        [
            ToolbarEvent::ToggleActionsAdvanced(false),
            ToolbarEvent::ToggleActionsAdvanced(true),
        ],
        [
            ToolbarEvent::ToggleZoomActions(false),
            ToolbarEvent::ToggleZoomActions(true),
        ],
        [
            ToolbarEvent::TogglePagesSection(false),
            ToolbarEvent::TogglePagesSection(true),
        ],
        [
            ToolbarEvent::ToggleBoardsSection(false),
            ToolbarEvent::ToggleBoardsSection(true),
        ],
        [
            ToolbarEvent::TogglePresets(false),
            ToolbarEvent::TogglePresets(true),
        ],
        [
            ToolbarEvent::ToggleStepSection(false),
            ToolbarEvent::ToggleStepSection(true),
        ],
        [
            ToolbarEvent::ToggleTextControls(false),
            ToolbarEvent::ToggleTextControls(true),
        ],
        // A section row in the customization list: the one item id whose
        // visibility is an authored preference rather than runtime-UI state.
        [
            ToolbarEvent::SetToolbarItemHidden(presets_item, true),
            ToolbarEvent::SetToolbarItemHidden(presets_item, false),
        ],
        [
            ToolbarEvent::ToggleContextAwareUi(false),
            ToolbarEvent::ToggleContextAwareUi(true),
        ],
        [
            ToolbarEvent::TogglePresetToasts(false),
            ToolbarEvent::TogglePresetToasts(true),
        ],
        [
            ToolbarEvent::ToggleIdleFade(false),
            ToolbarEvent::ToggleIdleFade(true),
        ],
        [
            ToolbarEvent::ToggleToolPreview(false),
            ToolbarEvent::ToggleToolPreview(true),
        ],
        [
            ToolbarEvent::ToggleDelaySliders(true),
            ToolbarEvent::ToggleDelaySliders(false),
        ],
        [
            ToolbarEvent::ToggleCustomSection(true),
            ToolbarEvent::ToggleCustomSection(false),
        ],
        [
            ToolbarEvent::SetToolbarLayoutMode(ToolbarLayoutMode::Simple),
            ToolbarEvent::SetToolbarLayoutMode(ToolbarLayoutMode::Advanced),
        ],
        [
            ToolbarEvent::ToggleStatusBar(false),
            ToolbarEvent::ToggleStatusBar(true),
        ],
        [
            ToolbarEvent::SetStatusBarInteractive(false),
            ToolbarEvent::SetStatusBarInteractive(true),
        ],
        [
            ToolbarEvent::SetStatusBarItemVisible(StatusBarItem::Color, false),
            ToolbarEvent::SetStatusBarItemVisible(StatusBarItem::Color, true),
        ],
        [
            ToolbarEvent::ToggleStatusBoardBadge(false),
            ToolbarEvent::ToggleStatusBoardBadge(true),
        ],
        [
            ToolbarEvent::ToggleStatusPageBadge(false),
            ToolbarEvent::ToggleStatusPageBadge(true),
        ],
        [
            ToolbarEvent::ToggleFloatingBadgeAlways(true),
            ToolbarEvent::ToggleFloatingBadgeAlways(false),
        ],
        [
            ToolbarEvent::ToggleHighlightToolRing(true),
            ToolbarEvent::ToggleHighlightToolRing(false),
        ],
        [
            ToolbarEvent::ToggleInputHud(true),
            ToolbarEvent::ToggleInputHud(false),
        ],
    ] {
        let mut changed = state.apply_toolbar_event(first.clone());
        assert!(
            !state.is_session_dirty(),
            "{first:?} changes the effective config for this run, not the session"
        );
        changed |= state.apply_toolbar_event(second.clone());
        assert!(
            !state.is_session_dirty(),
            "{second:?} changes the effective config for this run, not the session"
        );
        assert!(
            changed,
            "{first:?} and {second:?} both no-opped, so neither assertion above tested anything"
        );
    }
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
