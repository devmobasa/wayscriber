use super::super::outcome::ActionRoute;
use crate::domain::Action;
use crate::input::state::InputState;

pub(crate) fn close_properties_panel_before_action(state: &mut InputState) {
    state.close_properties_panel();
}

pub(crate) fn dispatch_action(
    state: &mut InputState,
    resources: crate::input::state::InputTextResources<'_>,
    action: Action,
    route: ActionRoute,
) {
    match route {
        ActionRoute::Core => {
            state.handle_core_action_with_measurer(resources.measurer, action);
        }
        ActionRoute::History => {
            state.handle_history_action_with_measurer(resources.measurer, action);
        }
        ActionRoute::Selection => {
            state.handle_selection_action_with_measurer(resources.measurer, action);
        }
        ActionRoute::Tool => {
            state.handle_tool_action_with_measurer(resources.measurer, action);
        }
        ActionRoute::BoardPages => {
            state.handle_board_pages_action_with_measurer(resources.measurer, action);
        }
        ActionRoute::Ui => {
            state.handle_ui_action_with_resources(resources, action);
        }
        ActionRoute::Color => {
            state.handle_color_action_with_measurer(resources.measurer, action);
        }
        ActionRoute::CaptureZoom => {
            state.handle_capture_zoom_action(action);
        }
        ActionRoute::Preset => {
            state.handle_preset_action_with_measurer(resources.measurer, action);
        }
    }
}
