use super::*;

// ============================================================================
// Helpers
// ============================================================================

/// Top-left corner of the pill for `position`, clamped so the pill never
/// leaves the screen even when it is as wide as the budget allows.
pub(super) fn pill_origin(
    position: StatusPosition,
    screen_width: f64,
    screen_height: f64,
    pill_width: f64,
    pill_height: f64,
) -> (f64, f64) {
    let inset = STATUS_BAR_EDGE_INSET;
    let (bx, by) = match position {
        StatusPosition::TopLeft => (inset, inset),
        StatusPosition::TopRight => (screen_width - inset - pill_width, inset),
        StatusPosition::BottomLeft => (inset, screen_height - inset - pill_height),
        StatusPosition::BottomRight => (
            screen_width - inset - pill_width,
            screen_height - inset - pill_height,
        ),
    };
    (
        bx.clamp(inset, (screen_width - inset - pill_width).max(inset)),
        by.clamp(inset, (screen_height - inset - pill_height).max(inset)),
    )
}

pub(super) fn tool_display_name(input_state: &InputState, tool: Tool) -> &'static str {
    match &input_state.state {
        DrawingState::TextInput { .. } => match input_state.text_input_mode {
            TextInputMode::Plain => action_display_label(Action::EnterTextMode),
            TextInputMode::StickyNote => action_display_label(Action::EnterStickyNoteMode),
        },
        DrawingState::Drawing { tool, .. } => tool_action_label(*tool),
        DrawingState::BuildingPolygon { .. } => "Freeform Polygon",
        DrawingState::MovingSelection { .. } => "Move",
        DrawingState::Selecting { .. } => "Select",
        DrawingState::ResizingText { .. } | DrawingState::ResizingSelection { .. } => "Resize",
        DrawingState::BendingArrow { .. } => "Bend",
        DrawingState::AdjustingSpotlightMagnification { .. } => "Magnify",
        DrawingState::PendingTextClick { .. } | DrawingState::Idle => tool_action_label(tool),
    }
}

pub(super) fn help_binding_label(input_state: &InputState) -> String {
    let mut labels = input_state.action_binding_labels(Action::ToggleHelp);
    if labels.iter().any(|label| label == "F1") {
        // Prefer showing F1 in the status bar when both defaults are bound.
        labels.retain(|label| label != "F10");
    }
    format_binding_labels(&labels)
}

/// Hidden-toolbar hint chip text, styled like the help chip: "{binding}
/// Toolbar" (e.g. "F9 Toolbar"), or bare "Toolbar" when the toggle is
/// unbound (the chip stays clickable either way).
pub(super) fn toolbar_hint_label(input_state: &InputState) -> String {
    let labels = input_state.action_binding_labels(Action::ToggleToolbar);
    match join_binding_labels(&labels) {
        Some(binding) => format!("{binding} Toolbar"),
        None => "Toolbar".to_string(),
    }
}

fn tool_action_label(tool: Tool) -> &'static str {
    action_for_tool(tool)
        .map(action_display_label)
        .unwrap_or("Select")
}
