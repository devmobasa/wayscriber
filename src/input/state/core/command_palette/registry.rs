//! Registry of all commands available in the command palette.

use crate::config::action_meta::{ACTION_META, ActionMeta};

pub type CommandEntry = ActionMeta;

pub fn command_palette_entries() -> impl Iterator<Item = &'static ActionMeta> {
    ACTION_META.iter().filter(|meta| meta.in_command_palette)
}
