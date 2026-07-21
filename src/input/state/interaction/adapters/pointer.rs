use super::super::active::active_interaction_kind;
use super::super::event::PointerPoints;
use super::super::outcome::{
    ActiveInteractionKind, CancelTarget, ConsumedBy, InteractionSideEffect, NoRouteReason,
    PointerSideEffect, RoutingOutcome,
};
use crate::draw::Shape;
use crate::input::MouseButton;
use crate::input::state::core::MenuCommand;
use crate::input::state::{ContextMenuKind, DrawingState, InputState};

pub(crate) fn update_pointer_positions(state: &mut InputState, points: PointerPoints) {
    let screen = points.screen();
    let canvas = points.canvas();
    state.update_pointer_positions(screen.x(), screen.y(), canvas.x(), canvas.y());
}

pub(crate) fn handle_radial_menu_press(
    state: &mut InputState,
    button: MouseButton,
    points: PointerPoints,
) -> Option<RoutingOutcome> {
    let screen = points.screen();
    let canvas = points.canvas();
    state
        .handle_radial_menu_press(button, screen.x(), screen.y(), canvas.x(), canvas.y())
        .then_some(RoutingOutcome::Consumed(ConsumedBy::RadialMenu))
}

pub(crate) fn handle_building_polygon_non_left_press(
    state: &mut InputState,
    button: MouseButton,
    points: PointerPoints,
) -> Option<RoutingOutcome> {
    if !matches!(state.state, DrawingState::BuildingPolygon { .. }) {
        return None;
    }

    let screen = points.screen();
    let canvas = points.canvas();
    state.update_pointer_positions(screen.x(), screen.y(), canvas.x(), canvas.y());
    match button {
        MouseButton::Right => {
            state.cancel_active_interaction();
            Some(RoutingOutcome::Canceled(CancelTarget::ActiveInteraction(
                ActiveInteractionKind::BuildingPolygon,
            )))
        }
        MouseButton::Middle => Some(RoutingOutcome::Consumed(ConsumedBy::ToolButton)),
        MouseButton::Left => None,
    }
}

pub(crate) fn handle_color_picker_press(
    state: &mut InputState,
    button: MouseButton,
    points: PointerPoints,
) -> Option<RoutingOutcome> {
    let screen = points.screen();
    state
        .handle_color_picker_press(button, screen.x(), screen.y())
        .then_some(RoutingOutcome::Consumed(ConsumedBy::ColorPickerPopup))
}

pub(crate) fn handle_board_picker_press(
    state: &mut InputState,
    button: MouseButton,
    points: PointerPoints,
) -> Option<RoutingOutcome> {
    let screen = points.screen();
    state
        .handle_board_picker_press(button, screen.x(), screen.y())
        .then_some(RoutingOutcome::Consumed(ConsumedBy::BoardPicker))
}

pub(crate) fn handle_properties_panel_press(
    state: &mut InputState,
    button: MouseButton,
    points: PointerPoints,
) -> Option<RoutingOutcome> {
    let screen = points.screen();
    state
        .handle_properties_panel_press(button, screen.x(), screen.y())
        .then_some(RoutingOutcome::Consumed(ConsumedBy::PropertiesPanel))
}

pub(crate) fn handle_left_context_menu_press(
    state: &mut InputState,
    points: PointerPoints,
) -> Option<RoutingOutcome> {
    if !state.is_context_menu_open() {
        return None;
    }
    let screen = points.screen();
    let canvas = points.canvas();
    state.update_pointer_positions(screen.x(), screen.y(), canvas.x(), canvas.y());
    state.trigger_click_highlight(canvas.x(), canvas.y());
    state.handle_context_menu_press(screen.x(), screen.y());
    Some(RoutingOutcome::Consumed(ConsumedBy::ContextMenu))
}

/// Swallow left presses on the interactive status HUD so chip clicks never
/// start a stroke. The pointer backend consumes HUD presses earlier via its
/// press→release contract; this guard covers input paths that route presses
/// directly (tablet, touch fallbacks). No activation happens on press: the
/// pending flag set here makes the matching release activate the chip in
/// `route_pointer_release`.
pub(crate) fn handle_status_hud_press(
    state: &mut InputState,
    button: MouseButton,
    points: PointerPoints,
) -> Option<RoutingOutcome> {
    if button != MouseButton::Left {
        return None;
    }
    let screen = points.screen();
    if !state.status_hud_contains(screen.x(), screen.y()) {
        return None;
    }
    state.set_status_hud_press_pending();
    Some(RoutingOutcome::Consumed(ConsumedBy::StatusHud))
}

/// Swallow left presses on the interactive bottom-right zoom chip so button
/// clicks never start a stroke. Mirrors [`handle_status_hud_press`]: no
/// activation on press — the pending flag set here makes the matching release
/// activate the button in `route_pointer_release`. Covers input paths that
/// route presses directly (tablet, touch fallbacks); the pointer/touch
/// backends consume chip presses earlier via their own press→release contract.
pub(crate) fn handle_zoom_chip_press(
    state: &mut InputState,
    button: MouseButton,
    points: PointerPoints,
) -> Option<RoutingOutcome> {
    if button != MouseButton::Left {
        return None;
    }
    let screen = points.screen();
    if !state.zoom_chip_contains(screen.x(), screen.y()) {
        return None;
    }
    // Record the press as `Button(kind)` or `Passive` (the `NN%` readout /
    // inter-piece gap) so the matching release stays consumed either way, firing
    // an action only on the same button. The press is consumed regardless, so no
    // stroke starts under the chip.
    let pressed = state.zoom_chip_press_at(screen.x(), screen.y());
    state.set_zoom_chip_press_pending(pressed);
    Some(RoutingOutcome::Consumed(ConsumedBy::ZoomChip))
}

pub(crate) fn close_properties_panel_before_tool_routing(state: &mut InputState) {
    state.close_properties_panel();
}

pub(crate) fn handle_tool_button_press(
    state: &mut InputState,
    button: MouseButton,
    points: PointerPoints,
) -> Option<RoutingOutcome> {
    let binding = state.drag_binding_for_button(button);
    let tool = state.tool_for_button_press(button, binding.tool)?;
    let before = active_interaction_kind(state);
    let screen = points.screen();
    let canvas = points.canvas();
    state.handle_tool_button_press_at(
        button,
        tool,
        binding.color,
        (screen.x(), screen.y()),
        (canvas.x(), canvas.y()),
    );
    let after = active_interaction_kind(state);
    match (before, after) {
        (None, Some(kind)) => Some(RoutingOutcome::Started(kind)),
        (Some(ActiveInteractionKind::TextInput), Some(ActiveInteractionKind::TextInput)) => {
            Some(RoutingOutcome::Consumed(ConsumedBy::TextInput))
        }
        _ => Some(RoutingOutcome::Consumed(ConsumedBy::ToolButton)),
    }
}

pub(crate) fn handle_unbound_left_press(
    state: &mut InputState,
    points: PointerPoints,
) -> RoutingOutcome {
    let screen = points.screen();
    let canvas = points.canvas();
    state.update_pointer_positions(screen.x(), screen.y(), canvas.x(), canvas.y());
    state.trigger_click_highlight(canvas.x(), canvas.y());

    if state.handle_context_menu_press(screen.x(), screen.y()) {
        return RoutingOutcome::Consumed(ConsumedBy::ContextMenu);
    }

    match &mut state.state {
        DrawingState::Idle => RoutingOutcome::NoRoute(NoRouteReason::NoPointerBinding),
        DrawingState::TextInput { x, y, .. } => {
            *x = canvas.x();
            *y = canvas.y();
            state.update_text_preview_dirty();
            state.needs_redraw = true;
            RoutingOutcome::Consumed(ConsumedBy::TextInput)
        }
        DrawingState::BuildingPolygon { .. } => {
            state.handle_building_polygon_left_click(canvas.x(), canvas.y());
            RoutingOutcome::Continued(ActiveInteractionKind::BuildingPolygon)
        }
        DrawingState::Drawing { .. }
        | DrawingState::MovingSelection { .. }
        | DrawingState::Selecting { .. }
        | DrawingState::PendingTextClick { .. }
        | DrawingState::ResizingText { .. }
        | DrawingState::ResizingSelection { .. } => {
            RoutingOutcome::NoRoute(NoRouteReason::NoPointerBinding)
        }
    }
}

pub(crate) fn handle_right_press(state: &mut InputState, points: PointerPoints) -> RoutingOutcome {
    let screen = points.screen();
    let canvas = points.canvas();
    if state.should_toggle_radial_menu_from_mouse(MouseButton::Right) {
        state.toggle_radial_menu(screen.x() as f64, screen.y() as f64);
        return RoutingOutcome::Consumed(ConsumedBy::RadialMenuToggle);
    }

    state.update_pointer_positions(screen.x(), screen.y(), canvas.x(), canvas.y());
    state.last_text_click = None;
    if let Some(kind) = active_interaction_kind(state)
        && state.try_cancel_active_interaction()
    {
        return RoutingOutcome::Canceled(CancelTarget::ActiveInteraction(kind));
    }
    if state.zoom_active() {
        return RoutingOutcome::SideEffect(InteractionSideEffect::Pointer(
            PointerSideEffect::RightClickSuppressedByZoom,
        ));
    }
    if !state.context_menu_enabled() {
        return RoutingOutcome::SideEffect(InteractionSideEffect::Pointer(
            PointerSideEffect::RightClickContextMenuDisabled,
        ));
    }

    open_context_menu_from_right_click(state, screen.x(), screen.y(), canvas.x(), canvas.y());
    RoutingOutcome::Consumed(ConsumedBy::RightClickContextMenu)
}

pub(crate) fn handle_middle_press(state: &mut InputState, points: PointerPoints) -> RoutingOutcome {
    let screen = points.screen();
    if state.should_toggle_radial_menu_from_mouse(MouseButton::Middle) {
        state.toggle_radial_menu(screen.x() as f64, screen.y() as f64);
        RoutingOutcome::Consumed(ConsumedBy::RadialMenuToggle)
    } else {
        RoutingOutcome::NoRoute(NoRouteReason::NoPointerBinding)
    }
}

fn open_context_menu_from_right_click(
    state: &mut InputState,
    screen_x: i32,
    screen_y: i32,
    canvas_x: i32,
    canvas_y: i32,
) {
    let hit_shape = state.hit_test_at(canvas_x, canvas_y);
    let mut focus_edit = false;
    if let Some(id) = hit_shape {
        if state.modifiers.shift {
            state.extend_selection([id]);
        } else if !state.selected_shape_ids().contains(&id) {
            state.set_selection(vec![id]);
        }
        let selection = state.selected_shape_ids().to_vec();
        focus_edit = selection.len() == 1
            && state
                .boards
                .active_frame()
                .shape(selection[0])
                .map(|shape| matches!(shape.shape, Shape::Text { .. } | Shape::StickyNote { .. }))
                .unwrap_or(false);
        state.open_context_menu(
            (screen_x, screen_y),
            selection,
            ContextMenuKind::Shape,
            hit_shape,
        );
    } else {
        state.clear_selection();
        state.open_context_menu(
            (screen_x, screen_y),
            Vec::new(),
            ContextMenuKind::Canvas,
            None,
        );
    }

    state.update_context_menu_hover_from_pointer(screen_x, screen_y);
    if focus_edit {
        state.focus_context_menu_command(MenuCommand::EditText);
    }
    if state.is_context_menu_open() {
        state.pending_onboarding_usage.used_context_menu_right_click = true;
    }
    state.needs_redraw = true;
}

pub(crate) fn handle_radial_menu_motion(
    state: &mut InputState,
    points: PointerPoints,
) -> Option<RoutingOutcome> {
    if !state.is_radial_menu_open() {
        return None;
    }
    let screen = points.screen();
    let x = screen.x() as f64;
    let y = screen.y() as f64;
    if state.radial_menu_is_size_dragging() {
        // Drag capture: while the size gauge is held, every motion adjusts
        // thickness, even outside the band.
        state.radial_menu_drag_size_to(x, y);
    } else {
        state.update_radial_menu_hover(x, y);
        state.radial_menu_sample_flick(x, y);
    }
    Some(RoutingOutcome::Consumed(ConsumedBy::RadialMenu))
}

pub(crate) fn handle_color_picker_motion(
    state: &mut InputState,
    points: PointerPoints,
) -> Option<RoutingOutcome> {
    if !state.is_color_picker_popup_open() {
        return None;
    }
    let screen = points.screen();
    if state.color_picker_popup_is_dragging()
        && let Some(layout) = state.color_picker_popup_layout()
    {
        let fx = screen.x() as f64;
        let fy = screen.y() as f64;
        let norm_x = ((fx - layout.gradient_x) / layout.gradient_w).clamp(0.0, 1.0);
        let norm_y = ((fy - layout.gradient_y) / layout.gradient_h).clamp(0.0, 1.0);
        state.color_picker_popup_set_from_gradient(norm_x, norm_y);
        state.color_picker_popup_set_hover(None);
    } else {
        state.color_picker_popup_set_hover(Some((screen.x() as f64, screen.y() as f64)));
    }
    Some(RoutingOutcome::Consumed(ConsumedBy::ColorPickerPopup))
}

pub(crate) fn handle_board_picker_motion(
    state: &mut InputState,
    points: PointerPoints,
) -> Option<RoutingOutcome> {
    if !state.is_board_picker_open() {
        return None;
    }
    let screen = points.screen();
    if state.board_picker_is_page_dragging() {
        state.board_picker_update_page_drag_from_pointer(screen.x(), screen.y());
    } else if state.board_picker_is_dragging() {
        state.board_picker_update_drag_from_pointer(screen.x(), screen.y());
    } else {
        state.update_board_picker_hover_from_pointer(screen.x(), screen.y());
    }
    Some(RoutingOutcome::Consumed(ConsumedBy::BoardPicker))
}

pub(crate) fn handle_properties_panel_motion(
    state: &mut InputState,
    points: PointerPoints,
) -> Option<RoutingOutcome> {
    if !state.is_properties_panel_open() {
        return None;
    }
    if state.properties_panel_layout().is_some() {
        let screen = points.screen();
        state.update_properties_panel_hover_from_pointer(screen.x(), screen.y());
    }
    Some(RoutingOutcome::Consumed(ConsumedBy::PropertiesPanel))
}

pub(crate) fn handle_context_menu_motion(
    state: &mut InputState,
    points: PointerPoints,
) -> Option<RoutingOutcome> {
    if !state.is_context_menu_open() {
        return None;
    }
    let screen = points.screen();
    state.update_context_menu_hover_from_pointer(screen.x(), screen.y());
    Some(RoutingOutcome::Consumed(ConsumedBy::ContextMenu))
}

pub(crate) fn handle_release_overlays(
    state: &mut InputState,
    button: MouseButton,
    points: PointerPoints,
) -> Option<RoutingOutcome> {
    if button != MouseButton::Left {
        return None;
    }
    let screen = points.screen();
    if state.handle_color_picker_popup_release_at(screen.x(), screen.y()) {
        return Some(RoutingOutcome::Consumed(ConsumedBy::ColorPickerPopup));
    }
    if state.handle_context_menu_release_at(screen.x(), screen.y()) {
        return Some(RoutingOutcome::Consumed(ConsumedBy::ContextMenu));
    }
    if state.handle_board_picker_release_at(screen.x(), screen.y()) {
        return Some(RoutingOutcome::Consumed(ConsumedBy::BoardPicker));
    }
    if state.handle_properties_panel_release_at(screen.x(), screen.y()) {
        return Some(RoutingOutcome::Consumed(ConsumedBy::PropertiesPanel));
    }
    None
}

pub(crate) fn handle_radial_menu_release(
    state: &mut InputState,
    button: MouseButton,
    points: PointerPoints,
) -> Option<RoutingOutcome> {
    let screen = points.screen();
    state
        .radial_menu_handle_release(button, screen.x() as f64, screen.y() as f64)
        .then_some(RoutingOutcome::Consumed(ConsumedBy::RadialMenu))
}

pub(crate) fn finish_pointer_interaction(state: &mut InputState, points: PointerPoints) {
    let canvas = points.canvas();
    state.finish_pointer_interaction_at(canvas.x(), canvas.y());
}
