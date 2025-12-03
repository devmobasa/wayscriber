use wayland_client::protocol::wl_seat;

/// Focus/pointer/toolbar interaction data owned by WaylandState and shared with handlers.
#[derive(Debug, Default)]
pub struct StateData {
    pub(super) has_keyboard_focus: bool,
    pub(super) has_pointer_focus: bool,
    pub(super) current_mouse_x: i32,
    pub(super) current_mouse_y: i32,
    pub(super) current_seat: Option<wl_seat::WlSeat>,
    pub(super) last_activation_serial: Option<u32>,
    pub(super) pointer_over_toolbar: bool,
    pub(super) toolbar_dragging: bool,
    pub(super) current_keyboard_interactivity:
        Option<smithay_client_toolkit::shell::wlr_layer::KeyboardInteractivity>,
    pub(super) toolbar_needs_recreate: bool,
    pub(super) pending_activation_token: Option<String>,
    pub(super) pending_freeze_on_start: bool,
    pub(super) frozen_enabled: bool,
    pub(super) preferred_output_identity: Option<String>,
    pub(super) xdg_fullscreen: bool,
}

impl StateData {
    pub fn new() -> Self {
        Self {
            has_keyboard_focus: false,
            has_pointer_focus: false,
            current_mouse_x: 0,
            current_mouse_y: 0,
            current_seat: None,
            last_activation_serial: None,
            pointer_over_toolbar: false,
            toolbar_dragging: false,
            current_keyboard_interactivity: None,
            toolbar_needs_recreate: true,
            pending_activation_token: None,
            pending_freeze_on_start: false,
            frozen_enabled: false,
            preferred_output_identity: None,
            xdg_fullscreen: false,
        }
    }
}
