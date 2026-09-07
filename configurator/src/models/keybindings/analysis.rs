//! Parsed values live for one analysis pass; edits never invalidate a persistent cache.
use super::conflicts::{ShortcutClaim, legacy_action_for, shortcuts_conflict};
use super::{KeybindingField, KeybindingsDraft, parse_keybindings};
use wayscriber::config::Shortcut;

pub(super) struct ParsedShortcuts<'a> {
    pub(super) draft: &'a KeybindingsDraft,
    entries: Vec<(KeybindingField, Result<Vec<Shortcut>, String>)>,
}

impl<'a> ParsedShortcuts<'a> {
    pub(super) fn new(draft: &'a KeybindingsDraft) -> Self {
        Self {
            draft,
            entries: draft
                .entries
                .iter()
                .map(|entry| (entry.field, parse_keybindings(&entry.value)))
                .collect(),
        }
    }

    pub(super) fn bindings(&self, field: KeybindingField) -> Result<&[Shortcut], &str> {
        self.entries
            .iter()
            .find(|(candidate, _)| *candidate == field)
            .map(|(_, parsed)| parsed.as_ref().map(Vec::as_slice).map_err(String::as_str))
            .unwrap_or(Ok(&[]))
    }

    pub(super) fn claimants(&self, binding: &Shortcut) -> Vec<ShortcutClaim> {
        let mut claims = Vec::new();
        for (field, parsed) in &self.entries {
            if let Ok(parsed) = parsed {
                for candidate in parsed {
                    if shortcuts_conflict(candidate, binding)
                        && !claims.iter().any(|claim: &ShortcutClaim| {
                            claim.field == Some(*field) && claim.held == *candidate
                        })
                    {
                        claims.push(ShortcutClaim::from_field(*field, candidate.clone()));
                    }
                }
            }
        }
        if let Some(action) = legacy_action_for(self.draft, binding) {
            claims.push(ShortcutClaim::legacy_tablet(action, binding.clone()));
        }
        claims
    }

    pub(super) fn other_claimants(
        &self,
        field: KeybindingField,
        binding: &Shortcut,
    ) -> Vec<ShortcutClaim> {
        self.claimants(binding)
            .into_iter()
            .filter(|claim| claim.field != Some(field) || claim.held != *binding)
            .collect()
    }

    pub(super) fn has_internal_duplicate(&self, field: KeybindingField) -> bool {
        self.bindings(field).is_ok_and(|bindings| {
            bindings.iter().enumerate().any(|(index, binding)| {
                bindings[index + 1..]
                    .iter()
                    .any(|other| shortcuts_conflict(binding, other))
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::super::keybinding_fields;
    use super::*;
    #[test]
    fn summary_parses_each_field_once_per_draft_even_with_many_conflicts() {
        let defaults =
            KeybindingsDraft::from_config(&wayscriber::config::KeybindingsConfig::default());
        let mut draft = defaults.clone();
        for field in keybinding_fields() {
            draft.set(field, "Ctrl+Q, Ctrl+Q > C, Ctrl+Q".into());
        }
        super::super::take_parse_calls();
        let summary = super::super::ShortcutManagerSummary::from_drafts(&draft, &defaults);
        assert_eq!(
            super::super::take_parse_calls(),
            2 * keybinding_fields().len()
        );
        assert!(summary.rows().iter().all(|row| row.has_conflict));
    }
}
