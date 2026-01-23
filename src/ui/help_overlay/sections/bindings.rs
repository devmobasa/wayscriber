use std::collections::{HashMap, HashSet};

use crate::config::{Action, action_meta_iter};
use crate::input::InputState;
use crate::label_format::{format_binding_labels_or, join_binding_labels};

#[derive(Default)]
pub struct HelpOverlayBindings {
    labels: HashMap<Action, Vec<String>>,
    cache_key: String,
}

impl HelpOverlayBindings {
    pub fn from_input_state(state: &InputState) -> Self {
        let mut labels = HashMap::new();
        for meta in action_meta_iter().filter(|meta| meta.in_help) {
            let bindings = state.action_binding_labels(meta.action);
            if !bindings.is_empty() {
                labels.insert(meta.action, bindings);
            }
        }

        let mut cache_parts = Vec::new();
        for meta in action_meta_iter().filter(|meta| meta.in_help) {
            if let Some(values) = labels.get(&meta.action) {
                cache_parts.push(format!("{:?}={}", meta.action, values.join("/")));
            }
        }

        Self {
            labels,
            cache_key: cache_parts.join("|"),
        }
    }

    pub(crate) fn labels_for(&self, action: Action) -> Option<&[String]> {
        self.labels.get(&action).map(|values| values.as_slice())
    }

    pub(crate) fn cache_key(&self) -> &str {
        self.cache_key.as_str()
    }
}

fn collect_labels(bindings: &HelpOverlayBindings, actions: &[Action]) -> Vec<String> {
    let mut labels = Vec::new();
    let mut seen = HashSet::new();
    for action in actions {
        if let Some(values) = bindings.labels_for(*action) {
            for value in values {
                if seen.insert(value.clone()) {
                    labels.push(value.clone());
                }
            }
        }
    }
    labels
}

pub(super) fn joined_labels(bindings: &HelpOverlayBindings, actions: &[Action]) -> Option<String> {
    join_binding_labels(&collect_labels(bindings, actions))
}

pub(super) fn binding_or_fallback(
    bindings: &HelpOverlayBindings,
    action: Action,
    fallback: &str,
) -> String {
    format_binding_labels_or(&collect_labels(bindings, &[action]), fallback)
}

pub(super) fn bindings_or_fallback(
    bindings: &HelpOverlayBindings,
    actions: &[Action],
    fallback: &str,
) -> String {
    format_binding_labels_or(&collect_labels(bindings, actions), fallback)
}

pub(super) fn bindings_compact_or_fallback(
    bindings: &HelpOverlayBindings,
    actions: &[Action],
    fallback: &str,
) -> String {
    let labels = collect_labels(bindings, actions);
    if labels.is_empty() {
        return fallback.to_string();
    }
    if let Some(compact) = compact_numeric_range(&labels) {
        return compact;
    }
    const MAX_LABELS: usize = 3;
    if labels.len() > MAX_LABELS {
        let mut compact = labels[..MAX_LABELS].join(" / ");
        compact.push_str(" / ...");
        return compact;
    }
    join_binding_labels(&labels).unwrap_or_else(|| fallback.to_string())
}

pub(super) fn primary_or_fallback(
    bindings: &HelpOverlayBindings,
    action: Action,
    fallback: &str,
) -> String {
    bindings
        .labels_for(action)
        .and_then(|values| values.first())
        .cloned()
        .unwrap_or_else(|| fallback.to_string())
}

fn compact_numeric_range(labels: &[String]) -> Option<String> {
    if labels.len() <= 1 {
        return None;
    }
    let mut prefix: Option<String> = None;
    let mut numbers: Vec<u32> = Vec::with_capacity(labels.len());
    for label in labels {
        let (label_prefix, number) = split_numeric_suffix(label)?;
        if let Some(existing) = prefix.as_ref() {
            if existing != &label_prefix {
                return None;
            }
        } else {
            prefix = Some(label_prefix);
        }
        numbers.push(number);
    }
    numbers.sort_unstable();
    numbers.dedup();
    if numbers.len() <= 1 {
        return None;
    }
    let min = *numbers.first()?;
    let max = *numbers.last()?;
    if max - min + 1 != numbers.len() as u32 {
        return None;
    }
    let prefix = prefix.unwrap_or_default();
    Some(format!("{prefix}{min}..{max}"))
}

fn split_numeric_suffix(label: &str) -> Option<(String, u32)> {
    let mut split_index = label.len();
    for (idx, ch) in label.char_indices().rev() {
        if ch.is_ascii_digit() {
            split_index = idx;
        } else {
            break;
        }
    }
    if split_index == label.len() {
        return None;
    }
    let (prefix, digits) = label.split_at(split_index);
    let number = digits.parse::<u32>().ok()?;
    Some((prefix.to_string(), number))
}
