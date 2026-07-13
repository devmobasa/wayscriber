//! GTK side-palette unit tests.

use std::collections::HashMap;

use super::*;
use crate::config::{Action, KeyBinding};
use crate::input::state::test_support::make_test_input_state;
use crate::ui::toolbar::ToolbarBindingHints;

/// Rebinding an action shown in a side-pane button tooltip (here the
/// command-palette button) must change the structure key so the pane
/// rebuilds and the tooltip stops showing the old shortcut.
#[test]
fn side_structure_rebuilds_when_a_button_shortcut_changes() {
    let mut state = make_test_input_state();
    let initial = ToolbarSnapshot::from_input_with_bindings(
        &state,
        ToolbarBindingHints::from_input_state(&state),
    );
    let initial_key = StructureKey::of(&initial);

    state.set_action_bindings(HashMap::from([(
        Action::ToggleCommandPalette,
        vec![KeyBinding::parse("Ctrl+Alt+K").expect("binding")],
    )]));

    let changed = ToolbarSnapshot::from_input_with_bindings(
        &state,
        ToolbarBindingHints::from_input_state(&state),
    );
    let changed_key = StructureKey::of(&changed);

    assert!(
        initial_key != changed_key,
        "a shortcut change must rebuild the side pane so tooltips refresh"
    );
}
