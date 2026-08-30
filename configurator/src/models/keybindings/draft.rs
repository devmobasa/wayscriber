use std::collections::HashSet;

use wayscriber::config::keybindings::KeybindingsConfig;
use wayscriber::config::{Action, Config, KeybindingAuthorship};

use super::super::error::FormError;
use super::field::{KeybindingField, keybinding_fields};
use super::parse::parse_keybinding_list;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LegacyTabletShortcuts {
    pub stylus_primary: Option<Action>,
    pub stylus_secondary: Option<Action>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct KeybindingsDraft {
    pub entries: Vec<KeybindingEntry>,
    pub legacy_tablet: LegacyTabletShortcuts,
    authorship: KeybindingAuthorship,
    baseline_entries: Vec<KeybindingEntry>,
    baseline_authorship: KeybindingAuthorship,
}

#[derive(Debug, Clone, PartialEq)]
pub struct KeybindingEntry {
    pub field: KeybindingField,
    pub value: String,
}

impl KeybindingsDraft {
    pub fn from_config(config: &KeybindingsConfig) -> Self {
        let entries: Vec<KeybindingEntry> = keybinding_fields()
            .into_iter()
            .map(|field| KeybindingEntry {
                value: field.get(config).join(", "),
                field,
            })
            .collect();
        Self {
            baseline_entries: entries.clone(),
            entries,
            legacy_tablet: LegacyTabletShortcuts::default(),
            authorship: KeybindingAuthorship::FromFile(HashSet::new()),
            baseline_authorship: KeybindingAuthorship::FromFile(HashSet::new()),
        }
    }

    pub fn set_authorship(&mut self, authorship: KeybindingAuthorship) {
        self.baseline_authorship = authorship.clone();
        self.authorship = authorship;
    }

    pub fn set_legacy_from_config(&mut self, config: &Config) {
        #[cfg(feature = "tablet-input")]
        {
            self.legacy_tablet = LegacyTabletShortcuts {
                stylus_primary: config.tablet.stylus_button.action,
                stylus_secondary: config.tablet.stylus_button2.action,
            };
        }
        #[cfg(not(feature = "tablet-input"))]
        {
            let _ = config;
        }
    }

    pub fn set(&mut self, field: KeybindingField, value: String) {
        if let Some(entry) = self.entries.iter_mut().find(|entry| entry.field == field) {
            entry.value = value;
            self.sync_authorship_with_baseline(field);
        }
    }

    pub fn restore_default(&mut self, field: KeybindingField, value: String) {
        if let Some(entry) = self.entries.iter_mut().find(|entry| entry.field == field) {
            entry.value = value;
            self.sync_authorship_with_baseline(field);
        }
    }

    fn sync_authorship_with_baseline(&mut self, field: KeybindingField) {
        let key = field.field_key();
        let value_matches_baseline = self
            .baseline_entries
            .iter()
            .find(|entry| entry.field == field)
            .zip(self.entries.iter().find(|entry| entry.field == field))
            .is_some_and(|(baseline, current)| baseline.value == current.value);
        if !value_matches_baseline || self.baseline_authorship.is_explicit(key) {
            // A changed list is authored by the draft. Returning an existing
            // TOML array to its loaded value/default also stays authored.
            self.authorship.mark_explicit(key);
        } else {
            // Returning an omitted field to its loaded value produces no
            // document delta. Restore source state as well so the draft
            // becomes clean and does not claim authorship the merge cannot
            // persist.
            self.authorship.clear_explicit(key);
        }
    }

    pub fn is_authored(&self, field: KeybindingField) -> bool {
        self.authorship.is_explicit(field.field_key())
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

    /// Whether this field's text names exactly `bindings`.
    ///
    /// Read the way [`Self::to_config`] reads it — comma-separated and
    /// trimmed — rather than compared byte for byte: `"A,B"` and `"A, B"` are
    /// the same list, and a formatting difference must not be mistaken for an
    /// edit by a caller asking whether the field still holds a known value.
    pub fn parses_to(&self, field: KeybindingField, bindings: &[String]) -> bool {
        self.value_for(field)
            .and_then(|value| parse_keybinding_list(value).ok())
            .is_some_and(|parsed| parsed == bindings)
    }

    pub fn value_for(&self, field: KeybindingField) -> Option<&str> {
        self.entries
            .iter()
            .find(|entry| entry.field == field)
            .map(|entry| entry.value.as_str())
    }
}
