use std::collections::HashMap;

use super::super::{Action, KeyBinding};
use super::types::KeybindingsConfig;

mod board;
mod capture;
mod colors;
mod core;
mod presets;
mod selection;
mod tools;
mod ui;
mod zoom;

struct BindingInserter<'a> {
    map: &'a mut HashMap<KeyBinding, Action>,
    ordered: Option<&'a mut HashMap<Action, Vec<KeyBinding>>>,
}

impl<'a> BindingInserter<'a> {
    fn new(map: &'a mut HashMap<KeyBinding, Action>) -> Self {
        Self { map, ordered: None }
    }

    fn new_with_order(
        map: &'a mut HashMap<KeyBinding, Action>,
        ordered: &'a mut HashMap<Action, Vec<KeyBinding>>,
    ) -> Self {
        Self {
            map,
            ordered: Some(ordered),
        }
    }

    fn insert(&mut self, binding_str: &str, action: Action) -> Result<(), String> {
        let binding = KeyBinding::parse(binding_str)?;
        if let Some(existing_action) = self.map.insert(binding.clone(), action) {
            return Err(format!(
                "Duplicate keybinding '{}' assigned to both {:?} and {:?}",
                binding_str, existing_action, action
            ));
        }
        if let Some(ordered) = self.ordered.as_mut() {
            ordered.entry(action).or_default().push(binding);
        }
        Ok(())
    }

    fn insert_all(&mut self, bindings: &[String], action: Action) -> Result<(), String> {
        for binding_str in bindings {
            self.insert(binding_str, action)?;
        }
        Ok(())
    }
}

impl KeybindingsConfig {
    /// Build a lookup map from keybindings to actions for efficient matching.
    /// Returns an error if any keybinding string is invalid or if duplicates are detected.
    pub fn build_action_map(&self) -> Result<HashMap<KeyBinding, Action>, String> {
        let mut map = HashMap::new();
        let mut inserter = BindingInserter::new(&mut map);

        self.insert_core_bindings(&mut inserter)?;
        self.insert_selection_bindings(&mut inserter)?;
        self.insert_tool_bindings(&mut inserter)?;
        self.insert_board_bindings(&mut inserter)?;
        self.insert_ui_bindings(&mut inserter)?;
        self.insert_color_bindings(&mut inserter)?;
        self.insert_capture_bindings(&mut inserter)?;
        self.insert_zoom_bindings(&mut inserter)?;
        self.insert_preset_bindings(&mut inserter)?;

        Ok(map)
    }

    /// Build an ordered list of keybindings per action.
    /// Returns an error if any keybinding string is invalid or if duplicates are detected.
    pub fn build_action_bindings(&self) -> Result<HashMap<Action, Vec<KeyBinding>>, String> {
        let mut map = HashMap::new();
        let mut ordered = HashMap::new();
        let mut inserter = BindingInserter::new_with_order(&mut map, &mut ordered);

        self.insert_core_bindings(&mut inserter)?;
        self.insert_selection_bindings(&mut inserter)?;
        self.insert_tool_bindings(&mut inserter)?;
        self.insert_board_bindings(&mut inserter)?;
        self.insert_ui_bindings(&mut inserter)?;
        self.insert_color_bindings(&mut inserter)?;
        self.insert_capture_bindings(&mut inserter)?;
        self.insert_zoom_bindings(&mut inserter)?;
        self.insert_preset_bindings(&mut inserter)?;

        Ok(ordered)
    }
}
