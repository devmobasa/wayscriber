use super::super::color::ColorInput;
use super::super::fields::{
    FontWeightOption, QuadField, SessionStorageModeOption, TextField, ToggleField, TripletField,
};
use super::super::{ColorMode, NamedColorOption};
use super::ConfigDraft;
use wayscriber::config::{ColorSpec, Config, ToolPresetConfig};
use wayscriber::input::Tool;

#[test]
fn config_draft_to_config_reports_errors() {
    let mut draft = ConfigDraft::from_config(&Config::default());
    draft.drawing_default_thickness = "nope".to_string();
    draft.click_highlight_duration_ms = "nan".to_string();
    draft.drawing_color = ColorInput {
        mode: ColorMode::Named,
        name: " ".to_string(),
        rgb: ["0".to_string(), "0".to_string(), "0".to_string()],
        selected_named: NamedColorOption::Custom,
    };

    let errors = draft
        .to_config(&Config::default())
        .expect_err("expected validation errors");
    let fields: Vec<&str> = errors.iter().map(|err| err.field.as_str()).collect();

    assert!(fields.contains(&"drawing.default_thickness"));
    assert!(fields.contains(&"ui.click_highlight.duration_ms"));
    assert!(fields.contains(&"drawing.default_color"));
}

#[test]
fn config_draft_to_config_trims_custom_directory() {
    let mut draft = ConfigDraft::from_config(&Config::default());
    draft.session_storage_mode = SessionStorageModeOption::Custom;
    draft.session_custom_directory = "   ".to_string();

    let config = draft
        .to_config(&Config::default())
        .expect("to_config should succeed");
    assert!(config.session.custom_directory.is_none());
}

#[test]
fn setters_update_draft_state() {
    let mut draft = ConfigDraft::from_config(&Config::default());

    draft.set_text(TextField::DrawingFontWeight, "weird".to_string());
    assert_eq!(draft.drawing_font_weight, "weird");
    assert_eq!(draft.drawing_font_weight_option, FontWeightOption::Custom);

    draft.set_text(TextField::DrawingColorName, "green".to_string());
    assert_eq!(draft.drawing_color.name, "green");
    assert_eq!(draft.drawing_color.selected_named, NamedColorOption::Green);

    draft.set_triplet(TripletField::DrawingColorRgb, 1, "0.5".to_string());
    assert_eq!(draft.drawing_color.rgb[1], "0.5");

    draft.set_quad(QuadField::StatusBarBg, 2, "0.75".to_string());
    assert_eq!(draft.status_bar_bg_color.components[2], "0.75");

    draft.set_toggle(ToggleField::BoardsAutoCreate, true);
    draft.set_toggle(ToggleField::ArrowHeadAtEnd, true);
    assert!(draft.boards.auto_create);
    assert!(draft.arrow_head_at_end);
}

#[test]
fn config_draft_round_trips_presets_and_history() {
    let mut config = Config::default();
    config.history.undo_all_delay_ms = 500;
    config.history.redo_all_delay_ms = 700;
    config.history.custom_section_enabled = true;
    config.history.custom_undo_delay_ms = 200;
    config.history.custom_redo_delay_ms = 300;
    config.history.custom_undo_steps = 12;
    config.history.custom_redo_steps = 9;

    config.presets.slot_count = 3;
    let preset = ToolPresetConfig {
        name: Some("Primary".to_string()),
        tool: Tool::Pen,
        color: ColorSpec::Name("blue".to_string()),
        size: 5.0,
        eraser_kind: None,
        eraser_mode: None,
        marker_opacity: Some(0.5),
        fill_enabled: Some(true),
        font_size: Some(14.0),
        text_background_enabled: Some(false),
        arrow_length: Some(20.0),
        arrow_angle: Some(30.0),
        arrow_head_at_end: Some(true),
        show_status_bar: Some(false),
    };
    config.presets.set_slot(1, Some(preset));

    let draft = ConfigDraft::from_config(&config);
    let round_trip = draft
        .to_config(&config)
        .expect("expected config to round trip");

    assert_eq!(
        round_trip.history.undo_all_delay_ms,
        config.history.undo_all_delay_ms
    );
    assert_eq!(
        round_trip.history.redo_all_delay_ms,
        config.history.redo_all_delay_ms
    );
    assert_eq!(
        round_trip.history.custom_section_enabled,
        config.history.custom_section_enabled
    );
    assert_eq!(
        round_trip.history.custom_undo_delay_ms,
        config.history.custom_undo_delay_ms
    );
    assert_eq!(
        round_trip.history.custom_redo_delay_ms,
        config.history.custom_redo_delay_ms
    );
    assert_eq!(
        round_trip.history.custom_undo_steps,
        config.history.custom_undo_steps
    );
    assert_eq!(
        round_trip.history.custom_redo_steps,
        config.history.custom_redo_steps
    );
    assert_eq!(round_trip.presets.slot_count, config.presets.slot_count);
    assert_eq!(round_trip.presets.get_slot(1), config.presets.get_slot(1));
}
