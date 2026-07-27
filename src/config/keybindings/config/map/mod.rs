use std::collections::HashMap;

use super::super::{Action, KeyBinding};
use super::types::KeybindingsConfig;

mod board;
mod capture;
mod colors;
mod core;
mod edit;
mod presets;
mod selection;
mod tools;
mod ui;
mod zoom;

/// One key claimed by more than one action.
///
/// `actions` follows the keymap traversal order (core, selection, tools,
/// board, ui, colors, capture, zoom, presets, and the declared order inside
/// each group), so the first entry is the earliest claimant. A key a single
/// action lists more than once yields a one-element `actions`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeybindingConflict {
    binding: KeyBinding,
    actions: Vec<Action>,
}

impl KeybindingConflict {
    /// The parsed shortcut every listed action claims.
    pub fn binding(&self) -> &KeyBinding {
        &self.binding
    }

    /// Every action claiming the shortcut, in keymap traversal order.
    pub fn actions(&self) -> &[Action] {
        &self.actions
    }
}

#[derive(Default)]
struct ConflictLog {
    entries: Vec<KeybindingConflict>,
    positions: HashMap<KeyBinding, usize>,
}

impl ConflictLog {
    fn record(&mut self, binding: &KeyBinding, first: Action, next: Action) {
        match self.positions.get(binding) {
            Some(&position) => {
                let actions = &mut self.entries[position].actions;
                if !actions.contains(&next) {
                    actions.push(next);
                }
            }
            None => {
                self.positions.insert(binding.clone(), self.entries.len());
                let mut actions = vec![first];
                if next != first {
                    actions.push(next);
                }
                self.entries.push(KeybindingConflict {
                    binding: binding.clone(),
                    actions,
                });
            }
        }
    }
}

struct BindingInserter<'a> {
    map: &'a mut HashMap<KeyBinding, Action>,
    ordered: Option<&'a mut HashMap<Action, Vec<KeyBinding>>>,
    conflicts: Option<&'a mut ConflictLog>,
}

impl<'a> BindingInserter<'a> {
    fn new(map: &'a mut HashMap<KeyBinding, Action>) -> Self {
        Self {
            map,
            ordered: None,
            conflicts: None,
        }
    }

    fn new_with_order(
        map: &'a mut HashMap<KeyBinding, Action>,
        ordered: &'a mut HashMap<Action, Vec<KeyBinding>>,
    ) -> Self {
        Self {
            map,
            ordered: Some(ordered),
            conflicts: None,
        }
    }

    fn new_collecting(
        map: &'a mut HashMap<KeyBinding, Action>,
        conflicts: &'a mut ConflictLog,
    ) -> Self {
        Self {
            map,
            ordered: None,
            conflicts: Some(conflicts),
        }
    }

    fn insert(&mut self, binding_str: &str, action: Action) -> Result<(), String> {
        let binding = KeyBinding::parse(binding_str)?;
        if let Some(existing_action) = self.map.insert(binding.clone(), action) {
            let Some(conflicts) = self.conflicts.as_mut() else {
                return Err(format!(
                    "Duplicate keybinding '{}' assigned to both {:?} and {:?}",
                    binding_str, existing_action, action
                ));
            };
            conflicts.record(&binding, existing_action, action);
            // Keep the earliest claimant so a third collision on the same key
            // still reports the actions in traversal order.
            self.map.insert(binding, existing_action);
            return Ok(());
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
    /// The single traversal every keymap view shares. Its order is part of the
    /// conflict-resolution contract (see
    /// `Config::resolve_keybinding_conflicts`), so groups are visited in a
    /// fixed sequence rather than an incidental one.
    fn insert_every_binding(&self, inserter: &mut BindingInserter<'_>) -> Result<(), String> {
        self.insert_core_bindings(inserter)?;
        self.insert_selection_bindings(inserter)?;
        self.insert_tool_bindings(inserter)?;
        self.insert_board_bindings(inserter)?;
        self.insert_ui_bindings(inserter)?;
        self.insert_color_bindings(inserter)?;
        self.insert_capture_bindings(inserter)?;
        self.insert_zoom_bindings(inserter)?;
        self.insert_preset_bindings(inserter)?;
        Ok(())
    }

    /// Build a lookup map from keybindings to actions for efficient matching.
    /// Returns an error if any keybinding string is invalid or if duplicates are detected.
    pub fn build_action_map(&self) -> Result<HashMap<KeyBinding, Action>, String> {
        let mut map = HashMap::new();
        let mut inserter = BindingInserter::new(&mut map);
        self.insert_every_binding(&mut inserter)?;
        Ok(map)
    }

    /// Build an ordered list of keybindings per action.
    /// Returns an error if any keybinding string is invalid or if duplicates are detected.
    pub fn build_action_bindings(&self) -> Result<HashMap<Action, Vec<KeyBinding>>, String> {
        let mut map = HashMap::new();
        let mut ordered = HashMap::new();
        let mut inserter = BindingInserter::new_with_order(&mut map, &mut ordered);
        self.insert_every_binding(&mut inserter)?;
        Ok(ordered)
    }

    /// Report every key claimed by more than one action instead of failing on
    /// the first collision.
    ///
    /// Duplicates are data here, not errors: validation resolves them one key
    /// at a time so a single collision can never cost the user the rest of
    /// their shortcuts (#293). Only an unparseable binding string is still an
    /// error, because there is nothing to compare it against.
    pub fn collect_binding_conflicts(&self) -> Result<Vec<KeybindingConflict>, String> {
        let mut map = HashMap::new();
        let mut conflicts = ConflictLog::default();
        let mut inserter = BindingInserter::new_collecting(&mut map, &mut conflicts);
        self.insert_every_binding(&mut inserter)?;
        Ok(conflicts.entries)
    }
}
