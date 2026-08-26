use crate::domain::Action;

use super::super::{InputState, interaction};

impl InputState {
    /// Handle an action that a non-key caller has already resolved.
    ///
    /// Bound keys enter [`interaction::route_action`] directly, so action-wide
    /// gesture preflights live at that shared boundary rather than here.
    pub(crate) fn handle_action(&mut self, action: Action) {
        let _ = interaction::route_action(self, action);
    }
}
