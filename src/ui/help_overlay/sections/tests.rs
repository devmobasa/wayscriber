use super::{HelpOverlayBindings, build_section_sets};
use crate::config::{Action, action_label};

#[test]
fn gesture_hints_remain_present() {
    let bindings = HelpOverlayBindings::default();
    let sections = build_section_sets(&bindings, false, false, true, true).all;
    let rows: Vec<(&str, &str)> = sections
        .iter()
        .flat_map(|section| section.rows.iter())
        .map(|row| (row.key.as_str(), row.action))
        .collect();

    let expected = [
        ("Shift+Drag", action_label(Action::SelectLineTool)),
        ("Ctrl+Drag", action_label(Action::SelectRectTool)),
        ("Tab+Drag", action_label(Action::SelectEllipseTool)),
        ("Ctrl+Shift+Drag", action_label(Action::SelectArrowTool)),
        ("Drag", "Selection tool"),
        ("Ctrl+Shift+Alt+Left/Right", "Previous/next output"),
        ("Selection properties panel", "Text background"),
        ("Middle drag / arrow keys", "Pan view"),
        ("Middle Click", action_label(Action::ToggleRadialMenu)),
    ];

    for (key, action) in expected {
        assert!(
            rows.iter()
                .any(|(row_key, row_action)| *row_key == key && *row_action == action),
            "Missing gesture hint row: '{key}' -> '{action}'"
        );
    }
}
