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
    use crate::config::{
        BoardsConfig, KeyBinding, KeybindingsConfig, PresenterModeConfig, RadialMenuMouseBinding,
    };
    use crate::draw::{Color, FontDescriptor};
    use crate::input::{ClickHighlightSettings, EraserMode};
    use std::collections::HashMap;

    fn make_state() -> InputState {
        let keybindings = KeybindingsConfig::default();
        let action_map = keybindings
            .build_action_map()
            .expect("default keybindings map");
        let action_bindings = keybindings
            .build_action_bindings()
            .expect("default keybindings bindings");

        let mut state = InputState::with_defaults(
            Color {
                r: 1.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
            4.0,
            4.0,
            EraserMode::Brush,
            0.32,
            false,
            32.0,
            FontDescriptor::default(),
            false,
            20.0,
            30.0,
            false,
            true,
            BoardsConfig::default(),
            action_map,
            usize::MAX,
            ClickHighlightSettings::disabled(),
            0,
            0,
            true,
            0,
            0,
            5,
            5,
            PresenterModeConfig::default(),
        );
        state.set_action_bindings(action_bindings);
        state
    }

    #[test]
    fn shortcut_for_help_prefers_f1_over_other_bindings() {
        let state = make_state();
        assert_eq!(state.shortcut_for_action(Action::ToggleHelp), Some("F1".to_string()));
    }

    #[test]
    fn shortcut_for_radial_menu_uses_mouse_binding_when_no_key_binding_exists() {
        let state = make_state();
        assert_eq!(
            state.shortcut_for_action(Action::ToggleRadialMenu),
            Some("Middle Click".to_string())
        );
    }

    #[test]
    fn shortcut_for_radial_menu_combines_mouse_and_keyboard_bindings() {
        let mut state = make_state();
        let mut action_bindings = HashMap::new();
        action_bindings.insert(
            Action::ToggleRadialMenu,
            vec![KeyBinding::parse("Ctrl+R").expect("binding")],
        );
        state.set_action_bindings(action_bindings);

        assert_eq!(
            state.shortcut_for_action(Action::ToggleRadialMenu),
            Some("Middle Click / Ctrl+R".to_string())
        );
    }

    #[test]
    fn shortcut_for_radial_menu_returns_keyboard_only_when_mouse_binding_disabled() {
        let mut state = make_state();
        state.radial_menu_mouse_binding = RadialMenuMouseBinding::Disabled;
        let mut action_bindings = HashMap::new();
        action_bindings.insert(
            Action::ToggleRadialMenu,
            vec![KeyBinding::parse("Ctrl+R").expect("binding")],
        );
        state.set_action_bindings(action_bindings);

        assert_eq!(
            state.shortcut_for_action(Action::ToggleRadialMenu),
            Some("Ctrl+R".to_string())
        );
    }

    #[test]
    fn shortcut_for_radial_menu_returns_none_when_fully_unbound() {
        let mut state = make_state();
        state.radial_menu_mouse_binding = RadialMenuMouseBinding::Disabled;
        state.set_action_bindings(HashMap::new());

        assert_eq!(state.shortcut_for_action(Action::ToggleRadialMenu), None);
    }
}
