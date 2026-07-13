//! GTK top-strip unit tests.

use std::collections::HashMap;

use super::*;
use crate::config::KeyBinding;
use crate::input::state::test_support::make_test_input_state;
use crate::ui::toolbar::ToolbarBindingHints;

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
