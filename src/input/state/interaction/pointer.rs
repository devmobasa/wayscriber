use super::adapters;
use super::event::{PointerMotion, PointerPress, PointerRelease};
use super::outcome::{ConsumedBy, NoRouteReason, RoutingOutcome};
use crate::input::MouseButton;
use crate::input::state::InputState;

pub(crate) fn route_pointer_press(state: &mut InputState, event: PointerPress) -> RoutingOutcome {
    let points = event.points();
    // A new press invalidates any HUD press still awaiting its release, so a
    // stale flag can never swallow the release of an unrelated interaction.
    state.clear_status_hud_press_pending();
    if let Some(outcome) =
        adapters::handle_building_polygon_non_left_press(state, event.button(), points)
    {
        return outcome;
    }
    if let Some(outcome) = adapters::handle_radial_menu_press(state, event.button(), points) {
        return outcome;
    }
    // The precise-entry popup is keyboard-only: any overlay press cancels
    // it and the press then routes normally.
    let _ = state.cancel_precision_entry();
    if let Some(outcome) = adapters::handle_color_picker_press(state, event.button(), points) {
        return outcome;
    }
    if let Some(outcome) = adapters::handle_board_picker_press(state, event.button(), points) {
        return outcome;
    }
    if let Some(outcome) = adapters::handle_properties_panel_press(state, event.button(), points) {
        return outcome;
    }
    if event.button() == MouseButton::Left
        && let Some(outcome) = adapters::handle_left_context_menu_press(state, points)
    {
        return outcome;
    }

    if let Some(outcome) = adapters::handle_status_hud_press(state, event.button(), points) {
        return outcome;
    }

    adapters::close_properties_panel_before_tool_routing(state);

    if let Some(outcome) = adapters::handle_tool_button_press(state, event.button(), points) {
        return outcome;
    }

    match event.button() {
        MouseButton::Right => adapters::handle_right_press(state, points),
        MouseButton::Left => adapters::handle_unbound_left_press(state, points),
        MouseButton::Middle => adapters::handle_middle_press(state, points),
    }
}

pub(crate) fn route_pointer_motion(state: &mut InputState, event: PointerMotion) -> RoutingOutcome {
    let points = event.points();
    adapters::update_pointer_positions(state, points);
    if let Some(outcome) = adapters::handle_radial_menu_motion(state, points) {
        return outcome;
    }
    if let Some(outcome) = adapters::handle_color_picker_motion(state, points) {
        return outcome;
    }
    if let Some(outcome) = adapters::handle_board_picker_motion(state, points) {
        return outcome;
    }
    if let Some(outcome) = adapters::handle_properties_panel_motion(state, points) {
        return outcome;
    }
    if let Some(outcome) = adapters::handle_active_motion(state, points) {
        return outcome;
    }
    if let Some(outcome) = adapters::handle_context_menu_motion(state, points) {
        return outcome;
    }
    adapters::handle_drawing_or_idle_motion(state, points)
}

pub(crate) fn route_pointer_release(
    state: &mut InputState,
    event: PointerRelease,
) -> RoutingOutcome {
    let points = event.points();
    adapters::update_pointer_positions(state, points);

    // Status HUD press→release contract for paths that route presses through
    // this chain (tablet, touch fallbacks): a HUD press consumed by
    // `handle_status_hud_press` activates its chip on release-inside. The
    // pointer/touch backends consume HUD presses before routing and track
    // their own pending flag, so the two flags are never both set for one
    // press. Actions (help) dispatch through the same action routing the
    // backend uses via `dispatch_input_action`.
    if event.button() == MouseButton::Left && state.take_status_hud_press_pending() {
        let screen = points.screen();
        let (_, action) = state.check_status_hud_click(screen.x(), screen.y());
        if let Some(action) = action {
            state.handle_action(action);
        }
        state.needs_redraw = true;
        return RoutingOutcome::Consumed(ConsumedBy::StatusHud);
    }

    if let Some(outcome) = adapters::handle_radial_menu_release(state) {
        return outcome;
    }

    if let Some(outcome) = adapters::handle_release_overlays(state, event.button(), points) {
        return outcome;
    }

    let Some(kind) = adapters::releasable_active_kind(state) else {
        return if event.button() != MouseButton::Left && !adapters::has_active_drag(state) {
            RoutingOutcome::NoRoute(NoRouteReason::NonLeftReleaseWithoutActiveDrag)
        } else {
            RoutingOutcome::NoRoute(NoRouteReason::NoActiveInteraction)
        };
    };

    if !adapters::release_button_matches_active_drag(state, event.button()) {
        return RoutingOutcome::NoRoute(NoRouteReason::ReleaseButtonMismatch);
    }

    adapters::finish_pointer_interaction(state, points);
    RoutingOutcome::Finished(kind)
}
