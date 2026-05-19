use super::actions::route_action;
use super::adapters;
use super::outcome::{NoRouteReason, RoutingOutcome};
use crate::input::events::Key;
use crate::input::state::InputState;

pub(crate) fn route_key_press(state: &mut InputState, key: Key) -> RoutingOutcome {
    if let Some(outcome) = adapters::handle_tour_key(state, key) {
        return outcome;
    }
    if let Some(outcome) = adapters::handle_command_palette_key(state, key) {
        return outcome;
    }
    if let Some(outcome) = adapters::handle_help_overlay_key(state, key) {
        return outcome;
    }
    if let Some(outcome) = adapters::handle_radial_menu_key(state, key) {
        return outcome;
    }
    if let Some(outcome) = adapters::handle_color_picker_key(state, key) {
        return outcome;
    }
    if let Some(outcome) = adapters::handle_context_menu_key(state, key) {
        return outcome;
    }
    if let Some(outcome) = adapters::handle_board_picker_key(state, key) {
        return outcome;
    }
    if let Some(outcome) = adapters::handle_global_modifier_key(state, key) {
        return outcome;
    }
    if let Some(outcome) = adapters::handle_properties_panel_key(state, key) {
        return outcome;
    }
    if let Some(outcome) = adapters::handle_pending_delete_cancel_key(state, key) {
        return outcome;
    }
    if let Some(outcome) = adapters::handle_idle_selection_cancel_key(state, key) {
        return outcome;
    }
    if let Some(outcome) = adapters::handle_text_input_key(state, key) {
        return outcome;
    }
    if let Some(outcome) = adapters::handle_drawing_escape_cancel_key(state, key) {
        return outcome;
    }

    match adapters::action_for_key_binding(state, key) {
        Ok(Some(action)) => return route_action(state, action),
        Ok(None) => {}
        Err(NoRouteReason::UnsupportedKey) => {
            return RoutingOutcome::NoRoute(NoRouteReason::UnsupportedKey);
        }
        Err(reason) => return RoutingOutcome::NoRoute(reason),
    }

    if let Some(outcome) = adapters::handle_return_edit_selected_text_key(state, key) {
        return outcome;
    }

    RoutingOutcome::NoRoute(NoRouteReason::NoKeyBinding)
}
