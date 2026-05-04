use super::*;
use crate::input::{DragBinding, DragButtonBindings, DragToolBindings};

fn left_drag_bindings(
    drag: Tool,
    shift_drag: Tool,
    ctrl_drag: Tool,
    ctrl_shift_drag: Tool,
    tab_drag: Tool,
) -> DragToolBindings {
    DragToolBindings {
        left: DragButtonBindings {
            drag: DragBinding::from_tool(drag),
            shift_drag: DragBinding::from_tool(shift_drag),
            ctrl_drag: DragBinding::from_tool(ctrl_drag),
            ctrl_shift_drag: DragBinding::from_tool(ctrl_shift_drag),
            tab_drag: DragBinding::from_tool(tab_drag),
        },
        right: DragButtonBindings::button_default(),
        middle: DragButtonBindings::button_default(),
    }
}

#[test]
fn mouse_drag_creates_shapes_for_each_tool() {
    let mut state = create_test_input_state();

    // Pen
    state.on_mouse_press(MouseButton::Left, 0, 0);
    state.on_mouse_motion(10, 10);
    state.on_mouse_release(MouseButton::Left, 10, 10);
    assert_eq!(state.boards.active_frame().shapes.len(), 1);
    state.clear_selection();

    // Line (Shift)
    state.modifiers.shift = true;
    state.on_mouse_press(MouseButton::Left, 20, 20);
    state.on_mouse_release(MouseButton::Left, 25, 25);
    assert_eq!(state.boards.active_frame().shapes.len(), 2);
    state.clear_selection();

    // Rectangle (Ctrl)
    state.modifiers.shift = false;
    state.modifiers.ctrl = true;
    state.on_mouse_press(MouseButton::Left, 40, 40);
    state.on_mouse_release(MouseButton::Left, 45, 45);
    assert_eq!(state.boards.active_frame().shapes.len(), 3);
    state.clear_selection();

    // Ellipse (Tab)
    state.modifiers.ctrl = false;
    state.modifiers.tab = true;
    state.on_mouse_press(MouseButton::Left, 60, 60);
    state.on_mouse_release(MouseButton::Left, 64, 64);
    assert_eq!(state.boards.active_frame().shapes.len(), 4);
    state.clear_selection();

    // Arrow (Ctrl+Shift)
    state.modifiers.tab = false;
    state.modifiers.ctrl = true;
    state.modifiers.shift = true;
    state.on_mouse_press(MouseButton::Left, 80, 80);
    state.on_mouse_release(MouseButton::Left, 86, 86);
    assert_eq!(state.boards.active_frame().shapes.len(), 5);
}

#[test]
fn custom_drag_bindings_remap_default_and_modifier_tools() {
    let mut state = create_test_input_state();
    assert!(state.set_drag_tool_bindings(left_drag_bindings(
        Tool::Arrow,
        Tool::Eraser,
        Tool::Pen,
        Tool::Rect,
        Tool::Ellipse,
    )));

    assert_eq!(state.active_tool(), Tool::Arrow);
    assert!(state.set_tool_override(Some(Tool::Arrow)));
    assert_eq!(state.active_tool(), Tool::Arrow);

    state.modifiers.ctrl = true;
    assert_eq!(state.active_tool(), Tool::Pen);

    state.modifiers.ctrl = false;
    state.modifiers.shift = true;
    assert_eq!(state.active_tool(), Tool::Eraser);

    state.modifiers.ctrl = true;
    assert_eq!(state.active_tool(), Tool::Rect);
}

#[test]
fn blur_drag_requests_frozen_capture_on_press() {
    let mut state = create_test_input_state();
    assert!(state.set_drag_tool_bindings(left_drag_bindings(
        Tool::Blur,
        Tool::Line,
        Tool::Rect,
        Tool::Arrow,
        Tool::Ellipse,
    )));

    state.on_mouse_press(MouseButton::Left, 12, 14);

    assert!(state.take_pending_frozen_toggle());
    assert!(matches!(
        state.state,
        DrawingState::Drawing {
            tool: Tool::Blur,
            ..
        }
    ));
}

#[test]
fn drag_mapped_highlight_reports_highlight_active() {
    let mut state = create_test_input_state();
    assert!(state.set_drag_tool_bindings(left_drag_bindings(
        Tool::Highlight,
        Tool::Line,
        Tool::Rect,
        Tool::Arrow,
        Tool::Ellipse,
    )));

    assert_eq!(state.active_tool(), Tool::Highlight);
    assert!(state.highlight_tool_active());

    state.modifiers.shift = true;
    assert_eq!(state.active_tool(), Tool::Line);
    assert!(!state.highlight_tool_active());
}

#[test]
fn right_button_drag_uses_configured_tool() {
    let mut state = create_test_input_state();
    let mut bindings = DragToolBindings::default();
    bindings.right.drag = DragBinding::from_tool(Tool::Line);
    assert!(state.set_drag_tool_bindings(bindings));

    state.on_mouse_press(MouseButton::Right, 10, 20);
    assert!(matches!(
        state.state,
        DrawingState::Drawing {
            tool: Tool::Line,
            ..
        }
    ));
    state.on_mouse_release(MouseButton::Right, 30, 40);

    assert_eq!(state.boards.active_frame().shapes.len(), 1);
    assert!(matches!(
        state.boards.active_frame().shapes[0].shape,
        Shape::Line { .. }
    ));
}

#[test]
fn configured_non_left_drag_closes_context_menu_before_drawing() {
    let mut state = create_test_input_state();
    let mut bindings = DragToolBindings::default();
    bindings.right.drag = DragBinding::from_tool(Tool::Pen);
    assert!(state.set_drag_tool_bindings(bindings));
    state.open_context_menu((0, 0), Vec::new(), ContextMenuKind::Canvas, None);

    state.on_mouse_press(MouseButton::Right, 300, 300);
    state.on_mouse_motion(320, 320);
    state.on_mouse_release(MouseButton::Right, 320, 320);

    assert!(!state.is_context_menu_open());
    assert!(matches!(state.state, DrawingState::Idle));
    assert!(state.boards.active_frame().shapes.is_empty());
}

#[test]
fn drag_binding_color_overrides_stroke_without_changing_current_color() {
    let mut state = create_test_input_state();
    let original_color = state.current_color;
    let blue = Color {
        r: 0.0,
        g: 0.0,
        b: 1.0,
        a: 1.0,
    };
    let mut bindings = DragToolBindings::default();
    bindings.right.drag = DragBinding::new(crate::input::DragTool::Pen, Some(blue));
    assert!(state.set_drag_tool_bindings(bindings));

    state.on_mouse_press(MouseButton::Right, 0, 0);
    state.on_mouse_motion(10, 10);
    state.on_mouse_release(MouseButton::Right, 10, 10);

    assert_eq!(state.current_color, original_color);
    match &state.boards.active_frame().shapes[0].shape {
        Shape::Freehand { color, .. } => assert_eq!(*color, blue),
        other => panic!("expected freehand shape, got {other:?}"),
    }
}

#[test]
fn toggle_click_highlight_action_changes_state() {
    let mut state = create_test_input_state();
    assert!(!state.click_highlight_enabled());

    state.handle_action(Action::ToggleClickHighlight);
    assert!(state.click_highlight_enabled());
    assert!(state.needs_redraw);

    state.needs_redraw = false;
    state.handle_action(Action::ToggleClickHighlight);
    assert!(!state.click_highlight_enabled());
    assert!(state.needs_redraw);
}

#[test]
fn highlight_tool_prevents_drawing() {
    let mut state = create_test_input_state();
    assert_eq!(state.active_tool(), Tool::Pen);
    assert!(!state.highlight_tool_active());

    state.handle_action(Action::ToggleHighlightTool);
    assert!(state.highlight_tool_active());
    assert_eq!(state.active_tool(), Tool::Highlight);

    // Enable highlight effect to ensure no shapes are added while clicks happen
    state.handle_action(Action::ToggleClickHighlight);

    let initial_shapes = state.boards.active_frame().shapes.len();
    state.on_mouse_press(MouseButton::Left, 10, 10);
    state.on_mouse_release(MouseButton::Left, 20, 20);
    assert_eq!(state.boards.active_frame().shapes.len(), initial_shapes);
    assert!(matches!(state.state, DrawingState::Idle));

    // Toggle highlight tool off and ensure pen drawing resumes
    state.handle_action(Action::ToggleHighlightTool);
    assert!(!state.highlight_tool_active());
    state.on_mouse_press(MouseButton::Left, 0, 0);
    state.on_mouse_release(MouseButton::Left, 5, 5);
    assert_eq!(state.boards.active_frame().shapes.len(), initial_shapes + 1);
}

#[test]
fn sync_highlight_color_marks_dirty_when_pen_color_changes() {
    let mut state = create_test_input_state();
    state.needs_redraw = false;
    state.tool_settings.pen.color = Color {
        r: 0.25,
        g: 0.5,
        b: 0.75,
        a: 1.0,
    };
    state.sync_highlight_color();
    assert!(state.needs_redraw);
}
