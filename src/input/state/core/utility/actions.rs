use super::super::base::InputState;
use crate::config::{PointerButton, PointerTrigger, Shortcut, ShortcutTrigger};
#[cfg(feature = "tablet-input")]
use crate::config::{StylusButton, StylusTrigger};
use crate::domain::Action;
use crate::input::state::core::utility::SequenceMatch;
use crate::label_format::format_binding_labels;
use std::collections::HashMap;
use std::time::{Duration, Instant};

impl InputState {
    /// Look up a complete single keyboard chord. Does not advance sequences.
    pub(crate) fn find_action(&self, key_str: &str) -> Option<Action> {
        self.keymap.find_action(key_str, self.modifiers)
    }

    pub(crate) fn find_trigger_action(&self, trigger: &ShortcutTrigger) -> Option<Action> {
        self.keymap.find_trigger_action(trigger)
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
        self.keymap.consume_pointer_button(code);
    }

    pub(crate) fn take_consumed_pointer_shortcut_button(&mut self, code: u32) -> bool {
        self.keymap.take_consumed_pointer_button(code)
    }

    pub fn set_action_bindings(&mut self, action_bindings: HashMap<Action, Vec<Shortcut>>) {
        self.keymap.set_action_bindings(action_bindings);
    }

    pub(crate) fn keymap_revision(&self) -> u64 {
        self.keymap.revision()
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
        self.keymap.set_maps(action_map, action_bindings);
        self.needs_redraw = true;
    }

    pub(crate) fn clear_pending_sequence(&mut self) {
        self.keymap.clear_pending_sequence();
    }

    /// Match a keyboard chord, advancing or dispatching a sequence when needed.
    pub(crate) fn match_keyboard_chord(
        &mut self,
        key_str: &str,
        is_repeat: bool,
        now: Instant,
    ) -> SequenceMatch {
        self.keymap
            .match_keyboard_chord(key_str, self.modifiers, is_repeat, now)
    }

    /// Retry a shifted-punctuation fallback against the pending sequence as it
    /// stood before the primary label mutated it.
    pub(crate) fn match_keyboard_chord_with_fallback(
        &mut self,
        key_str: &str,
        fallback: &str,
        is_repeat: bool,
        now: Instant,
    ) -> SequenceMatch {
        self.keymap.match_keyboard_chord_with_fallback(
            key_str,
            fallback,
            self.modifiers,
            is_repeat,
            now,
        )
    }

    pub(crate) fn sequence_timeout(&self, now: Instant) -> Option<Duration> {
        self.keymap.sequence_timeout(now)
    }

    pub(crate) fn expire_pending_sequence(&mut self, now: Instant) -> bool {
        self.keymap.expire_pending_sequence(now)
    }

    pub fn action_binding_labels(&self, action: Action) -> Vec<String> {
        self.keymap.action_binding_labels(action)
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
