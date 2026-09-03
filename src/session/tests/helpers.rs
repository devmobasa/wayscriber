use crate::config::{Action, Shortcut};
use crate::input::InputState;
use std::collections::HashMap;

pub(super) fn dummy_input_state() -> InputState {
    let mut action_map = HashMap::new();
    action_map.insert(Shortcut::parse("Escape").unwrap(), Action::Exit);
    crate::input::state::test_support::TestInputStateBuilder::default()
        .action_map(action_map)
        .action_bindings(HashMap::new())
        .thickness(3.0)
        .eraser_size(12.0)
        .build()
}
