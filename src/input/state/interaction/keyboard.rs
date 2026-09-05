use super::actions::route_action_with_resources;
use super::adapters;
use super::outcome::{ConsumedBy, NoRouteReason, RoutingOutcome};
use crate::input::events::Key;
use crate::input::state::actions::key_press::bindings::{
    fallback_unshifted_label, key_to_action_label,
};
use crate::input::state::{DrawingState, InputState};
use std::time::Instant;

use super::super::core::SequenceMatch;

pub(crate) fn route_key_press_with_resources(
    state: &mut InputState,
    resources: crate::input::state::InputTextResources<'_>,
    key: Key,
) -> RoutingOutcome {
    route_key_event(state, resources, key, false)
}

pub(crate) fn route_key_repeat_with_resources(
    state: &mut InputState,
    resources: crate::input::state::InputTextResources<'_>,
    key: Key,
) -> RoutingOutcome {
    route_key_event(state, resources, key, true)
}

fn route_key_event(
    state: &mut InputState,
    resources: crate::input::state::InputTextResources<'_>,
    key: Key,
    is_repeat: bool,
) -> RoutingOutcome {
    if state.engaged_modal().is_some()
        || matches!(state.state, DrawingState::TextInput { .. })
        || state.screen_modal_is_engaged()
    {
        state.clear_pending_sequence();
    }

    if let Some(outcome) = adapters::handle_tour_key(state, key) {
        return outcome;
    }
    if let Some(outcome) = adapters::handle_command_palette_key(state, resources, key) {
        return outcome;
    }
    if let Some(outcome) = adapters::handle_help_overlay_key(state, key) {
        return outcome;
    }
    if let Some(outcome) = adapters::handle_radial_menu_key(state, key) {
        return outcome;
    }
    if let Some(outcome) = adapters::handle_precision_entry_key(state, resources, key) {
        return outcome;
    }
    if let Some(outcome) = adapters::handle_color_picker_key(state, key) {
        return outcome;
    }
    if let Some(outcome) = adapters::handle_font_picker_key(
        state,
        resources.measurer,
        key,
        font_picker_text(key).as_deref(),
    ) {
        return outcome;
    }
    if let Some(outcome) = adapters::handle_context_menu_key(state, resources, key) {
        return outcome;
    }
    if let Some(outcome) = adapters::handle_board_picker_key(state, resources.measurer, key) {
        return outcome;
    }
    if let Some(outcome) = adapters::handle_global_modifier_key(state, key) {
        return outcome;
    }
    if let Some(outcome) = adapters::handle_properties_panel_key(state, resources.measurer, key) {
        return outcome;
    }
    if let Some(outcome) = adapters::handle_top_popover_dismiss_key(state, key) {
        return outcome;
    }
    if let Some(outcome) = adapters::handle_pending_delete_cancel_key(state, key) {
        return outcome;
    }
    if let Some(outcome) =
        adapters::handle_idle_selection_cancel_key(state, resources.measurer, key)
    {
        return outcome;
    }
    if let Some(outcome) = adapters::handle_text_input_key(state, resources, key) {
        return outcome;
    }
    if let Some(outcome) = adapters::handle_building_polygon_key(state, resources.measurer, key) {
        return outcome;
    }
    if let Some(outcome) =
        adapters::handle_drawing_escape_cancel_key(state, resources.measurer, key)
    {
        return outcome;
    }

    match match_action_for_key_binding(state, key, is_repeat) {
        Ok(SequenceMatch::Dispatched(action)) => {
            return route_action_with_resources(state, resources, action);
        }
        Ok(SequenceMatch::Pending) => {
            return RoutingOutcome::Consumed(ConsumedBy::SequencePrefix);
        }
        Ok(SequenceMatch::None) => {}
        Err(NoRouteReason::UnsupportedKey) => {
            return RoutingOutcome::NoRoute(NoRouteReason::UnsupportedKey);
        }
        Err(reason) => return RoutingOutcome::NoRoute(reason),
    }

    if let Some(outcome) =
        adapters::handle_return_edit_selected_text_key(state, resources.measurer, key)
    {
        return outcome;
    }

    RoutingOutcome::NoRoute(NoRouteReason::NoKeyBinding)
}

fn match_action_for_key_binding(
    state: &mut InputState,
    key: Key,
    is_repeat: bool,
) -> Result<SequenceMatch, NoRouteReason> {
    let Some(key_str) = key_to_action_label(key) else {
        return Err(NoRouteReason::UnsupportedKey);
    };

    let now = Instant::now();
    if state.modifiers.shift
        && let Some(fallback) = fallback_unshifted_label(&key_str)
    {
        return Ok(state.match_keyboard_chord_with_fallback(&key_str, fallback, is_repeat, now));
    }

    Ok(state.match_keyboard_chord(&key_str, is_repeat, now))
}

/// The character a key contributes to the font picker's query.
///
/// The router carries `Key`, not the compositor's text, so a printable key is
/// reconstructed from `Key::Char`. Everything else contributes nothing and the
/// picker's own match arms decide what it means.
fn font_picker_text(key: Key) -> Option<String> {
    match key {
        Key::Char(ch) if !ch.is_control() => Some(ch.to_string()),
        _ => None,
    }
}
