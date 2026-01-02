use std::collections::{HashMap, hash_map::Entry};

use super::super::{Action, KeyBinding};
use super::types::{DuplicateKeybindingPolicy, KeybindingsConfig};

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
    policy: DuplicateKeybindingPolicy,
}

impl<'a> BindingInserter<'a> {
    fn new(map: &'a mut HashMap<KeyBinding, Action>, policy: DuplicateKeybindingPolicy) -> Self {
        Self { map, policy }
    }

    fn insert(&mut self, binding_str: &str, action: Action) -> Result<(), String> {
        let binding = KeyBinding::parse(binding_str)?;
        let normalized = binding.to_string();
        let label = if normalized != binding_str {
            format!("{normalized} (from '{binding_str}')")
        } else {
            normalized
        };

        match self.map.entry(binding) {
            Entry::Occupied(mut entry) => {
                let existing_action = *entry.get();
                match self.policy {
                    DuplicateKeybindingPolicy::Error => Err(format!(
                        "Duplicate keybinding '{}' assigned to both {:?} and {:?}",
                        label, existing_action, action
                    )),
                    DuplicateKeybindingPolicy::KeepFirst => {
                        log::warn!(
                            "Duplicate keybinding '{}' assigned to both {:?} and {:?}; keeping {:?} (policy: keep_first)",
                            label,
                            existing_action,
                            action,
                            existing_action
                        );
                        Ok(())
                    }
                    DuplicateKeybindingPolicy::KeepLast => {
                        log::warn!(
                            "Duplicate keybinding '{}' assigned to both {:?} and {:?}; overriding with {:?} (policy: keep_last)",
                            label,
                            existing_action,
                            action,
                            action
                        );
                        entry.insert(action);
                        Ok(())
                    }
                }
            }
            Entry::Vacant(entry) => {
                entry.insert(action);
                Ok(())
            }
        }
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
    /// Returns an error if any keybinding string is invalid.
    /// Duplicate keybindings are handled according to `policy`.
    pub fn build_action_map_with_policy(
        &self,
        policy: DuplicateKeybindingPolicy,
    ) -> Result<HashMap<KeyBinding, Action>, String> {
        let mut map = HashMap::new();
        let mut inserter = BindingInserter::new(&mut map, policy);

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

    /// Build a lookup map from keybindings to actions for efficient matching.
    /// Returns an error if any keybinding string is invalid.
    /// Duplicate keybindings are handled according to `duplicate_policy`.
    pub fn build_action_map(&self) -> Result<HashMap<KeyBinding, Action>, String> {
        self.build_action_map_with_policy(self.duplicate_policy)
    }
}
