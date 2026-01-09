use super::structs::InputState;

impl InputState {
    /// Resets all tracked keyboard modifiers to the "released" state.
    ///
    /// This is used as a safety net when external UI (portals, other windows)
    /// or focus transitions may cause us to miss key release events from
    /// the compositor, which would otherwise leave modifiers "stuck" and break
    /// shortcut handling and tool selection.
    pub fn reset_modifiers(&mut self) {
        self.modifiers.shift = false;
        self.modifiers.ctrl = false;
        self.modifiers.alt = false;
        self.modifiers.tab = false;
        self.clear_hold_to_draw();
    }

    /// Synchronize modifier state from backend-provided values (e.g. compositor).
    ///
    /// This lets us correct cases where a key release event was missed but the compositor's
    /// authoritative modifier state is still accurate.
    pub fn sync_modifiers(&mut self, shift: bool, ctrl: bool, alt: bool) {
        self.modifiers.shift = shift;
        self.modifiers.ctrl = ctrl;
        self.modifiers.alt = alt;
        // Tab has no direct compositor flag; leave it unchanged.
    }
}
