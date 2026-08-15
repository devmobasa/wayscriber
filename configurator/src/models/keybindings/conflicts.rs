//! Draft-wide shortcut conflict lookup and explicit replacement.

use wayscriber::config::{Action, Shortcut, ShortcutTrigger, StylusButton, action_label};

use super::draft::KeybindingsDraft;
use super::edit::{FieldEditError, append_binding, remove_binding};
use super::field::KeybindingField;
use super::parse::parse_keybindings;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BindingSource {
    Keybindings,
    LegacyTablet,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShortcutClaim {
    pub field: Option<KeybindingField>,
    pub label: String,
    pub source: BindingSource,
    pub held: Shortcut,
}

impl ShortcutClaim {
    fn from_field(field: KeybindingField, held: Shortcut) -> Self {
        Self {
            field: Some(field),
            label: field.label().to_string(),
            source: BindingSource::Keybindings,
            held,
        }
    }

    fn legacy_tablet(action: Action, held: Shortcut) -> Self {
        Self {
            field: None,
            label: format!("{} (legacy tablet barrel button)", action_label(action)),
            source: BindingSource::LegacyTablet,
            held,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PendingShortcutConflict {
    Recorded {
        target: KeybindingField,
        binding: Shortcut,
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
            Self::Recorded { claimants, .. }
                if !claimants.is_empty()
                    && claimants
                        .iter()
                        .all(|claim| claim.source == BindingSource::LegacyTablet) =>
            {
                "Move Legacy Binding"
            }
            Self::Recorded { .. } => "Replace",
            Self::Text { .. } => "Resolve Conflicts",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextShortcutConflict {
    pub binding: Shortcut,
    pub claimants: Vec<ShortcutClaim>,
}

fn shortcuts_conflict(left: &Shortcut, right: &Shortcut) -> bool {
    left == right || left.prefix_conflicts_with(right)
}

/// Every action in the draft that currently claims `binding`, including the
/// target when it already lists that chord (a duplicate inside one action).
pub fn claimants_for(draft: &KeybindingsDraft, binding: &Shortcut) -> Vec<ShortcutClaim> {
    let mut claims = Vec::new();
    for entry in &draft.entries {
        if let Ok(parsed) = parse_keybindings(&entry.value) {
            for candidate in parsed {
                if shortcuts_conflict(&candidate, binding)
                    && !claims.iter().any(|claim: &ShortcutClaim| {
                        claim.field == Some(entry.field) && claim.held == candidate
                    })
                {
                    claims.push(ShortcutClaim::from_field(entry.field, candidate));
                }
            }
        }
    }
    if let Some(action) = legacy_action_for(draft, binding) {
        claims.push(ShortcutClaim::legacy_tablet(action, binding.clone()));
    }
    claims
}

pub fn other_claimants(
    draft: &KeybindingsDraft,
    target: KeybindingField,
    binding: &Shortcut,
) -> Vec<ShortcutClaim> {
    claimants_for(draft, binding)
        .into_iter()
        .filter(|claim| claim.field != Some(target) || claim.held != *binding)
        .collect()
}

pub fn field_has_internal_duplicate(draft: &KeybindingsDraft, field: KeybindingField) -> bool {
    let Some(value) = draft.value_for(field) else {
        return false;
    };
    let Ok(parsed) = parse_keybindings(value) else {
        return false;
    };
    parsed.iter().enumerate().any(|(index, binding)| {
        parsed
            .iter()
            .skip(index + 1)
            .any(|other| shortcuts_conflict(other, binding))
    })
}

pub fn text_conflicts_for(
    draft: &KeybindingsDraft,
    target: KeybindingField,
    parsed: &[Shortcut],
) -> Vec<TextShortcutConflict> {
    let mut conflicts = Vec::new();
    let mut seen = Vec::new();
    for binding in parsed {
        if seen
            .iter()
            .any(|existing| shortcuts_conflict(existing, binding))
        {
            if !conflicts
                .iter()
                .any(|conflict: &TextShortcutConflict| conflict.binding == *binding)
            {
                conflicts.push(TextShortcutConflict {
                    binding: binding.clone(),
                    claimants: vec![ShortcutClaim::from_field(target, binding.clone())],
                });
            }
            continue;
        }
        seen.push(binding.clone());
        let claimants: Vec<_> = other_claimants(draft, target, binding)
            .into_iter()
            .filter(|claim| claim.field != Some(target))
            .collect();
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
    binding: &Shortcut,
    claimants: &[ShortcutClaim],
) -> Result<(), FieldEditError> {
    let snapshot = draft.clone();
    for claim in claimants {
        if claim.field == Some(target) {
            if claim.held != *binding
                && let Err(error) = remove_binding(draft, target, &claim.held)
            {
                *draft = snapshot;
                return Err(error);
            }
            continue;
        }
        if claim.source == BindingSource::LegacyTablet {
            clear_legacy_for(draft, binding);
            continue;
        }
        let Some(field) = claim.field else {
            continue;
        };
        if let Err(error) = remove_binding(draft, field, &claim.held) {
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
            if claim.field == Some(target) {
                continue;
            }
            if claim.source == BindingSource::LegacyTablet {
                clear_legacy_for(draft, &conflict.binding);
                continue;
            }
            let Some(field) = claim.field else {
                continue;
            };
            if let Err(error) = remove_binding(draft, field, &claim.held) {
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

pub fn recorded_conflict_prompt(binding: &Shortcut, claimants: &[ShortcutClaim]) -> String {
    let mut lines = vec![format!("{binding} is already assigned to:")];
    for claim in claimants {
        if claim.held == *binding {
            lines.push(format!("- {}", claim.label));
        } else {
            lines.push(format!(
                "- {} ({})",
                claim.label,
                claim.held.display_label()
            ));
        }
    }
    lines.push(String::new());
    if claimants
        .iter()
        .all(|claim| claim.source == BindingSource::LegacyTablet)
        && !claimants.is_empty()
    {
        lines.push(
            "Move that legacy barrel-button assignment into this action's shortcuts?".to_string(),
        );
    } else if claimants.iter().any(|claim| claim.held != *binding) {
        lines.push(
            "Replace those assignments? Prefix and extension shortcuts cannot coexist.".to_string(),
        );
    } else {
        lines.push("Replace those assignments?".to_string());
    }
    lines.join("\n")
}

fn legacy_action_for(draft: &KeybindingsDraft, binding: &Shortcut) -> Option<Action> {
    let trigger = binding.as_trigger()?;
    let ShortcutTrigger::Stylus(trigger) = trigger else {
        return None;
    };
    if trigger.ctrl || trigger.shift || trigger.alt || trigger.logo {
        return None;
    }
    match trigger.button {
        StylusButton::Primary => draft.legacy_tablet.stylus_primary,
        StylusButton::Secondary => draft.legacy_tablet.stylus_secondary,
    }
}

fn clear_legacy_for(draft: &mut KeybindingsDraft, binding: &Shortcut) {
    let Some(trigger) = binding.as_trigger() else {
        return;
    };
    let ShortcutTrigger::Stylus(trigger) = trigger else {
        return;
    };
    if trigger.ctrl || trigger.shift || trigger.alt || trigger.logo {
        return;
    }
    match trigger.button {
        StylusButton::Primary => draft.legacy_tablet.stylus_primary = None,
        StylusButton::Secondary => draft.legacy_tablet.stylus_secondary = None,
    }
}

fn text_conflict_prompt(conflicts: &[TextShortcutConflict]) -> String {
    let mut lines = vec!["These shortcuts conflict:".to_string()];
    for conflict in conflicts {
        let names = conflict
            .claimants
            .iter()
            .map(|claim| {
                if claim.held == conflict.binding {
                    claim.label.clone()
                } else {
                    format!("{} ({})", claim.label, claim.held.display_label())
                }
            })
            .collect::<Vec<_>>()
            .join(", ");
        lines.push(format!("- {}: {names}", conflict.binding.display_label()));
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
        let binding = Shortcut::parse("Ctrl+Shift+X").expect("parses");
        let claimants = claimants_for(&draft, &binding);
        let fields: Vec<_> = claimants.iter().map(|claim| claim.field).collect();
        assert!(fields.contains(&Some(KeybindingField::ClearCanvas)));
        assert!(fields.contains(&Some(KeybindingField::ToggleToolbar)));
        assert!(fields.contains(&Some(KeybindingField::Undo)));
        assert_eq!(claimants.len(), 3);
    }

    #[test]
    fn conflict_lookup_treats_meta_and_super_as_the_same_binding() {
        let mut draft = draft();
        draft.set(KeybindingField::Undo, "Meta+X".to_string());
        let binding = Shortcut::parse("Super+X").expect("parses");
        let claimants = claimants_for(&draft, &binding);
        assert_eq!(claimants.len(), 1);
        assert_eq!(claimants[0].field, Some(KeybindingField::Undo));
    }

    #[test]
    fn duplicate_inside_one_action_is_a_conflict() {
        let mut draft = draft();
        draft.set(KeybindingField::ClearCanvas, "E, e".to_string());
        assert!(field_has_internal_duplicate(
            &draft,
            KeybindingField::ClearCanvas
        ));
        let binding = Shortcut::parse("E").expect("parses");
        let claimants = claimants_for(&draft, &binding);
        assert_eq!(claimants.len(), 1);
        assert_eq!(claimants[0].field, Some(KeybindingField::ClearCanvas));
    }

    #[test]
    fn replace_removes_only_the_contested_binding() {
        let mut draft = draft();
        draft.set(
            KeybindingField::ToggleHelp,
            "F10, F1, Ctrl+Shift+X".to_string(),
        );
        draft.set(KeybindingField::Undo, "Ctrl+Z, Ctrl+Shift+X".to_string());
        let binding = Shortcut::parse("Ctrl+Shift+X").expect("parses");
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
            binding: Shortcut::parse("E").expect("parses"),
            claimants: other_claimants(
                &draft,
                KeybindingField::Undo,
                &Shortcut::parse("E").expect("parses"),
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
                Shortcut::parse("E").expect("parses"),
                Shortcut::parse("Ctrl+Z").expect("parses"),
                Shortcut::parse("E").expect("parses"),
            ],
        );
        assert_eq!(conflicts.len(), 2);
        assert_eq!(conflicts[0].binding.to_string(), "E");
        assert_eq!(
            conflicts[0].claimants[0].field,
            Some(KeybindingField::ClearCanvas)
        );
        assert_eq!(conflicts[1].binding.to_string(), "Ctrl+Z");
        assert_eq!(conflicts[1].claimants[0].field, Some(KeybindingField::Undo));
    }

    #[test]
    fn legacy_stylus_assignment_conflicts_with_canonical_stylus_trigger() {
        let mut draft = draft();
        draft.legacy_tablet.stylus_primary = Some(wayscriber::config::Action::ToggleRadialMenu);
        let binding = Shortcut::parse("StylusPrimary").expect("parses");
        let claimants = other_claimants(&draft, KeybindingField::Undo, &binding);
        assert_eq!(claimants.len(), 1);
        assert_eq!(claimants[0].source, BindingSource::LegacyTablet);
        assert!(claimants[0].label.contains("legacy tablet"));

        let pending = PendingShortcutConflict::Recorded {
            target: KeybindingField::Undo,
            binding: binding.clone(),
            claimants: claimants.clone(),
        };
        assert_eq!(pending.replace_label(), "Move Legacy Binding");

        apply_recorded_replace(&mut draft, KeybindingField::Undo, &binding, &claimants)
            .expect("move");
        assert_eq!(draft.legacy_tablet.stylus_primary, None);
        assert!(
            draft
                .value_for(KeybindingField::Undo)
                .is_some_and(|value| value.contains("StylusPrimary"))
        );
    }

    #[test]
    fn modifiered_stylus_trigger_does_not_claim_the_legacy_barrel() {
        let mut draft = draft();
        draft.legacy_tablet.stylus_primary = Some(wayscriber::config::Action::ToggleRadialMenu);
        let binding = Shortcut::parse("Ctrl+StylusPrimary").expect("parses");
        assert!(other_claimants(&draft, KeybindingField::Undo, &binding).is_empty());
    }

    #[test]
    fn prefix_conflict_names_both_shortcuts() {
        let mut draft = draft();
        let prefix = Shortcut::parse("Ctrl+Alt+Shift+K").expect("parses");
        let sequence = Shortcut::parse("Ctrl+Alt+Shift+K > Ctrl+Alt+Shift+C").expect("parses");
        draft.set(KeybindingField::ToggleFloatingBadge, prefix.to_string());
        let claimants = other_claimants(&draft, KeybindingField::Undo, &sequence);
        assert_eq!(claimants.len(), 1);
        assert_eq!(
            claimants[0].field,
            Some(KeybindingField::ToggleFloatingBadge)
        );
        assert_eq!(claimants[0].held, prefix);
        let prompt = recorded_conflict_prompt(&sequence, &claimants);
        assert!(prompt.contains(&sequence.to_string()), "{prompt}");
        assert!(prompt.contains(&prefix.to_string()), "{prompt}");
        assert!(prompt.contains("Toggle Board/Page Badge"), "{prompt}");
        assert!(prompt.contains("cannot coexist"), "{prompt}");
    }

    #[test]
    fn prefix_replace_removes_the_conflicting_shortcut_only() {
        let mut draft = draft();
        let prefix = Shortcut::parse("Ctrl+Alt+Shift+K").expect("parses");
        let sequence = Shortcut::parse("Ctrl+Alt+Shift+K > Ctrl+Alt+Shift+C").expect("parses");
        draft.set(
            KeybindingField::ToggleFloatingBadge,
            format!("F8, {sequence}"),
        );
        let claimants = other_claimants(&draft, KeybindingField::Undo, &prefix);
        apply_recorded_replace(&mut draft, KeybindingField::Undo, &prefix, &claimants)
            .expect("replace");
        assert_eq!(
            draft.value_for(KeybindingField::ToggleFloatingBadge),
            Some("F8")
        );
        assert!(
            draft
                .value_for(KeybindingField::Undo)
                .is_some_and(|value| value.contains(&prefix.to_string()))
        );
    }
}
