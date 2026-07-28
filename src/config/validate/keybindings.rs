use std::fmt;

use super::super::CURRENT_CONFIG_REVISION;
use super::super::action_meta::action_label;
use super::super::keybindings::{Action, KeyBinding, KeybindingConflict, KeybindingsConfig};
use super::Config;

const LEGACY_COMMAND_PALETTE_DEFAULT: &[&str] = &["Ctrl+K"];
const CURRENT_COMMAND_PALETTE_DEFAULT: &[&str] = &["Ctrl+K", "Ctrl+Shift+P"];
const LEGACY_FULL_SCREEN_CAPTURE_DEFAULT: &[&str] = &["Ctrl+Shift+P"];
const CURRENT_FULL_SCREEN_CAPTURE_DEFAULT: &[&str] = &["Ctrl+Alt+F"];
const LEGACY_TOGGLE_TOOLBAR_DEFAULT: &[&str] = &["F2", "F9"];
const CURRENT_TOGGLE_TOOLBAR_DEFAULT: &[&str] = &["F9"];
const CURRENT_CYCLE_TOOLBAR_DISPLAY_DEFAULT: &[&str] = &["F2"];
const CURRENT_TOGGLE_INPUT_HUD_DEFAULT: &[&str] = &["Ctrl+Shift+K"];

fn bindings_equal(bindings: &[String], expected: &[&str]) -> bool {
    bindings
        .iter()
        .map(String::as_str)
        .eq(expected.iter().copied())
}

fn bindings_from(expected: &[&str]) -> Vec<String> {
    expected
        .iter()
        .map(|binding| (*binding).to_string())
        .collect()
}

/// Whether an action other than `owner` already claims `binding`.
///
/// Candidates go through the parser and then [`KeyBinding`] equality, so
/// `shift+ctrl+k` counts as a claim on `Ctrl+Shift+K`: modifier order, spacing,
/// and key case are all things the keymap ignores when a key is pressed. A
/// binding string that does not parse is not a claim, because it binds nothing
/// at runtime either; the keymap reports that typo separately.
fn binding_claimed_by_another_action(
    keybindings: &KeybindingsConfig,
    owner: Action,
    binding: &KeyBinding,
) -> bool {
    KeybindingsConfig::configurable_actions()
        .iter()
        .filter(|action| **action != owner)
        .any(|action| {
            keybindings
                .bindings_for_action(*action)
                .is_some_and(|bindings| {
                    bindings.iter().any(|candidate| {
                        KeyBinding::parse(candidate).is_ok_and(|parsed| parsed == *binding)
                    })
                })
        })
}

/// One binding string the parser rejected while loading a configuration.
///
/// The string is dropped from the keymap this session builds, because a key
/// that cannot be parsed cannot be pressed either and keeping it used to fail
/// the whole map. The file keeps the typo — a save writes just the delta its
/// caller asked for — so the user has to be told, not just the log.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvalidKeybinding {
    action: Action,
    binding: String,
    error: String,
}

impl InvalidKeybinding {
    /// The rejected string exactly as the file spells it.
    pub fn binding(&self) -> &str {
        &self.binding
    }

    /// The `[keybindings]` key the string was removed from.
    pub fn config_key(&self) -> Option<&'static str> {
        KeybindingsConfig::config_key_for_action(self.action)
    }

    /// Toast-sized wording; [`fmt::Display`] carries the long form.
    pub fn summary(&self) -> String {
        format!(
            "{} is not a valid shortcut for {}.",
            self.binding,
            action_label(self.action)
        )
    }
}

impl fmt::Display for InvalidKeybinding {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "`{}` for {} could not be parsed: {} — it is ignored for this session",
            self.binding,
            action_label(self.action),
            self.error
        )
    }
}

/// One duplicate shortcut resolved while loading a configuration.
///
/// The resolution applies to the running session only: a save writes just the
/// delta its caller asked for, so nothing here is ever written to
/// `config.toml`. The user has to see it to be able to fix it, which is why
/// this is returned rather than only logged.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeybindingConflictResolution {
    key: String,
    kept: Action,
    dropped: Action,
}

impl KeybindingConflictResolution {
    /// The conflicting shortcut in its normalized form (`Ctrl+Shift+P`).
    pub fn key(&self) -> &str {
        &self.key
    }

    /// The action that keeps the shortcut for this session.
    pub fn kept(&self) -> Action {
        self.kept
    }

    /// The action the shortcut was removed from for this session.
    pub fn dropped(&self) -> Action {
        self.dropped
    }

    /// Whether one action listed the same shortcut more than once, rather than
    /// two actions claiming it.
    pub fn is_self_duplicate(&self) -> bool {
        self.kept == self.dropped
    }

    /// The `[keybindings]` key the shortcut was removed from.
    pub fn dropped_config_key(&self) -> Option<&'static str> {
        KeybindingsConfig::config_key_for_action(self.dropped)
    }

    /// Toast-sized wording; [`fmt::Display`] carries the long form.
    pub fn summary(&self) -> String {
        if self.is_self_duplicate() {
            format!(
                "{} is listed more than once for {}.",
                self.key,
                action_label(self.kept)
            )
        } else {
            format!(
                "{} kept for {}, dropped from {}.",
                self.key,
                action_label(self.kept),
                action_label(self.dropped)
            )
        }
    }
}

impl fmt::Display for KeybindingConflictResolution {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_self_duplicate() {
            return write!(
                formatter,
                "`{}` is listed more than once for {}; the repeats are ignored for this session",
                self.key,
                action_label(self.kept)
            );
        }
        write!(
            formatter,
            "`{}` is bound to both {} and {}; {} keeps it for this session and {} loses it",
            self.key,
            action_label(self.kept),
            action_label(self.dropped),
            action_label(self.kept),
            action_label(self.dropped)
        )
    }
}

/// Removes one shortcut from one action's binding list.
///
/// `keep_first` leaves the earliest occurrence in place, which is what the
/// action that wins a conflict needs when it also listed the key twice.
/// Returns whether anything was removed.
fn drop_binding(
    keybindings: &mut KeybindingsConfig,
    action: Action,
    binding: &KeyBinding,
    keep_first: bool,
) -> bool {
    let Some(current) = keybindings.bindings_for_action(action) else {
        return false;
    };
    let mut bindings = current.to_vec();
    let before = bindings.len();
    let mut kept_one = false;
    bindings.retain(|candidate| {
        if !KeyBinding::parse(candidate).is_ok_and(|parsed| parsed == *binding) {
            return true;
        }
        if keep_first && !kept_one {
            kept_one = true;
            return true;
        }
        false
    });
    if bindings.len() == before {
        return false;
    }
    // `bindings_for_action` already proved the action has a stored field, so
    // the only failure mode of the setter cannot happen here.
    let _ = keybindings.set_bindings_for_action(action, bindings);
    true
}

/// Picks the action that keeps a contested shortcut.
///
/// A binding list that still equals its compiled-in default was almost
/// certainly filled in by serde rather than typed by the user, so an authored
/// list always outranks a defaulted one. Everything else is decided by the
/// keymap traversal order, where the earlier action wins.
///
/// `as_written` must be the configuration before any conflict was resolved, so
/// that the classification cannot drift while a pass runs.
fn conflict_winner(
    as_written: &KeybindingsConfig,
    defaults: &KeybindingsConfig,
    conflict: &KeybindingConflict,
) -> Option<Action> {
    let first = conflict.actions().first().copied()?;
    let authored = conflict.actions().iter().copied().find(|action| {
        as_written.bindings_for_action(*action) != defaults.bindings_for_action(*action)
    });
    Some(authored.unwrap_or(first))
}

/// Everything loading had to change in `[keybindings]`, in the order the
/// passes run: a string that does not parse is removed before duplicates are
/// arbitrated, so the arbitration only ever sees keys that can actually fire.
pub(super) struct KeybindingValidation {
    pub(super) invalid: Vec<InvalidKeybinding>,
    pub(super) conflicts: Vec<KeybindingConflictResolution>,
}

impl Config {
    pub(super) fn validate_keybindings(&mut self) -> KeybindingValidation {
        self.apply_keybinding_migrations();
        let invalid = self.drop_unparseable_bindings();
        let conflicts = self.resolve_keybinding_conflicts();
        KeybindingValidation { invalid, conflicts }
    }

    /// Removes the binding strings the parser rejects.
    ///
    /// A typo binds nothing at runtime, but leaving it in place used to fail
    /// `build_action_map` for the whole config, and the caller of that swapped
    /// in the complete shipped defaults for the session — the same total loss
    /// of customization as #293, from a single mistyped key. Dropping only the
    /// offending strings keeps every other authored shortcut working.
    ///
    /// Like a resolved conflict, the removal is session-only: a save records
    /// just the delta its caller asked for, so `config.toml` keeps the typo
    /// until the user fixes it.
    fn drop_unparseable_bindings(&mut self) -> Vec<InvalidKeybinding> {
        let mut invalid = Vec::new();
        for action in KeybindingsConfig::configurable_actions() {
            let Some(current) = self.keybindings.bindings_for_action(*action) else {
                continue;
            };
            let mut kept = Vec::with_capacity(current.len());
            let mut dropped = false;
            for binding in current {
                match KeyBinding::parse(binding) {
                    Ok(_) => kept.push(binding.clone()),
                    Err(error) => {
                        dropped = true;
                        invalid.push(InvalidKeybinding {
                            action: *action,
                            binding: binding.clone(),
                            error,
                        });
                    }
                }
            }
            if dropped {
                // `bindings_for_action` already proved the action has a stored
                // field, so the only failure mode of the setter cannot happen.
                let _ = self.keybindings.set_bindings_for_action(*action, kept);
            }
        }

        for entry in &invalid {
            log::warn!("Invalid shortcut in the keybindings config: {entry}");
        }
        invalid
    }

    /// Resolves duplicate shortcuts one key at a time.
    ///
    /// Collisions usually come from us, not the user: a field the file omits is
    /// filled in by serde with the current default, so a shipped default can
    /// land on a shortcut the file assigns to something else (`F2` for
    /// `cycle_toolbar_display` against an authored `toggle_toolbar`, for
    /// instance). Replacing the whole section with defaults for that — what
    /// this used to do — cost users every customization they had (#293).
    ///
    /// Each conflicting key is therefore removed from exactly one action:
    ///
    /// 1. an authored binding list (one that differs from its compiled-in
    ///    default) always beats a list that still equals its default, so the
    ///    serde-filled side is the one that loses the key;
    /// 2. ties — two authored lists, or two lists that both still equal their
    ///    defaults, the latter being a bug in the shipped defaults that
    ///    `default_keybindings_have_no_conflicts` guards against — are broken
    ///    by the keymap traversal order (core, selection, tools, board, ui,
    ///    colors, capture, zoom, presets, declared order inside each group),
    ///    and the earlier action keeps the key.
    ///
    /// The rest of both actions' bindings always survive, and the resolution
    /// stays in memory: a save records only the delta its caller asked for, so
    /// none of this reaches `config.toml`.
    fn resolve_keybinding_conflicts(&mut self) -> Vec<KeybindingConflictResolution> {
        let conflicts = match self.keybindings.collect_binding_conflicts() {
            Ok(conflicts) => conflicts,
            Err(error) => {
                // Unreachable: `drop_unparseable_bindings` runs first and the
                // parser is the only thing collection can fail on. Kept as a
                // safeguard so a future collection failure degrades to "no
                // conflicts arbitrated" instead of a panic.
                log::warn!("Invalid keybinding configuration: {error}. Ignoring that binding.");
                return Vec::new();
            }
        };
        if conflicts.is_empty() {
            return Vec::new();
        }

        let defaults = KeybindingsConfig::default();
        // Classify against the config as written: resolving one key mutates a
        // list, and a defaulted list that just lost a key would otherwise look
        // authored while arbitrating the next one.
        let authored = self.keybindings.clone();
        let mut resolutions = Vec::new();
        for conflict in conflicts {
            let Some(kept) = conflict_winner(&authored, &defaults, &conflict) else {
                continue;
            };
            let key = conflict.binding().to_string();
            let mut repeated = false;
            for &action in conflict.actions() {
                if action == kept {
                    repeated =
                        drop_binding(&mut self.keybindings, action, conflict.binding(), true);
                    continue;
                }
                if drop_binding(&mut self.keybindings, action, conflict.binding(), false) {
                    resolutions.push(KeybindingConflictResolution {
                        key: key.clone(),
                        kept,
                        dropped: action,
                    });
                }
            }
            // Reported even when other actions contested the key too: the
            // repeat in the winner's own list was still removed, and hearing
            // only about the cross-action side would leave that edit
            // unexplained.
            if repeated {
                resolutions.push(KeybindingConflictResolution {
                    key,
                    kept,
                    dropped: kept,
                });
            }
        }

        for resolution in &resolutions {
            log::warn!("Conflicting shortcut in the keybindings config: {resolution}");
        }
        resolutions
    }

    pub(crate) fn apply_keybinding_migrations(&mut self) {
        if self.config_revision >= CURRENT_CONFIG_REVISION {
            return;
        }
        // Each step is gated on the revision that introduced it, so a config
        // saved at a later revision never re-runs an earlier heuristic (a
        // deliberately restored legacy value must survive future upgrades).
        if self.config_revision < 1 {
            self.migrate_command_palette_and_capture_defaults();
        }
        if self.config_revision < 2 {
            self.migrate_toggle_toolbar_f2_split();
        }
        if self.config_revision < 3 {
            self.migrate_input_hud_default_shortcut();
        }
        self.config_revision = CURRENT_CONFIG_REVISION;
    }

    /// Revision 1: `Ctrl+K`-only command palette and `Ctrl+Shift+P`
    /// full-screen capture defaults moved to `Ctrl+K`/`Ctrl+Shift+P` and
    /// `Ctrl+Alt+F`.
    fn migrate_command_palette_and_capture_defaults(&mut self) {
        let command_is_legacy = bindings_equal(
            &self.keybindings.ui.toggle_command_palette,
            LEGACY_COMMAND_PALETTE_DEFAULT,
        );
        let command_is_current = bindings_equal(
            &self.keybindings.ui.toggle_command_palette,
            CURRENT_COMMAND_PALETTE_DEFAULT,
        );
        let capture_is_legacy = bindings_equal(
            &self.keybindings.capture.capture_full_screen,
            LEGACY_FULL_SCREEN_CAPTURE_DEFAULT,
        );
        let capture_is_current = bindings_equal(
            &self.keybindings.capture.capture_full_screen,
            CURRENT_FULL_SCREEN_CAPTURE_DEFAULT,
        );

        // A missing field is filled by serde with its current default, so
        // accept legacy/current combinations as long as neither side is
        // customized. This keeps minimal legacy configs valid too.
        if (command_is_legacy || command_is_current)
            && (capture_is_legacy || capture_is_current)
            && (command_is_legacy || capture_is_legacy)
        {
            self.keybindings.ui.toggle_command_palette =
                bindings_from(CURRENT_COMMAND_PALETTE_DEFAULT);
            self.keybindings.capture.capture_full_screen =
                bindings_from(CURRENT_FULL_SCREEN_CAPTURE_DEFAULT);
            log::info!("Migrated legacy command-palette and full-screen capture default shortcuts");
        }
    }

    /// Revision 2: `F2` moved from the `toggle_toolbar` default pair to the
    /// new `cycle_toolbar_display` action. Without this step a config that
    /// explicitly lists the old `["F2", "F9"]` default would collide with
    /// the serde-defaulted `cycle_toolbar_display = ["F2"]`, and keybinding
    /// validation would then wipe every custom binding back to defaults.
    fn migrate_toggle_toolbar_f2_split(&mut self) {
        // `cycle_toolbar_display` did not exist before revision 2, so a
        // pre-revision file normally carries the serde default (`["F2"]`).
        // Any other value means the user already adopted the new field
        // deliberately — leave both sides untouched.
        if !bindings_equal(
            &self.keybindings.ui.cycle_toolbar_display,
            CURRENT_CYCLE_TOOLBAR_DISPLAY_DEFAULT,
        ) {
            return;
        }
        if bindings_equal(
            &self.keybindings.ui.toggle_toolbar,
            LEGACY_TOGGLE_TOOLBAR_DEFAULT,
        ) {
            // The shipped default pair: F2 moves to the cycle action and F9
            // keeps toggling visibility.
            self.keybindings.ui.toggle_toolbar = bindings_from(CURRENT_TOGGLE_TOOLBAR_DEFAULT);
            log::info!(
                "Migrated legacy toggle_toolbar default pair; F2 now cycles the toolbar display"
            );
        } else if self
            .keybindings
            .ui
            .toggle_toolbar
            .iter()
            .any(|binding| binding.trim().eq_ignore_ascii_case("F2"))
        {
            // A deliberate custom set that includes F2: the user's F2 keeps
            // its old toggle meaning and the cycle action starts unbound.
            self.keybindings.ui.cycle_toolbar_display = Vec::new();
            log::info!(
                "Preserved custom F2 toggle_toolbar binding; cycle_toolbar_display left unbound"
            );
        }
    }

    /// Revision 3: `toggle_input_hud` shipped with a `Ctrl+Shift+K` default
    /// and no revision bump, so every file written until now inherits that
    /// shortcut from serde — including files that bound `Ctrl+Shift+K` to
    /// something else. The user never authored that collision and cannot see
    /// it in their own file.
    ///
    /// The authored binding therefore wins and the newcomer starts unbound,
    /// the same trade `migrate_toggle_toolbar_f2_split` makes for `F2`.
    fn migrate_input_hud_default_shortcut(&mut self) {
        // The action shipped before this revision existed, so a pre-revision
        // file may already mention it. Anything other than the default means
        // the user adopted the new field deliberately — leave both sides
        // untouched.
        if !bindings_equal(
            &self.keybindings.ui.toggle_input_hud,
            CURRENT_TOGGLE_INPUT_HUD_DEFAULT,
        ) {
            return;
        }
        let Some(contested) = CURRENT_TOGGLE_INPUT_HUD_DEFAULT
            .first()
            .and_then(|binding| KeyBinding::parse(binding).ok())
        else {
            return;
        };
        if !binding_claimed_by_another_action(&self.keybindings, Action::ToggleInputHud, &contested)
        {
            return;
        }
        self.keybindings.ui.toggle_input_hud = Vec::new();
        log::info!("Preserved the existing {contested} binding; toggle_input_hud left unbound");
    }
}
