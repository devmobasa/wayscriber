use wayscriber::config::keybindings::KeybindingsConfig;

use super::draft::KeybindingsDraft;
use super::field::KeybindingField;
use super::parse::parse_keybinding_list;

#[test]
fn parse_keybinding_list_trims_and_ignores_empty() {
    let parsed = parse_keybinding_list(" Ctrl+Z, , Alt+K ").expect("parse succeeds");
    assert_eq!(parsed, vec!["Ctrl+Z".to_string(), "Alt+K".to_string()]);
}

#[test]
fn keybindings_draft_to_config_updates_fields() {
    let mut draft = KeybindingsDraft::from_config(&KeybindingsConfig::default());
    draft.set(KeybindingField::Exit, "Ctrl+Q, Escape".to_string());

    let config = draft.to_config().expect("to_config should succeed");
    assert_eq!(
        config.exit,
        vec!["Ctrl+Q".to_string(), "Escape".to_string()]
    );
}
