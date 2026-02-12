use super::super::base::InputState;
use crate::config::Action;

impl InputState {
    /// Get the display string for the first keybinding of an action.
    /// Returns None if no binding exists.
    pub fn shortcut_for_action(&self, action: Action) -> Option<String> {
        let mut labels = self.action_binding_labels(action);
        if action == Action::ToggleHelp
            && let Some(idx) = labels.iter().position(|label| label == "F1")
        {
            return Some(labels.swap_remove(idx));
        }
        labels.into_iter().next()
    }
}
