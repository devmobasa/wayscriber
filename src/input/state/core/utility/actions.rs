use super::super::base::InputState;
use crate::config::{KeyBinding, PointerButton, PointerTrigger, Shortcut, ShortcutTrigger};
#[cfg(feature = "tablet-input")]
use crate::config::{StylusButton, StylusTrigger};
use crate::domain::Action;
use crate::input::state::core::utility::{SequenceMatch, SequenceTrie};
use crate::label_format::format_binding_labels;
use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

impl InputState {
    /// Look up a complete single keyboard chord. Does not advance sequences.
    pub(crate) fn find_action(&self, key_str: &str) -> Option<Action> {
        let shortcut = Shortcut::Single(ShortcutTrigger::Keyboard(KeyBinding {
            key: key_str.to_string(),
            ctrl: self.modifiers.ctrl,
            shift: self.modifiers.shift,
            alt: self.modifiers.alt,
            logo: self.modifiers.logo,
        }));
        self.action_map.get(&shortcut).copied()
    }

    pub(crate) fn find_trigger_action(&self, trigger: &ShortcutTrigger) -> Option<Action> {
        self.action_map
            .get(&Shortcut::Single(trigger.clone()))
            .copied()
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

    pub fn set_action_bindings(&mut self, action_bindings: HashMap<Action, Vec<Shortcut>>) {
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
        action_map: HashMap<Shortcut, Action>,
        action_bindings: HashMap<Action, Vec<Shortcut>>,
    ) {
        self.action_map = action_map;
        self.action_bindings = action_bindings;
        self.sequence_trie = SequenceTrie::from_action_map(&self.action_map);
        self.clear_pending_sequence();
        self.keymap_revision = self.keymap_revision.wrapping_add(1);
        self.needs_redraw = true;
    }

    pub(crate) fn clear_pending_sequence(&mut self) {
        self.pending_sequence = None;
    }

    /// Match a keyboard chord, advancing or dispatching a sequence when needed.
    pub(crate) fn match_keyboard_chord(
        &mut self,
        key_str: &str,
        is_repeat: bool,
        now: Instant,
    ) -> SequenceMatch {
        let chord = KeyBinding {
            key: key_str.to_string(),
            ctrl: self.modifiers.ctrl,
            shift: self.modifiers.shift,
            alt: self.modifiers.alt,
            logo: self.modifiers.logo,
        };
        self.sequence_trie
            .match_chord(&mut self.pending_sequence, &chord, now, is_repeat)
    }

    pub(crate) fn sequence_timeout(&self, now: Instant) -> Option<Duration> {
        self.pending_sequence
            .as_ref()
            .map(|pending| pending.deadline().saturating_duration_since(now))
    }

    pub(crate) fn expire_pending_sequence(&mut self, now: Instant) -> bool {
        let expired = self
            .pending_sequence
            .as_ref()
            .is_some_and(|pending| now >= pending.deadline());
        if expired {
            self.clear_pending_sequence();
        }
        expired
    }

    pub fn action_binding_labels(&self, action: Action) -> Vec<String> {
        if let Some(bindings) = self.action_bindings.get(&action) {
            let mut labels = Vec::new();
            let mut seen = HashSet::new();
            for binding in bindings {
                let label = binding.display_label();
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
            .map(|(binding, _)| binding.display_label())
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
