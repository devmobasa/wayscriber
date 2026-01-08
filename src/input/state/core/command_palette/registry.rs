//! Registry of all commands available in the command palette.

use crate::config::action_meta::{ActionMeta, action_meta_iter};

pub type CommandEntry = ActionMeta;

pub fn command_palette_entries() -> impl Iterator<Item = &'static ActionMeta> {
    action_meta_iter().filter(|meta| meta.in_command_palette)
}
