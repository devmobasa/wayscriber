use crate::domain::Action;

use super::super::{InputState, interaction};

impl InputState {
    /// Handle an action that a non-key caller has already resolved.
    ///
    /// Bound keys enter [`interaction::route_action_with_resources`] directly, so action-wide
    /// gesture preflights live at that shared boundary rather than here.
    pub(crate) fn handle_action(&mut self, action: Action) {
        crate::input::state::with_legacy_text_resources(|resources| {
            self.handle_action_with_resources(resources, action)
        });
    }

    pub(crate) fn handle_action_with_resources(
        &mut self,
        resources: crate::input::state::InputTextResources<'_>,
        action: Action,
    ) {
        let _ = interaction::route_action_with_resources(self, resources, action);
    }
}
