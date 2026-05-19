use crate::config::Action;

use super::super::{InputState, interaction};

impl InputState {
    /// Handle an action triggered by a keybinding.
    pub(crate) fn handle_action(&mut self, action: Action) {
        let _ = interaction::route_action(self, action);
    }
}
