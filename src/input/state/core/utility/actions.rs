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

        if self.modifiers.shift
            && let Some(unshifted) = unshift_digit_key(key_str)
        {
            for (binding, action) in &self.action_map {
                if binding.matches(
                    unshifted,
                    self.modifiers.ctrl,
                    self.modifiers.shift,
                    self.modifiers.alt,
                ) {
                    return Some(*action);
                }
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

fn unshift_digit_key(key_str: &str) -> Option<&'static str> {
    match key_str {
        "!" => Some("1"),
        "@" => Some("2"),
        "#" => Some("3"),
        "$" => Some("4"),
        "%" => Some("5"),
        "^" => Some("6"),
        "&" => Some("7"),
        "*" => Some("8"),
        "(" => Some("9"),
        ")" => Some("0"),
        _ => None,
    }
}
