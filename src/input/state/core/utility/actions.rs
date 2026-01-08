use super::super::base::InputState;
use crate::config::{Action, KeyBinding};
use std::collections::{HashMap, HashSet};

impl InputState {
    /// Look up an action for the given key and modifiers.
    pub(crate) fn find_action(&self, key_str: &str) -> Option<Action> {
        for (binding, action) in &self.action_map {
            if binding.matches(
                key_str,
                self.modifiers.ctrl,
                self.modifiers.shift,
                self.modifiers.alt,
            ) {
                return Some(*action);
            }
        }
        None
    }

    pub fn set_action_bindings(&mut self, action_bindings: HashMap<Action, Vec<KeyBinding>>) {
        self.action_bindings = action_bindings;
    }

    pub fn action_binding_labels(&self, action: Action) -> Vec<String> {
        if let Some(bindings) = self.action_bindings.get(&action) {
            let mut labels = Vec::new();
            let mut seen = HashSet::new();
            for binding in bindings {
                let label = binding.to_string();
                if seen.insert(label.clone()) {
                    labels.push(label);
                }
            }
            return labels;
        }
        let mut labels: Vec<String> = self
            .action_map
            .iter()
            .filter(|(_, mapped)| **mapped == action)
            .map(|(binding, _)| binding.to_string())
            .collect();
        labels.sort();
        labels.dedup();
        labels
    }

    #[allow(dead_code)]
    pub fn action_binding_primary_label(&self, action: Action) -> Option<String> {
        self.action_binding_labels(action).into_iter().next()
    }

    #[allow(dead_code)]
    pub fn action_binding_label(&self, action: Action) -> String {
        let labels = self.action_binding_labels(action);
        if labels.is_empty() {
            "Not bound".to_string()
        } else {
            labels.join(" / ")
        }
    }
}
