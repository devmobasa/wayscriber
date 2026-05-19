use super::super::event::PointerPoints;
use super::super::outcome::{
    ActiveInteractionKind, InteractionSideEffect, NoRouteReason, PointerSideEffect, RoutingOutcome,
};
use crate::input::state::mouse::TEXT_CLICK_DRAG_THRESHOLD;
use crate::input::state::{DrawingState, InputState};
use crate::input::tool::{ToolMotionBehavior, ToolMotionSizeSource, ToolPressBehavior};
use crate::input::{EraserMode, MouseButton, Tool};
use std::sync::Arc;

pub(crate) fn handle_active_motion(
    state: &mut InputState,
    points: PointerPoints,
) -> Option<RoutingOutcome> {
    let canvas = points.canvas();
    if let DrawingState::ResizingText {
        shape_id,
        base_x,
        size,
        ..
    } = &state.state
    {
        let new_width = state.clamp_text_wrap_width(*base_x, canvas.x(), *size);
        let _ = state.update_text_wrap_width(*shape_id, new_width);
        return Some(RoutingOutcome::Continued(
            ActiveInteractionKind::ResizingText,
        ));
    }

    if let DrawingState::PendingTextClick {
        x: start_x,
        y: start_y,
        tool,
        ..
    } = &state.state
    {
        let dx = canvas.x() - *start_x;
        let dy = canvas.y() - *start_y;
        if dx.abs() >= TEXT_CLICK_DRAG_THRESHOLD || dy.abs() >= TEXT_CLICK_DRAG_THRESHOLD {
            let tool = *tool;
            if matches!(
                tool.press_behavior(),
                ToolPressBehavior::StartDrawing { .. }
            ) {
                let drawing_thickness = state.thickness_for_tool(tool);
                let mut points = vec![(*start_x, *start_y)];
                let mut point_thicknesses = vec![drawing_thickness as f32];
                if let Some(sample_size) = motion_sample_size(state, tool) {
                    points.push((canvas.x(), canvas.y()));
                    point_thicknesses.push(sample_size as f32);
                }
                state.state = DrawingState::Drawing {
                    tool,
                    start_x: *start_x,
                    start_y: *start_y,
                    points,
                    point_thicknesses,
                };
                state.last_text_click = None;
                state.last_provisional_bounds = None;
                state.update_provisional_dirty(canvas.x(), canvas.y());
                state.needs_redraw = true;
            }
        }
        return Some(RoutingOutcome::Continued(
            ActiveInteractionKind::PendingTextClick,
        ));
    }

    if let DrawingState::MovingSelection { last_x, last_y, .. } = &state.state {
        let dx = canvas.x() - *last_x;
        let dy = canvas.y() - *last_y;
        if (dx != 0 || dy != 0)
            && state.apply_translation_to_selection(dx, dy)
            && let DrawingState::MovingSelection {
                last_x,
                last_y,
                moved,
                ..
            } = &mut state.state
        {
            *last_x = canvas.x();
            *last_y = canvas.y();
            *moved = true;
        }
        return Some(RoutingOutcome::Continued(
            ActiveInteractionKind::MovingSelection,
        ));
    }

    if let DrawingState::ResizingSelection {
        handle,
        original_bounds,
        start_x,
        start_y,
        snapshots,
    } = &state.state
    {
        let dx = canvas.x() - *start_x;
        let dy = canvas.y() - *start_y;
        let handle = *handle;
        let original_bounds = *original_bounds;
        let snapshots = Arc::clone(snapshots);
        state.apply_selection_resize(handle, &original_bounds, dx, dy, snapshots.as_ref());
        state.needs_redraw = true;
        return Some(RoutingOutcome::Continued(
            ActiveInteractionKind::ResizingSelection,
        ));
    }

    if matches!(state.state, DrawingState::Selecting { .. }) {
        state.update_provisional_dirty(canvas.x(), canvas.y());
        state.needs_redraw = true;
        return Some(RoutingOutcome::Continued(
            ActiveInteractionKind::BoxSelecting,
        ));
    }

    None
}

pub(crate) fn handle_drawing_or_idle_motion(
    state: &mut InputState,
    points: PointerPoints,
) -> RoutingOutcome {
    let canvas = points.canvas();
    let mut drawing = false;
    let sample_size = if let DrawingState::Drawing { tool, .. } = &state.state {
        motion_sample_size(state, *tool)
    } else {
        None
    };
    if let DrawingState::Drawing {
        points,
        point_thicknesses,
        ..
    } = &mut state.state
    {
        if let Some(thickness) = sample_size {
            points.push((canvas.x(), canvas.y()));
            point_thicknesses.push(thickness as f32);
        }
        drawing = true;
    }

    if drawing {
        state.update_provisional_dirty(canvas.x(), canvas.y());
        state.needs_redraw = true;
        RoutingOutcome::Continued(ActiveInteractionKind::Drawing)
    } else if state.eraser_mode == EraserMode::Stroke
        && state.active_tool() == Tool::Eraser
        && matches!(state.state, DrawingState::Idle)
    {
        state.needs_redraw = true;
        RoutingOutcome::SideEffect(InteractionSideEffect::Pointer(
            PointerSideEffect::IdleEraserHover,
        ))
    } else {
        RoutingOutcome::NoRoute(NoRouteReason::NoActiveInteraction)
    }
}

pub(crate) fn releasable_active_kind(state: &InputState) -> Option<ActiveInteractionKind> {
    match state.state {
        DrawingState::Drawing { .. } => Some(ActiveInteractionKind::Drawing),
        DrawingState::MovingSelection { .. } => Some(ActiveInteractionKind::MovingSelection),
        DrawingState::Selecting { .. } => Some(ActiveInteractionKind::BoxSelecting),
        DrawingState::PendingTextClick { .. } => Some(ActiveInteractionKind::PendingTextClick),
        DrawingState::ResizingText { .. } => Some(ActiveInteractionKind::ResizingText),
        DrawingState::ResizingSelection { .. } => Some(ActiveInteractionKind::ResizingSelection),
        DrawingState::Idle | DrawingState::TextInput { .. } => None,
    }
}

pub(crate) fn has_active_drag(state: &InputState) -> bool {
    state.active_drag_button.is_some()
}

pub(crate) fn release_button_matches_active_drag(state: &InputState, button: MouseButton) -> bool {
    state.pointer_drag_button_matches(button)
}

fn motion_sample_size(state: &InputState, tool: Tool) -> Option<f64> {
    match tool.motion_behavior() {
        ToolMotionBehavior::NoPathAccumulation => None,
        ToolMotionBehavior::AccumulatePath {
            size_source: ToolMotionSizeSource::ToolSize,
        } => Some(state.tool_settings.get(tool).thickness),
        ToolMotionBehavior::AccumulatePath {
            size_source: ToolMotionSizeSource::EraserSize,
        } => Some(state.eraser_size),
    }
}
