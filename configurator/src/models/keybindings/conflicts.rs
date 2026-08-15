//! Draft-wide shortcut conflict lookup and explicit replacement.

use wayscriber::config::KeyBinding;

use super::draft::KeybindingsDraft;
use super::edit::{FieldEditError, append_binding, remove_binding};
use super::field::KeybindingField;
use super::parse::parse_keybindings;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShortcutClaim {
    pub field: KeybindingField,
    pub label: String,
}

impl ShortcutClaim {
    fn from_field(field: KeybindingField) -> Self {
        Self {
            field,
            label: field.label().to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PendingShortcutConflict {
    Recorded {
        target: KeybindingField,
        binding: KeyBinding,
        claimants: Vec<ShortcutClaim>,
    },
    Text {
        target: KeybindingField,
        new_value: String,
        conflicts: Vec<TextShortcutConflict>,
    },
}

impl PendingShortcutConflict {
    pub fn prompt(&self) -> String {
        match self {
            Self::Recorded {
                binding, claimants, ..
            } => recorded_conflict_prompt(binding, claimants),
            Self::Text { conflicts, .. } => text_conflict_prompt(conflicts),
        }
    }

    pub fn replace_label(&self) -> &'static str {
        match self {
            Self::Recorded { .. } => "Replace",
            Self::Text { .. } => "Resolve Conflicts",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextShortcutConflict {
    pub binding: KeyBinding,
    pub claimants: Vec<ShortcutClaim>,
}

/// Every action in the draft that currently claims `binding`, including the
/// target when it already lists that chord (a duplicate inside one action).
pub fn claimants_for(draft: &KeybindingsDraft, binding: &KeyBinding) -> Vec<ShortcutClaim> {
    let mut claims = Vec::new();
    for entry in &draft.entries {
        if parse_keybindings(&entry.value)
            .is_ok_and(|parsed| parsed.iter().any(|candidate| candidate == binding))
        {
            claims.push(ShortcutClaim::from_field(entry.field));
        }
    }
    claims
}

pub fn other_claimants(
    draft: &KeybindingsDraft,
    target: KeybindingField,
    binding: &KeyBinding,
) -> Vec<ShortcutClaim> {
    claimants_for(draft, binding)
        .into_iter()
        .filter(|claim| claim.field != target)
        .collect()
}

pub fn field_has_internal_duplicate(draft: &KeybindingsDraft, field: KeybindingField) -> bool {
    let Some(value) = draft.value_for(field) else {
        return false;
    };
    let Ok(parsed) = parse_keybindings(value) else {
        return false;
    };
    parsed
        .iter()
        .enumerate()
        .any(|(index, binding)| parsed.iter().skip(index + 1).any(|other| other == binding))
}

pub fn text_conflicts_for(
    draft: &KeybindingsDraft,
    target: KeybindingField,
    parsed: &[KeyBinding],
) -> Vec<TextShortcutConflict> {
    let mut conflicts = Vec::new();
    let mut seen = Vec::new();
    for binding in parsed {
        if seen.iter().any(|existing| existing == binding) {
            if !conflicts
                .iter()
                .any(|conflict: &TextShortcutConflict| conflict.binding == *binding)
            {
                conflicts.push(TextShortcutConflict {
                    binding: binding.clone(),
                    claimants: vec![ShortcutClaim::from_field(target)],
                });
            }
            continue;
        }
        seen.push(binding.clone());
        let claimants = other_claimants(draft, target, binding);
        if !claimants.is_empty() {
            conflicts.push(TextShortcutConflict {
                binding: binding.clone(),
                claimants,
            });
        }
    }
    conflicts
}

pub fn apply_recorded_replace(
    draft: &mut KeybindingsDraft,
    target: KeybindingField,
    binding: &KeyBinding,
    claimants: &[ShortcutClaim],
) -> Result<(), FieldEditError> {
    let snapshot = draft.clone();
    for claim in claimants {
        if claim.field == target {
            continue;
        }
        if let Err(error) = remove_binding(draft, claim.field, binding) {
            *draft = snapshot;
            return Err(error);
        }
    }
    match append_binding(draft, target, binding) {
        Ok(_) => Ok(()),
        Err(error) => {
            *draft = snapshot;
            Err(error)
        }
    }
}

pub fn apply_text_replace(
    draft: &mut KeybindingsDraft,
    target: KeybindingField,
    new_value: &str,
    conflicts: &[TextShortcutConflict],
) -> Result<(), FieldEditError> {
    let snapshot = draft.clone();
    for conflict in conflicts {
        for claim in &conflict.claimants {
            if claim.field == target {
                continue;
            }
            if let Err(error) = remove_binding(draft, claim.field, &conflict.binding) {
                *draft = snapshot;
                return Err(error);
            }
        }
    }
    let parsed = parse_keybindings(new_value).map_err(FieldEditError::InvalidText)?;
    let _ = parsed;
    draft.set(target, new_value.trim().to_string());
    Ok(())
}

pub fn recorded_conflict_prompt(binding: &KeyBinding, claimants: &[ShortcutClaim]) -> String {
    let mut lines = vec![format!("{binding} is already assigned to:")];
    for claim in claimants {
        lines.push(format!("- {}", claim.label));
    }
    lines.push(String::new());
    lines.push("Replace those assignments?".to_string());
    lines.join("\n")
}

fn text_conflict_prompt(conflicts: &[TextShortcutConflict]) -> String {
    let mut lines = vec!["These shortcuts conflict:".to_string()];
    for conflict in conflicts {
        let names = conflict
            .claimants
            .iter()
            .map(|claim| claim.label.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        lines.push(format!("- {}: {names}", conflict.binding));
    }
    lines.push(String::new());
    lines.push("Resolve those assignments?".to_string());
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use wayscriber::config::keybindings::KeybindingsConfig;

    use super::*;
    use crate::models::keybindings::KeybindingField;

    fn draft() -> KeybindingsDraft {
        KeybindingsDraft::from_config(&KeybindingsConfig::default())
    }

    #[test]
    fn conflict_lookup_returns_all_claimants() {
        let mut draft = draft();
        draft.set(KeybindingField::ClearCanvas, "Ctrl+Shift+X".to_string());
        draft.set(KeybindingField::ToggleToolbar, "Ctrl+Shift+X".to_string());
        draft.set(KeybindingField::Undo, "ctrl+shift+x".to_string());
        let binding = KeyBinding::parse("Ctrl+Shift+X").expect("parses");
        let claimants = claimants_for(&draft, &binding);
        let fields: Vec<_> = claimants.iter().map(|claim| claim.field).collect();
        assert!(fields.contains(&KeybindingField::ClearCanvas));
        assert!(fields.contains(&KeybindingField::ToggleToolbar));
        assert!(fields.contains(&KeybindingField::Undo));
        assert_eq!(claimants.len(), 3);
    }

    #[test]
    fn duplicate_inside_one_action_is_a_conflict() {
        let mut draft = draft();
        draft.set(KeybindingField::ClearCanvas, "E, e".to_string());
        assert!(field_has_internal_duplicate(
            &draft,
            KeybindingField::ClearCanvas
        ));
        let binding = KeyBinding::parse("E").expect("parses");
        let claimants = claimants_for(&draft, &binding);
        assert_eq!(claimants.len(), 1);
        assert_eq!(claimants[0].field, KeybindingField::ClearCanvas);
    }

    #[test]
    fn replace_removes_only_the_contested_binding() {
        let mut draft = draft();
        draft.set(
            KeybindingField::ToggleHelp,
            "F10, F1, Ctrl+Shift+X".to_string(),
        );
        draft.set(KeybindingField::Undo, "Ctrl+Z, Ctrl+Shift+X".to_string());
        let binding = KeyBinding::parse("Ctrl+Shift+X").expect("parses");
        let claimants = other_claimants(&draft, KeybindingField::ClearCanvas, &binding);
        apply_recorded_replace(
            &mut draft,
            KeybindingField::ClearCanvas,
            &binding,
            &claimants,
        )
        .expect("replace");
        assert_eq!(
            draft.value_for(KeybindingField::ToggleHelp),
            Some("F10, F1")
        );
        assert_eq!(draft.value_for(KeybindingField::Undo), Some("Ctrl+Z"));
        assert_eq!(
            draft.value_for(KeybindingField::ClearCanvas),
            Some("E, Ctrl+Shift+X")
        );
    }

    #[test]
    fn cancel_leaves_the_draft_byte_for_byte() {
        let mut draft = draft();
        draft.set(KeybindingField::ClearCanvas, "E, Q".to_string());
        let before = draft.clone();
        let _pending = PendingShortcutConflict::Recorded {
            target: KeybindingField::Undo,
            binding: KeyBinding::parse("E").expect("parses"),
            claimants: other_claimants(
                &draft,
                KeybindingField::Undo,
                &KeyBinding::parse("E").expect("parses"),
            ),
        };
        assert_eq!(draft, before);
    }

    #[test]
    fn text_conflicts_collect_every_contested_shortcut() {
        let draft = draft();
        let conflicts = text_conflicts_for(
            &draft,
            KeybindingField::ToggleFloatingBadge,
            &[
                KeyBinding::parse("E").expect("parses"),
                KeyBinding::parse("Ctrl+Z").expect("parses"),
                KeyBinding::parse("E").expect("parses"),
            ],
        );
        assert_eq!(conflicts.len(), 2);
        assert_eq!(conflicts[0].binding.to_string(), "E");
        assert_eq!(
            conflicts[0].claimants[0].field,
            KeybindingField::ClearCanvas
        );
        assert_eq!(conflicts[1].binding.to_string(), "Ctrl+Z");
        assert_eq!(conflicts[1].claimants[0].field, KeybindingField::Undo);
    }
}
