use wayscriber::config::keybindings::KeybindingsConfig;

use super::draft::KeybindingsDraft;
use super::field::KeybindingField;
use super::parse::parse_keybinding_list;
use crate::models::KeybindingsTabId;

#[test]
fn parse_keybinding_list_trims_and_ignores_empty() {
    let parsed = parse_keybinding_list(" Ctrl+Z, , Alt+K ").expect("parse succeeds");
    assert_eq!(parsed, vec!["Ctrl+Z".to_string(), "Alt+K".to_string()]);
}

#[test]
fn keybindings_draft_to_config_updates_fields() {
    let mut draft = KeybindingsDraft::from_config(&KeybindingsConfig::default());
    draft.set(KeybindingField::Exit, "Ctrl+Q, Escape".to_string());

    let config = draft.to_config().expect("to_config should succeed");
    assert_eq!(
        config.core.exit,
        vec!["Ctrl+Q".to_string(), "Escape".to_string()]
    );
}

#[test]
fn board_pdf_export_keybinding_field_is_visible_and_in_capture_tab() {
    assert!(
        KeybindingField::all().contains(&KeybindingField::ExportBoardPdfFile),
        "PDF export field should appear in ordered keybinding list"
    );
    assert!(
        KeybindingField::all().contains(&KeybindingField::ExportAllBoardsPdfFile),
        "All-board PDF export field should appear in ordered keybinding list"
    );
    assert_eq!(
        KeybindingField::ExportBoardPdfFile.tab(),
        KeybindingsTabId::CaptureView
    );
    assert_eq!(
        KeybindingField::ExportAllBoardsPdfFile.tab(),
        KeybindingsTabId::CaptureView
    );
}

#[test]
fn screen_eyedropper_keybinding_field_is_visible_and_in_drawing_tab() {
    assert!(KeybindingField::all().contains(&KeybindingField::PickScreenColor));
    assert_eq!(
        KeybindingField::PickScreenColor.tab(),
        KeybindingsTabId::Drawing
    );
}

#[test]
fn screen_eyedropper_keybinding_field_reads_and_writes_config() {
    let mut config = KeybindingsConfig::default();
    assert_eq!(
        KeybindingField::PickScreenColor.get(&config),
        &vec!["I".to_string()]
    );

    KeybindingField::PickScreenColor.set(&mut config, vec!["Ctrl+Shift+P".to_string()]);

    assert_eq!(
        config.colors.pick_screen_color,
        vec!["Ctrl+Shift+P".to_string()]
    );
}

#[test]
fn board_pdf_export_keybinding_field_reads_and_writes_config() {
    let mut config = KeybindingsConfig::default();
    assert!(KeybindingField::ExportBoardPdfFile.get(&config).is_empty());

    KeybindingField::ExportBoardPdfFile.set(&mut config, vec!["Ctrl+Alt+P".to_string()]);

    assert_eq!(
        config.capture.export_board_pdf_file,
        vec!["Ctrl+Alt+P".to_string()]
    );

    KeybindingField::ExportAllBoardsPdfFile.set(&mut config, vec!["Ctrl+Alt+A".to_string()]);
    assert_eq!(
        config.capture.export_all_boards_pdf_file,
        vec!["Ctrl+Alt+A".to_string()]
    );
}
