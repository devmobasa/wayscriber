use super::*;
use crate::config::{BoardsConfig, KeybindingsConfig, PresenterModeConfig};
use crate::draw::{Color, FontDescriptor};
use crate::input::{ClickHighlightSettings, EraserMode};

fn make_state() -> InputState {
    let keybindings = KeybindingsConfig::default();
    let action_map = keybindings
        .build_action_map()
        .expect("default keybindings map");

    let mut state = InputState::with_defaults(
        Color {
            r: 1.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        },
        4.0,
        4.0,
        EraserMode::Brush,
        0.32,
        false,
        32.0,
        FontDescriptor::default(),
        false,
        20.0,
        30.0,
        false,
        true,
        BoardsConfig::default(),
        action_map,
        usize::MAX,
        ClickHighlightSettings::disabled(),
        0,
        0,
        true,
        0,
        0,
        5,
        5,
        PresenterModeConfig::default(),
    );
    state.update_screen_dimensions(200, 120);
    let _ = state.take_dirty_regions();
    state
}

fn add_rect(
    state: &mut InputState,
    color: Color,
    fill: bool,
    locked: bool,
) -> crate::draw::ShapeId {
    let id = state.boards.active_frame_mut().add_shape(Shape::Rect {
        x: 10,
        y: 20,
        w: 30,
        h: 40,
        fill,
        color,
        thick: 2.0,
    });
    if locked {
        let index = state
            .boards
            .active_frame()
            .find_index(id)
            .expect("shape index");
        state.boards.active_frame_mut().shapes[index].locked = true;
    }
    id
}

#[test]
fn selection_primary_color_skips_locked_shapes() {
    let mut state = make_state();
    let locked = add_rect(
        &mut state,
        Color {
            r: 0.0,
            g: 0.0,
            b: 1.0,
            a: 1.0,
        },
        false,
        true,
    );
    let unlocked = add_rect(
        &mut state,
        Color {
            r: 1.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        },
        false,
        false,
    );
    state.set_selection(vec![locked, unlocked]);

    assert_eq!(
        state.selection_primary_color(),
        Some(Color {
            r: 1.0,
            g: 0.0,
            b: 0.0,
            a: 1.0
        })
    );
}

#[test]
fn selection_bool_target_returns_true_for_mixed_or_locked_only_values() {
    let mut state = make_state();
    let first = add_rect(
        &mut state,
        Color {
            r: 1.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        },
        false,
        false,
    );
    let second = add_rect(
        &mut state,
        Color {
            r: 0.0,
            g: 1.0,
            b: 0.0,
            a: 1.0,
        },
        true,
        false,
    );
    state.set_selection(vec![first, second]);

    assert_eq!(
        state.selection_bool_target(|shape| match shape {
            Shape::Rect { fill, .. } => Some(*fill),
            _ => None,
        }),
        Some(true)
    );

    let mut locked_state = make_state();
    let locked = add_rect(
        &mut locked_state,
        Color {
            r: 1.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        },
        false,
        true,
    );
    locked_state.set_selection(vec![locked]);
    assert_eq!(
        locked_state.selection_bool_target(|shape| match shape {
            Shape::Rect { fill, .. } => Some(*fill),
            _ => None,
        }),
        Some(true)
    );
}

#[test]
fn selection_bool_target_flips_uniform_unlocked_value() {
    let mut state = make_state();
    let first = add_rect(
        &mut state,
        Color {
            r: 1.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        },
        false,
        false,
    );
    let second = add_rect(
        &mut state,
        Color {
            r: 0.0,
            g: 1.0,
            b: 0.0,
            a: 1.0,
        },
        false,
        false,
    );
    state.set_selection(vec![first, second]);

    assert_eq!(
        state.selection_bool_target(|shape| match shape {
            Shape::Rect { fill, .. } => Some(*fill),
            _ => None,
        }),
        Some(true)
    );

    let frame = state.boards.active_frame_mut();
    if let Shape::Rect { fill, .. } = &mut frame.shape_mut(first).expect("first shape").shape {
        *fill = true;
    }
    if let Shape::Rect { fill, .. } = &mut frame.shape_mut(second).expect("second shape").shape {
        *fill = true;
    }

    assert_eq!(
        state.selection_bool_target(|shape| match shape {
            Shape::Rect { fill, .. } => Some(*fill),
            _ => None,
        }),
        Some(false)
    );
}

#[test]
fn apply_selection_change_reports_applicable_locked_and_changed_counts() {
    let mut state = make_state();
    let unlocked = add_rect(
        &mut state,
        Color {
            r: 1.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        },
        false,
        false,
    );
    let locked = add_rect(
        &mut state,
        Color {
            r: 0.0,
            g: 0.0,
            b: 1.0,
            a: 1.0,
        },
        false,
        true,
    );
    state.set_selection(vec![unlocked, locked]);
    state.needs_redraw = false;
    state.session_dirty = false;

    let result = state.apply_selection_change(
        |shape| matches!(shape, Shape::Rect { .. }),
        |shape| match shape {
            Shape::Rect { fill, .. } => {
                *fill = true;
                true
            }
            _ => false,
        },
    );

    assert_eq!(result.applicable, 2);
    assert_eq!(result.locked, 1);
    assert_eq!(result.changed, 1);
    assert!(state.needs_redraw);
    assert!(state.session_dirty);
    assert_eq!(state.boards.active_frame().undo_stack_len(), 1);
    assert!(!state.take_dirty_regions().is_empty());
}

#[test]
fn report_selection_apply_result_emits_expected_toasts() {
    let mut state = make_state();

    assert!(!state.report_selection_apply_result(
        SelectionApplyResult {
            changed: 0,
            locked: 0,
            applicable: 0,
        },
        "fill",
    ));
    assert_eq!(
        state.ui_toast.as_ref().map(|toast| toast.message.as_str()),
        Some("No fill to edit in selection.")
    );

    assert!(!state.report_selection_apply_result(
        SelectionApplyResult {
            changed: 0,
            locked: 2,
            applicable: 2,
        },
        "color",
    ));
    assert_eq!(
        state.ui_toast.as_ref().map(|toast| toast.message.as_str()),
        Some("All color shapes are locked.")
    );

    assert!(!state.report_selection_apply_result(
        SelectionApplyResult {
            changed: 0,
            locked: 1,
            applicable: 2,
        },
        "fill",
    ));
    assert_eq!(
        state.ui_toast.as_ref().map(|toast| toast.message.as_str()),
        Some("No changes applied.")
    );

    assert!(state.report_selection_apply_result(
        SelectionApplyResult {
            changed: 1,
            locked: 2,
            applicable: 3,
        },
        "fill",
    ));
    assert_eq!(
        state.ui_toast.as_ref().map(|toast| toast.message.as_str()),
        Some("2 locked shape(s) unchanged.")
    );
}
