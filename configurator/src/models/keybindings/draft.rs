use wayscriber::config::keybindings::KeybindingsConfig;

use super::super::error::FormError;
use super::field::KeybindingField;
use super::parse::parse_keybinding_list;

#[derive(Debug, Clone, PartialEq)]
pub struct KeybindingsDraft {
    pub entries: Vec<KeybindingEntry>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct KeybindingEntry {
    pub field: KeybindingField,
    pub value: String,
}

impl KeybindingsDraft {
    pub fn from_config(config: &KeybindingsConfig) -> Self {
        let entries = KeybindingField::all()
            .into_iter()
            .map(|field| KeybindingEntry {
                value: field.get(config).join(", "),
                field,
            })
            .collect();
        Self { entries }
    }

    pub fn set(&mut self, field: KeybindingField, value: String) {
        if let Some(entry) = self.entries.iter_mut().find(|entry| entry.field == field) {
            entry.value = value;
        }
    }

    pub fn to_config(&self) -> Result<KeybindingsConfig, Vec<FormError>> {
        let mut config = KeybindingsConfig::default();
        let mut errors = Vec::new();

        for entry in &self.entries {
            match parse_keybinding_list(&entry.value) {
                Ok(list) => entry.field.set(&mut config, list),
                Err(err) => errors.push(FormError::new(
                    format!("keybindings.{}", entry.field.field_key()),
                    err,
                )),
            }
        }

        if errors.is_empty() {
            Ok(config)
        } else {
            Err(errors)
        }
    }

    pub fn value_for(&self, field: KeybindingField) -> Option<&str> {
        self.entries
            .iter()
            .find(|entry| entry.field == field)
            .map(|entry| entry.value.as_str())
    }
}
