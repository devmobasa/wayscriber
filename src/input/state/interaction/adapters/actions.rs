use super::super::outcome::ActionRoute;
use crate::domain::Action;
use crate::input::state::InputState;

pub(crate) fn close_properties_panel_before_action(state: &mut InputState) {
    state.close_properties_panel();
}

pub(crate) fn dispatch_action(state: &mut InputState, action: Action, route: ActionRoute) {
    match route {
        ActionRoute::Core => {
            state.handle_core_action(action);
        }
        ActionRoute::History => {
            state.handle_history_action(action);
        }
        ActionRoute::Selection => {
            state.handle_selection_action(action);
        }
        ActionRoute::Tool => {
            state.handle_tool_action(action);
        }
        ActionRoute::BoardPages => {
            state.handle_board_pages_action(action);
        }
        ActionRoute::Ui => {
            state.handle_ui_action(action);
        }
        ActionRoute::Color => {
            state.handle_color_action(action);
        }
        ActionRoute::CaptureZoom => {
            state.handle_capture_zoom_action(action);
        }
        ActionRoute::Preset => {
            state.handle_preset_action(action);
        }
    }
}
