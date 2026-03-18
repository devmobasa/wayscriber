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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{KeyBinding, RadialMenuMouseBinding};
    use crate::input::state::test_support::make_test_input_state_with_action_bindings;
    use std::collections::HashMap;

    fn binding_map(entries: &[(Action, &[&str])]) -> HashMap<Action, Vec<KeyBinding>> {
        entries
            .iter()
            .map(|(action, values)| {
                (
                    *action,
                    values
                        .iter()
                        .map(|value| KeyBinding::parse(value).expect("binding"))
                        .collect(),
                )
            })
            .collect()
    }

    #[test]
    fn shortcut_for_help_prefers_f1_over_other_bindings() {
        let state = make_test_input_state_with_action_bindings(binding_map(&[(
            Action::ToggleHelp,
            &["Ctrl+Alt+Shift+H", "F1"],
        )]));
        assert_eq!(
            state.shortcut_for_action(Action::ToggleHelp),
            Some("F1".to_string())
        );
    }

    #[test]
    fn shortcut_for_radial_menu_uses_mouse_binding_when_no_key_binding_exists() {
        let mut state = make_test_input_state_with_action_bindings(HashMap::new());
        state.radial_menu_mouse_binding = RadialMenuMouseBinding::Middle;
        assert_eq!(
            state.shortcut_for_action(Action::ToggleRadialMenu),
            Some("Middle Click".to_string())
        );
    }

    #[test]
    fn shortcut_for_radial_menu_combines_mouse_and_keyboard_bindings() {
        let mut state = make_test_input_state_with_action_bindings(binding_map(&[(
            Action::ToggleRadialMenu,
            &["Ctrl+R"],
        )]));
        state.radial_menu_mouse_binding = RadialMenuMouseBinding::Middle;

        assert_eq!(
            state.shortcut_for_action(Action::ToggleRadialMenu),
            Some("Middle Click / Ctrl+R".to_string())
        );
    }

    #[test]
    fn shortcut_for_radial_menu_returns_keyboard_only_when_mouse_binding_disabled() {
        let mut state = make_test_input_state_with_action_bindings(binding_map(&[(
            Action::ToggleRadialMenu,
            &["Ctrl+R"],
        )]));
        state.radial_menu_mouse_binding = RadialMenuMouseBinding::Disabled;

        assert_eq!(
            state.shortcut_for_action(Action::ToggleRadialMenu),
            Some("Ctrl+R".to_string())
        );
    }

    #[test]
    fn shortcut_for_radial_menu_returns_none_when_fully_unbound() {
        let mut state = make_test_input_state_with_action_bindings(HashMap::new());
        state.radial_menu_mouse_binding = RadialMenuMouseBinding::Disabled;

        assert_eq!(state.shortcut_for_action(Action::ToggleRadialMenu), None);
    }
}
