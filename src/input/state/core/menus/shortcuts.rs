use super::super::base::InputState;
use crate::config::Action;

impl InputState {
    /// Get the display string for the first keybinding of an action.
    /// Returns None if no binding exists.
    pub fn shortcut_for_action(&self, action: Action) -> Option<String> {
        if action == Action::ToggleRadialMenu {
            return self.radial_menu_shortcut_label();
        }

        let mut labels = self.action_binding_labels(action);
        if action == Action::ToggleHelp
            && let Some(idx) = labels.iter().position(|label| label == "F1")
        {
            return Some(labels.swap_remove(idx));
        }
        labels.into_iter().next()
    }

    fn radial_menu_shortcut_label(&self) -> Option<String> {
        use crate::config::RadialMenuMouseBinding;

        let mouse_label = match self.radial_menu_mouse_binding {
            RadialMenuMouseBinding::Middle => Some("Middle Click".to_string()),
            RadialMenuMouseBinding::Right => Some("Right Click".to_string()),
            RadialMenuMouseBinding::Disabled => None,
        };
        let key_label = self
            .action_binding_labels(Action::ToggleRadialMenu)
            .into_iter()
            .next();

        match (mouse_label, key_label) {
            (Some(mouse), Some(key)) => Some(format!("{mouse} / {key}")),
            (Some(mouse), None) => Some(mouse),
            (None, Some(key)) => Some(key),
            (None, None) => None,
        }
    }
}
