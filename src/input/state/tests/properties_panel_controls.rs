//! Pointer, wheel, and keyboard use of the properties panel's controls.

use super::*;
use crate::domain::color::{PALETTE_GREEN, PALETTE_RED};
use crate::draw::{ArrowStyle, ShapeId, TextMeasurer};
use crate::input::state::{PropertiesPanelHit, PropertiesPanelLock};

const SCREEN: (u32, u32) = (1280, 800);

fn lay_out(state: &mut InputState) {
    let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, 1, 1).unwrap();
    let ctx = cairo::Context::new(&surface).unwrap();
    state.update_properties_panel_layout(&ctx, SCREEN.0, SCREEN.1);
}

fn open(state: &mut InputState, ids: Vec<ShapeId>) {
    state.set_selection(ids);
    assert!(state.show_properties_panel_with(&TextMeasurer::default()));
    lay_out(state);
}

fn add_rect(state: &mut InputState, color: Color, fill: bool) -> ShapeId {
    state.boards.active_frame_mut().add_shape(Shape::Rect {
        x: 100,
        y: 100,
        w: 60,
        h: 40,
        fill,
        color,
        thick: 3.0,
    })
}

fn add_arrow(state: &mut InputState, style: ArrowStyle, head_at_end: bool) -> ShapeId {
    state.boards.active_frame_mut().add_shape(Shape::Arrow {
        x1: 100,
        y1: 200,
        x2: 300,
        y2: 240,
        color: PALETTE_RED,
        thick: 3.0,
        arrow_length: 24.0,
        arrow_angle: 35.0,
        head_at_end,
        style,
        bend: 0.0,
        label: None,
    })
}

fn row(state: &InputState, label: &str) -> usize {
    state
        .properties_panel()
        .expect("panel")
        .entries
        .iter()
        .position(|entry| entry.label == label)
        .expect(label)
}

/// The center of `hit` as drawn this frame.
fn point(state: &InputState, hit: PropertiesPanelHit) -> (i32, i32) {
    let layout = state.properties_panel_layout().expect("layout");
    let rect = layout
        .hit_rect(state.properties_panel().expect("panel"), hit)
        .unwrap_or_else(|| panic!("{hit:?} is drawn"));
    let (x, y) = rect.center();
    (x.round() as i32, y.round() as i32)
}

/// A full left click on `hit`: press and release on the same spot.
fn click(state: &mut InputState, hit: PropertiesPanelHit) {
    let (x, y) = point(state, hit);
    click_at(state, (x, y), (x, y));
}

fn click_at(state: &mut InputState, press: (i32, i32), release: (i32, i32)) {
    assert!(state.handle_properties_panel_press(MouseButton::Left, press.0, press.1));
    assert!(state.handle_properties_panel_release_at_with_measurer(
        &TextMeasurer::default(),
        release.0,
        release.1
    ));
    lay_out(state);
}

fn rect_of(state: &InputState, id: ShapeId) -> (Color, bool, f64) {
    match &state.boards.active_frame().shape(id).expect("rect").shape {
        Shape::Rect {
            color, fill, thick, ..
        } => (*color, *fill, *thick),
        other => panic!("expected rect, got {other:?}"),
    }
}

fn arrow_of(state: &InputState, id: ShapeId) -> (ArrowStyle, bool, f64) {
    match &state.boards.active_frame().shape(id).expect("arrow").shape {
        Shape::Arrow {
            style,
            head_at_end,
            bend,
            ..
        } => (*style, *head_at_end, *bend),
        other => panic!("expected arrow, got {other:?}"),
    }
}

fn undo_depth(state: &InputState) -> usize {
    state.boards.active_frame().undo_stack_len()
}

#[test]
fn clicking_a_swatch_recolors_the_selection_and_rings_that_swatch() {
    let mut state = create_test_input_state();
    let id = add_rect(&mut state, PALETTE_RED, false);
    open(&mut state, vec![id]);
    let color_row = row(&state, "Color");
    let green = state
        .properties_panel()
        .unwrap()
        .swatches
        .iter()
        .position(|swatch| swatch.color == PALETTE_GREEN)
        .expect("green swatch");

    click(
        &mut state,
        PropertiesPanelHit::Swatch {
            row: color_row,
            index: green,
        },
    );

    assert_eq!(rect_of(&state, id).0, PALETTE_GREEN);
    let panel = state.properties_panel().expect("panel");
    assert_eq!(panel.entries[color_row].value, "Green");
    assert_eq!(panel.current_swatch(&panel.entries[color_row]), Some(green));
}

#[test]
fn clicking_the_swatch_the_selection_already_has_is_a_quiet_no_op() {
    let mut state = create_test_input_state();
    let id = add_rect(&mut state, PALETTE_RED, false);
    open(&mut state, vec![id]);
    let color_row = row(&state, "Color");
    let depth = undo_depth(&state);

    click(
        &mut state,
        PropertiesPanelHit::Swatch {
            row: color_row,
            index: 0,
        },
    );

    assert_eq!(rect_of(&state, id).0, PALETTE_RED);
    assert_eq!(undo_depth(&state), depth, "no edit, so no undo entry");
    assert!(!state.has_active_toast(), "and no \"No changes\" toast");
}

#[test]
fn stepper_buttons_step_thickness_down_and_up() {
    let mut state = create_test_input_state();
    let id = add_rect(&mut state, PALETTE_RED, false);
    open(&mut state, vec![id]);
    let thickness = row(&state, "Thickness");

    click(&mut state, PropertiesPanelHit::StepUp(thickness));
    assert_eq!(rect_of(&state, id).2, 4.0);
    click(&mut state, PropertiesPanelHit::StepDown(thickness));
    click(&mut state, PropertiesPanelHit::StepDown(thickness));
    assert_eq!(rect_of(&state, id).2, 2.0);
    assert_eq!(
        state.properties_panel().unwrap().entries[thickness].value,
        "2.0px"
    );
}

#[test]
fn a_press_that_leaves_its_control_before_the_release_changes_nothing() {
    let mut state = create_test_input_state();
    let id = add_rect(&mut state, PALETTE_RED, false);
    open(&mut state, vec![id]);
    let thickness = row(&state, "Thickness");
    let up = point(&state, PropertiesPanelHit::StepUp(thickness));
    let down = point(&state, PropertiesPanelHit::StepDown(thickness));

    click_at(&mut state, up, down);

    assert_eq!(rect_of(&state, id).2, 3.0);
    assert!(state.is_properties_panel_open());
}

#[test]
fn the_switch_and_the_rest_of_its_row_both_toggle_fill() {
    let mut state = create_test_input_state();
    let id = add_rect(&mut state, PALETTE_RED, false);
    open(&mut state, vec![id]);
    let fill = row(&state, "Fill");

    click(&mut state, PropertiesPanelHit::Toggle(fill));
    assert!(rect_of(&state, id).1);
    assert_eq!(state.properties_panel().unwrap().entries[fill].value, "On");

    click(&mut state, PropertiesPanelHit::Row(fill));
    assert!(!rect_of(&state, id).1);
}

#[test]
fn a_mixed_fill_selection_turns_on_from_the_switch() {
    let mut state = create_test_input_state();
    let filled = add_rect(&mut state, PALETTE_RED, true);
    let outlined = add_rect(&mut state, PALETTE_RED, false);
    open(&mut state, vec![filled, outlined]);
    let fill = row(&state, "Fill");
    assert_eq!(
        state.properties_panel().unwrap().entries[fill].state,
        SelectionPropertyValue::Toggle(None)
    );

    click(&mut state, PropertiesPanelHit::Toggle(fill));

    assert!(rect_of(&state, filled).1 && rect_of(&state, outlined).1);
}

#[test]
fn head_segments_move_the_arrow_head_and_ignore_the_active_one() {
    let mut state = create_test_input_state();
    let id = add_arrow(&mut state, ArrowStyle::Standard, true);
    open(&mut state, vec![id]);
    let head = row(&state, "Arrow head");
    let depth = undo_depth(&state);

    click(
        &mut state,
        PropertiesPanelHit::ArrowHead {
            row: head,
            at_end: true,
        },
    );
    assert_eq!(undo_depth(&state), depth, "End is already set");

    click(
        &mut state,
        PropertiesPanelHit::ArrowHead {
            row: head,
            at_end: false,
        },
    );
    assert!(!arrow_of(&state, id).1);
    assert_eq!(
        state.properties_panel().unwrap().entries[head].value,
        "Start"
    );
}

#[test]
fn style_buttons_pick_that_style_and_curved_gets_an_arc() {
    let mut state = create_test_input_state();
    let id = add_arrow(&mut state, ArrowStyle::Standard, true);
    open(&mut state, vec![id]);
    let style = row(&state, "Arrow style");

    click(
        &mut state,
        PropertiesPanelHit::ArrowStyle {
            row: style,
            style: ArrowStyle::Double,
        },
    );
    assert_eq!(arrow_of(&state, id).0, ArrowStyle::Double);

    click(
        &mut state,
        PropertiesPanelHit::ArrowStyle {
            row: style,
            style: ArrowStyle::Curved,
        },
    );
    let (current, _, bend) = arrow_of(&state, id);
    assert_eq!(current, ArrowStyle::Curved);
    assert_ne!(bend, 0.0, "a curved arrow needs an arc to show");
    assert_eq!(
        state.properties_panel().unwrap().entries[style].state,
        SelectionPropertyValue::ArrowStyle(Some(ArrowStyle::Curved))
    );
}

#[test]
fn the_lock_button_locks_the_selection_and_its_rows_go_inert() {
    let mut state = create_test_input_state();
    let id = add_rect(&mut state, PALETTE_RED, false);
    open(&mut state, vec![id]);
    let thickness = row(&state, "Thickness");
    let up = point(&state, PropertiesPanelHit::StepUp(thickness));

    click(&mut state, PropertiesPanelHit::Lock);

    assert!(state.boards.active_frame().shape(id).expect("rect").locked);
    let panel = state.properties_panel().expect("panel");
    assert_eq!(panel.lock, PropertiesPanelLock::Locked);
    assert!(panel.entries.iter().all(|entry| entry.disabled));
    assert_eq!(state.properties_panel_active_hit_at(up.0, up.1), None);
    click_at(&mut state, up, up);
    assert_eq!(rect_of(&state, id).2, 3.0, "a locked row ignores clicks");

    click(&mut state, PropertiesPanelHit::Lock);
    assert!(!state.boards.active_frame().shape(id).expect("rect").locked);
    assert!(
        !state.properties_panel().unwrap().entries[thickness].disabled,
        "unlocking brings the rows back"
    );
}

#[test]
fn a_press_on_the_header_keeps_the_panel_open() {
    let mut state = create_test_input_state();
    let id = add_rect(&mut state, PALETTE_RED, false);
    open(&mut state, vec![id]);

    click(&mut state, PropertiesPanelHit::Title);

    assert!(state.is_properties_panel_open());
}

#[test]
fn wheel_over_a_row_steps_it_and_the_rest_of_the_panel_swallows_it() {
    let measurer = TextMeasurer::default();
    let mut state = create_test_input_state();
    let id = add_rect(&mut state, PALETTE_RED, false);
    open(&mut state, vec![id]);
    let thickness = row(&state, "Thickness");
    let (x, y) = point(&state, PropertiesPanelHit::Row(thickness));

    // Scrolling up (a negative axis direction) raises the value.
    assert!(state.properties_panel_wheel_with(&measurer, x, y, -1));
    assert_eq!(rect_of(&state, id).2, 4.0);
    assert!(state.properties_panel_wheel_with(&measurer, x, y, 1));
    assert_eq!(rect_of(&state, id).2, 3.0);

    let (title_x, title_y) = point(&state, PropertiesPanelHit::Title);
    assert!(
        state.properties_panel_wheel_with(&measurer, title_x, title_y, -1),
        "the header takes the tick so the tool behind does not"
    );
    assert_eq!(rect_of(&state, id).2, 3.0);

    let layout = *state.properties_panel_layout().unwrap();
    let off_panel = (layout.origin_x - 20.0) as i32;
    assert!(!state.properties_panel_wheel_with(&measurer, off_panel, y, -1));
}

#[test]
fn a_click_focuses_its_row_quietly_while_arrow_keys_show_the_ring() {
    let measurer = TextMeasurer::default();
    let mut state = create_test_input_state();
    let id = add_rect(&mut state, PALETTE_RED, false);
    open(&mut state, vec![id]);
    let thickness = row(&state, "Thickness");

    click(&mut state, PropertiesPanelHit::StepUp(thickness));
    let panel = state.properties_panel().unwrap();
    assert_eq!(panel.keyboard_focus, Some(thickness));
    assert!(!panel.focus_visible, "a click draws no focus ring");

    // The arrow keys continue on the clicked row.
    assert!(state.handle_properties_panel_key_with_measurer(&measurer, Key::Right));
    assert_eq!(rect_of(&state, id).2, 5.0);

    assert!(state.handle_properties_panel_key_with_measurer(&measurer, Key::Down));
    assert!(state.properties_panel().unwrap().focus_visible);
}

#[test]
fn shift_arrow_keys_step_numbers_five_at_a_time() {
    let measurer = TextMeasurer::default();
    let mut state = create_test_input_state();
    let id = add_rect(&mut state, PALETTE_RED, false);
    open(&mut state, vec![id]);
    let thickness = row(&state, "Thickness");
    state.set_properties_panel_focus(Some(thickness));

    state.modifiers.shift = true;
    assert!(state.handle_properties_panel_key_with_measurer(&measurer, Key::Right));

    assert_eq!(rect_of(&state, id).2, 8.0);
}

#[test]
fn more_colors_opens_the_picker_over_the_panel_and_ok_recolors_the_selection() {
    let mut state = create_test_input_state();
    let id = add_rect(&mut state, PALETTE_RED, false);
    open(&mut state, vec![id]);
    let color_row = row(&state, "Color");
    let custom = Color {
        r: 0.2,
        g: 0.4,
        b: 0.6,
        a: 1.0,
    };

    click(&mut state, PropertiesPanelHit::MoreColors(color_row));

    assert!(state.is_color_picker_popup_open());
    assert!(
        state.is_properties_panel_open(),
        "the panel stays open under the picker"
    );
    assert_eq!(state.color_picker_popup_title(), "Selection Color");
    assert_eq!(state.color_picker_popup_current_color(), Some(PALETTE_RED));
    assert!(state.modal_owns_wheel(), "the picker covers the panel");

    state.color_picker_popup_set_color(custom);
    assert_eq!(
        rect_of(&state, id).0,
        PALETTE_RED,
        "nothing changes before OK"
    );
    state.apply_color_picker_popup();

    assert!(!state.is_color_picker_popup_open());
    assert_eq!(rect_of(&state, id).0, custom);
    assert_eq!(state.recent_colors().first(), Some(&custom));
    lay_out(&mut state);
    assert_eq!(
        state.properties_panel().unwrap().entries[color_row].value,
        "Custom"
    );
}

#[test]
fn cancelling_the_selection_picker_leaves_the_shapes_alone() {
    let mut state = create_test_input_state();
    let id = add_rect(&mut state, PALETTE_RED, false);
    open(&mut state, vec![id]);
    let color_row = row(&state, "Color");
    click(&mut state, PropertiesPanelHit::MoreColors(color_row));

    state.color_picker_popup_set_color(PALETTE_GREEN);
    state.close_color_picker_popup(true);

    assert_eq!(rect_of(&state, id).0, PALETTE_RED);
    assert!(state.is_properties_panel_open());
}

#[test]
fn ok_on_a_mixed_selection_applies_even_the_color_it_opened_on() {
    let mut state = create_test_input_state();
    let red = add_rect(&mut state, PALETTE_RED, false);
    let green = add_rect(&mut state, PALETTE_GREEN, false);
    open(&mut state, vec![red, green]);
    let color_row = row(&state, "Color");
    click(&mut state, PropertiesPanelHit::MoreColors(color_row));
    let opened_on = state.color_picker_popup_current_color().expect("picker");

    state.apply_color_picker_popup();

    assert_eq!(rect_of(&state, red).0, opened_on);
    assert_eq!(rect_of(&state, green).0, opened_on);
}

#[test]
fn a_same_hue_swatch_still_changes_a_shape_of_another_opacity() {
    let mut state = create_test_input_state();
    let faint_red = Color {
        a: 0.25,
        ..PALETTE_RED
    };
    let id = add_rect(&mut state, faint_red, false);
    open(&mut state, vec![id]);
    let color_row = row(&state, "Color");
    let red = state
        .properties_panel()
        .unwrap()
        .swatches
        .iter()
        .position(|swatch| swatch.color == PALETTE_RED)
        .expect("red swatch");

    click(
        &mut state,
        PropertiesPanelHit::Swatch {
            row: color_row,
            index: red,
        },
    );

    assert_eq!(rect_of(&state, id).0, PALETTE_RED, "now opaque");
}

#[test]
fn a_marker_keeps_its_opacity_so_its_own_hue_is_a_quiet_no_op() {
    let mut state = create_test_input_state();
    let marker = state
        .boards
        .active_frame_mut()
        .add_shape(Shape::MarkerStroke {
            points: vec![(100, 100), (200, 120)],
            color: Color {
                a: 0.3,
                ..PALETTE_RED
            },
            thick: 12.0,
        });
    open(&mut state, vec![marker]);
    let color_row = row(&state, "Color");
    let depth = undo_depth(&state);

    click(
        &mut state,
        PropertiesPanelHit::Swatch {
            row: color_row,
            index: 0,
        },
    );

    assert_eq!(undo_depth(&state), depth);
    assert!(!state.has_active_toast());
    match &state.boards.active_frame().shape(marker).unwrap().shape {
        Shape::MarkerStroke { color, .. } => assert_eq!(color.a, 0.3),
        other => panic!("expected marker, got {other:?}"),
    }
}

#[test]
fn ok_evens_out_shapes_that_differ_only_in_opacity() {
    let mut state = create_test_input_state();
    let solid = add_rect(&mut state, PALETTE_RED, false);
    let faint = add_rect(
        &mut state,
        Color {
            a: 0.25,
            ..PALETTE_RED
        },
        false,
    );
    open(&mut state, vec![solid, faint]);
    let color_row = row(&state, "Color");
    click(&mut state, PropertiesPanelHit::MoreColors(color_row));
    assert_eq!(state.color_picker_popup_current_color(), Some(PALETTE_RED));

    state.apply_color_picker_popup();

    assert_eq!(rect_of(&state, solid).0, PALETTE_RED);
    assert_eq!(rect_of(&state, faint).0, PALETTE_RED);
}
