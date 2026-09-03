use super::*;
use crate::draw::ArrowStyle;
use crate::input::BOARD_ID_WHITEBOARD;
use crate::util::Rect;

fn add_rect(state: &mut InputState, x: i32, y: i32, w: i32, h: i32) -> crate::draw::ShapeId {
    state.boards.active_frame_mut().add_shape(Shape::Rect {
        x,
        y,
        w,
        h,
        fill: false,
        color: state.style.current_color,
        thick: state.style.current_thickness,
    })
}

fn entry_index(state: &InputState, label: &str) -> usize {
    state
        .properties_panel()
        .expect("properties panel")
        .entries
        .iter()
        .position(|entry| entry.label == label)
        .expect(label)
}

#[test]
fn show_properties_panel_for_single_shape_reports_type_layer_and_lock_state() {
    let mut state = create_test_input_state();
    let shape_id = add_rect(&mut state, 10, 20, 30, 40);
    state.set_selection(vec![shape_id]);

    assert!(state.show_properties_panel());

    let panel = state.properties_panel().expect("properties panel");
    assert_eq!(panel.title, "Shape Properties");
    assert!(!panel.multiple_selection);
    assert!(
        panel
            .lines
            .iter()
            .any(|line| line == &format!("Shape ID: {shape_id}"))
    );
    assert!(panel.lines.iter().any(|line| line == "Type: Rectangle"));
    assert!(panel.lines.iter().any(|line| line == "Layer: 1 of 1"));
    assert!(panel.lines.iter().any(|line| line == "Locked: No"));
    assert!(panel.lines.iter().any(|line| line.starts_with("Bounds: ")));
}

#[test]
fn show_properties_panel_for_multi_selection_includes_locked_count_and_summary() {
    let mut state = create_test_input_state();
    let first = add_rect(&mut state, 10, 10, 20, 20);
    let second = add_rect(&mut state, 50, 15, 10, 15);
    let second_index = state
        .boards
        .active_frame()
        .find_index(second)
        .expect("second index");
    state.boards.active_frame_mut().shapes[second_index].locked = true;
    state.set_selection(vec![first, second]);

    assert!(state.show_properties_panel());

    let panel = state.properties_panel().expect("properties panel");
    assert_eq!(panel.title, "Selection Properties");
    assert!(panel.multiple_selection);
    assert!(panel.lines.iter().any(|line| line == "Shapes selected: 2"));
    assert!(panel.lines.iter().any(|line| line == "Locked: 1/2"));
    assert!(panel.lines.iter().any(|line| line.starts_with("Bounds: ")));
}

#[test]
fn style_pill_selection_docking_routes_through_the_properties_apply_machinery() {
    use crate::input::SelectionPropertyKind;
    use crate::ui::toolbar::{ToolbarEvent, ToolbarSnapshot};

    let mut state = create_test_input_state();
    let shape_id = add_rect(&mut state, 10, 20, 30, 40);
    state.set_selection(vec![shape_id]);
    assert!(state.apply_toolbar_event(ToolbarEvent::SelectTool(Tool::Select)));

    // The pill mirrors the popup's entry list without the popup opening.
    let entries = state.selection_pill_entries();
    assert!(state.properties_panel().is_none());
    assert!(
        entries
            .iter()
            .any(|entry| entry.kind == SelectionPropertyKind::Thickness)
    );

    // The snapshot docks the same entries under the select tool.
    let snapshot = ToolbarSnapshot::from_input(&state);
    assert_eq!(snapshot.selection_properties, entries);

    // A pill event adjusts the shape through the shared apply machinery.
    let thickness_of = |state: &InputState| match &state
        .boards
        .active_frame()
        .shape(shape_id)
        .expect("selected shape")
        .shape
    {
        Shape::Rect { thick, .. } => *thick,
        other => panic!("unexpected shape {other:?}"),
    };
    let before = thickness_of(&state);
    assert!(
        state.apply_toolbar_event(ToolbarEvent::AdjustSelectionProperty {
            kind: SelectionPropertyKind::Thickness,
            direction: 1,
        })
    );
    assert!(thickness_of(&state) > before);

    // Locked shapes surface as disabled entries and refuse adjustment.
    let index = state
        .boards
        .active_frame()
        .find_index(shape_id)
        .expect("shape index");
    state.boards.active_frame_mut().shapes[index].locked = true;
    let entries = state.selection_pill_entries();
    assert!(
        entries.iter().all(|entry| entry.disabled),
        "locked selection disables every entry: {entries:?}"
    );
    let locked_before = thickness_of(&state);
    assert!(
        !state.apply_toolbar_event(ToolbarEvent::AdjustSelectionProperty {
            kind: SelectionPropertyKind::Thickness,
            direction: 1,
        })
    );
    assert_eq!(thickness_of(&state), locked_before);

    // Clearing the selection empties the docked list again.
    state.clear_selection();
    assert!(state.selection_pill_entries().is_empty());
    assert!(
        ToolbarSnapshot::from_input(&state)
            .selection_properties
            .is_empty()
    );
}

#[test]
fn spotlight_magnification_property_steps_the_selected_shape_and_is_undoable() {
    let mut state = create_test_input_state();
    let shape_id = state.boards.active_frame_mut().add_shape(Shape::Spotlight {
        cx: 100,
        cy: 80,
        rx: 40,
        ry: 25,
        magnification: 1.5,
    });
    state.set_selection(vec![shape_id]);

    let entries = state.selection_pill_entries();
    let entry = entries
        .iter()
        .find(|entry| entry.kind == SelectionPropertyKind::SpotlightMagnification)
        .expect("spotlight magnification property");
    assert_eq!(entry.label, "Magnification");
    assert_eq!(entry.value, "1.5x");

    assert!(
        state.adjust_selection_property_kind(SelectionPropertyKind::SpotlightMagnification, 1,)
    );
    let magnification = |state: &InputState| match &state
        .boards
        .active_frame()
        .shape(shape_id)
        .expect("spotlight")
        .shape
    {
        Shape::Spotlight { magnification, .. } => *magnification,
        other => panic!("expected spotlight, got {other:?}"),
    };
    assert_eq!(magnification(&state), 1.75);
    assert!(state.take_pending_spotlight_magnifier_feedback());

    state.handle_action(Action::Undo);
    assert_eq!(magnification(&state), 1.5);
}

#[test]
fn close_properties_panel_clears_panel_and_requests_redraw() {
    let mut state = create_test_input_state();
    let shape_id = add_rect(&mut state, 5, 5, 10, 10);
    state.set_selection(vec![shape_id]);
    assert!(state.show_properties_panel());
    state.needs_redraw = false;

    state.close_properties_panel();

    assert!(state.properties_panel().is_none());
    assert!(state.properties_panel_layout().is_none());
    assert!(state.needs_redraw);
}

#[test]
fn show_properties_panel_anchors_to_screen_space_on_panned_boards() {
    let mut state = create_test_input_state();
    state.switch_board(BOARD_ID_WHITEBOARD);
    assert!(state.boards.active_frame_mut().set_view_offset(100, 50));
    state.update_pointer_position(400, 300);
    let shape_id = add_rect(&mut state, 140, 90, 20, 20);
    state.set_selection(vec![shape_id]);

    assert!(state.show_properties_panel());

    let panel = state.properties_panel().expect("properties panel");
    assert_eq!(panel.anchor_rect, Rect::new(38, 38, 24, 24));
}

#[test]
fn activate_fill_entry_toggles_rectangle_fill_and_refreshes_panel_value() {
    let mut state = create_test_input_state();
    let shape_id = add_rect(&mut state, 5, 5, 20, 20);
    state.set_selection(vec![shape_id]);
    assert!(state.show_properties_panel());
    let fill_index = entry_index(&state, "Fill");
    state.set_properties_panel_focus(Some(fill_index));

    assert!(state.activate_properties_panel_entry());

    match &state
        .boards
        .active_frame()
        .shape(shape_id)
        .expect("shape")
        .shape
    {
        Shape::Rect { fill, .. } => assert!(*fill),
        other => panic!("expected rect, got {other:?}"),
    }
    assert_eq!(
        state.properties_panel().expect("panel").entries[fill_index].value,
        "On"
    );
}

#[test]
fn adjust_font_size_entry_increases_text_size_and_refreshes_panel_value() {
    let mut state = create_test_input_state();
    let shape_id = state.boards.active_frame_mut().add_shape(Shape::Text {
        x: 10,
        y: 20,
        text: "Note".to_string(),
        color: state.style.current_color,
        size: 18.0,
        font_descriptor: state.style.font_descriptor.clone(),
        background_enabled: false,
        wrap_width: None,
    });
    state.set_selection(vec![shape_id]);
    assert!(state.show_properties_panel());
    let font_index = entry_index(&state, "Font size");
    state.set_properties_panel_focus(Some(font_index));

    assert!(state.adjust_properties_panel_entry(1));

    match &state
        .boards
        .active_frame()
        .shape(shape_id)
        .expect("shape")
        .shape
    {
        Shape::Text { size, .. } => assert_eq!(*size, 20.0),
        other => panic!("expected text, got {other:?}"),
    }
    assert_eq!(
        state.properties_panel().expect("panel").entries[font_index].value,
        "20pt"
    );
}

#[test]
fn activate_text_background_entry_on_mixed_selection_turns_all_backgrounds_on() {
    let mut state = create_test_input_state();
    let first = state.boards.active_frame_mut().add_shape(Shape::Text {
        x: 10,
        y: 20,
        text: "One".to_string(),
        color: state.style.current_color,
        size: 18.0,
        font_descriptor: state.style.font_descriptor.clone(),
        background_enabled: false,
        wrap_width: None,
    });
    let second = state.boards.active_frame_mut().add_shape(Shape::Text {
        x: 40,
        y: 50,
        text: "Two".to_string(),
        color: state.style.current_color,
        size: 18.0,
        font_descriptor: state.style.font_descriptor.clone(),
        background_enabled: true,
        wrap_width: None,
    });
    state.set_selection(vec![first, second]);
    assert!(state.show_properties_panel());
    let bg_index = entry_index(&state, "Text background");
    state.set_properties_panel_focus(Some(bg_index));

    assert!(state.activate_properties_panel_entry());

    for id in [first, second] {
        match &state
            .boards
            .active_frame()
            .shape(id)
            .expect("text shape")
            .shape
        {
            Shape::Text {
                background_enabled, ..
            } => assert!(*background_enabled),
            other => panic!("expected text, got {other:?}"),
        }
    }
    assert_eq!(
        state.properties_panel().expect("panel").entries[bg_index].value,
        "On"
    );
}

#[test]
fn adjust_arrow_length_entry_clamps_to_max_and_refreshes_panel_value() {
    let mut state = create_test_input_state();
    let shape_id = state.boards.active_frame_mut().add_shape(Shape::Arrow {
        x1: 0,
        y1: 0,
        x2: 20,
        y2: 10,
        color: state.style.current_color,
        thick: 3.0,
        arrow_length: 49.0,
        arrow_angle: 30.0,
        head_at_end: true,
        style: ArrowStyle::Standard,
        bend: 0.0,
        label: None,
    });
    state.set_selection(vec![shape_id]);
    assert!(state.show_properties_panel());
    let length_index = entry_index(&state, "Arrow length");
    state.set_properties_panel_focus(Some(length_index));

    assert!(state.adjust_properties_panel_entry(1));
    assert!(!state.adjust_properties_panel_entry(1));

    match &state
        .boards
        .active_frame()
        .shape(shape_id)
        .expect("arrow")
        .shape
    {
        Shape::Arrow { arrow_length, .. } => assert_eq!(*arrow_length, 50.0),
        other => panic!("expected arrow, got {other:?}"),
    }
    assert_eq!(
        state.properties_panel().expect("panel").entries[length_index].value,
        "50px"
    );
}

fn add_spotlight(state: &mut InputState, magnification: f64) -> crate::draw::ShapeId {
    state.boards.active_frame_mut().add_shape(Shape::Spotlight {
        cx: 100,
        cy: 80,
        rx: 40,
        ry: 25,
        magnification,
    })
}

fn spotlight_magnification(state: &InputState, id: crate::draw::ShapeId) -> f64 {
    match &state
        .boards
        .active_frame()
        .shape(id)
        .expect("spotlight")
        .shape
    {
        Shape::Spotlight { magnification, .. } => *magnification,
        other => panic!("expected spotlight, got {other:?}"),
    }
}

fn magnification_entry(state: &InputState) -> Option<crate::input::SelectionPropertyEntry> {
    state
        .selection_pill_entries()
        .into_iter()
        .find(|entry| entry.kind == SelectionPropertyKind::SpotlightMagnification)
}

#[test]
fn a_mixed_magnification_selection_reads_mixed_and_still_steps_every_shape() {
    let mut state = create_test_input_state();
    let low = add_spotlight(&mut state, 1.5);
    let high = add_spotlight(&mut state, 3.0);
    state.set_selection(vec![low, high]);

    let entry = magnification_entry(&state).expect("magnification property");
    assert_eq!(entry.value, "Mixed");
    assert!(!entry.disabled, "a mixed selection is still editable");

    assert!(state.adjust_selection_property_kind(SelectionPropertyKind::SpotlightMagnification, 1));
    assert_eq!(spotlight_magnification(&state, low), 1.75);
    assert_eq!(spotlight_magnification(&state, high), 3.25);

    // One step, one undo entry, for the whole selection.
    state.handle_action(Action::Undo);
    assert_eq!(spotlight_magnification(&state, low), 1.5);
    assert_eq!(spotlight_magnification(&state, high), 3.0);
}

#[test]
fn a_locked_spotlight_reports_locked_and_refuses_magnification_changes() {
    let mut state = create_test_input_state();
    let shape_id = add_spotlight(&mut state, 2.0);
    state.set_selection(vec![shape_id]);
    let index = state
        .boards
        .active_frame()
        .find_index(shape_id)
        .expect("shape index");
    state.boards.active_frame_mut().shapes[index].locked = true;

    let entry = magnification_entry(&state).expect("magnification property");
    assert_eq!(entry.value, "Locked");
    assert!(entry.disabled);

    assert!(
        !state.adjust_selection_property_kind(SelectionPropertyKind::SpotlightMagnification, 1)
    );
    assert_eq!(spotlight_magnification(&state, shape_id), 2.0);
}

#[test]
fn magnification_steps_stop_at_both_ends_of_the_supported_range() {
    let mut state = create_test_input_state();
    let lowest = add_spotlight(&mut state, crate::draw::MIN_SPOTLIGHT_MAGNIFICATION);
    state.set_selection(vec![lowest]);
    assert!(
        !state.adjust_selection_property_kind(SelectionPropertyKind::SpotlightMagnification, -1),
        "stepping below 1x must be a no-op, not a silent clamp with an undo entry"
    );
    assert_eq!(
        spotlight_magnification(&state, lowest),
        crate::draw::MIN_SPOTLIGHT_MAGNIFICATION
    );

    let highest = add_spotlight(&mut state, crate::draw::MAX_SPOTLIGHT_MAGNIFICATION);
    state.set_selection(vec![highest]);
    assert!(
        !state.adjust_selection_property_kind(SelectionPropertyKind::SpotlightMagnification, 1)
    );
    assert_eq!(
        spotlight_magnification(&state, highest),
        crate::draw::MAX_SPOTLIGHT_MAGNIFICATION
    );
}

#[test]
fn magnification_only_touches_the_spotlights_in_a_multi_kind_selection() {
    let mut state = create_test_input_state();
    let spotlight = add_spotlight(&mut state, 2.0);
    let rect = add_rect(&mut state, 5, 5, 10, 10);
    state.set_selection(vec![spotlight, rect]);

    let entry = magnification_entry(&state).expect("magnification property");
    assert_eq!(
        entry.value, "2x",
        "the one spotlight still reports its factor"
    );

    let rect_before = format!(
        "{:?}",
        state.boards.active_frame().shape(rect).expect("rect").shape
    );
    assert!(state.adjust_selection_property_kind(SelectionPropertyKind::SpotlightMagnification, 1));
    assert_eq!(spotlight_magnification(&state, spotlight), 2.25);
    assert_eq!(
        format!(
            "{:?}",
            state.boards.active_frame().shape(rect).expect("rect").shape
        ),
        rect_before,
        "a shape with no magnification must be left alone"
    );
}

#[test]
fn editing_a_selected_spotlight_leaves_the_next_shape_default_alone() {
    let mut state = create_test_input_state();
    let default_before = state.style.spotlight_magnification;
    let shape_id = add_spotlight(&mut state, 2.0);
    state.set_selection(vec![shape_id]);

    assert!(state.adjust_selection_property_kind(SelectionPropertyKind::SpotlightMagnification, 1));

    assert_eq!(spotlight_magnification(&state, shape_id), 2.25);
    assert_eq!(
        state.style.spotlight_magnification, default_before,
        "editing one shape must not rewrite what the next Spotlight will use"
    );
}

#[test]
fn the_selection_reports_its_own_highest_magnification() {
    let mut state = create_test_input_state();
    assert_eq!(state.selection_spotlight_magnification(), None);

    let rect = add_rect(&mut state, 5, 5, 10, 10);
    state.set_selection(vec![rect]);
    assert_eq!(
        state.selection_spotlight_magnification(),
        None,
        "a selection with no spotlight has no magnification to report"
    );

    let low = add_spotlight(&mut state, 1.5);
    let high = add_spotlight(&mut state, 3.0);
    state.set_selection(vec![rect, low, high]);
    assert_eq!(state.selection_spotlight_magnification(), Some(3.0));
}
