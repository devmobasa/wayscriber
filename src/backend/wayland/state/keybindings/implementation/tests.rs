use super::*;
use crate::config::Config;
use crate::config::io::{is_stale_source_error, persist_keybinding_edit_at};
use crate::config::persist_keybinding_edit;
use crate::config::test_helpers::with_temp_config_home;
use std::fs;
use std::path::{Path, PathBuf};

/// Written under the real (flat) `[keybindings]` keys, on a chord no
/// shipped default claims, so the reload assertion is about this run's edit
/// rather than about conflict resolution.
const AUTHORED_SHORTCUTS: &str =
    "[keybindings]\nundo = ['Ctrl+Z']\nselect_pen_tool = ['Ctrl+Alt+Shift+P']\n";

fn replace(action: Action, binding: &str) -> KeybindingEditRequest {
    KeybindingEditRequest {
        action,
        operation: KeybindingEditOperation::Replace(vec![binding.to_string()]),
    }
}

/// The prepare a palette makes with nothing outstanding.
///
/// Spelled out here rather than at every call site because it is the
/// assumption most of these tests are making: an empty projection is a run
/// whose queue is empty, where the running keymap is the whole of what the
/// check has to go on.
fn prepare(
    keybindings: &KeybindingsConfig,
    request: KeybindingEditRequest,
) -> Result<KeybindingEditWrite, String> {
    prepare_keybinding_edit(keybindings, &[], request)
}

/// The same, with edits already queued and unanswered.
fn prepare_behind(
    keybindings: &KeybindingsConfig,
    in_flight: &[(Action, &str)],
    request: KeybindingEditRequest,
) -> Result<KeybindingEditWrite, String> {
    let in_flight: Vec<ProjectedShortcut> = in_flight
        .iter()
        .map(|(action, binding)| ProjectedShortcut {
            action: *action,
            bindings: vec![(*binding).to_string()],
        })
        .collect();
    prepare_keybinding_edit(keybindings, &in_flight, request)
}

mod completion;
mod persistence;
mod preparation;
