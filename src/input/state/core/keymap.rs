//! Keyboard shortcuts, sequence matching, and pointer-drag bindings.

use crate::config::{
    Action, KeyBinding, KeybindingsConfig, MouseDragToolsConfig, Shortcut, ShortcutTrigger,
};
use crate::draw::Color;
use crate::input::state::core::utility::{PendingSequence, SequenceMatch, SequenceTrie};
use crate::input::{DragToolBindings, Modifiers, MouseButton};
use log::warn;
use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

/// Configured keybindings plus transient keyboard and pointer-dispatch state.
#[derive(Debug, Clone)]
pub(in crate::input::state) struct Keymap {
    action_map: HashMap<Shortcut, Action>,
    action_bindings: HashMap<Action, Vec<Shortcut>>,
    sequence_trie: SequenceTrie,
    pending_sequence: Option<PendingSequence>,
    keymap_revision: u64,
    drag_tool_bindings: DragToolBindings,
    consumed_pointer_buttons: HashSet<u32>,
    keybinding_capture_action: Option<Action>,
    active_drag_button: Option<MouseButton>,
    active_drag_color: Option<Color>,
}

impl Keymap {
    pub(in crate::input::state) fn from_config(
        keybindings: &KeybindingsConfig,
        drag_tools: &MouseDragToolsConfig,
    ) -> Self {
        let action_map = build_action_map(keybindings);
        let action_bindings = build_action_bindings(keybindings);
        Self::from_maps(
            action_map,
            action_bindings,
            DragToolBindings::from_config(drag_tools),
        )
    }

    pub(in crate::input::state) fn from_maps(
        action_map: HashMap<Shortcut, Action>,
        action_bindings: HashMap<Action, Vec<Shortcut>>,
        drag_tool_bindings: DragToolBindings,
    ) -> Self {
        let sequence_trie = SequenceTrie::from_action_map(&action_map);
        Self {
            action_map,
            action_bindings,
            sequence_trie,
            pending_sequence: None,
            keymap_revision: 0,
            drag_tool_bindings,
            consumed_pointer_buttons: HashSet::new(),
            keybinding_capture_action: None,
            active_drag_button: None,
            active_drag_color: None,
        }
    }

    pub(in crate::input::state) fn find_action(
        &self,
        key_str: &str,
        modifiers: Modifiers,
    ) -> Option<Action> {
        let shortcut = Shortcut::Single(ShortcutTrigger::Keyboard(KeyBinding {
            key: key_str.to_string(),
            ctrl: modifiers.ctrl,
            shift: modifiers.shift,
            alt: modifiers.alt,
            logo: modifiers.logo,
        }));
        self.action_map.get(&shortcut).copied()
    }

    pub(in crate::input::state) fn find_trigger_action(
        &self,
        trigger: &ShortcutTrigger,
    ) -> Option<Action> {
        self.action_map
            .get(&Shortcut::Single(trigger.clone()))
            .copied()
    }

    pub(in crate::input::state) fn revision(&self) -> u64 {
        self.keymap_revision
    }

    #[cfg(test)]
    pub(in crate::input::state) fn set_action_map(
        &mut self,
        action_map: HashMap<Shortcut, Action>,
    ) {
        self.action_map = action_map;
        self.sequence_trie = SequenceTrie::from_action_map(&self.action_map);
        self.pending_sequence = None;
    }

    pub(in crate::input::state) fn set_action_bindings(
        &mut self,
        action_bindings: HashMap<Action, Vec<Shortcut>>,
    ) {
        self.action_bindings = action_bindings;
        self.keymap_revision = self.keymap_revision.wrapping_add(1);
    }

    pub(in crate::input::state) fn set_maps(
        &mut self,
        action_map: HashMap<Shortcut, Action>,
        action_bindings: HashMap<Action, Vec<Shortcut>>,
    ) {
        self.action_map = action_map;
        self.action_bindings = action_bindings;
        self.sequence_trie = SequenceTrie::from_action_map(&self.action_map);
        self.pending_sequence = None;
        self.keymap_revision = self.keymap_revision.wrapping_add(1);
    }

    pub(in crate::input::state) fn clear_pending_sequence(&mut self) {
        self.pending_sequence = None;
    }

    pub(in crate::input::state) fn consume_pointer_button(&mut self, code: u32) {
        self.consumed_pointer_buttons.insert(code);
    }

    pub(in crate::input::state) fn take_consumed_pointer_button(&mut self, code: u32) -> bool {
        self.consumed_pointer_buttons.remove(&code)
    }

    pub(in crate::input::state) fn clear_consumed_pointer_buttons(&mut self) {
        self.consumed_pointer_buttons.clear();
    }

    pub(in crate::input::state) fn drag_tool_bindings(&self) -> DragToolBindings {
        self.drag_tool_bindings
    }

    pub(in crate::input::state) fn set_drag_tool_bindings(
        &mut self,
        bindings: DragToolBindings,
    ) -> bool {
        if self.drag_tool_bindings == bindings {
            return false;
        }
        self.drag_tool_bindings = bindings;
        true
    }

    pub(in crate::input::state) fn drag_binding_for_button(
        &self,
        button: MouseButton,
        modifiers: Modifiers,
    ) -> crate::input::DragBinding {
        self.drag_tool_bindings
            .binding_for_button_modifier(button, modifiers.active_drag_modifier())
    }

    pub(in crate::input::state) fn begin_pointer_drag(
        &mut self,
        button: MouseButton,
        color: Option<Color>,
    ) {
        self.active_drag_button = Some(button);
        self.active_drag_color = color;
    }

    pub(in crate::input::state) fn end_pointer_drag(&mut self) {
        self.active_drag_button = None;
        self.active_drag_color = None;
    }

    pub(in crate::input::state) fn pointer_drag_active(&self) -> bool {
        self.active_drag_button.is_some()
    }

    pub(in crate::input::state) fn pointer_drag_button_matches(&self, button: MouseButton) -> bool {
        self.active_drag_button == Some(button)
    }

    pub(in crate::input::state) fn active_drag_color(&self) -> Option<Color> {
        self.active_drag_color
    }

    pub(in crate::input::state) fn active_drag_state(
        &self,
    ) -> (Option<MouseButton>, Option<Color>) {
        (self.active_drag_button, self.active_drag_color)
    }

    pub(in crate::input::state) fn restore_active_drag_state(
        &mut self,
        button: Option<MouseButton>,
        color: Option<Color>,
    ) {
        self.active_drag_button = button;
        self.active_drag_color = color;
    }

    pub(in crate::input::state) fn capture_action(&self) -> Option<Action> {
        self.keybinding_capture_action
    }

    pub(in crate::input::state) fn begin_capture(&mut self, action: Action) {
        self.keybinding_capture_action = Some(action);
    }

    pub(in crate::input::state) fn clear_capture(&mut self) {
        self.keybinding_capture_action = None;
    }

    pub(in crate::input::state) fn take_capture(&mut self) -> Option<Action> {
        self.keybinding_capture_action.take()
    }

    pub(in crate::input::state) fn match_keyboard_chord(
        &mut self,
        key_str: &str,
        modifiers: Modifiers,
        is_repeat: bool,
        now: Instant,
    ) -> SequenceMatch {
        let chord = KeyBinding {
            key: key_str.to_string(),
            ctrl: modifiers.ctrl,
            shift: modifiers.shift,
            alt: modifiers.alt,
            logo: modifiers.logo,
        };
        self.sequence_trie
            .match_chord(&mut self.pending_sequence, &chord, now, is_repeat)
    }

    pub(in crate::input::state) fn match_keyboard_chord_with_fallback(
        &mut self,
        key_str: &str,
        fallback: &str,
        modifiers: Modifiers,
        is_repeat: bool,
        now: Instant,
    ) -> SequenceMatch {
        let snapshot = self.pending_sequence.clone();
        match self.match_keyboard_chord(key_str, modifiers, is_repeat, now) {
            SequenceMatch::None => {
                self.pending_sequence = snapshot;
                self.match_keyboard_chord(fallback, modifiers, is_repeat, now)
            }
            other => other,
        }
    }

    pub(in crate::input::state) fn sequence_timeout(&self, now: Instant) -> Option<Duration> {
        self.pending_sequence
            .as_ref()
            .map(|pending| pending.deadline().saturating_duration_since(now))
    }

    pub(in crate::input::state) fn expire_pending_sequence(&mut self, now: Instant) -> bool {
        let expired = self
            .pending_sequence
            .as_ref()
            .is_some_and(|pending| now >= pending.deadline());
        if expired {
            self.pending_sequence = None;
        }
        expired
    }

    pub(in crate::input::state) fn action_binding_labels(&self, action: Action) -> Vec<String> {
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

    #[cfg(test)]
    pub(in crate::input::state) fn action_map_for_test(&self) -> &HashMap<Shortcut, Action> {
        &self.action_map
    }

    #[cfg(test)]
    pub(in crate::input::state) fn action_bindings_for_test(
        &self,
    ) -> &HashMap<Action, Vec<Shortcut>> {
        &self.action_bindings
    }
}

fn build_action_map(keybindings: &KeybindingsConfig) -> HashMap<Shortcut, Action> {
    match keybindings.build_action_map() {
        Ok(map) => map,
        Err(error) => {
            warn!("Invalid keybindings config: {error}. Falling back to defaults.");
            KeybindingsConfig::default()
                .build_action_map()
                .unwrap_or_else(|error| {
                    warn!(
                        "Failed to build default keybindings: {error}. Continuing with no bindings."
                    );
                    HashMap::new()
                })
        }
    }
}

fn build_action_bindings(keybindings: &KeybindingsConfig) -> HashMap<Action, Vec<Shortcut>> {
    match keybindings.build_action_bindings() {
        Ok(map) => map,
        Err(error) => {
            warn!("Invalid keybindings config: {error}. Falling back to defaults.");
            KeybindingsConfig::default()
                .build_action_bindings()
                .unwrap_or_else(|error| {
                    warn!(
                        "Failed to build default keybindings: {error}. Continuing with no bindings."
                    );
                    HashMap::new()
                })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Keymap;
    use crate::config::{KeybindingsConfig, MouseDragToolsConfig, Shortcut};
    use crate::domain::Action;
    use crate::draw::RED;
    use crate::input::state::core::utility::SequenceMatch;
    use crate::input::{Modifiers, MouseButton};
    use std::collections::HashMap;
    use std::time::{Duration, Instant};

    #[test]
    fn sequence_matching_expires_the_pending_prefix_without_dispatching() {
        let mut keybindings = KeybindingsConfig::default();
        keybindings.ui.toggle_floating_badge =
            vec!["Ctrl+Alt+Shift+K > Ctrl+Alt+Shift+C".to_string()];
        let mut keymap = Keymap::from_config(&keybindings, &MouseDragToolsConfig::default());
        let modifiers = Modifiers {
            ctrl: true,
            alt: true,
            shift: true,
            ..Modifiers::default()
        };
        let now = Instant::now();

        assert_eq!(
            keymap.match_keyboard_chord("k", modifiers, false, now),
            SequenceMatch::Pending
        );
        let timeout = Duration::from_secs(1);
        assert_eq!(keymap.sequence_timeout(now), Some(timeout));
        assert!(keymap.expire_pending_sequence(now + timeout));
        assert_eq!(
            keymap.match_keyboard_chord("c", modifiers, false, now + timeout),
            SequenceMatch::None
        );
        assert_ne!(
            keymap.match_keyboard_chord("k", modifiers, false, now),
            SequenceMatch::Dispatched(Action::ToggleFloatingBadge)
        );
    }

    #[test]
    fn installing_rebuilt_maps_bumps_revision_and_clears_pending_sequence() {
        let mut keybindings = KeybindingsConfig::default();
        keybindings.ui.toggle_floating_badge =
            vec!["Ctrl+Alt+Shift+K > Ctrl+Alt+Shift+C".to_string()];
        let mut keymap = Keymap::from_config(&keybindings, &MouseDragToolsConfig::default());
        let modifiers = Modifiers {
            ctrl: true,
            alt: true,
            shift: true,
            ..Modifiers::default()
        };
        let now = Instant::now();
        assert_eq!(
            keymap.match_keyboard_chord("k", modifiers, false, now),
            SequenceMatch::Pending
        );
        let initial_revision = keymap.revision();
        let replacement = HashMap::from([(
            Shortcut::parse("F6").expect("replacement shortcut"),
            Action::ToggleHelp,
        )]);

        keymap.set_maps(replacement, HashMap::new());

        assert_eq!(keymap.revision(), initial_revision.wrapping_add(1));
        assert_eq!(
            keymap.match_keyboard_chord("c", modifiers, false, now),
            SequenceMatch::None
        );
        assert_eq!(
            keymap.find_action("F6", Modifiers::default()),
            Some(Action::ToggleHelp)
        );
    }

    #[test]
    fn consumed_pointer_buttons_are_taken_once_and_clear_together() {
        let mut keymap = Keymap::from_config(
            &KeybindingsConfig::default(),
            &MouseDragToolsConfig::default(),
        );

        keymap.consume_pointer_button(0x113);
        assert!(keymap.take_consumed_pointer_button(0x113));
        assert!(!keymap.take_consumed_pointer_button(0x113));

        keymap.consume_pointer_button(0x113);
        keymap.consume_pointer_button(0x114);
        keymap.clear_consumed_pointer_buttons();
        assert!(!keymap.take_consumed_pointer_button(0x113));
        assert!(!keymap.take_consumed_pointer_button(0x114));
    }

    #[test]
    fn pointer_drag_keeps_button_and_color_together_until_end() {
        let mut keymap = Keymap::from_config(
            &KeybindingsConfig::default(),
            &MouseDragToolsConfig::default(),
        );

        keymap.begin_pointer_drag(MouseButton::Right, Some(RED));

        assert!(keymap.pointer_drag_button_matches(MouseButton::Right));
        assert!(!keymap.pointer_drag_button_matches(MouseButton::Left));
        assert_eq!(keymap.active_drag_color(), Some(RED));

        keymap.end_pointer_drag();
        assert!(!keymap.pointer_drag_button_matches(MouseButton::Right));
        assert_eq!(keymap.active_drag_color(), None);
    }
}
