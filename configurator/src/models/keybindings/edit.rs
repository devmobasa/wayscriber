//! Parsed draft operations for one action's shortcut list.
//!
//! Mutations work on authored comma-separated text and only rewrite a field
//! when the user explicitly adds, removes, resets, or applies a parsed list.
//! Invalid text stays in place until one of those replacements happens.

use wayscriber::config::ShortcutTrigger;

use super::draft::KeybindingsDraft;
use super::field::KeybindingField;
use super::parse::{authored_shortcut_parts, parse_keybindings};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShortcutTextEditor {
    pub field: KeybindingField,
    pub text: String,
}

impl ShortcutTextEditor {
    pub fn new(field: KeybindingField, text: impl Into<String>) -> Self {
        Self {
            field,
            text: text.into(),
        }
    }

    pub fn parse_error(&self) -> Option<String> {
        parse_keybindings(&self.text).err()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppendOutcome {
    Added,
    AlreadyPresent,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FieldEditError {
    InvalidText(String),
}

impl FieldEditError {
    pub fn message(&self) -> &str {
        match self {
            Self::InvalidText(message) => message,
        }
    }
}

pub fn append_binding(
    draft: &mut KeybindingsDraft,
    field: KeybindingField,
    binding: &ShortcutTrigger,
) -> Result<AppendOutcome, FieldEditError> {
    let authored = draft.value_for(field).unwrap_or("").to_string();
    let parsed = parse_keybindings(&authored).map_err(FieldEditError::InvalidText)?;
    if parsed.iter().any(|existing| existing == binding) {
        return Ok(AppendOutcome::AlreadyPresent);
    }
    let added = binding.to_string();
    let next = if authored.trim().is_empty() {
        added
    } else {
        format!("{}, {added}", authored.trim())
    };
    draft.set(field, next);
    Ok(AppendOutcome::Added)
}

pub fn remove_binding(
    draft: &mut KeybindingsDraft,
    field: KeybindingField,
    binding: &ShortcutTrigger,
) -> Result<(), FieldEditError> {
    let authored = draft.value_for(field).unwrap_or("").to_string();
    parse_keybindings(&authored).map_err(FieldEditError::InvalidText)?;
    let mut removed = false;
    let mut kept = Vec::new();
    for part in authored_shortcut_parts(&authored) {
        if !removed && ShortcutTrigger::parse(part).is_ok_and(|parsed| parsed == *binding) {
            removed = true;
            continue;
        }
        kept.push(part.to_string());
    }
    draft.set(field, kept.join(", "));
    Ok(())
}

pub fn reset_field(
    draft: &mut KeybindingsDraft,
    defaults: &KeybindingsDraft,
    field: KeybindingField,
) {
    let value = defaults.value_for(field).unwrap_or_default().to_string();
    draft.set(field, value);
}

#[cfg(test)]
fn apply_parsed_text(
    draft: &mut KeybindingsDraft,
    field: KeybindingField,
    text: &str,
) -> Result<Vec<ShortcutTrigger>, FieldEditError> {
    let parsed = parse_keybindings(text).map_err(FieldEditError::InvalidText)?;
    draft.set(field, text.trim().to_string());
    Ok(parsed)
}

pub fn parsed_lists_equal(left: &str, right: &str) -> bool {
    match (parse_keybindings(left), parse_keybindings(right)) {
        (Ok(left), Ok(right)) => left == right,
        _ => false,
    }
}

pub fn field_matches_defaults(
    draft: &KeybindingsDraft,
    defaults: &KeybindingsDraft,
    field: KeybindingField,
) -> bool {
    let current = draft.value_for(field).unwrap_or("");
    let default = defaults.value_for(field).unwrap_or("");
    parsed_lists_equal(current, default)
}

pub fn reset_tooltip(defaults: &KeybindingsDraft, field: KeybindingField) -> String {
    let default = defaults.value_for(field).unwrap_or("").trim();
    if default.is_empty() {
        "Reset to default: Unbound".to_string()
    } else if default.contains(',') {
        format!("Reset to defaults: {default}")
    } else {
        format!("Reset to default: {default}")
    }
}

pub fn serialize_bindings(bindings: &[ShortcutTrigger]) -> String {
    bindings
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
    use wayscriber::config::keybindings::KeybindingsConfig;

    use super::*;
    use crate::models::keybindings::KeybindingField;

    fn draft() -> (KeybindingsDraft, KeybindingsDraft) {
        let defaults = KeybindingsDraft::from_config(&KeybindingsConfig::default());
        (defaults.clone(), defaults)
    }

    #[test]
    fn equivalent_spelling_cannot_create_a_duplicate_chip() {
        let (mut draft, _defaults) = draft();
        draft.set(KeybindingField::Undo, "ctrl+z".to_string());
        let binding = ShortcutTrigger::parse("Ctrl+Z").expect("parses");
        let outcome = append_binding(&mut draft, KeybindingField::Undo, &binding).expect("append");
        assert_eq!(outcome, AppendOutcome::AlreadyPresent);
        assert_eq!(draft.value_for(KeybindingField::Undo), Some("ctrl+z"));
    }

    #[test]
    fn removing_one_chip_preserves_siblings_and_authored_spelling() {
        let (mut draft, _defaults) = draft();
        draft.set(KeybindingField::Redo, "ctrl+shift+z, Ctrl+Y".to_string());
        let binding = ShortcutTrigger::parse("Ctrl+Y").expect("parses");
        remove_binding(&mut draft, KeybindingField::Redo, &binding).expect("remove");
        assert_eq!(draft.value_for(KeybindingField::Redo), Some("ctrl+shift+z"));
    }

    #[test]
    fn removing_the_last_chip_unbinds_instead_of_resetting() {
        let (mut draft, defaults) = draft();
        let binding = ShortcutTrigger::parse("E").expect("parses");
        remove_binding(&mut draft, KeybindingField::ClearCanvas, &binding).expect("remove");
        assert_eq!(draft.value_for(KeybindingField::ClearCanvas), Some(""));
        assert!(!field_matches_defaults(
            &draft,
            &defaults,
            KeybindingField::ClearCanvas
        ));
    }

    #[test]
    fn reset_handles_one_default_multiple_defaults_and_unbound() {
        let (mut draft, defaults) = draft();
        draft.set(KeybindingField::ClearCanvas, "X".to_string());
        draft.set(KeybindingField::Redo, "".to_string());
        draft.set(KeybindingField::ToggleFloatingBadge, "F8".to_string());

        reset_field(&mut draft, &defaults, KeybindingField::ClearCanvas);
        reset_field(&mut draft, &defaults, KeybindingField::Redo);
        reset_field(&mut draft, &defaults, KeybindingField::ToggleFloatingBadge);

        assert_eq!(draft.value_for(KeybindingField::ClearCanvas), Some("E"));
        assert_eq!(
            draft.value_for(KeybindingField::Redo),
            Some("Ctrl+Shift+Z, Ctrl+Y")
        );
        assert_eq!(
            draft.value_for(KeybindingField::ToggleFloatingBadge),
            Some("")
        );
        assert_eq!(
            reset_tooltip(&defaults, KeybindingField::ClearCanvas),
            "Reset to default: E"
        );
        assert_eq!(
            reset_tooltip(&defaults, KeybindingField::Redo),
            "Reset to defaults: Ctrl+Shift+Z, Ctrl+Y"
        );
        assert_eq!(
            reset_tooltip(&defaults, KeybindingField::ToggleFloatingBadge),
            "Reset to default: Unbound"
        );
    }

    #[test]
    fn invalid_raw_text_blocks_parsed_edits_and_stays_visible() {
        let (mut draft, _defaults) = draft();
        draft.set(KeybindingField::Exit, "Ctrl+Shift".to_string());
        let binding = ShortcutTrigger::parse("F5").expect("parses");
        let error = append_binding(&mut draft, KeybindingField::Exit, &binding)
            .expect_err("invalid text cannot append");
        assert!(error.message().contains("No key specified"));
        assert_eq!(draft.value_for(KeybindingField::Exit), Some("Ctrl+Shift"));
        let error = remove_binding(&mut draft, KeybindingField::Exit, &binding)
            .expect_err("invalid text cannot remove");
        assert!(error.message().contains("No key specified"));
        assert_eq!(draft.value_for(KeybindingField::Exit), Some("Ctrl+Shift"));
    }

    #[test]
    fn apply_parsed_text_keeps_the_authored_string() {
        let (mut draft, _defaults) = draft();
        apply_parsed_text(&mut draft, KeybindingField::Undo, " ctrl+z , Ctrl+Shift+Z ")
            .expect("parses");
        assert_eq!(
            draft.value_for(KeybindingField::Undo),
            Some("ctrl+z , Ctrl+Shift+Z")
        );
    }

    #[test]
    fn whitespace_differences_are_not_a_change_from_defaults() {
        let (mut draft, defaults) = draft();
        draft.set(KeybindingField::Redo, "Ctrl+Shift+Z,Ctrl+Y".to_string());
        assert!(field_matches_defaults(
            &draft,
            &defaults,
            KeybindingField::Redo
        ));
    }

    #[test]
    fn mouse_and_stylus_strings_append_as_chips() {
        let (mut draft, _defaults) = draft();
        draft.set(KeybindingField::Undo, "".to_string());
        let binding = ShortcutTrigger::parse("Ctrl+MouseBack").expect("parses");
        append_binding(&mut draft, KeybindingField::Undo, &binding).expect("append");
        assert_eq!(
            draft.value_for(KeybindingField::Undo),
            Some("Ctrl+MouseBack")
        );
    }
}
