use super::*;
use crate::draw::ShapeId;

fn add_pressure_shape(state: &mut InputState, locked: bool) -> ShapeId {
    let id = state
        .boards
        .active_frame_mut()
        .add_shape(Shape::FreehandPressure {
            points: vec![(0, 0, 2.0), (10, 10, 4.0)],
            color: state.current_color,
        });
    if locked {
        let frame = state.boards.active_frame_mut();
        if let Some(drawn) = frame.shape_mut(id) {
            drawn.locked = true;
        }
    }
    id
}

fn add_text_shape(state: &mut InputState) -> ShapeId {
    state.boards.active_frame_mut().add_shape(Shape::Text {
        x: 40,
        y: 60,
        text: "Note".to_string(),
        color: state.current_color,
        size: state.current_font_size,
        font_descriptor: state.font_descriptor.clone(),
        background_enabled: state.text_background_enabled,
        wrap_width: None,
    })
}

fn open_panel(state: &mut InputState) {
    assert!(state.show_properties_panel());
}

fn thickness_entry_index(state: &InputState) -> Option<usize> {
    state.properties_panel().and_then(|panel| {
        panel
            .entries
            .iter()
            .position(|entry| entry.label == "Thickness")
    })
}

fn pressure_points(state: &InputState, id: ShapeId) -> Vec<f32> {
    let frame = state.boards.active_frame();
    match frame.shape(id).map(|drawn| &drawn.shape) {
        Some(Shape::FreehandPressure { points, .. }) => points.iter().map(|(_, _, t)| *t).collect(),
        other => panic!("expected FreehandPressure shape, got: {:?}", other),
    }
}

fn text_snapshot(
    state: &InputState,
    id: ShapeId,
) -> (
    i32,
    i32,
    String,
    Color,
    f64,
    FontDescriptor,
    bool,
    Option<i32>,
) {
    let frame = state.boards.active_frame();
    match frame.shape(id).map(|drawn| &drawn.shape) {
        Some(Shape::Text {
            x,
            y,
            text,
            color,
            size,
            font_descriptor,
            background_enabled,
            wrap_width,
        }) => (
            *x,
            *y,
            text.clone(),
            *color,
            *size,
            font_descriptor.clone(),
            *background_enabled,
            *wrap_width,
        ),
        other => panic!("expected Text shape, got: {:?}", other),
    }
}

#[test]
fn pressure_entry_mode_never_hides_thickness_entry() {
    let mut state = create_test_input_state();
    state.pressure_thickness_entry_mode = PressureThicknessEntryMode::Never;
    state.pressure_thickness_edit_mode = PressureThicknessEditMode::Add;

    let id = add_pressure_shape(&mut state, false);
    state.set_selection(vec![id]);

    open_panel(&mut state);
    assert!(thickness_entry_index(&state).is_none());
}

#[test]
fn pressure_entry_mode_pressure_only_requires_all_pressure() {
    let mut state = create_test_input_state();
    state.pressure_thickness_entry_mode = PressureThicknessEntryMode::PressureOnly;
    state.pressure_thickness_edit_mode = PressureThicknessEditMode::Add;

    let pressure_id = add_pressure_shape(&mut state, false);
    let text_id = add_text_shape(&mut state);
    state.set_selection(vec![pressure_id, text_id]);

    open_panel(&mut state);
    assert!(thickness_entry_index(&state).is_none());
}

#[test]
fn pressure_entry_enabled_when_edit_mode_add_and_unlocked() {
    let mut state = create_test_input_state();
    state.pressure_thickness_entry_mode = PressureThicknessEntryMode::PressureOnly;
    state.pressure_thickness_edit_mode = PressureThicknessEditMode::Add;

    let id = add_pressure_shape(&mut state, false);
    state.set_selection(vec![id]);

    open_panel(&mut state);
    let index = thickness_entry_index(&state).expect("thickness entry should be present");
    let entry = &state
        .properties_panel()
        .expect("properties panel should be open")
        .entries[index];
    assert!(!entry.disabled);

    state.set_properties_panel_focus(Some(index));
    assert!(state.adjust_properties_panel_entry(1));

    let updated = pressure_points(&state, id);
    assert!((updated[0] - 3.0).abs() < 0.01);
    assert!((updated[1] - 5.0).abs() < 0.01);
}

#[test]
fn pressure_entry_add_mode_decrements_thickness() {
    let mut state = create_test_input_state();
    state.pressure_thickness_entry_mode = PressureThicknessEntryMode::PressureOnly;
    state.pressure_thickness_edit_mode = PressureThicknessEditMode::Add;

    let id = add_pressure_shape(&mut state, false);
    state.set_selection(vec![id]);

    open_panel(&mut state);
    let index = thickness_entry_index(&state).expect("thickness entry should be present");
    let entry = &state
        .properties_panel()
        .expect("properties panel should be open")
        .entries[index];
    assert!(!entry.disabled);

    state.set_properties_panel_focus(Some(index));
    assert!(state.adjust_properties_panel_entry(-1));

    let updated = pressure_points(&state, id);
    assert!((updated[0] - 1.0).abs() < 0.01);
    assert!((updated[1] - 3.0).abs() < 0.01);
}

#[test]
fn pressure_entry_disabled_when_edit_mode_disabled() {
    let mut state = create_test_input_state();
    state.pressure_thickness_entry_mode = PressureThicknessEntryMode::PressureOnly;
    state.pressure_thickness_edit_mode = PressureThicknessEditMode::Disabled;

    let id = add_pressure_shape(&mut state, false);
    state.set_selection(vec![id]);

    open_panel(&mut state);
    let index = thickness_entry_index(&state).expect("thickness entry should be present");
    let entry = &state
        .properties_panel()
        .expect("properties panel should be open")
        .entries[index];
    assert!(entry.disabled);
    assert_eq!(entry.value, "Varies (pressure)");

    state.set_properties_panel_focus(Some(index));
    assert!(!state.adjust_properties_panel_entry(1));

    let updated = pressure_points(&state, id);
    assert!((updated[0] - 2.0).abs() < 0.01);
    assert!((updated[1] - 4.0).abs() < 0.01);
}

#[test]
fn pressure_entry_locked_when_all_locked() {
    let mut state = create_test_input_state();
    state.pressure_thickness_entry_mode = PressureThicknessEntryMode::PressureOnly;
    state.pressure_thickness_edit_mode = PressureThicknessEditMode::Add;

    let id = add_pressure_shape(&mut state, true);
    state.set_selection(vec![id]);

    open_panel(&mut state);
    let index = thickness_entry_index(&state).expect("thickness entry should be present");
    let entry = &state
        .properties_panel()
        .expect("properties panel should be open")
        .entries[index];
    assert!(entry.disabled);
    assert_eq!(entry.value, "Locked");
}

#[test]
fn pressure_entry_mode_any_pressure_shows_for_mixed_selection() {
    let mut state = create_test_input_state();
    state.pressure_thickness_entry_mode = PressureThicknessEntryMode::AnyPressure;
    state.pressure_thickness_edit_mode = PressureThicknessEditMode::Disabled;

    let pressure_id = add_pressure_shape(&mut state, false);
    let text_id = add_text_shape(&mut state);
    state.set_selection(vec![pressure_id, text_id]);

    open_panel(&mut state);
    let index = thickness_entry_index(&state).expect("thickness entry should be present");
    let entry = &state
        .properties_panel()
        .expect("properties panel should be open")
        .entries[index];
    assert!(entry.disabled);
    assert_eq!(entry.value, "Varies (pressure)");
}

#[test]
fn pressure_entry_mode_any_pressure_mixed_lock_states_is_editable() {
    let mut state = create_test_input_state();
    state.pressure_thickness_entry_mode = PressureThicknessEntryMode::AnyPressure;
    state.pressure_thickness_edit_mode = PressureThicknessEditMode::Add;

    let locked_id = add_pressure_shape(&mut state, true);
    let unlocked_id = add_pressure_shape(&mut state, false);
    state.set_selection(vec![locked_id, unlocked_id]);

    open_panel(&mut state);
    let index = thickness_entry_index(&state).expect("thickness entry should be present");
    let entry = &state
        .properties_panel()
        .expect("properties panel should be open")
        .entries[index];
    assert!(!entry.disabled);
    assert_eq!(entry.value, "Varies (pressure)");
}

#[test]
fn pressure_entry_mode_any_pressure_add_updates_pressure_only() {
    let mut state = create_test_input_state();
    state.pressure_thickness_entry_mode = PressureThicknessEntryMode::AnyPressure;
    state.pressure_thickness_edit_mode = PressureThicknessEditMode::Add;

    let pressure_id = add_pressure_shape(&mut state, false);
    let text_id = add_text_shape(&mut state);
    let before = text_snapshot(&state, text_id);
    state.set_selection(vec![pressure_id, text_id]);

    open_panel(&mut state);
    let index = thickness_entry_index(&state).expect("thickness entry should be present");
    let entry = &state
        .properties_panel()
        .expect("properties panel should be open")
        .entries[index];
    assert!(!entry.disabled);
    assert_eq!(entry.value, "Varies (pressure)");

    state.set_properties_panel_focus(Some(index));
    assert!(state.adjust_properties_panel_entry(1));

    let updated = pressure_points(&state, pressure_id);
    assert!((updated[0] - 3.0).abs() < 0.01);
    assert!((updated[1] - 5.0).abs() < 0.01);
    assert_eq!(before, text_snapshot(&state, text_id));
}

#[test]
fn pressure_entry_mode_any_pressure_scale_updates_pressure_only() {
    let mut state = create_test_input_state();
    state.pressure_thickness_entry_mode = PressureThicknessEntryMode::AnyPressure;
    state.pressure_thickness_edit_mode = PressureThicknessEditMode::Scale;
    state.pressure_thickness_scale_step = 0.1;

    let pressure_id = add_pressure_shape(&mut state, false);
    let text_id = add_text_shape(&mut state);
    let before = text_snapshot(&state, text_id);
    state.set_selection(vec![pressure_id, text_id]);

    open_panel(&mut state);
    let index = thickness_entry_index(&state).expect("thickness entry should be present");
    let entry = &state
        .properties_panel()
        .expect("properties panel should be open")
        .entries[index];
    assert!(!entry.disabled);

    state.set_properties_panel_focus(Some(index));
    assert!(state.adjust_properties_panel_entry(1));

    let updated = pressure_points(&state, pressure_id);
    assert!((updated[0] - 2.2).abs() < 0.01);
    assert!((updated[1] - 4.4).abs() < 0.01);
    assert_eq!(before, text_snapshot(&state, text_id));
}

#[test]
fn pressure_entry_scale_mode_applies_step() {
    let mut state = create_test_input_state();
    state.pressure_thickness_entry_mode = PressureThicknessEntryMode::PressureOnly;
    state.pressure_thickness_edit_mode = PressureThicknessEditMode::Scale;
    state.pressure_thickness_scale_step = 0.1;

    let id = add_pressure_shape(&mut state, false);
    state.set_selection(vec![id]);

    open_panel(&mut state);
    let index = thickness_entry_index(&state).expect("thickness entry should be present");
    let entry = &state
        .properties_panel()
        .expect("properties panel should be open")
        .entries[index];
    assert!(!entry.disabled);

    state.set_properties_panel_focus(Some(index));
    assert!(state.adjust_properties_panel_entry(1));

    let updated = pressure_points(&state, id);
    assert!((updated[0] - 2.2).abs() < 0.01);
    assert!((updated[1] - 4.4).abs() < 0.01);
}

#[test]
fn pressure_entry_scale_mode_decrements_thickness() {
    let mut state = create_test_input_state();
    state.pressure_thickness_entry_mode = PressureThicknessEntryMode::PressureOnly;
    state.pressure_thickness_edit_mode = PressureThicknessEditMode::Scale;
    state.pressure_thickness_scale_step = 0.1;

    let id = add_pressure_shape(&mut state, false);
    state.set_selection(vec![id]);

    open_panel(&mut state);
    let index = thickness_entry_index(&state).expect("thickness entry should be present");
    let entry = &state
        .properties_panel()
        .expect("properties panel should be open")
        .entries[index];
    assert!(!entry.disabled);

    state.set_properties_panel_focus(Some(index));
    assert!(state.adjust_properties_panel_entry(-1));

    let updated = pressure_points(&state, id);
    assert!((updated[0] - 1.8).abs() < 0.01);
    assert!((updated[1] - 3.6).abs() < 0.01);
}
