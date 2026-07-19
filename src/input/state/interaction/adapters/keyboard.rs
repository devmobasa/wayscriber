use super::super::active::active_interaction_kind;
use super::super::outcome::{
    ActiveInteractionKind, CancelTarget, ConsumedBy, InteractionSideEffect, KeyboardSideEffect,
    NoRouteReason, RoutingOutcome,
};
use crate::domain::Action;
use crate::input::events::Key;
use crate::input::state::actions::key_press::bindings::{
    fallback_unshifted_label, key_to_action_label,
};
use crate::input::state::{DrawingState, InputState};

pub(crate) fn handle_tour_key(state: &mut InputState, key: Key) -> Option<RoutingOutcome> {
    (state.tour_active && state.handle_tour_key(key))
        .then_some(RoutingOutcome::Consumed(ConsumedBy::Tour))
}

pub(crate) fn handle_command_palette_key(
    state: &mut InputState,
    key: Key,
) -> Option<RoutingOutcome> {
    (state.command_palette_is_engaged() && state.handle_command_palette_key(key))
        .then_some(RoutingOutcome::Consumed(ConsumedBy::CommandPalette))
}

pub(crate) fn handle_help_overlay_key(state: &mut InputState, key: Key) -> Option<RoutingOutcome> {
    (state.show_help && state.handle_help_overlay_key(key))
        .then_some(RoutingOutcome::Consumed(ConsumedBy::HelpOverlay))
}

pub(crate) fn handle_radial_menu_key(state: &mut InputState, key: Key) -> Option<RoutingOutcome> {
    if !state.is_radial_menu_open() {
        return None;
    }

    if state.handle_modifier_key_press(key) {
        return Some(modifier_key_side_effect());
    }

    if matches!(key, Key::Escape) {
        state.close_radial_menu();
        return Some(RoutingOutcome::Consumed(ConsumedBy::RadialMenu));
    }

    if let Some(key_str) = key_to_action_label(key) {
        let mapped_action = state.find_action(&key_str).or_else(|| {
            if state.modifiers.shift {
                fallback_unshifted_label(&key_str).and_then(|fallback| state.find_action(fallback))
            } else {
                None
            }
        });
        if matches!(mapped_action, Some(Action::ToggleRadialMenu)) {
            state.close_radial_menu();
        }
    }

    Some(RoutingOutcome::Consumed(ConsumedBy::RadialMenu))
}

pub(crate) fn handle_precision_entry_key(
    state: &mut InputState,
    key: Key,
) -> Option<RoutingOutcome> {
    (state.is_precision_entry_open() && state.handle_precision_entry_key(key))
        .then_some(RoutingOutcome::Consumed(ConsumedBy::PrecisionEntry))
}

pub(crate) fn handle_color_picker_key(state: &mut InputState, key: Key) -> Option<RoutingOutcome> {
    (state.is_color_picker_popup_open() && state.handle_color_picker_popup_key(key))
        .then_some(RoutingOutcome::Consumed(ConsumedBy::ColorPickerPopup))
}

pub(crate) fn handle_context_menu_key(state: &mut InputState, key: Key) -> Option<RoutingOutcome> {
    (state.is_context_menu_open() && state.handle_context_menu_key(key))
        .then_some(RoutingOutcome::Consumed(ConsumedBy::ContextMenu))
}

pub(crate) fn handle_board_picker_key(state: &mut InputState, key: Key) -> Option<RoutingOutcome> {
    (state.is_board_picker_open() && state.handle_board_picker_key(key))
        .then_some(RoutingOutcome::Consumed(ConsumedBy::BoardPicker))
}

pub(crate) fn handle_global_modifier_key(
    state: &mut InputState,
    key: Key,
) -> Option<RoutingOutcome> {
    state
        .handle_modifier_key_press(key)
        .then_some(modifier_key_side_effect())
}

pub(crate) fn handle_properties_panel_key(
    state: &mut InputState,
    key: Key,
) -> Option<RoutingOutcome> {
    if !state.is_properties_panel_open() {
        return None;
    }

    let _ = state.handle_properties_panel_key(key);
    Some(RoutingOutcome::Consumed(ConsumedBy::PropertiesPanel))
}

pub(crate) fn handle_pending_delete_cancel_key(
    state: &mut InputState,
    key: Key,
) -> Option<RoutingOutcome> {
    if matches!(key, Key::Escape) && state.has_pending_board_delete() {
        state.cancel_pending_board_delete();
        return Some(RoutingOutcome::Canceled(CancelTarget::PendingBoardDelete));
    }
    if matches!(key, Key::Escape) && state.has_pending_page_delete() {
        state.cancel_pending_page_delete();
        return Some(RoutingOutcome::Canceled(CancelTarget::PendingPageDelete));
    }

    None
}

pub(crate) fn handle_idle_selection_cancel_key(
    state: &mut InputState,
    key: Key,
) -> Option<RoutingOutcome> {
    if matches!(key, Key::Escape)
        && matches!(state.state, DrawingState::Idle)
        && state.has_selection()
    {
        let bounds = state.selection_bounding_box(state.selected_shape_ids());
        state.clear_selection();
        state.mark_selection_dirty_region(bounds);
        state.needs_redraw = true;
        return Some(RoutingOutcome::Canceled(CancelTarget::Selection));
    }

    None
}

pub(crate) fn handle_text_input_key(state: &mut InputState, key: Key) -> Option<RoutingOutcome> {
    if matches!(&state.state, DrawingState::TextInput { .. }) {
        state.handle_text_input_key(key);
        return Some(RoutingOutcome::Consumed(ConsumedBy::TextInput));
    }

    None
}

pub(crate) fn handle_building_polygon_key(
    state: &mut InputState,
    key: Key,
) -> Option<RoutingOutcome> {
    if !matches!(state.state, DrawingState::BuildingPolygon { .. }) {
        return None;
    }

    match key {
        Key::Return => {
            state.finish_building_polygon();
            Some(RoutingOutcome::Finished(
                ActiveInteractionKind::BuildingPolygon,
            ))
        }
        Key::Escape => {
            state.cancel_active_interaction();
            Some(RoutingOutcome::Canceled(CancelTarget::ActiveInteraction(
                ActiveInteractionKind::BuildingPolygon,
            )))
        }
        Key::Backspace => {
            state.pop_building_polygon_point();
            Some(RoutingOutcome::Continued(
                ActiveInteractionKind::BuildingPolygon,
            ))
        }
        _ => None,
    }
}

pub(crate) fn handle_drawing_escape_cancel_key(
    state: &mut InputState,
    key: Key,
) -> Option<RoutingOutcome> {
    if matches!(key, Key::Escape)
        && let Some(ActiveInteractionKind::Drawing) = active_interaction_kind(state)
        && let Some(Action::Exit) = state.find_action("Escape")
    {
        state.try_cancel_active_interaction();
        return Some(RoutingOutcome::Canceled(CancelTarget::ActiveInteraction(
            ActiveInteractionKind::Drawing,
        )));
    }

    None
}

pub(crate) fn action_for_key_binding(
    state: &InputState,
    key: Key,
) -> Result<Option<Action>, NoRouteReason> {
    let Some(key_str) = key_to_action_label(key) else {
        return Err(NoRouteReason::UnsupportedKey);
    };

    if let Some(action) = state.find_action(&key_str) {
        return Ok(Some(action));
    }
    if state.modifiers.shift
        && let Some(fallback) = fallback_unshifted_label(&key_str)
        && let Some(action) = state.find_action(fallback)
    {
        return Ok(Some(action));
    }

    Ok(None)
}

pub(crate) fn handle_return_edit_selected_text_key(
    state: &mut InputState,
    key: Key,
) -> Option<RoutingOutcome> {
    if matches!(key, Key::Return)
        && !state.modifiers.ctrl
        && !state.modifiers.shift
        && !state.modifiers.alt
        && matches!(state.state, DrawingState::Idle)
    {
        if state.edit_selected_text() {
            return Some(RoutingOutcome::Started(ActiveInteractionKind::TextInput));
        }
        return Some(return_edit_miss_side_effect());
    }

    None
}

fn modifier_key_side_effect() -> RoutingOutcome {
    RoutingOutcome::SideEffect(InteractionSideEffect::Keyboard(
        KeyboardSideEffect::ModifierUpdated,
    ))
}

fn return_edit_miss_side_effect() -> RoutingOutcome {
    RoutingOutcome::SideEffect(InteractionSideEffect::Keyboard(
        KeyboardSideEffect::ReturnEditSelectedTextMiss,
    ))
}
