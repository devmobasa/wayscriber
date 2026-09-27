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
        fill_color: None,
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

fn arrow_length_of(state: &InputState, id: ShapeId) -> f64 {
    match &state.boards.active_frame().shape(id).expect("arrow").shape {
        Shape::Arrow { arrow_length, .. } => *arrow_length,
        other => panic!("expected arrow, got {other:?}"),
    }
}

#[test]
fn stepping_thickness_lands_on_whole_pixels() {
    let measurer = TextMeasurer::default();
    let mut state = create_test_input_state();
    let id = add_rect(&mut state, PALETTE_RED, false);
    if let Shape::Rect { thick, .. } =
        &mut state.boards.active_frame_mut().shape_mut(id).unwrap().shape
    {
        *thick = 3.2;
    }
    open(&mut state, vec![id]);
    state.set_properties_panel_focus(Some(row(&state, "Thickness")));

    assert!(state.handle_properties_panel_key_with_measurer(&measurer, Key::Right));
    assert_eq!(rect_of(&state, id).2, 4.0);
    assert!(state.handle_properties_panel_key_with_measurer(&measurer, Key::Left));
    assert_eq!(rect_of(&state, id).2, 3.0);
}

#[test]
fn stepper_buttons_step_a_number_down_and_up() {
    let mut state = create_test_input_state();
    let id = add_arrow(&mut state, ArrowStyle::Standard, true);
    open(&mut state, vec![id]);
    let length = row(&state, "Arrow length");

    click(&mut state, PropertiesPanelHit::StepUp(length));
    assert_eq!(arrow_length_of(&state, id), 26.0);
    click(&mut state, PropertiesPanelHit::StepDown(length));
    click(&mut state, PropertiesPanelHit::StepDown(length));
    assert_eq!(arrow_length_of(&state, id), 22.0);
    assert_eq!(
        state.properties_panel().unwrap().entries[length].value,
        "22px"
    );
}

#[test]
fn a_press_that_leaves_its_control_before_the_release_changes_nothing() {
    let mut state = create_test_input_state();
    let id = add_arrow(&mut state, ArrowStyle::Standard, true);
    open(&mut state, vec![id]);
    let length = row(&state, "Arrow length");
    let up = point(&state, PropertiesPanelHit::StepUp(length));
    let down = point(&state, PropertiesPanelHit::StepDown(length));

    click_at(&mut state, up, down);

    assert_eq!(arrow_length_of(&state, id), 24.0);
    assert!(state.is_properties_panel_open());
}

fn fill_of(state: &InputState, id: ShapeId) -> (bool, Option<Color>) {
    match &state.boards.active_frame().shape(id).expect("rect").shape {
        Shape::Rect {
            fill, fill_color, ..
        } => (*fill, *fill_color),
        other => panic!("expected rect, got {other:?}"),
    }
}

fn swatch_index(state: &InputState, color: Color) -> usize {
    state
        .properties_panel()
        .unwrap()
        .swatches
        .iter()
        .position(|swatch| swatch.color == color)
        .expect("swatch")
}

#[test]
fn a_fill_swatch_fills_with_its_own_color_and_no_fill_keeps_it_for_later() {
    let measurer = TextMeasurer::default();
    let mut state = create_test_input_state();
    let id = add_rect(&mut state, PALETTE_RED, false);
    open(&mut state, vec![id]);
    let fill = row(&state, "Fill");
    assert_eq!(
        state.properties_panel().unwrap().entries[fill].value,
        "None"
    );
    let blue = swatch_index(&state, crate::domain::color::PALETTE_BLUE);

    click(
        &mut state,
        PropertiesPanelHit::Swatch {
            row: fill,
            index: blue,
        },
    );
    assert_eq!(
        fill_of(&state, id),
        (true, Some(crate::domain::color::PALETTE_BLUE))
    );
    assert_eq!(
        rect_of(&state, id).0,
        PALETTE_RED,
        "the border keeps its color"
    );
    let panel = state.properties_panel().unwrap();
    assert_eq!(panel.entries[fill].value, "Blue");
    assert_eq!(panel.current_swatch(&panel.entries[fill]), Some(blue));

    click(&mut state, PropertiesPanelHit::NoFill(fill));
    assert_eq!(
        fill_of(&state, id),
        (false, Some(crate::domain::color::PALETTE_BLUE)),
        "no fill keeps the color for when the fill comes back"
    );
    assert_eq!(
        state.properties_panel().unwrap().entries[fill].value,
        "None"
    );

    state.set_properties_panel_focus(Some(fill));
    assert!(state.handle_properties_panel_key_with_measurer(&measurer, Key::Return));
    assert_eq!(
        fill_of(&state, id),
        (true, Some(crate::domain::color::PALETTE_BLUE))
    );
}

#[test]
fn a_mixed_fill_selection_takes_one_fill_from_a_swatch() {
    let mut state = create_test_input_state();
    let filled = add_rect(&mut state, PALETTE_RED, true);
    let outlined = add_rect(&mut state, PALETTE_RED, false);
    open(&mut state, vec![filled, outlined]);
    let fill = row(&state, "Fill");
    assert_eq!(
        state.properties_panel().unwrap().entries[fill].state,
        SelectionPropertyValue::Fill(None)
    );
    let green = swatch_index(&state, PALETTE_GREEN);

    click(
        &mut state,
        PropertiesPanelHit::Swatch {
            row: fill,
            index: green,
        },
    );

    assert_eq!(fill_of(&state, filled), (true, Some(PALETTE_GREEN)));
    assert_eq!(fill_of(&state, outlined), (true, Some(PALETTE_GREEN)));
}

#[test]
fn a_fill_shares_the_shapes_opacity() {
    let measurer = TextMeasurer::default();
    let mut state = create_test_input_state();
    let half_red = Color {
        a: 0.5,
        ..PALETTE_RED
    };
    let id = add_rect(&mut state, half_red, false);
    open(&mut state, vec![id]);
    let fill = row(&state, "Fill");
    let green = swatch_index(&state, PALETTE_GREEN);

    click(
        &mut state,
        PropertiesPanelHit::Swatch {
            row: fill,
            index: green,
        },
    );
    assert_eq!(
        fill_of(&state, id).1,
        Some(Color {
            a: 0.5,
            ..PALETTE_GREEN
        }),
        "an opaque swatch fills at the shape's opacity"
    );

    state.set_properties_panel_focus(Some(row(&state, "Opacity")));
    assert!(state.handle_properties_panel_key_with_measurer(&measurer, Key::Right));
    assert!((rect_of(&state, id).0.a - 0.55).abs() < 1e-9);
    assert!(
        (fill_of(&state, id).1.unwrap().a - 0.55).abs() < 1e-9,
        "the fill follows the border's opacity"
    );
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
    let up = point(&state, PropertiesPanelHit::Slider(thickness));

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
    let id = add_arrow(&mut state, ArrowStyle::Standard, true);
    open(&mut state, vec![id]);
    let length = row(&state, "Arrow length");

    click(&mut state, PropertiesPanelHit::StepUp(length));
    let panel = state.properties_panel().unwrap();
    assert_eq!(panel.keyboard_focus, Some(length));
    assert!(!panel.focus_visible, "a click draws no focus ring");

    // The arrow keys continue on the clicked row.
    assert!(state.handle_properties_panel_key_with_measurer(&measurer, Key::Right));
    assert_eq!(arrow_length_of(&state, id), 28.0);

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
fn an_opaque_swatch_changes_the_hue_and_keeps_the_shapes_opacity() {
    let mut state = create_test_input_state();
    let faint_red = Color {
        a: 0.25,
        ..PALETTE_RED
    };
    let id = add_rect(&mut state, faint_red, false);
    open(&mut state, vec![id]);
    let color_row = row(&state, "Color");
    let swatch = |state: &InputState, color: Color| {
        state
            .properties_panel()
            .unwrap()
            .swatches
            .iter()
            .position(|swatch| swatch.color == color)
            .expect("swatch")
    };
    let depth = undo_depth(&state);

    let red = swatch(&state, PALETTE_RED);
    click(
        &mut state,
        PropertiesPanelHit::Swatch {
            row: color_row,
            index: red,
        },
    );
    assert_eq!(rect_of(&state, id).0, faint_red, "same hue: nothing to do");
    assert_eq!(undo_depth(&state), depth);

    let green = swatch(&state, PALETTE_GREEN);
    click(
        &mut state,
        PropertiesPanelHit::Swatch {
            row: color_row,
            index: green,
        },
    );
    assert_eq!(
        rect_of(&state, id).0,
        Color {
            a: 0.25,
            ..PALETTE_GREEN
        },
        "the new hue at the shape's own opacity"
    );
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

fn opacity_variant_palette() -> crate::config::QuickColorPalette {
    let entry = |label: &str, color: Color| crate::config::QuickColorPaletteEntry {
        label: label.to_string(),
        color,
    };
    crate::config::QuickColorPalette::from_entries(vec![
        entry("Red", PALETTE_RED),
        entry(
            "Faint red",
            Color {
                a: 0.4,
                ..PALETTE_RED
            },
        ),
        entry("Green", PALETTE_GREEN),
    ])
}

#[test]
fn stepping_the_color_tells_opacity_variants_of_one_hue_apart() {
    let measurer = TextMeasurer::default();
    let mut state = create_test_input_state();
    state.set_quick_colors(opacity_variant_palette());
    let id = add_rect(&mut state, PALETTE_RED, false);
    open(&mut state, vec![id]);
    state.set_properties_panel_focus(Some(row(&state, "Color")));

    let mut seen = Vec::new();
    for _ in 0..3 {
        assert!(state.handle_properties_panel_key_with_measurer(&measurer, Key::Right));
        seen.push(rect_of(&state, id).0);
    }

    // The translucent swatch sets its opacity; the opaque ones after it
    // change the hue and keep that opacity, which the Opacity row owns.
    let faint = |color: Color| Color { a: 0.4, ..color };
    assert_eq!(
        seen,
        vec![faint(PALETTE_RED), faint(PALETTE_GREEN), faint(PALETTE_RED)]
    );
    let panel = state.properties_panel().unwrap();
    let color_row = row(&state, "Color");
    assert_eq!(panel.entries[color_row].value, "Faint red");
    assert_eq!(panel.current_swatch(&panel.entries[color_row]), Some(1));
}

#[test]
fn stepping_a_marker_skips_the_opacity_variant_it_cannot_take() {
    let measurer = TextMeasurer::default();
    let mut state = create_test_input_state();
    state.set_quick_colors(opacity_variant_palette());
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
    state.set_properties_panel_focus(Some(row(&state, "Color")));

    assert!(state.handle_properties_panel_key_with_measurer(&measurer, Key::Right));

    match &state.boards.active_frame().shape(marker).unwrap().shape {
        Shape::MarkerStroke { color, .. } => {
            assert_eq!(
                *color,
                Color {
                    a: 0.3,
                    ..PALETTE_GREEN
                }
            );
        }
        other => panic!("expected marker, got {other:?}"),
    }
}

fn stack_order(state: &InputState) -> Vec<ShapeId> {
    state
        .boards
        .active_frame()
        .shapes
        .iter()
        .map(|shape| shape.id)
        .collect()
}

#[test]
fn order_buttons_move_the_shape_and_dim_at_the_ends_of_the_stack() {
    use crate::input::state::PanelAction;

    let mut state = create_test_input_state();
    let bottom = add_rect(&mut state, PALETTE_RED, false);
    let middle = add_rect(&mut state, PALETTE_GREEN, false);
    let top = add_rect(&mut state, PALETTE_RED, false);
    open(&mut state, vec![bottom]);
    let to_back = point(&state, PropertiesPanelHit::Action(PanelAction::ToBack));
    assert_eq!(
        state.properties_panel_active_hit_at(to_back.0, to_back.1),
        None,
        "the bottom shape cannot go lower"
    );

    click(&mut state, PropertiesPanelHit::Action(PanelAction::Forward));
    assert_eq!(stack_order(&state), vec![middle, bottom, top]);
    click(&mut state, PropertiesPanelHit::Action(PanelAction::ToFront));
    assert_eq!(stack_order(&state), vec![middle, top, bottom]);

    let panel = state.properties_panel().expect("panel stays open");
    assert!(!panel.action_enabled(PanelAction::ToFront));
    assert!(panel.action_enabled(PanelAction::Backward));
    assert!(
        panel
            .subtitle
            .as_deref()
            .unwrap()
            .starts_with("Layer 3 of 3"),
        "the header follows the move"
    );
}

#[test]
fn duplicate_keeps_the_panel_on_the_copy_and_delete_closes_it() {
    use crate::input::state::PanelAction;

    let mut state = create_test_input_state();
    let original = add_rect(&mut state, PALETTE_RED, false);
    open(&mut state, vec![original]);

    click(
        &mut state,
        PropertiesPanelHit::Action(PanelAction::Duplicate),
    );
    assert_eq!(state.boards.active_frame().shapes.len(), 2);
    let copy = state.selected_shape_ids().to_vec();
    assert_eq!(copy.len(), 1);
    assert_ne!(copy[0], original);
    assert!(state.is_properties_panel_open());

    click(&mut state, PropertiesPanelHit::Action(PanelAction::Delete));
    assert_eq!(stack_order(&state), vec![original]);
    lay_out(&mut state);
    assert!(
        !state.is_properties_panel_open(),
        "nothing selected is left to show"
    );
}

#[test]
fn a_locked_selection_can_be_reordered_but_not_duplicated_or_deleted() {
    use crate::input::state::PanelAction;

    let mut state = create_test_input_state();
    let locked = add_rect(&mut state, PALETTE_RED, false);
    let _above = add_rect(&mut state, PALETTE_GREEN, false);
    let index = state.boards.active_frame().find_index(locked).unwrap();
    state.boards.active_frame_mut().shapes[index].locked = true;
    open(&mut state, vec![locked]);

    let panel = state.properties_panel().unwrap();
    assert!(!panel.action_enabled(PanelAction::Duplicate));
    assert!(!panel.action_enabled(PanelAction::Delete));
    assert!(panel.action_enabled(PanelAction::Forward));
    let delete = point(&state, PropertiesPanelHit::Action(PanelAction::Delete));
    click_at(&mut state, delete, delete);
    assert_eq!(state.boards.active_frame().shapes.len(), 2);
}

#[test]
fn the_opacity_row_steps_by_five_percent_and_a_marker_stays_translucent() {
    let measurer = TextMeasurer::default();
    let mut state = create_test_input_state();
    let rect = add_rect(&mut state, PALETTE_RED, false);
    let marker = state
        .boards
        .active_frame_mut()
        .add_shape(Shape::MarkerStroke {
            points: vec![(100, 100), (200, 120)],
            color: Color {
                a: 0.85,
                ..PALETTE_RED
            },
            thick: 12.0,
        });
    open(&mut state, vec![rect, marker]);
    let opacity = row(&state, "Opacity");
    assert_eq!(
        state.properties_panel().unwrap().entries[opacity].value,
        "Mixed"
    );
    state.set_properties_panel_focus(Some(opacity));

    assert!(state.handle_properties_panel_key_with_measurer(&measurer, Key::Right));
    let marker_alpha =
        |state: &InputState| match &state.boards.active_frame().shape(marker).unwrap().shape {
            Shape::MarkerStroke { color, .. } => color.a,
            other => panic!("expected marker, got {other:?}"),
        };
    assert_eq!(rect_of(&state, rect).0.a, 1.0, "already at the top");
    assert!(
        (marker_alpha(&state) - 0.9).abs() < 1e-9,
        "a marker tops out at 90%"
    );

    assert!(state.handle_properties_panel_key_with_measurer(&measurer, Key::Left));
    assert!((rect_of(&state, rect).0.a - 0.95).abs() < 1e-9);
    assert!((marker_alpha(&state) - 0.85).abs() < 1e-9);
}

#[test]
fn dragging_the_thickness_slider_follows_the_pointer_and_undoes_in_one_step() {
    let measurer = TextMeasurer::default();
    let engine = crate::ui_text::UiTextEngine::default();
    let resources = crate::input::state::InputTextResources {
        measurer: &measurer,
        ui_engine: &engine,
    };
    let mut state = create_test_input_state();
    let id = add_rect(&mut state, PALETTE_RED, false);
    open(&mut state, vec![id]);
    let thickness = row(&state, "Thickness");
    let track = state
        .properties_panel_layout()
        .unwrap()
        .hit_rect(
            state.properties_panel().unwrap(),
            PropertiesPanelHit::Slider(thickness),
        )
        .expect("track");
    let y = track.center().1 as i32;
    let depth = undo_depth(&state);

    assert!(state.handle_properties_panel_press_with_measurer(
        &measurer,
        MouseButton::Left,
        track.right() as i32 - 1,
        y
    ));
    assert_eq!(
        rect_of(&state, id).2,
        50.0,
        "the value jumps to the pointer"
    );
    lay_out(&mut state);
    state.move_properties_panel_pointer_with(&measurer, track.x as i32 - 40, y);
    assert_eq!(
        rect_of(&state, id).2,
        1.0,
        "a drag past the end stays at it"
    );
    assert_eq!(undo_depth(&state), depth, "nothing recorded mid-drag");
    lay_out(&mut state);
    state.move_properties_panel_pointer_with(&measurer, track.center().0 as i32, y);
    let middle = rect_of(&state, id).2;
    assert!(middle > 20.0 && middle < 30.0, "{middle}");
    assert_eq!(
        state.properties_panel().unwrap().entries[thickness].value,
        format!("{middle:.1}px"),
        "the readout follows the drag"
    );

    state.release_properties_panel_at_with(&measurer, track.center().0 as i32, y);
    assert_eq!(undo_depth(&state), depth + 1, "the whole drag is one entry");
    state.handle_action_with_resources(resources, Action::Undo);
    assert_eq!(rect_of(&state, id).2, 3.0);
}

#[test]
fn closing_the_panel_mid_drag_keeps_the_value_as_one_undo_step() {
    let measurer = TextMeasurer::default();
    let mut state = create_test_input_state();
    let id = add_rect(&mut state, PALETTE_RED, false);
    open(&mut state, vec![id]);
    let opacity = row(&state, "Opacity");
    let track = state
        .properties_panel_layout()
        .unwrap()
        .hit_rect(
            state.properties_panel().unwrap(),
            PropertiesPanelHit::Slider(opacity),
        )
        .expect("track");
    let depth = undo_depth(&state);

    assert!(state.handle_properties_panel_press_with_measurer(
        &measurer,
        MouseButton::Left,
        track.x as i32,
        track.center().1 as i32
    ));
    state.close_properties_panel();

    assert!((rect_of(&state, id).0.a - 0.05).abs() < 1e-9);
    assert_eq!(undo_depth(&state), depth + 1);
    assert!(!state.is_properties_slider_dragging());
}

#[test]
fn the_picker_sets_opacity_exactly() {
    let mut state = create_test_input_state();
    let id = add_rect(&mut state, PALETTE_RED, false);
    open(&mut state, vec![id]);
    let color_row = row(&state, "Color");
    click(&mut state, PropertiesPanelHit::MoreColors(color_row));
    let half_green = Color {
        a: 0.5,
        ..PALETTE_GREEN
    };

    state.color_picker_popup_set_color(half_green);
    state.apply_color_picker_popup();

    assert_eq!(rect_of(&state, id).0, half_green);
}

fn add_styled_rect(state: &mut InputState, color: Color, thick: f64, fill: bool) -> ShapeId {
    state.boards.active_frame_mut().add_shape(Shape::Rect {
        x: 100,
        y: 100,
        w: 60,
        h: 40,
        fill,
        fill_color: None,
        color,
        thick,
    })
}

#[test]
fn a_selection_saves_its_style_into_a_preset_slot_and_another_takes_it_on() {
    use crate::input::state::PanelAction;

    let mut state = create_test_input_state();
    let empty_slot = state
        .preset_slots
        .presets()
        .iter()
        .position(Option::is_none)
        .expect("an empty slot")
        + 1;
    let source = add_styled_rect(&mut state, PALETTE_GREEN, 8.0, true);
    open(&mut state, vec![source]);
    let slot = PropertiesPanelHit::Action(PanelAction::Preset(empty_slot));
    let (x, y) = point(&state, slot);
    assert_eq!(
        state.properties_panel_active_hit_at(x, y),
        None,
        "an empty slot has nothing to apply"
    );

    click(
        &mut state,
        PropertiesPanelHit::Action(PanelAction::SavePreset),
    );
    assert!(state.properties_panel().unwrap().preset_save_mode);
    assert_eq!(
        state.properties_panel().unwrap().tooltip(slot),
        Some(format!("Save to preset {empty_slot}"))
    );
    click(&mut state, slot);

    assert!(!state.properties_panel().unwrap().preset_save_mode);
    let saved = state.preset_slots.preset(empty_slot).expect("saved preset");
    assert_eq!(saved.tool, Tool::Rect);
    assert_eq!(saved.preview_color(), PALETTE_GREEN);
    assert_eq!(saved.size, 8.0);
    assert_eq!(saved.fill_enabled, Some(true));
    assert!(matches!(
        state.take_pending_preset_action(),
        Some(crate::input::state::PresetAction::Save { slot, .. }) if slot == empty_slot
    ));

    let tool_before = state.active_tool();
    let target = add_styled_rect(&mut state, PALETTE_RED, 3.0, false);
    open(&mut state, vec![target]);
    let depth = undo_depth(&state);
    click(&mut state, slot);

    assert_eq!(rect_of(&state, target), (PALETTE_GREEN, true, 8.0));
    assert_eq!(
        undo_depth(&state),
        depth + 1,
        "one undo entry for the whole style"
    );
    assert_eq!(state.active_tool(), tool_before, "the tool stays as it was");
}

#[test]
fn escape_disarms_saving_before_it_closes_the_panel() {
    use crate::input::state::PanelAction;

    let measurer = TextMeasurer::default();
    let mut state = create_test_input_state();
    let id = add_rect(&mut state, PALETTE_RED, false);
    open(&mut state, vec![id]);
    click(
        &mut state,
        PropertiesPanelHit::Action(PanelAction::SavePreset),
    );

    assert!(state.handle_properties_panel_key_with_measurer(&measurer, Key::Escape));
    assert!(state.is_properties_panel_open());
    assert!(!state.properties_panel().unwrap().preset_save_mode);

    assert!(state.handle_properties_panel_key_with_measurer(&measurer, Key::Escape));
    assert!(!state.is_properties_panel_open());
}

#[test]
fn a_shape_no_tool_draws_cannot_be_saved_as_a_preset() {
    use crate::input::state::PanelAction;

    let mut state = create_test_input_state();
    let note = state.boards.active_frame_mut().add_shape(Shape::Text {
        x: 100,
        y: 100,
        text: "Note".into(),
        color: PALETTE_RED,
        size: 18.0,
        font_descriptor: Default::default(),
        background_enabled: false,
        wrap_width: None,
    });
    open(&mut state, vec![note]);

    assert!(
        !state
            .properties_panel()
            .unwrap()
            .action_enabled(PanelAction::SavePreset)
    );
}

#[test]
fn a_key_pressed_mid_drag_lands_after_the_drag_in_history() {
    let measurer = TextMeasurer::default();
    let engine = crate::ui_text::UiTextEngine::default();
    let resources = crate::input::state::InputTextResources {
        measurer: &measurer,
        ui_engine: &engine,
    };
    let mut state = create_test_input_state();
    let id = add_rect(&mut state, PALETTE_RED, false);
    open(&mut state, vec![id]);
    let thickness = row(&state, "Thickness");
    let track = state
        .properties_panel_layout()
        .unwrap()
        .hit_rect(
            state.properties_panel().unwrap(),
            PropertiesPanelHit::Slider(thickness),
        )
        .expect("track");
    let (x, y) = (track.right() as i32 - 1, track.center().1 as i32);

    assert!(state.handle_properties_panel_press_with_measurer(&measurer, MouseButton::Left, x, y));
    assert_eq!(rect_of(&state, id).2, 50.0);
    assert!(state.handle_properties_panel_key_with_measurer(&measurer, Key::Left));
    assert!(
        !state.is_properties_slider_dragging(),
        "the key ended the drag"
    );
    assert_eq!(rect_of(&state, id).2, 49.0);
    state.release_properties_panel_at_with(&measurer, x, y);
    assert_eq!(
        rect_of(&state, id).2,
        49.0,
        "the release does not jump back"
    );

    state.handle_action_with_resources(resources, Action::Undo);
    assert_eq!(rect_of(&state, id).2, 50.0, "the key's step undoes first");
    state.handle_action_with_resources(resources, Action::Undo);
    assert_eq!(
        rect_of(&state, id).2,
        3.0,
        "then the drag, back to the start"
    );
}

#[test]
fn undo_mid_drag_first_lands_the_drag() {
    let measurer = TextMeasurer::default();
    let engine = crate::ui_text::UiTextEngine::default();
    let resources = crate::input::state::InputTextResources {
        measurer: &measurer,
        ui_engine: &engine,
    };
    let mut state = create_test_input_state();
    let id = add_rect(&mut state, PALETTE_RED, false);
    open(&mut state, vec![id]);
    let thickness = row(&state, "Thickness");
    let track = state
        .properties_panel_layout()
        .unwrap()
        .hit_rect(
            state.properties_panel().unwrap(),
            PropertiesPanelHit::Slider(thickness),
        )
        .expect("track");
    assert!(state.handle_properties_panel_press_with_measurer(
        &measurer,
        MouseButton::Left,
        track.right() as i32 - 1,
        track.center().1 as i32
    ));

    state.handle_action_with_resources(resources, Action::Undo);

    assert!(!state.is_properties_slider_dragging());
    assert_eq!(rect_of(&state, id).2, 3.0, "the drag landed, then undid");
    state.handle_action_with_resources(resources, Action::Redo);
    assert_eq!(rect_of(&state, id).2, 50.0);
}

#[test]
fn a_preset_saved_from_a_separately_filled_shape_brings_its_fill_along() {
    use crate::input::state::PanelAction;

    let mut state = create_test_input_state();
    let empty_slot = state
        .preset_slots
        .presets()
        .iter()
        .position(Option::is_none)
        .expect("an empty slot")
        + 1;
    let source = state.boards.active_frame_mut().add_shape(Shape::Rect {
        x: 100,
        y: 100,
        w: 60,
        h: 40,
        fill: true,
        fill_color: Some(PALETTE_GREEN),
        color: PALETTE_RED,
        thick: 3.0,
    });
    open(&mut state, vec![source]);
    let slot = PropertiesPanelHit::Action(PanelAction::Preset(empty_slot));
    click(
        &mut state,
        PropertiesPanelHit::Action(PanelAction::SavePreset),
    );
    click(&mut state, slot);

    let target = add_styled_rect(&mut state, crate::domain::color::PALETTE_BLUE, 5.0, true);
    open(&mut state, vec![target]);
    click(&mut state, slot);

    assert_eq!(rect_of(&state, target).0, PALETTE_RED, "the border");
    assert_eq!(
        fill_of(&state, target),
        (true, Some(PALETTE_GREEN)),
        "the fill"
    );

    // A preset whose fill follows its border resets a separate fill.
    let plain_slot = state
        .preset_slots
        .presets()
        .iter()
        .position(Option::is_none)
        .expect("another empty slot")
        + 1;
    let plain = add_styled_rect(&mut state, PALETTE_RED, 3.0, true);
    open(&mut state, vec![plain]);
    click(
        &mut state,
        PropertiesPanelHit::Action(PanelAction::SavePreset),
    );
    click(
        &mut state,
        PropertiesPanelHit::Action(PanelAction::Preset(plain_slot)),
    );
    open(&mut state, vec![target]);
    click(
        &mut state,
        PropertiesPanelHit::Action(PanelAction::Preset(plain_slot)),
    );
    assert_eq!(fill_of(&state, target), (true, None));
}
