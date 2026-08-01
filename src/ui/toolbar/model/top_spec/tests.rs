use super::*;
use crate::config::{ToolbarItemOrderGroup, ToolbarItemsConfig};
use crate::input::state::test_support::make_test_input_state;
use crate::ui::toolbar::ToolbarBindingHints;

fn snapshot() -> ToolbarSnapshot {
    let state = make_test_input_state();
    ToolbarSnapshot::from_input_with_bindings(&state, ToolbarBindingHints::default())
}

fn strip_control_ids(spec: &TopToolbarSpec) -> Vec<String> {
    spec.strip()
        .iter()
        .filter_map(|node| match node {
            TopToolbarNode::Control(control) => Some(control.id().render_id().into_owned()),
            TopToolbarNode::Divider(_) => None,
        })
        .collect()
}

fn chrome_ids(spec: &TopToolbarSpec) -> Vec<String> {
    spec.chrome()
        .iter()
        .map(|control| control.id().render_id().into_owned())
        .collect()
}

mod behavior;
mod structure;
