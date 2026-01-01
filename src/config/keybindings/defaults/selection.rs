pub(crate) fn default_duplicate_selection() -> Vec<String> {
    vec!["Ctrl+D".to_string()]
}

pub(crate) fn default_copy_selection() -> Vec<String> {
    vec!["Ctrl+Alt+C".to_string()]
}

pub(crate) fn default_paste_selection() -> Vec<String> {
    vec!["Ctrl+Alt+V".to_string()]
}

pub(crate) fn default_select_all() -> Vec<String> {
    vec!["Ctrl+A".to_string()]
}

pub(crate) fn default_move_selection_to_front() -> Vec<String> {
    vec!["]".to_string()]
}

pub(crate) fn default_move_selection_to_back() -> Vec<String> {
    vec!["[".to_string()]
}

pub(crate) fn default_nudge_selection_up() -> Vec<String> {
    vec!["ArrowUp".to_string()]
}

pub(crate) fn default_nudge_selection_down() -> Vec<String> {
    vec!["ArrowDown".to_string()]
}

pub(crate) fn default_nudge_selection_left() -> Vec<String> {
    vec!["ArrowLeft".to_string(), "Shift+PageUp".to_string()]
}

pub(crate) fn default_nudge_selection_right() -> Vec<String> {
    vec!["ArrowRight".to_string(), "Shift+PageDown".to_string()]
}

pub(crate) fn default_nudge_selection_up_large() -> Vec<String> {
    vec!["PageUp".to_string()]
}

pub(crate) fn default_nudge_selection_down_large() -> Vec<String> {
    vec!["PageDown".to_string()]
}

pub(crate) fn default_move_selection_to_start() -> Vec<String> {
    vec!["Home".to_string()]
}

pub(crate) fn default_move_selection_to_end() -> Vec<String> {
    vec!["End".to_string()]
}

pub(crate) fn default_move_selection_to_top() -> Vec<String> {
    vec!["Ctrl+Home".to_string()]
}

pub(crate) fn default_move_selection_to_bottom() -> Vec<String> {
    vec!["Ctrl+End".to_string()]
}

pub(crate) fn default_delete_selection() -> Vec<String> {
    vec!["Delete".to_string()]
}
