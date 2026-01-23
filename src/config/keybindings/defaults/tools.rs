pub(crate) fn default_increase_thickness() -> Vec<String> {
    vec!["+".to_string(), "=".to_string()]
}

pub(crate) fn default_decrease_thickness() -> Vec<String> {
    vec!["-".to_string(), "_".to_string()]
}

pub(crate) fn default_increase_marker_opacity() -> Vec<String> {
    vec!["Ctrl+Alt+ArrowUp".to_string()]
}

pub(crate) fn default_decrease_marker_opacity() -> Vec<String> {
    vec!["Ctrl+Alt+ArrowDown".to_string()]
}

pub(crate) fn default_select_selection_tool() -> Vec<String> {
    vec!["V".to_string()]
}

pub(crate) fn default_select_marker_tool() -> Vec<String> {
    vec!["H".to_string()]
}

pub(crate) fn default_select_eraser_tool() -> Vec<String> {
    vec!["D".to_string()]
}

pub(crate) fn default_toggle_eraser_mode() -> Vec<String> {
    vec!["Ctrl+Shift+E".to_string()]
}

pub(crate) fn default_select_pen_tool() -> Vec<String> {
    vec!["F".to_string()]
}

pub(crate) fn default_select_line_tool() -> Vec<String> {
    Vec::new()
}

pub(crate) fn default_select_rect_tool() -> Vec<String> {
    Vec::new()
}

pub(crate) fn default_select_ellipse_tool() -> Vec<String> {
    Vec::new()
}

pub(crate) fn default_select_arrow_tool() -> Vec<String> {
    Vec::new()
}

pub(crate) fn default_select_highlight_tool() -> Vec<String> {
    Vec::new()
}

pub(crate) fn default_toggle_highlight_tool() -> Vec<String> {
    vec!["Ctrl+Alt+H".to_string()]
}

pub(crate) fn default_increase_font_size() -> Vec<String> {
    vec!["Ctrl+Shift++".to_string(), "Ctrl+Shift+=".to_string()]
}

pub(crate) fn default_decrease_font_size() -> Vec<String> {
    vec!["Ctrl+Shift+-".to_string(), "Ctrl+Shift+_".to_string()]
}

pub(crate) fn default_reset_arrow_labels() -> Vec<String> {
    vec!["Ctrl+Shift+R".to_string()]
}
