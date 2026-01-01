// Default keybinding lists.
pub(crate) fn default_exit() -> Vec<String> {
    vec!["Escape".to_string(), "Ctrl+Q".to_string()]
}

pub(crate) fn default_enter_text_mode() -> Vec<String> {
    vec!["T".to_string()]
}

pub(crate) fn default_enter_sticky_note_mode() -> Vec<String> {
    vec!["N".to_string()]
}

pub(crate) fn default_clear_canvas() -> Vec<String> {
    vec!["E".to_string()]
}

pub(crate) fn default_undo() -> Vec<String> {
    vec!["Ctrl+Z".to_string()]
}

pub(crate) fn default_redo() -> Vec<String> {
    vec!["Ctrl+Shift+Z".to_string(), "Ctrl+Y".to_string()]
}
