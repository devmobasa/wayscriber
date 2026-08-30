use wayscriber::config::action_label;
use wayscriber::config::action_meta_iter;
use wayscriber::config::keybindings::KeybindingsConfig;

use super::draft::KeybindingsDraft;
use super::field::{KeybindingField, keybinding_fields, keybinding_tab};
use super::parse::{parse_keybinding_list, parse_keybindings};
use crate::models::KeybindingsTabId;

#[test]
fn parse_keybinding_list_trims_and_ignores_empty() {
    let parsed = parse_keybinding_list(" Ctrl+Z, , Alt+K ").expect("parse succeeds");
    assert_eq!(parsed, vec!["Ctrl+Z".to_string(), "Alt+K".to_string()]);
}

#[test]
fn parse_keybinding_list_accepts_a_sequence_beside_a_single() {
    let parsed = parse_keybindings("F5, Ctrl+K > Ctrl+C").expect("parse succeeds");
    assert_eq!(parsed[0].to_string(), "F5");
    assert_eq!(parsed[1].to_string(), "Ctrl+K > Ctrl+C");
    assert_eq!(parsed[1].display_label(), "Ctrl+K then Ctrl+C");
}

#[test]
fn parse_keybinding_list_rejects_unparseable_shortcuts() {
    let error = parse_keybinding_list("Ctrl+Shift, Escape").expect_err("modifiers-only is invalid");
    assert!(
        error.contains("No key specified"),
        "parser error should name the problem: {error}"
    );
}

#[test]
fn presentation_order_covers_each_generated_keybinding_field_once() {
    let fields = keybinding_fields();
    let unique: std::collections::HashSet<_> = fields.iter().copied().collect();

    assert_eq!(unique.len(), fields.len());
    assert_eq!(
        unique.len(),
        wayscriber::config::ConfigurableAction::all().len()
    );
    for field in wayscriber::config::ConfigurableAction::all() {
        assert!(
            unique.contains(field),
            "missing presentation row for {field:?}"
        );
    }
}

#[test]
fn keybindings_draft_to_config_rejects_unparseable_shortcuts() {
    let mut draft = KeybindingsDraft::from_config(&KeybindingsConfig::default());
    draft.set(KeybindingField::Exit, "Ctrl+Shift".to_string());

    let errors = draft
        .to_config()
        .expect_err("unparseable shortcuts must not save");
    assert!(
        errors
            .iter()
            .any(|err| err.field == "keybindings.exit" && err.message.contains("No key specified")),
        "expected a field error for exit, got {errors:?}"
    );
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
fn command_palette_and_full_screen_capture_fields_expose_current_defaults() {
    let config = KeybindingsConfig::default();

    assert_eq!(
        KeybindingField::ToggleCommandPalette.get(&config),
        &vec!["Ctrl+K".to_string(), "Ctrl+Shift+P".to_string()]
    );
    assert_eq!(
        KeybindingField::CaptureFullScreen.get(&config),
        &vec!["Ctrl+Alt+F".to_string()]
    );
}

#[test]
fn board_pdf_export_keybinding_field_is_visible_and_in_capture_tab() {
    assert!(
        keybinding_fields().contains(&KeybindingField::ExportBoardPdfFile),
        "PDF export field should appear in ordered keybinding list"
    );
    assert!(
        keybinding_fields().contains(&KeybindingField::ExportAllBoardsPdfFile),
        "All-board PDF export field should appear in ordered keybinding list"
    );
    assert_eq!(
        keybinding_tab(KeybindingField::ExportBoardPdfFile),
        KeybindingsTabId::CaptureView
    );
    assert_eq!(
        keybinding_tab(KeybindingField::ExportAllBoardsPdfFile),
        KeybindingsTabId::CaptureView
    );
}

#[test]
fn screen_eyedropper_keybinding_field_is_visible_and_in_drawing_tab() {
    assert!(keybinding_fields().contains(&KeybindingField::PickScreenColor));
    assert_eq!(
        keybinding_tab(KeybindingField::PickScreenColor),
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
fn chrome_visibility_keybinding_fields_are_visible_and_in_ui_tab() {
    assert!(keybinding_fields().contains(&KeybindingField::ToggleFloatingBadge));
    assert!(keybinding_fields().contains(&KeybindingField::ToggleZoomChip));
    assert_eq!(
        keybinding_tab(KeybindingField::ToggleFloatingBadge),
        KeybindingsTabId::UiModes
    );
    assert_eq!(
        keybinding_tab(KeybindingField::ToggleZoomChip),
        KeybindingsTabId::UiModes
    );
}

#[test]
fn chrome_visibility_keybinding_fields_read_and_write_config() {
    let mut config = KeybindingsConfig::default();
    // Unbound by default (palette-first actions), but a custom binding must
    // survive the configurator's read → edit → write round trip.
    assert!(KeybindingField::ToggleFloatingBadge.get(&config).is_empty());
    assert!(KeybindingField::ToggleZoomChip.get(&config).is_empty());

    KeybindingField::ToggleFloatingBadge.set(&mut config, vec!["Ctrl+Shift+B".to_string()]);
    assert_eq!(
        config.ui.toggle_floating_badge,
        vec!["Ctrl+Shift+B".to_string()]
    );

    KeybindingField::ToggleZoomChip.set(&mut config, vec!["Ctrl+Shift+Z".to_string()]);
    assert_eq!(config.ui.toggle_zoom_chip, vec!["Ctrl+Shift+Z".to_string()]);
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

#[test]
fn interactive_region_capture_keybinding_is_visible_and_round_trips() {
    let field = KeybindingField::CaptureRegionInteractive;
    let mut config = KeybindingsConfig::default();

    assert!(keybinding_fields().contains(&field));
    assert_eq!(keybinding_tab(field), KeybindingsTabId::CaptureView);
    assert_eq!(field.label(), "Capture Region (Interactive)");
    assert_eq!(
        field.get(&config),
        &vec!["Ctrl+Shift+C".to_string()],
        "the shipped default"
    );

    field.set(&mut config, vec!["Ctrl+Alt+Shift+I".to_string()]);
    assert_eq!(
        config.capture.capture_region_interactive,
        ["Ctrl+Alt+Shift+I"]
    );
}

#[test]
fn measure_mode_keybinding_is_visible_and_round_trips() {
    let field = KeybindingField::MeasureMode;
    let mut config = KeybindingsConfig::default();

    assert!(keybinding_fields().contains(&field));
    assert_eq!(keybinding_tab(field), KeybindingsTabId::CaptureView);
    assert_eq!(field.label(), "Measure Mode");
    assert!(field.get(&config).is_empty());

    field.set(&mut config, vec!["Ctrl+Alt+M".to_string()]);
    assert_eq!(config.capture.measure_mode, ["Ctrl+Alt+M"]);
}

/// Saving in the configurator rebuilds `KeybindingsConfig` from `default()` and
/// writes back only the fields present in `keybinding_fields()`. A binding
/// missing from that registry is therefore silently reset to its default the
/// next time the user saves *any* unrelated setting.
///
/// This walks every configurable action, so a new binding added to the main
/// crate without a matching `KeybindingField` fails here rather than quietly
/// eating the user's config.
#[test]
fn every_configurable_action_survives_a_configurator_save() {
    let mut source = KeybindingsConfig::default();
    let mut expected = Vec::new();

    for meta in action_meta_iter() {
        if source.bindings_for_action(meta.action).is_none() {
            continue; // runtime-only action with no persisted field
        }
        // A value no default uses, so a reset to defaults is unmistakable.
        let sentinel = format!("Ctrl+Alt+Shift+F{}", expected.len() % 12 + 1);
        source
            .set_bindings_for_action(meta.action, vec![sentinel.clone()])
            .expect("action reported as configurable");
        expected.push((meta.action, sentinel));
    }

    let saved = KeybindingsDraft::from_config(&source)
        .to_config()
        .expect("draft round-trips without parse errors");

    let mut lost = Vec::new();
    for (action, sentinel) in expected {
        let bindings = saved
            .bindings_for_action(action)
            .expect("action is configurable");
        if bindings != [sentinel.as_str()] {
            lost.push(action);
        }
    }

    assert!(
        lost.is_empty(),
        "these actions have no KeybindingField, so saving would erase the user's \
         hand-written bindings: {lost:?}"
    );
}

/// The migration preview names its changes by the main crate's config key, so
/// a key with no field here is a proposal the user would accept and never
/// receive.
#[test]
fn every_configurable_config_key_resolves_to_a_field() {
    let mut unmapped = Vec::new();
    for action in KeybindingsConfig::configurable_actions() {
        let Some(key) = KeybindingsConfig::config_key_for_action(*action) else {
            continue; // runtime-only action with no persisted field
        };
        if KeybindingField::from_field_key(key).is_none() {
            unmapped.push(key);
        }
    }

    assert!(
        unmapped.is_empty(),
        "these `[keybindings]` keys have no KeybindingField, so an applied \
         migration would silently skip them: {unmapped:?}"
    );
}

#[test]
fn every_keybinding_field_label_matches_action_meta() {
    let mut mismatched = Vec::new();
    for field in keybinding_fields() {
        let action = field.action();
        if field.label() != action_label(action) {
            mismatched.push(field.field_key());
        }
    }
    assert!(
        mismatched.is_empty(),
        "these fields still use a private label instead of action_meta: {mismatched:?}"
    );
}

#[test]
fn blur_style_and_spotlight_bindings_read_and_write_config() {
    let mut config = KeybindingsConfig::default();
    config.tools.cycle_blur_style = vec!["Ctrl+Shift+Y".to_string()];
    config.tools.select_spotlight_tool = vec!["Ctrl+Shift+G".to_string()];

    let draft = KeybindingsDraft::from_config(&config);
    assert_eq!(
        draft.value_for(KeybindingField::CycleBlurStyle),
        Some("Ctrl+Shift+Y")
    );
    assert_eq!(
        draft.value_for(KeybindingField::SelectSpotlightTool),
        Some("Ctrl+Shift+G")
    );

    let saved = draft.to_config().expect("round-trip");
    assert_eq!(
        saved.tools.cycle_blur_style,
        vec!["Ctrl+Shift+Y".to_string()]
    );
    assert_eq!(
        saved.tools.select_spotlight_tool,
        vec!["Ctrl+Shift+G".to_string()]
    );
}
