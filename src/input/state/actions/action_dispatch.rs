use crate::domain::Action;

use super::super::{InputState, interaction};

impl InputState {
    /// Handle an action that a non-key caller has already resolved.
    ///
    /// Bound keys enter [`interaction::route_action_with_resources`] directly, so action-wide
    /// gesture preflights live at that shared boundary rather than here.
    pub(crate) fn handle_action_with_resources(
        &mut self,
        resources: crate::input::state::InputTextResources<'_>,
        action: Action,
    ) {
        let _ = interaction::route_action_with_resources(self, resources, action);
    }

    /// Handle an action for a control that sits away from the pointer, such as
    /// the zoom chip or a command palette row: a zoom it requests centres on
    /// `anchor` instead of on the control under the pointer.
    pub(crate) fn handle_action_anchored(
        &mut self,
        resources: crate::input::state::InputTextResources<'_>,
        action: Action,
        anchor: crate::input::state::ZoomAnchor,
    ) {
        let previous = self.zoom_action_anchor.replace(anchor);
        self.handle_action_with_resources(resources, action);
        self.zoom_action_anchor = previous;
    }
}
