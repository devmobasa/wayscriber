use super::super::types::{Badge, Section, row};
use super::{HelpOverlayBindings, build_section_sets, hide_unbound_rows};
use crate::config::{Action, action_label};
use crate::label_format::NOT_BOUND_LABEL;

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

#[test]
fn canvas_export_rows_remain_visible_when_capture_context_is_disabled() {
    let bindings = HelpOverlayBindings::default();
    let sections = build_section_sets(&bindings, false, true, true, false).all;
    let rows: Vec<&str> = sections
        .iter()
        .flat_map(|section| section.rows.iter())
        .map(|row| row.action)
        .collect();

    for action in [
        Action::ExportCanvasClipboard,
        Action::ExportCanvasFile,
        Action::ExportCanvasClipboardAndFile,
        Action::ExportBoardPdfFile,
        Action::ExportAllBoardsPdfFile,
    ] {
        assert!(
            rows.contains(&action_label(action)),
            "Missing canvas export help row for {}",
            action_label(action)
        );
    }
    assert!(
        !rows.contains(&"Full screen → clipboard"),
        "Screenshot rows should stay hidden when capture context is disabled"
    );
}

#[test]
fn interactive_region_capture_has_a_help_row_when_capture_is_enabled() {
    let bindings = HelpOverlayBindings::default();
    let sections = build_section_sets(&bindings, false, true, true, true).all;
    let rows = sections
        .iter()
        .flat_map(|section| section.rows.iter())
        .collect::<Vec<_>>();

    assert!(rows.iter().any(|row| {
        row.action == action_label(Action::CaptureRegionInteractive)
            && row.key == crate::label_format::NOT_BOUND_LABEL
    }));
}

#[test]
fn capture_free_measure_mode_stays_visible_when_capture_is_disabled() {
    let bindings = HelpOverlayBindings::default();
    let sections = build_section_sets(&bindings, false, true, true, false).all;
    let rows = sections
        .iter()
        .flat_map(|section| section.rows.iter())
        .collect::<Vec<_>>();

    assert!(rows.iter().any(|row| {
        row.action == action_label(Action::MeasureMode)
            && row.key == crate::label_format::NOT_BOUND_LABEL
    }));
}

#[test]
fn hiding_unbound_rows_keeps_badge_sections_and_drops_empty_ones() {
    let section = |title, rows, badges| Section {
        title,
        rows,
        badges,
        icon: None,
    };
    let sections = vec![
        section(
            "Mixed",
            vec![row("F", "Pen Tool"), row(NOT_BOUND_LABEL, "Blur Tool")],
            Vec::new(),
        ),
        section(
            "Unbound",
            vec![row(NOT_BOUND_LABEL, "Blur Tool")],
            Vec::new(),
        ),
        section(
            "Colors",
            vec![row(NOT_BOUND_LABEL, "Blur Tool")],
            vec![Badge {
                label: "R".into(),
                color: [1.0, 0.0, 0.0],
            }],
        ),
    ];

    let visible = hide_unbound_rows(&sections);

    let titles: Vec<&str> = visible.iter().map(|section| section.title).collect();
    assert_eq!(titles, ["Mixed", "Colors"]);
    assert_eq!(visible[0].rows.len(), 1);
    assert!(visible[1].rows.is_empty());
    assert_eq!(sections[0].rows.len(), 2, "the source sections stay intact");
}
