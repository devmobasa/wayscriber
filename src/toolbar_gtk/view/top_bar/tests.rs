//! GTK top-strip unit tests.

use std::collections::HashMap;

use super::*;
use crate::config::KeyBinding;
use crate::input::state::test_support::make_test_input_state;
use crate::ui::toolbar::ToolbarBindingHints;

#[test]
fn drag_updates_are_coalesced_to_the_latest_start_relative_offset() {
    let drag = FrameCoalescedDrag::default();
    let first = drag.begin();
    drag.update(first, 2.0, 3.0);
    drag.update(first, 5.0, 7.0);

    let frame = drag.take_frame(first).expect("latest motion is pending");
    assert_eq!(frame.delta, (5.0, 7.0));
    assert_eq!(frame.phase, GtkToolbarDragPhase::Move);
    assert!(drag.take_frame(first).is_none());

    drag.end(first, 5.0, 7.0);
    let frame = drag.take_frame(first).expect("drag end is pending");
    assert_eq!(frame.delta, (5.0, 7.0));
    assert_eq!(frame.phase, GtkToolbarDragPhase::End);
}

#[test]
fn rounded_offset_matches_the_integer_layer_margin() {
    assert_eq!(rounded_margin_and_offset(12.0, 3.6), (16, 4.0));
    assert_eq!(rounded_margin_and_offset(24.0, -24.0), (0, -24.0));
    assert_eq!(rounded_margin_and_offset(100.25, 4.4), (105, 4.75));
}

#[test]
fn rapid_start_relative_updates_do_not_accumulate() {
    let origin = (100.0, 200.0);
    let first = drag_frame_position(origin, (25.0, 40.0));
    let second = drag_frame_position(origin, (80.0, 90.0));

    assert_eq!(first, (125.0, 240.0));
    assert_eq!(second, (180.0, 290.0));
}

#[test]
fn consecutive_drags_keep_separate_final_frames() {
    let drag = FrameCoalescedDrag::default();
    let first = drag.begin();
    drag.update(first, 4.0, 6.0);
    drag.end(first, 4.0, 6.0);
    let second = drag.begin();
    drag.update(second, 1.0, 2.0);

    let first_frame = drag.take_frame(first).expect("first drag end is retained");
    assert_eq!(first_frame.delta, (4.0, 6.0));
    assert_eq!(first_frame.phase, GtkToolbarDragPhase::End);

    let second_frame = drag
        .take_frame(second)
        .expect("second drag motion is retained");
    assert_eq!(second_frame.delta, (1.0, 2.0));
    assert_eq!(second_frame.phase, GtkToolbarDragPhase::Move);
}

#[test]
fn top_structure_rebuilds_when_current_shortcuts_change() {
    let mut state = make_test_input_state();
    let initial = ToolbarSnapshot::from_input_with_bindings(
        &state,
        ToolbarBindingHints::from_input_state(&state),
    );
    let initial_plan = plan_top_strip(&initial);
    let initial_key = StructureKey::of(&initial, &initial_plan);

    state.set_action_bindings(HashMap::from([(
        Action::SelectPenTool,
        vec![KeyBinding::parse("9").expect("binding")],
    )]));
    let changed = ToolbarSnapshot::from_input_with_bindings(
        &state,
        ToolbarBindingHints::from_input_state(&state),
    );
    let changed_plan = plan_top_strip(&changed);
    let changed_key = StructureKey::of(&changed, &changed_plan);

    assert!(initial_key != changed_key);
    assert_eq!(changed.binding_hints.badge_for_tool(Tool::Pen), Some("9"));
}

#[test]
fn simple_layout_requests_its_smaller_natural_width() {
    let mut state = make_test_input_state();
    state.toolbar_use_icons = true;
    let regular = ToolbarSnapshot::from_input_with_bindings(
        &state,
        ToolbarBindingHints::from_input_state(&state),
    );

    state.toolbar_layout_mode = ToolbarLayoutMode::Simple;
    let simple = ToolbarSnapshot::from_input_with_bindings(
        &state,
        ToolbarBindingHints::from_input_state(&state),
    );

    let regular_width = top_default_width(&regular);
    let simple_width = top_default_width(&simple);
    assert!(simple_width < regular_width);
    assert_eq!(simple_width, top_toolbar_size(&simple).0 as i32);
}

#[test]
fn degraded_layout_requests_the_selected_plan_width() {
    let state = make_test_input_state();
    let mut snapshot = ToolbarSnapshot::from_input_with_bindings(
        &state,
        ToolbarBindingHints::from_input_state(&state),
    );
    snapshot.top_viewport_max = Some(700.0);

    let plan = plan_top_strip(&snapshot);
    assert!(plan.compact || plan.show_overflow || plan.swatch_count < 8);
    assert!(top_default_width(&snapshot) <= 700);
}
