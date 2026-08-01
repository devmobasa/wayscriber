use super::*;
use crate::input::Tool;
use crate::input::state::test_support::make_test_input_state;
use crate::ui::toolbar::ToolbarBindingHints;

fn snapshot() -> ToolbarSnapshot {
    let state = make_test_input_state();
    ToolbarSnapshot::from_input_with_bindings(&state, ToolbarBindingHints::default())
}

fn plan() -> TopStripPlan {
    TopStripPlan::unconstrained()
}

fn snapshot_for_tool(tool: Tool) -> ToolbarSnapshot {
    let mut snapshot = snapshot();
    snapshot.active_tool = tool;
    snapshot.tool_override = None;
    snapshot.thickness_targets_eraser = tool == Tool::Eraser;
    snapshot.thickness_targets_marker = tool == Tool::Marker;
    // Pin the pure per-tool morphs: these two settings are overrides
    // that extend any state (covered by a dedicated test below).
    snapshot.show_text_controls = false;
    snapshot.show_marker_opacity_section = false;
    snapshot
}

fn control_ids(spec: &StylePillSpec) -> Vec<String> {
    spec.controls()
        .iter()
        .map(|control| control.id().into_owned())
        .collect()
}

mod selection;
mod tool_states;
