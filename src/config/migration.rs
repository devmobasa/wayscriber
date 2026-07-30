//! What an older configuration would become if the user accepted the
//! migration recipes.
//!
//! Nothing here writes, and nothing here runs during a load: the recipes in
//! `validate/keybindings.rs` are proposal material, and this module turns them
//! into a list the configurator can show before the user decides.

use super::action_meta::action_label;
use super::keybindings::KeybindingsConfig;
use super::{CURRENT_CONFIG_REVISION, Config};

/// One `[keybindings]` field a migration proposes to rewrite.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigrationChange {
    config_key: &'static str,
    action_label: &'static str,
    before: Vec<String>,
    after: Vec<String>,
}

impl MigrationChange {
    /// The `[keybindings]` key the change applies to.
    pub fn config_key(&self) -> &'static str {
        self.config_key
    }

    /// The human-facing name of the action that key binds.
    pub fn action_label(&self) -> &'static str {
        self.action_label
    }

    /// The shortcuts the source spells out today. An omitted field reads as
    /// the default this build filled in, which is the value the migration is
    /// reacting to.
    pub fn before(&self) -> &[String] {
        &self.before
    }

    /// The shortcuts the migration proposes. Empty means the action would be
    /// left unbound.
    pub fn after(&self) -> &[String] {
        &self.after
    }
}

/// Everything a reviewed migration would change, plus the revision it would
/// stamp once the user saves it.
///
/// Producing one costs a config clone and a pass over the configurable
/// actions, and it mutates nothing the caller owns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigrationPreview {
    changes: Vec<MigrationChange>,
    proposed_revision: u32,
}

impl MigrationPreview {
    /// Computes the proposal for one authored configuration, or `None` when
    /// there is nothing to offer.
    ///
    /// `authored` has to be the configuration as the source spells it out
    /// ([`super::ConfigDocument::authored_config`]) rather than the resolved
    /// one: a proposal to change a binding that only exists because loading
    /// dropped a contested key would offer the user an edit their file never
    /// contained.
    ///
    /// `None` covers both "already current" and "the recipes propose nothing"
    /// — a file whose fields are customized, or one that spells nothing out,
    /// keeps its own values under every recipe's own gating, and a stamp with
    /// no accompanying change is not worth asking about.
    pub fn for_authored_config(authored: &Config) -> Option<Self> {
        if authored.config_revision >= CURRENT_CONFIG_REVISION {
            return None;
        }

        // The whole configuration is cloned rather than just its keybindings:
        // the recipes are methods on `Config`, and the clone carries the
        // source's keybinding presence along with the values, so a recipe that
        // asks what the file spelled out gets the same answer the preview was
        // computed from.
        let mut migrated = authored.clone();
        migrated.apply_keybinding_migrations();

        let changes = KeybindingsConfig::configurable_actions()
            .iter()
            .filter_map(|action| {
                let config_key = KeybindingsConfig::config_key_for_action(*action)?;
                let before = authored.keybindings.bindings_for_action(*action)?;
                let after = migrated.keybindings.bindings_for_action(*action)?;
                (before != after).then(|| MigrationChange {
                    config_key,
                    action_label: action_label(*action),
                    before: before.to_vec(),
                    after: after.to_vec(),
                })
            })
            .collect::<Vec<_>>();

        (!changes.is_empty()).then_some(Self {
            changes,
            proposed_revision: migrated.config_revision,
        })
    }

    /// The proposed field changes, in keymap traversal order.
    pub fn changes(&self) -> &[MigrationChange] {
        &self.changes
    }

    /// The `config_revision` an accepted migration records, so the same
    /// proposal is not offered again.
    pub fn proposed_revision(&self) -> u32 {
        self.proposed_revision
    }
}
