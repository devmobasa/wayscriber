use super::*;
use crate::config::{DragButtonConfig, MouseDragToolsConfig};
use crate::input::{DragBinding, DragTool, DragToolBindings};

#[test]
fn toggle_floating_badge_action_flips_runtime_visibility() {
    let mut state = create_test_input_state();
    assert!(
        state.ui_visibility.show_floating_badge,
        "badge visible by default"
    );

    state.handle_action(crate::config::Action::ToggleFloatingBadge);
    assert!(!state.ui_visibility.show_floating_badge);
    assert!(state.needs_redraw);
    // Persisting is the backend's job: it diffs the state before and after
    // the action rather than being handed a queued request.
    assert!(!state.has_pending_backend_actions());
    assert!(state.take_pending_backend_action().is_none());

    state.handle_action(crate::config::Action::ToggleFloatingBadge);
    assert!(state.ui_visibility.show_floating_badge);
}

/// The chrome toggles reach the backend through the state they change, not
/// through a queued request, however many times they are pressed.
#[test]
fn repeated_chrome_visibility_toggles_queue_no_backend_work() {
    let mut state = create_test_input_state();

    state.handle_action(crate::config::Action::ToggleFloatingBadge);
    state.handle_action(crate::config::Action::ToggleFloatingBadge);
    state.handle_action(crate::config::Action::ToggleZoomChip);
    state.handle_action(crate::config::Action::ToggleZoomChip);

    assert!(state.ui_visibility.show_floating_badge);
    assert!(state.ui_visibility.show_zoom_chip);
    assert!(state.take_pending_backend_action().is_none());
}

#[test]
fn test_adjust_font_size_increase() {
    let mut state = create_test_input_state();
    assert_eq!(state.current_font_size, 32.0);

    state.adjust_font_size(2.0);
    assert_eq!(state.current_font_size, 34.0);
    assert!(state.needs_redraw);
}

#[test]
fn apply_preset_updates_tool_and_settings() {
    let mut state = create_test_input_state();
    state.preset_slot_count = 3;
    state.presets[0] = Some(ToolPresetConfig {
        name: None,
        tool: Tool::Marker,
        color: ColorSpec::Name("blue".to_string()),
        size: 12.0,
        tool_settings: None,
        eraser_kind: Some(EraserKind::Rect),
        eraser_mode: Some(EraserMode::Stroke),
        marker_opacity: Some(0.6),
        fill_enabled: Some(true),
        font_size: Some(28.0),
        text_background_enabled: Some(true),
        arrow_length: Some(25.0),
        arrow_angle: Some(45.0),
        arrow_head_at_end: Some(true),
        polygon_sides: Some(8),
        show_status_bar: Some(false),
        drag_tools: None,
    });

    assert!(state.apply_preset(1));
    assert_eq!(state.active_tool(), Tool::Marker);
    assert_eq!(
        state.current_color,
        ColorSpec::Name("blue".to_string()).to_color()
    );
    assert_eq!(state.current_thickness, 12.0);
    assert_eq!(state.marker_opacity, 0.6);
    assert!(state.fill_enabled);
    assert_eq!(state.current_font_size, 28.0);
    assert!(state.text_background_enabled);
    assert_eq!(state.arrow_length, 25.0);
    assert_eq!(state.arrow_angle, 45.0);
    assert!(state.arrow_head_at_end);
    assert_eq!(state.polygon_sides, 8);
    assert_eq!(state.eraser_kind, EraserKind::Rect);
    assert_eq!(state.eraser_mode, EraserMode::Stroke);
    assert!(!state.ui_visibility.show_status_bar);
}

#[test]
fn apply_preset_merges_partial_left_drag_tool_bindings() {
    let mut state = create_test_input_state();
    state.preset_slot_count = 3;

    let mut existing_bindings = DragToolBindings::default();
    existing_bindings.left.shift_drag = DragBinding::from_tool(Tool::Eraser);
    assert!(state.set_drag_tool_bindings(existing_bindings));

    let mut left = DragButtonConfig::button_behavior();
    left.drag_tool = DragTool::Marker;

    state.presets[0] = Some(ToolPresetConfig {
        name: None,
        tool: Tool::Marker,
        color: ColorSpec::Name("blue".to_string()),
        size: 12.0,
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
        show_status_bar: None,
        drag_tools: Some(MouseDragToolsConfig::from_buttons(
            left,
            DragButtonConfig::button_behavior(),
            DragButtonConfig::button_behavior(),
        )),
    });

    assert!(state.apply_preset(1));
    assert_eq!(state.drag_tool_bindings.left.drag.tool, DragTool::Marker);
    assert_eq!(
        state.drag_tool_bindings.left.shift_drag.tool,
        DragTool::Eraser
    );
    assert_eq!(state.drag_tool_bindings.left.ctrl_drag.tool, DragTool::Rect);
    assert_eq!(
        state.drag_tool_bindings.left.ctrl_shift_drag.tool,
        DragTool::Arrow
    );
    assert_eq!(
        state.drag_tool_bindings.left.tab_drag.tool,
        DragTool::Ellipse
    );
}

#[test]
fn test_adjust_font_size_decrease() {
    let mut state = create_test_input_state();
    assert_eq!(state.current_font_size, 32.0);

    state.adjust_font_size(-2.0);
    assert_eq!(state.current_font_size, 30.0);
    assert!(state.needs_redraw);
}

#[test]
fn test_toggle_all_highlights_toggles_both() {
    let mut state = create_test_input_state();

    // Start off disabled
    assert!(!state.highlight_tool_active());
    assert!(!state.click_highlight_enabled());

    // Enable: should turn on both tool and click highlight
    let enabled = state.toggle_all_highlights();
    assert!(enabled);
    assert!(state.highlight_tool_active());
    assert!(state.click_highlight_enabled());

    // Disable: should turn off both
    let enabled_after = state.toggle_all_highlights();
    assert!(!enabled_after);
    assert!(!state.highlight_tool_active());
    assert!(!state.click_highlight_enabled());
}

#[test]
fn test_adjust_font_size_clamp_min() {
    let mut state = create_test_input_state();
    state.current_font_size = 10.0;

    // Try to go below minimum (8.0)
    state.adjust_font_size(-5.0);
    assert_eq!(state.current_font_size, 8.0);
}

#[test]
fn test_adjust_font_size_clamp_max() {
    let mut state = create_test_input_state();
    state.current_font_size = 70.0;

    // Try to go above maximum (72.0)
    state.adjust_font_size(5.0);
    assert_eq!(state.current_font_size, 72.0);
}

#[test]
fn test_adjust_font_size_at_boundaries() {
    let mut state = create_test_input_state();

    // Test at minimum boundary
    state.current_font_size = 8.0;
    state.adjust_font_size(0.0);
    assert_eq!(state.current_font_size, 8.0);

    // Test at maximum boundary
    state.current_font_size = 72.0;
    state.adjust_font_size(0.0);
    assert_eq!(state.current_font_size, 72.0);
}

#[test]
fn test_adjust_font_size_multiple_adjustments() {
    let mut state = create_test_input_state();
    assert_eq!(state.current_font_size, 32.0);

    // Simulate multiple Ctrl+Shift++ presses
    state.adjust_font_size(2.0);
    state.adjust_font_size(2.0);
    state.adjust_font_size(2.0);
    assert_eq!(state.current_font_size, 38.0);

    // Then decrease
    state.adjust_font_size(-2.0);
    state.adjust_font_size(-2.0);
    assert_eq!(state.current_font_size, 34.0);
}

/// Each chrome action queues its own durable entry carrying the value it
/// replaced, so persistence does not depend on which handler dispatched it --
/// a keybinding, a command-palette click, or a menu command all reach the
/// queue through `handle_action`.
#[test]
fn chrome_actions_queue_their_own_durable_entry() {
    use crate::input::state::PendingToolbarPersistence as Pending;

    for (action, expected) in [
        (
            crate::config::Action::ToggleStatusBar,
            Pending::StatusBar { previous: true },
        ),
        (
            crate::config::Action::ToggleFloatingBadge,
            Pending::FloatingBadge { previous: true },
        ),
        (
            crate::config::Action::ToggleZoomChip,
            Pending::ZoomChip { previous: true },
        ),
    ] {
        let mut state = create_test_input_state();
        state.handle_action(action);
        assert_eq!(
            state.take_pending_toolbar_persistence(),
            vec![expected],
            "{action:?} must queue its pre-change value"
        );
    }
}

/// Toggling a chrome preference while focus mode is active breaks focus in
/// the same action. That is still the user choosing, so it still persists --
/// which a before/after read of the focus flag would get exactly backwards.
#[test]
fn a_chrome_toggle_that_breaks_focus_mode_still_persists() {
    use crate::input::state::PendingToolbarPersistence as Pending;

    let mut state = create_test_input_state();
    state.handle_action(crate::config::Action::ToggleFocusMode);
    assert!(state.focus_mode_active());
    // Focus mode taking chrome over is not a preference.
    assert!(state.take_pending_toolbar_persistence().is_empty());

    let previous = state.ui_visibility.show_status_bar;
    state.handle_action(crate::config::Action::ToggleStatusBar);
    assert!(!state.focus_mode_active(), "the toggle breaks focus mode");
    assert_eq!(
        state.take_pending_toolbar_persistence(),
        vec![Pending::StatusBar { previous }],
    );
}

/// Focus mode hides chrome on the way in and puts it back on the way out;
/// neither is the user choosing to live without it.
#[test]
fn focus_mode_transitions_queue_no_durable_chrome_change() {
    let mut state = create_test_input_state();

    state.handle_action(crate::config::Action::ToggleFocusMode);
    assert!(!state.ui_visibility.show_status_bar);
    assert!(state.take_pending_toolbar_persistence().is_empty());

    state.handle_action(crate::config::Action::ToggleFocusMode);
    assert!(state.ui_visibility.show_status_bar);
    assert!(state.take_pending_toolbar_persistence().is_empty());
}

/// The queue coalesces per kind, so a burst across different chrome
/// preferences keeps one entry each rather than the first swallowing the rest.
#[test]
fn a_burst_across_chrome_kinds_keeps_one_entry_each() {
    use crate::input::state::PendingToolbarPersistence as Pending;

    let mut state = create_test_input_state();
    state.handle_action(crate::config::Action::ToggleStatusBar);
    state.handle_action(crate::config::Action::ToggleFloatingBadge);
    state.handle_action(crate::config::Action::ToggleZoomChip);

    assert_eq!(
        state.take_pending_toolbar_persistence(),
        vec![
            Pending::StatusBar { previous: true },
            Pending::FloatingBadge { previous: true },
            Pending::ZoomChip { previous: true },
        ],
    );
}

/// A burst that lands where it started is dropped: nothing durable changed.
#[test]
fn a_chrome_toggle_pressed_twice_queues_nothing() {
    let mut state = create_test_input_state();
    state.handle_action(crate::config::Action::ToggleStatusBar);
    state.handle_action(crate::config::Action::ToggleStatusBar);

    assert!(state.ui_visibility.show_status_bar);
    assert!(state.take_pending_toolbar_persistence().is_empty());
}

/// Selecting the highlight tool switches the click highlight on as a side
/// effect, and the explicit toggles move it directly; all of them persist.
#[test]
fn highlight_actions_queue_the_click_highlight() {
    use crate::input::state::PendingToolbarPersistence as Pending;

    for action in [
        crate::config::Action::ToggleClickHighlight,
        crate::config::Action::ToggleHighlightTool,
        crate::config::Action::SelectHighlightTool,
    ] {
        let mut state = create_test_input_state();
        let previous_enabled = state.click_highlight_enabled();
        let previous_tool_ring = state.highlight_tool_ring_enabled();
        state.handle_action(action);
        assert_eq!(
            state.take_pending_toolbar_persistence(),
            vec![Pending::ClickHighlight {
                previous_enabled,
                previous_tool_ring,
            }],
            "{action:?} must queue the click highlight"
        );
    }
}

/// A context-menu command is the same durable choice as the keybinding it
/// mirrors, so it has to reach the queue the same way.
#[test]
fn the_context_menu_highlight_command_queues_the_click_highlight() {
    use crate::input::state::PendingToolbarPersistence as Pending;
    use crate::input::state::core::MenuCommand;

    let mut state = create_test_input_state();
    let previous_enabled = state.click_highlight_enabled();
    let previous_tool_ring = state.highlight_tool_ring_enabled();

    state.execute_menu_command(MenuCommand::ToggleHighlightTool);

    assert_eq!(
        state.take_pending_toolbar_persistence(),
        vec![Pending::ClickHighlight {
            previous_enabled,
            previous_tool_ring,
        }],
    );
}
