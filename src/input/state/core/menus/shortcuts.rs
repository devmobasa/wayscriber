use super::super::base::InputState;
use crate::config::Action;

impl InputState {
    /// Get the display string for the first keybinding of an action.
    /// Returns None if no binding exists.
    pub fn shortcut_for_action(&self, action: Action) -> Option<String> {
        self.action_bindings
            .get(&action)
            .and_then(|bindings| bindings.first())
            .map(|binding| binding.to_string())
    }
}
