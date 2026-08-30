use wayscriber::configurator_destination::keybindings_section_for_action;

use super::KeybindingField;
use crate::models::KeybindingsTabId;

pub fn keybinding_tab(field: KeybindingField) -> KeybindingsTabId {
    keybindings_section_for_action(field.action())
        .expect("every configurable action has a Keybindings section")
        .into()
}
