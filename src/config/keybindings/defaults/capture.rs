pub(crate) fn default_capture_full_screen() -> Vec<String> {
    vec!["Ctrl+Alt+F".to_string()]
}

pub(crate) fn default_capture_active_window() -> Vec<String> {
    vec!["Ctrl+Shift+O".to_string()]
}

pub(crate) fn default_capture_selection() -> Vec<String> {
    vec!["Ctrl+Shift+I".to_string()]
}

pub(crate) fn default_capture_clipboard_full() -> Vec<String> {
    vec!["Ctrl+C".to_string()]
}

pub(crate) fn default_capture_file_full() -> Vec<String> {
    vec!["Ctrl+S".to_string()]
}

/// Deliberately empty: `Ctrl+Shift+C` now opens the interactive picker, whose
/// **Copy** action (`Ctrl+C` in Review) produces the same clipboard result.
/// Bind this action explicitly to get the one-step copy back.
pub(crate) fn default_capture_clipboard_selection() -> Vec<String> {
    Vec::new()
}

pub(crate) fn default_capture_file_selection() -> Vec<String> {
    vec!["Ctrl+Shift+S".to_string()]
}

pub(crate) fn default_capture_clipboard_region() -> Vec<String> {
    vec!["Ctrl+6".to_string()]
}

pub(crate) fn default_capture_file_region() -> Vec<String> {
    vec!["Ctrl+Alt+6".to_string()]
}

/// The region chord: interactive capture is a superset of the immediate
/// region-to-clipboard action it replaces here, since Review's **Copy**
/// reaches the same result with one more keystroke.
pub(crate) fn default_capture_region_interactive() -> Vec<String> {
    vec!["Ctrl+Shift+C".to_string()]
}

/// Deliberately empty: measure mode is palette-first until the user chooses a
/// shortcut that fits their compositor and existing bindings.
pub(crate) fn default_measure_mode() -> Vec<String> {
    Vec::new()
}

pub(crate) fn default_export_canvas_file() -> Vec<String> {
    Vec::new()
}

pub(crate) fn default_export_canvas_clipboard() -> Vec<String> {
    Vec::new()
}

pub(crate) fn default_export_canvas_clipboard_and_file() -> Vec<String> {
    Vec::new()
}

pub(crate) fn default_export_board_pdf_file() -> Vec<String> {
    Vec::new()
}

pub(crate) fn default_export_all_boards_pdf_file() -> Vec<String> {
    Vec::new()
}

pub(crate) fn default_open_capture_folder() -> Vec<String> {
    vec!["Ctrl+Alt+O".to_string()]
}

/// Deliberately empty: `O` is the orange quick color and no other
/// conflict-free chord is obviously right, so the user picks one.
pub(crate) fn default_copy_text_from_screen() -> Vec<String> {
    Vec::new()
}
