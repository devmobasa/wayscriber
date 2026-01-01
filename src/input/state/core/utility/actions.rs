use super::super::base::InputState;
use crate::config::Action;

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

    pub fn action_binding_label(&self, action: Action) -> String {
        let mut labels: Vec<String> = self
            .action_map
            .iter()
            .filter(|(_, mapped)| **mapped == action)
            .map(|(binding, _)| binding.to_string())
            .collect();
        labels.sort();
        labels.dedup();
        if labels.is_empty() {
            "Not bound".to_string()
        } else {
            labels.join(" / ")
        }
    }
}
