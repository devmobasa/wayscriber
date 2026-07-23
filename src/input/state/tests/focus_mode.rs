//! Focus mode: snapshot-hide of every chrome surface and exact restore.

use super::create_test_input_state;
use crate::config::{Action, ColorSpec, ToolPresetConfig, TopDisplayMode};
use crate::input::Tool;
use crate::input::state::{Toast, ToastPriority};

#[test]
fn focus_mode_hides_all_chrome_and_restores_exactly() {
    let mut state = create_test_input_state();
    // Non-default pre-state: a micro top strip and a hidden floating badge
    // must both survive the round trip untouched.
    state.handle_action(Action::CycleToolbarDisplay); // micro
    state.handle_action(Action::ToggleFloatingBadge); // badge hidden
    assert_eq!(state.top_display_state(), TopDisplayMode::Micro);
    assert!(!state.show_floating_badge);

    state.handle_action(Action::ToggleFocusMode);
    assert!(state.focus_mode_active());
    assert!(!state.toolbar_visible());
    assert!(!state.show_status_bar);
    assert!(!state.show_floating_badge);
    assert!(!state.zoom_chip_enabled());
    state.handle_action(Action::ToggleFocusMode);
    assert!(!state.focus_mode_active());
    assert!(state.toolbar_visible());
    assert!(state.show_status_bar);
    assert!(state.zoom_chip_enabled());
    assert!(
        state
            .ui_toast
            .as_ref()
            .and_then(|toast| toast.action.as_ref())
            .is_none_or(|action| action.action != Action::ToggleFocusMode),
        "restoring Focus Mode must retract its Restore action"
    );
    assert_eq!(
        state.top_display_state(),
        TopDisplayMode::Micro,
        "the micro strip form must survive the round trip"
    );
    assert!(
        !state.show_floating_badge,
        "a pre-hidden badge stays hidden after restore"
    );
}

#[test]
fn focus_mode_toast_offers_restore_action() {
    let mut state = create_test_input_state();
    state.handle_action(Action::ToggleFocusMode);

    let toast = state.ui_toast.as_ref().expect("focus mode toast");
    assert!(
        toast.message.starts_with("Focus mode"),
        "unexpected message: {}",
        toast.message
    );
    let action = toast.action.as_ref().expect("restore action chip");
    assert_eq!(action.action, Action::ToggleFocusMode);
}

#[test]
fn focus_mode_suppresses_fallback_mode_badges_but_keeps_restore_toast() {
    let mut state = create_test_input_state();
    state.set_zoom_status(true, false, 2.0, (0.0, 0.0));

    state.handle_action(Action::ToggleFocusMode);

    assert!(state.focus_mode_active());
    assert!(state.zoom_active());
    assert!(
        !state.fallback_mode_badges_visible(),
        "Focus Mode must suppress zoom, frozen, pan, and editing fallback badges"
    );
    assert_eq!(
        state
            .ui_toast
            .as_ref()
            .map(|toast| toast.action.as_ref().map(|action| action.action)),
        Some(Some(Action::ToggleFocusMode)),
        "the intentional Restore toast remains available"
    );
}

#[test]
fn manual_chrome_toggle_breaks_focus_mode() {
    let mut state = create_test_input_state();
    state.handle_action(Action::ToggleFocusMode);
    assert!(state.focus_mode_active());

    // F9 during focus mode: the user takes manual ownership. The toolbar
    // comes back, the snapshot is dropped, and the rest stays hidden.
    state.handle_action(Action::ToggleToolbar);
    assert!(!state.focus_mode_active());
    assert!(state.toolbar_visible());
    assert!(!state.show_status_bar);
    assert!(!state.zoom_chip_enabled());
    assert!(
        state
            .ui_toast
            .as_ref()
            .and_then(|toast| toast.action.as_ref())
            .is_none_or(|action| action.action != Action::ToggleFocusMode),
        "breaking Focus Mode must retract its stale Restore action"
    );

    // The next focus-mode press starts a fresh snapshot (hide again), not
    // a stale restore.
    state.handle_action(Action::ToggleFocusMode);
    assert!(state.focus_mode_active());
    assert!(!state.toolbar_visible());
}

#[test]
fn breaking_focus_mode_retracts_a_restore_toast_queued_behind_a_warning() {
    let mut state = create_test_input_state();
    state.push_toast(
        ToastPriority::Critical,
        "test.warning",
        Toast::error("Keep this warning"),
    );

    state.handle_action(Action::ToggleFocusMode);
    assert!(state.focus_mode_active());
    assert!(
        !state.toast_queue.is_empty(),
        "the lower-priority Restore toast should be queued"
    );

    state.handle_action(Action::ToggleToolbar);

    assert!(!state.focus_mode_active());
    assert!(
        state.toast_queue.is_empty(),
        "breaking Focus Mode must retract a queued Restore action"
    );
    assert_eq!(
        state.ui_toast.as_ref().map(|toast| toast.message.as_str()),
        Some("Keep this warning"),
        "unrelated active feedback must remain"
    );
}

#[test]
fn preset_status_bar_update_stays_hidden_until_focus_mode_restores_it() {
    let mut state = create_test_input_state();
    state.presets[0] = Some(ToolPresetConfig {
        name: None,
        tool: Tool::Pen,
        color: ColorSpec::Name("red".to_string()),
        size: 5.0,
        tool_settings: None,
        eraser_kind: None,
        eraser_mode: None,
        marker_opacity: None,
        fill_enabled: None,
        font_size: None,
        text_background_enabled: None,
        arrow_length: None,
        arrow_angle: None,
        arrow_head_at_end: None,
        polygon_sides: None,
        show_status_bar: Some(false),
        drag_tools: None,
    });

    state.handle_action(Action::ToggleFocusMode);
    assert!(state.focus_mode_active());

    assert!(state.apply_preset(1));
    assert!(state.focus_mode_active());
    assert!(!state.show_status_bar, "Focus Mode keeps chrome suppressed");

    state.handle_action(Action::ToggleFocusMode);
    assert!(
        !state.show_status_bar,
        "Focus restore must honor the status-bar value authored by the preset"
    );
}

#[test]
fn focus_mode_rescues_a_fully_hidden_ui() {
    let mut state = create_test_input_state();
    // Hide everything by hand: no snapshot exists.
    state.handle_action(Action::ToggleToolbar);
    state.handle_action(Action::ToggleStatusBar);
    state.handle_action(Action::ToggleFloatingBadge);
    state.handle_action(Action::ToggleZoomChip);
    assert!(!state.focus_mode_active());
    assert!(!state.toolbar_visible());
    assert!(!state.show_status_bar);
    assert!(!state.show_floating_badge);
    assert!(!state.zoom_chip_enabled());
    assert_eq!(
        state
            .ui_toast
            .as_ref()
            .and_then(|toast| toast.action.as_ref())
            .map(|action| action.action),
        Some(Action::ToggleToolbar),
        "the fully hidden state should expose the one-click recovery action"
    );

    // With nothing left to hide, the action restores the full UI instead
    // of snapshotting an empty screen.
    state.handle_action(Action::ToggleFocusMode);
    assert!(!state.focus_mode_active());
    assert!(state.toolbar_visible());
    assert!(state.show_status_bar);
    assert!(state.show_floating_badge);
    assert!(state.zoom_chip_enabled());
    assert!(
        state
            .ui_toast
            .as_ref()
            .and_then(|toast| toast.action.as_ref())
            .is_none_or(|action| action.action != Action::ToggleToolbar),
        "Focus rescue must retract the stale Show toolbar action"
    );
}

#[test]
fn focus_mode_hides_a_floating_badge_when_it_is_the_only_visible_chrome() {
    let mut state = create_test_input_state();
    assert!(
        state.boards.board_count() > 1 || state.boards.page_count() > 1,
        "the floating badge needs multiple boards or pages to render"
    );
    state.set_toolbar_visible(false);
    state.show_status_bar = false;
    state.show_zoom_chip = false;
    state.show_floating_badge = true;

    state.handle_action(Action::ToggleFocusMode);

    assert!(
        state.focus_mode_active(),
        "the visible floating badge must put Focus Mode on its hide arm"
    );
    assert!(!state.toolbar_visible());
    assert!(!state.show_status_bar);
    assert!(!state.show_floating_badge);
    assert!(!state.zoom_chip_enabled());
}

#[test]
fn focus_mode_hides_a_zoom_badge_when_it_is_the_only_visible_chrome() {
    let mut state = create_test_input_state();
    state.set_toolbar_visible(false);
    state.show_status_bar = false;
    state.show_floating_badge = false;
    state.show_zoom_chip = false;
    state.set_zoom_status(true, false, 2.0, (0.0, 0.0));
    assert!(state.zoom_active());
    assert!(!state.zoom_chip_enabled());

    state.handle_action(Action::ToggleFocusMode);

    assert!(
        state.focus_mode_active(),
        "the visible fallback zoom badge must put Focus Mode on its hide arm"
    );
    assert!(!state.fallback_mode_badges_visible());
    assert!(!state.toolbar_visible());
    assert!(!state.show_status_bar);
}

#[test]
fn focus_mode_never_enqueues_persistence() {
    // Focus mode's hide/restore is transient by contract: only the explicit
    // ToggleFloatingBadge/ToggleZoomChip actions write the master
    // visibility prefs to config.
    let mut state = create_test_input_state();
    let _ = state.take_pending_backend_action();

    state.handle_action(Action::ToggleFocusMode); // hide all
    assert!(state.take_pending_backend_action().is_none());

    state.handle_action(Action::ToggleFocusMode); // restore all
    assert!(state.take_pending_backend_action().is_none());
}

#[test]
fn queued_visibility_saves_keep_the_user_authored_values_during_focus_mode() {
    let mut state = create_test_input_state();
    state.show_floating_badge = false;
    state.show_zoom_chip = false;

    state.handle_action(Action::ToggleFloatingBadge);
    state.handle_action(Action::ToggleZoomChip);
    assert!(state.show_floating_badge);
    assert!(state.show_zoom_chip);

    // Suppress both live flags before the backend drains their save actions.
    state.handle_action(Action::ToggleFocusMode);
    assert!(!state.show_floating_badge);
    assert!(!state.show_zoom_chip);

    assert_eq!(
        state.take_pending_backend_action(),
        Some(crate::input::state::PendingBackendAction::PersistFloatingBadgeConfig(true))
    );
    assert_eq!(
        state.take_pending_backend_action(),
        Some(crate::input::state::PendingBackendAction::PersistZoomChipConfig(true))
    );
}

#[test]
fn presenter_mode_gates_focus_mode() {
    let mut state = create_test_input_state();
    state.toggle_presenter_mode();
    assert!(state.presenter_mode);

    state.handle_action(Action::ToggleFocusMode);
    assert!(
        !state.focus_mode_active(),
        "presenter mode owns chrome; focus mode must not double-snapshot"
    );
}

#[test]
fn focus_mode_exits_light_mode_before_taking_ownership() {
    let mut state = create_test_input_state();
    state.compositor_capabilities.layer_shell = true;
    state.handle_action(Action::ToggleLightMode);
    assert!(state.light_mode);

    state.handle_action(Action::ToggleFocusMode);

    assert!(!state.light_mode, "transient chrome owners must not nest");
    assert!(state.focus_mode_active());
    state.handle_action(Action::ToggleFocusMode);
    assert!(state.show_status_bar);
}

#[test]
fn light_mode_exits_focus_mode_before_taking_ownership() {
    let mut state = create_test_input_state();
    state.compositor_capabilities.layer_shell = true;
    state.handle_action(Action::ToggleFocusMode);
    assert!(state.focus_mode_active());

    state.handle_action(Action::ToggleLightMode);

    assert!(state.light_mode);
    assert!(
        !state.focus_mode_active(),
        "transient chrome owners must not nest"
    );
    state.handle_action(Action::ToggleLightMode);
    assert!(state.show_status_bar);
}

#[test]
fn unsupported_light_mode_does_not_break_focus_mode() {
    let mut state = create_test_input_state();
    state.compositor_capabilities.layer_shell = false;
    state.handle_action(Action::ToggleFocusMode);
    assert!(state.focus_mode_active());

    state.handle_action(Action::ToggleLightMode);

    assert!(!state.light_mode);
    assert!(
        state.focus_mode_active(),
        "a rejected mode switch must leave the active chrome owner untouched"
    );
    assert!(!state.show_status_bar);
}

#[test]
fn light_mode_drawing_exits_focus_mode_before_taking_ownership() {
    let mut state = create_test_input_state();
    state.compositor_capabilities.layer_shell = true;
    state.handle_action(Action::ToggleFocusMode);
    assert!(state.focus_mode_active());

    state.handle_action(Action::ToggleLightModeDrawing);

    assert!(state.light_mode);
    assert!(state.light_mode_drawing);
    assert!(
        !state.focus_mode_active(),
        "every Light Mode entry path must own chrome exclusively"
    );
    state.handle_action(Action::ToggleLightMode);
    assert!(state.show_status_bar);
}
