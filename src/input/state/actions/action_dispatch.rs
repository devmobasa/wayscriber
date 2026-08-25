use crate::domain::Action;

use super::super::{InputState, interaction};

impl InputState {
    /// Handle an action triggered by a keybinding.
    ///
    /// Any action closes an in-flight wheel adjustment of a loupe first. This
    /// is the one place every page switch, board switch, session load, undo,
    /// and redo passes through, and a gesture must never outlive the frame it
    /// started on: shape ids restart per frame, so a snapshot flushed after a
    /// page change would attach to an unrelated shape.
    pub(crate) fn handle_action(&mut self, action: Action) {
        self.flush_spotlight_magnification_gesture();
        let _ = interaction::route_action(self, action);
    }
}
