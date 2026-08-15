//! Keyboard sequence matcher for configured multi-step shortcuts.
//!
//! Single chords dispatch immediately. A sequence prefix is consumed and kept
//! pending until the next step, a mismatch (re-evaluated once from the root),
//! a one-second timeout, or an ownership boundary that clears pending state.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use crate::config::{KeyBinding, Shortcut};
use crate::domain::Action;

/// Inter-step timeout. A pending prefix expires without dispatching.
pub(crate) const SEQUENCE_STEP_TIMEOUT: Duration = Duration::from_secs(1);

#[derive(Debug, Clone)]
pub(crate) struct PendingSequence {
    steps: Vec<KeyBinding>,
    deadline: Instant,
}

impl PendingSequence {
    fn new(steps: Vec<KeyBinding>, now: Instant) -> Self {
        Self {
            steps,
            deadline: now + SEQUENCE_STEP_TIMEOUT,
        }
    }

    pub(crate) fn deadline(&self) -> Instant {
        self.deadline
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SequenceMatch {
    Dispatched(Action),
    Pending,
    None,
}

#[derive(Debug, Default, Clone)]
struct TrieNode {
    children: HashMap<KeyBinding, TrieNode>,
    action: Option<Action>,
}

/// Keyboard trie built from the current keymap.
#[derive(Debug, Default, Clone)]
pub(crate) struct SequenceTrie {
    root: TrieNode,
}

impl SequenceTrie {
    pub(crate) fn from_action_map(map: &HashMap<Shortcut, Action>) -> Self {
        let mut trie = Self::default();
        for (shortcut, action) in map {
            let Some(steps) = shortcut.keyboard_steps() else {
                continue;
            };
            trie.insert(steps, *action);
        }
        trie
    }

    fn insert(&mut self, steps: &[KeyBinding], action: Action) {
        let mut node = &mut self.root;
        for step in steps {
            node = node.children.entry(step.clone()).or_default();
        }
        node.action = Some(action);
    }

    fn node_for(&self, steps: &[KeyBinding]) -> Option<&TrieNode> {
        let mut node = &self.root;
        for step in steps {
            node = node.children.get(step)?;
        }
        Some(node)
    }

    pub(crate) fn match_chord(
        &self,
        pending: &mut Option<PendingSequence>,
        chord: &KeyBinding,
        now: Instant,
        is_repeat: bool,
    ) -> SequenceMatch {
        if pending
            .as_ref()
            .is_some_and(|pending| now >= pending.deadline)
        {
            *pending = None;
        }

        if is_repeat {
            if pending.is_some() {
                return SequenceMatch::Pending;
            }
            return self.dispatch_or_pending(std::slice::from_ref(chord), pending, now);
        }

        if let Some(current) = pending.take() {
            let mut steps = current.steps;
            steps.push(chord.clone());
            match self.lookup_steps(&steps, pending, now) {
                SequenceMatch::None => {
                    // Mismatch: cancel, then re-evaluate the new chord once.
                    self.dispatch_or_pending(std::slice::from_ref(chord), pending, now)
                }
                other => other,
            }
        } else {
            self.dispatch_or_pending(std::slice::from_ref(chord), pending, now)
        }
    }

    fn lookup_steps(
        &self,
        steps: &[KeyBinding],
        pending: &mut Option<PendingSequence>,
        now: Instant,
    ) -> SequenceMatch {
        let Some(node) = self.node_for(steps) else {
            return SequenceMatch::None;
        };
        if let Some(action) = node.action {
            *pending = None;
            return SequenceMatch::Dispatched(action);
        }
        if node.children.is_empty() {
            return SequenceMatch::None;
        }
        *pending = Some(PendingSequence::new(steps.to_vec(), now));
        SequenceMatch::Pending
    }

    fn dispatch_or_pending(
        &self,
        steps: &[KeyBinding],
        pending: &mut Option<PendingSequence>,
        now: Instant,
    ) -> SequenceMatch {
        self.lookup_steps(steps, pending, now)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ShortcutTrigger;

    fn chord(text: &str) -> KeyBinding {
        match ShortcutTrigger::parse(text).unwrap() {
            ShortcutTrigger::Keyboard(binding) => binding,
            other => panic!("expected a keyboard chord, got {other:?}"),
        }
    }

    fn trie(entries: &[(&str, Action)]) -> SequenceTrie {
        let map = entries
            .iter()
            .map(|(text, action)| (Shortcut::parse(text).unwrap(), *action))
            .collect();
        SequenceTrie::from_action_map(&map)
    }

    fn start() -> Instant {
        Instant::now()
    }

    #[test]
    fn idle_complete_single_dispatches_immediately() {
        let trie = trie(&[("F5", Action::SelectPenTool)]);
        let mut pending = None;
        let now = start();
        assert_eq!(
            trie.match_chord(&mut pending, &chord("F5"), now, false),
            SequenceMatch::Dispatched(Action::SelectPenTool)
        );
        assert!(pending.is_none());
    }

    #[test]
    fn prefix_enters_pending_and_matching_next_step_dispatches() {
        let trie = trie(&[("Ctrl+K > Ctrl+C", Action::CopySelection)]);
        let mut pending = None;
        let now = start();
        assert_eq!(
            trie.match_chord(&mut pending, &chord("Ctrl+K"), now, false),
            SequenceMatch::Pending
        );
        assert!(pending.is_some());
        assert_eq!(
            trie.match_chord(&mut pending, &chord("Ctrl+C"), now, false),
            SequenceMatch::Dispatched(Action::CopySelection)
        );
        assert!(pending.is_none());
    }

    #[test]
    fn mismatch_re_evaluates_once_from_the_root() {
        let trie = trie(&[
            ("Ctrl+K > Ctrl+C", Action::CopySelection),
            ("F5", Action::SelectPenTool),
        ]);
        let mut pending = None;
        let now = start();
        assert_eq!(
            trie.match_chord(&mut pending, &chord("Ctrl+K"), now, false),
            SequenceMatch::Pending
        );
        assert_eq!(
            trie.match_chord(&mut pending, &chord("F5"), now, false),
            SequenceMatch::Dispatched(Action::SelectPenTool)
        );
        assert!(pending.is_none());
    }

    #[test]
    fn timeout_cancels_without_dispatch() {
        let trie = trie(&[("Ctrl+K > Ctrl+C", Action::CopySelection)]);
        let mut pending = None;
        let now = start();
        assert_eq!(
            trie.match_chord(&mut pending, &chord("Ctrl+K"), now, false),
            SequenceMatch::Pending
        );
        let later = now + SEQUENCE_STEP_TIMEOUT;
        assert_eq!(
            trie.match_chord(&mut pending, &chord("Ctrl+C"), later, false),
            SequenceMatch::None
        );
        assert!(pending.is_none());
    }

    #[test]
    fn key_repeat_does_not_advance_a_pending_sequence() {
        let trie = trie(&[("Ctrl+K > Ctrl+C", Action::CopySelection)]);
        let mut pending = None;
        let now = start();
        assert_eq!(
            trie.match_chord(&mut pending, &chord("Ctrl+K"), now, false),
            SequenceMatch::Pending
        );
        assert_eq!(
            trie.match_chord(&mut pending, &chord("Ctrl+K"), now, true),
            SequenceMatch::Pending
        );
        assert!(pending.is_some());
        assert_eq!(
            trie.match_chord(&mut pending, &chord("Ctrl+C"), now, true),
            SequenceMatch::Pending
        );
        assert_eq!(
            trie.match_chord(&mut pending, &chord("Ctrl+C"), now, false),
            SequenceMatch::Dispatched(Action::CopySelection)
        );
    }

    #[test]
    fn three_step_sequence_advances_then_dispatches() {
        let trie = trie(&[("Ctrl+K > Ctrl+C > Ctrl+V", Action::PasteSelection)]);
        let mut pending = None;
        let now = start();
        assert_eq!(
            trie.match_chord(&mut pending, &chord("Ctrl+K"), now, false),
            SequenceMatch::Pending
        );
        assert_eq!(
            trie.match_chord(&mut pending, &chord("Ctrl+C"), now, false),
            SequenceMatch::Pending
        );
        assert_eq!(
            trie.match_chord(&mut pending, &chord("Ctrl+V"), now, false),
            SequenceMatch::Dispatched(Action::PasteSelection)
        );
    }
}
