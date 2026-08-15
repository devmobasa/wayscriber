use std::collections::HashSet;

/// Which `[keybindings]` fields a configuration actually spells out.
///
/// Serde fills every omitted field with its compiled-in default, so the parsed
/// [`KeybindingsConfig`](super::KeybindingsConfig) alone cannot say whether a
/// list was typed by the user or handed over by this build. That ambiguity is
/// what let a newly shipped default outrank an authored shortcut (#293, #315),
/// and comparing a list against the default to guess only moved the guess
/// around: a user who deliberately writes today's default is indistinguishable
/// from a user who wrote nothing. Authorship therefore comes from source
/// presence, recorded once at parse time and carried through resolution.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum KeybindingAuthorship {
    /// No source text was involved: every list counts as authored.
    ///
    /// This is what a configuration built in code holds — a test fixture, the
    /// shipped defaults, a configurator draft — and it makes the omitted-default
    /// pass a no-op, because nothing was omitted.
    #[default]
    AllExplicit,
    /// Parsed from a document: these `[keybindings]` keys were present in it.
    FromFile(HashSet<String>),
}

impl KeybindingAuthorship {
    /// Records which `[keybindings]` keys a TOML document spells out.
    ///
    /// The section is one flat table — its domain structs are
    /// `#[serde(flatten)]`ed and nothing is renamed — so the set of present
    /// keys is exactly the set of authored actions, and a key that no action
    /// owns is harmless here (the document loader reports it as an unknown
    /// setting separately).
    pub fn from_toml_source(input: &str) -> Self {
        let Ok(root) = input.parse::<toml::Table>() else {
            // Unreachable from the loaders, which both parse this same text
            // into a `Config` first. Claiming "nothing was authored" from a
            // source we could not read would let defaults outrank every list
            // in it, so fall back to treating the parsed values as authored.
            return Self::AllExplicit;
        };
        let keys = root
            .get("keybindings")
            .and_then(toml::Value::as_table)
            .map(|table| table.keys().cloned().collect())
            .unwrap_or_default();
        Self::FromFile(keys)
    }

    /// Records one `[keybindings]` key as authored.
    ///
    /// For an editor that rewrites a single key: that list has stopped being
    /// described by the source's presence set, while every other list still is.
    /// [`Config::mark_keybindings_explicit`](crate::config::Config::mark_keybindings_explicit)
    /// is the whole-section form, for an editor that rebuilds all of them.
    pub fn mark_explicit(&mut self, config_key: &str) {
        match self {
            // Nothing was omitted, so there is nothing to record.
            Self::AllExplicit => {}
            Self::FromFile(keys) => {
                keys.insert(config_key.to_string());
            }
        }
    }

    /// Drops one `[keybindings]` key from the file-presence set.
    ///
    /// An editor uses this when a field omitted by the loaded document returns
    /// to its baseline value, so a transient edit does not invent source
    /// authorship the value-based document merge cannot persist.
    /// [`Self::AllExplicit`] has no omitted keys, so this is a no-op there.
    pub fn clear_explicit(&mut self, config_key: &str) {
        if let Self::FromFile(keys) = self {
            keys.remove(config_key);
        }
    }

    /// Whether the source spelled out one `[keybindings]` key.
    pub fn is_explicit(&self, config_key: &str) -> bool {
        match self {
            Self::AllExplicit => true,
            Self::FromFile(keys) => keys.contains(config_key),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presence_records_only_the_keys_the_source_spells_out() {
        let authorship = KeybindingAuthorship::from_toml_source(
            "[keybindings]\nundo = [\"Ctrl+Z\"]\nclear_canvas = []\n",
        );

        assert!(authorship.is_explicit("undo"));
        assert!(
            authorship.is_explicit("clear_canvas"),
            "an explicit empty list is authored: it means unbound"
        );
        assert!(!authorship.is_explicit("redo"));
    }

    #[test]
    fn a_source_without_the_section_authors_nothing() {
        let authorship = KeybindingAuthorship::from_toml_source("[ui]\nshow_status_bar = false\n");

        assert_eq!(authorship, KeybindingAuthorship::FromFile(HashSet::new()));
        assert!(!authorship.is_explicit("undo"));
    }

    /// The section is flat, but a file may still write it with dotted keys.
    /// Both spellings are the same table, so both must record presence.
    #[test]
    fn dotted_keys_count_as_presence() {
        let authorship =
            KeybindingAuthorship::from_toml_source("keybindings.undo = [\"Ctrl+Z\"]\n");

        assert!(authorship.is_explicit("undo"));
        assert!(!authorship.is_explicit("redo"));
    }

    #[test]
    fn a_synthetic_configuration_treats_every_list_as_authored() {
        let authorship = KeybindingAuthorship::default();

        assert!(authorship.is_explicit("undo"));
        assert!(authorship.is_explicit("anything"));
    }

    #[test]
    fn clear_explicit_drops_file_presence_and_leaves_all_explicit_alone() {
        let mut from_file =
            KeybindingAuthorship::from_toml_source("[keybindings]\nundo = [\"Ctrl+Z\"]\n");
        from_file.clear_explicit("undo");
        assert!(!from_file.is_explicit("undo"));

        let mut all = KeybindingAuthorship::default();
        all.clear_explicit("undo");
        assert!(all.is_explicit("undo"));
    }
}
