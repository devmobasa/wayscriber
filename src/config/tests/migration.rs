//! The configurator's migration proposal, computed from an authored config.
//!
//! The recipes themselves are covered in `validate.rs`; these check what the
//! review flow is told about them.

use super::super::{CURRENT_CONFIG_REVISION, Config, MigrationPreview};

fn config_at_revision(revision: u32) -> Config {
    Config {
        config_revision: revision,
        ..Config::default()
    }
}

fn proposed_keys(preview: &MigrationPreview) -> Vec<&'static str> {
    preview
        .changes()
        .iter()
        .map(|change| change.config_key())
        .collect()
}

#[test]
fn a_current_revision_configuration_has_nothing_to_propose() {
    let config = config_at_revision(CURRENT_CONFIG_REVISION);

    assert!(MigrationPreview::for_authored_config(&config).is_none());
}

/// The revision counter is provenance, not a version check: a file stamped
/// past this build settled every question these recipes ask.
#[test]
fn a_future_revision_configuration_has_nothing_to_propose() {
    let config = config_at_revision(CURRENT_CONFIG_REVISION + 1);

    assert!(MigrationPreview::for_authored_config(&config).is_none());
}

/// An empty file spells no shortcut out, so serde hands every action this
/// build's default and every recipe's own gate declines. There is nothing to
/// review, which is why the banner has to stay away.
#[test]
fn a_configuration_that_spells_nothing_out_has_nothing_to_propose() {
    for revision in [0, 1, 2] {
        let config = config_at_revision(revision);

        assert!(
            MigrationPreview::for_authored_config(&config).is_none(),
            "revision {revision} proposed a change to a file with no authored shortcuts"
        );
    }
}

#[test]
fn revision_zero_proposes_the_command_palette_and_capture_split_together() {
    let mut config = config_at_revision(0);
    config.keybindings.ui.toggle_command_palette = vec!["Ctrl+K".to_string()];
    config.keybindings.capture.capture_full_screen = vec!["Ctrl+Shift+P".to_string()];

    let preview = MigrationPreview::for_authored_config(&config).expect("legacy pair is proposed");

    assert_eq!(
        proposed_keys(&preview),
        ["toggle_command_palette", "capture_full_screen"]
    );
    let palette = &preview.changes()[0];
    assert_eq!(palette.action_label(), "Command Palette");
    assert_eq!(palette.before(), ["Ctrl+K"]);
    assert_eq!(palette.after(), ["Ctrl+K", "Ctrl+Shift+P"]);
    let capture = &preview.changes()[1];
    assert_eq!(capture.before(), ["Ctrl+Shift+P"]);
    assert_eq!(capture.after(), ["Ctrl+Alt+F"]);
    assert_eq!(preview.proposed_revision(), CURRENT_CONFIG_REVISION);
}

/// Only the half the file spells out differently is offered: the other side
/// already reads as this build's default, so there is nothing to change there.
#[test]
fn revision_zero_proposes_only_the_legacy_half_of_the_pair() {
    let mut config = config_at_revision(0);
    config.keybindings.capture.capture_full_screen = vec!["Ctrl+Shift+P".to_string()];

    let preview =
        MigrationPreview::for_authored_config(&config).expect("legacy capture is proposed");

    assert_eq!(proposed_keys(&preview), ["capture_full_screen"]);
    assert_eq!(preview.changes()[0].after(), ["Ctrl+Alt+F"]);
}

/// The recipes gate themselves on untouched values, so a customized shortcut
/// is never part of a proposal.
#[test]
fn customized_shortcuts_are_never_proposed() {
    let mut custom_command = config_at_revision(0);
    custom_command.keybindings.ui.toggle_command_palette = vec!["Ctrl+Space".to_string()];
    custom_command.keybindings.capture.capture_full_screen = vec!["Ctrl+Shift+P".to_string()];

    assert!(MigrationPreview::for_authored_config(&custom_command).is_none());

    let mut custom_capture = config_at_revision(0);
    custom_capture.keybindings.ui.toggle_command_palette = vec!["Ctrl+K".to_string()];
    custom_capture.keybindings.capture.capture_full_screen = vec!["Ctrl+Alt+G".to_string()];

    assert!(MigrationPreview::for_authored_config(&custom_capture).is_none());
}

#[test]
fn revision_one_proposes_the_toolbar_f2_split() {
    let mut config = config_at_revision(1);
    config.keybindings.ui.toggle_toolbar = vec!["F2".to_string(), "F9".to_string()];

    let preview = MigrationPreview::for_authored_config(&config).expect("legacy pair is proposed");

    assert_eq!(proposed_keys(&preview), ["toggle_toolbar"]);
    assert_eq!(preview.changes()[0].before(), ["F2", "F9"]);
    assert_eq!(preview.changes()[0].after(), ["F9"]);
}

#[test]
fn revision_one_preserves_custom_f2_and_proposes_an_unbound_cycle_action() {
    let mut config = config_at_revision(1);
    config.keybindings.ui.toggle_toolbar = vec!["F2".to_string()];

    let preview = MigrationPreview::for_authored_config(&config)
        .expect("custom F2 needs an explicit opt-out");

    assert_eq!(proposed_keys(&preview), ["cycle_toolbar_display"]);
    assert_eq!(preview.changes()[0].before(), ["F2"]);
    assert!(preview.changes()[0].after().is_empty());
}

/// A revision-2 file settled the earlier steps already, so the only recipe
/// left is the one revision 3 introduced.
#[test]
fn revision_two_proposes_only_the_input_hud_step() {
    let mut config = config_at_revision(2);
    config.keybindings.ui.toggle_command_palette = vec!["Ctrl+K".to_string()];
    config.keybindings.ui.toggle_toolbar = vec!["F2".to_string(), "F9".to_string()];
    config.keybindings.capture.capture_clipboard_full = vec!["shift+ctrl+k".to_string()];

    let preview = MigrationPreview::for_authored_config(&config).expect("input HUD step applies");

    assert_eq!(proposed_keys(&preview), ["toggle_input_hud"]);
    assert_eq!(preview.changes()[0].before(), ["Ctrl+Shift+K"]);
    assert!(preview.changes()[0].after().is_empty());
    assert_eq!(preview.proposed_revision(), CURRENT_CONFIG_REVISION);
}

#[test]
fn revision_two_proposes_nothing_when_no_one_contests_the_input_hud_default() {
    let config = config_at_revision(2);

    assert!(MigrationPreview::for_authored_config(&config).is_none());
}

/// The preview is a proposal: the configuration it was computed from keeps
/// every value and its own revision until the user saves an applied draft.
#[test]
fn computing_a_preview_leaves_the_configuration_alone() {
    let mut config = config_at_revision(0);
    config.keybindings.ui.toggle_command_palette = vec!["Ctrl+K".to_string()];
    config.keybindings.capture.capture_full_screen = vec!["Ctrl+Shift+P".to_string()];
    let before = config.clone();

    let _ = MigrationPreview::for_authored_config(&config);

    assert_eq!(config.config_revision, before.config_revision);
    assert_eq!(
        config.keybindings.ui.toggle_command_palette,
        before.keybindings.ui.toggle_command_palette
    );
    assert_eq!(
        config.keybindings.capture.capture_full_screen,
        before.keybindings.capture.capture_full_screen
    );
}
