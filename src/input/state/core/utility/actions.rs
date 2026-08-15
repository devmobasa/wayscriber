use super::super::base::InputState;
use crate::config::{KeyBinding, PointerButton, PointerTrigger, ShortcutTrigger};
#[cfg(feature = "tablet-input")]
use crate::config::{StylusButton, StylusTrigger};
use crate::domain::Action;
use crate::label_format::format_binding_labels;
use std::collections::{HashMap, HashSet};

impl InputState {
    /// Look up an action for the given key and modifiers.
    pub(crate) fn find_action(&self, key_str: &str) -> Option<Action> {
        let trigger = ShortcutTrigger::Keyboard(KeyBinding {
            key: key_str.to_string(),
            ctrl: self.modifiers.ctrl,
            shift: self.modifiers.shift,
            alt: self.modifiers.alt,
            logo: self.modifiers.logo,
        });
        self.action_map.get(&trigger).copied()
    }

    pub(crate) fn find_trigger_action(&self, trigger: &ShortcutTrigger) -> Option<Action> {
        self.action_map.get(trigger).copied()
    }

    pub(crate) fn pointer_trigger(&self, button: PointerButton) -> ShortcutTrigger {
        ShortcutTrigger::Pointer(PointerTrigger {
            button,
            ctrl: self.modifiers.ctrl,
            shift: self.modifiers.shift,
            alt: self.modifiers.alt,
            logo: self.modifiers.logo,
        })
    }

    #[cfg(feature = "tablet-input")]
    pub(crate) fn stylus_trigger(&self, button: StylusButton) -> ShortcutTrigger {
        ShortcutTrigger::Stylus(StylusTrigger {
            button,
            ctrl: self.modifiers.ctrl,
            shift: self.modifiers.shift,
            alt: self.modifiers.alt,
            logo: self.modifiers.logo,
        })
    }

    pub(crate) fn consume_pointer_shortcut_button(&mut self, code: u32) {
        self.consumed_pointer_buttons.insert(code);
    }

    pub(crate) fn take_consumed_pointer_shortcut_button(&mut self, code: u32) -> bool {
        self.consumed_pointer_buttons.remove(&code)
    }

    pub fn set_action_bindings(&mut self, action_bindings: HashMap<Action, Vec<ShortcutTrigger>>) {
        self.action_bindings = action_bindings;
        self.keymap_revision = self.keymap_revision.wrapping_add(1);
    }

    /// Install a keymap rebuilt after a shortcut edit.
    ///
    /// Both halves move together because they are two views of one binding
    /// table: `action_map` dispatches a chord, `action_bindings` is what every
    /// badge and help row reads back.
    pub(crate) fn set_keybinding_maps(
        &mut self,
        action_map: HashMap<ShortcutTrigger, Action>,
        action_bindings: HashMap<Action, Vec<ShortcutTrigger>>,
    ) {
        self.action_map = action_map;
        self.action_bindings = action_bindings;
        self.keymap_revision = self.keymap_revision.wrapping_add(1);
        self.needs_redraw = true;
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
        format_binding_labels(&self.action_binding_labels(action))
    }
}
