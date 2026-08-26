use super::super::WaylandState;

impl WaylandState {
    /// Retire every keyboard-owned transient when focus is lost.
    ///
    /// Both the compositor's keyboard-leave callback and layer-output surface
    /// recreation enter through this state lifecycle boundary.
    pub(in crate::backend::wayland) fn teardown_keyboard_focus(&mut self) {
        self.set_keyboard_focus(false);
        self.set_overlay_ready(false);
        self.clear_toolbar_focus();
        self.input_state.clear_focus_owned_key_state();
        self.sync_region_square_modifier(false);
        self.clear_key_repeat();
        self.set_board_pan_key_held(false);
        self.stop_board_pan();
    }
}
