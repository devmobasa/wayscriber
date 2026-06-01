use super::super::color::ColorInput;
use super::super::fields::{
    DragMouseButton, DragToolField, DragToolOption, FontWeightOption, PdfFitModeOption,
    PdfLabelContentModeOption, PdfOrientationOption, PdfPageSizeOption,
    PdfTransparentBackgroundOption, QuadField, SessionStorageModeOption, TextField, ToggleField,
    ToolOption, TripletField,
};
use super::super::{ColorMode, NamedColorOption};
use super::{ConfigDraft, RenderProfileSelectionOption};
use wayscriber::config::{
    ColorSpec, Config, PdfFitMode, PdfLabelContentMode, PdfLabelPosition, PdfOrientation,
    PdfPageSize, PdfTransparentBackground, PresetToolStatesConfig, RenderColorMappingConfig,
    RenderProfileConfig, RenderProfileExportMode, ToolPresetConfig, XdgFocusLossBehavior,
};
use wayscriber::input::{DragTool, PerToolDrawingSettings, Tool};

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
fn config_draft_round_trips_light_mode_click_highlight_policy() {
    let mut config = Config::default();
    config.ui.click_highlight.force_in_light_mode = false;

    let draft = ConfigDraft::from_config(&config);
    assert!(!draft.click_highlight_force_in_light_mode);

    let round_trip = draft
        .to_config(&Config::default())
        .expect("expected config to round trip");
    assert!(!round_trip.ui.click_highlight.force_in_light_mode);
}

#[test]
fn config_draft_round_trips_render_profiles() {
    let mut config = Config::default();
    config.render_profiles.active = Some("print".to_string());
    config.render_profiles.apply_to_canvas = true;
    config.render_profiles.apply_to_ui = false;
    config.render_profiles.export = RenderProfileExportMode::Profile;
    config.render_profiles.export_profile = Some("export".to_string());
    config.render_profiles.profiles = vec![
        RenderProfileConfig {
            id: "print".to_string(),
            name: "Print".to_string(),
            mappings: vec![RenderColorMappingConfig {
                from: "#000000".to_string(),
                to: "#FFFFFF".to_string(),
            }],
        },
        RenderProfileConfig {
            id: "export".to_string(),
            name: "Export".to_string(),
            mappings: vec![RenderColorMappingConfig {
                from: "#FF0000".to_string(),
                to: "#00FF00".to_string(),
            }],
        },
    ];

    let draft = ConfigDraft::from_config(&config);
    let round_trip = draft
        .to_config(&config)
        .expect("expected render profiles to round trip");

    assert_eq!(round_trip.render_profiles.active.as_deref(), Some("print"));
    assert!(!round_trip.render_profiles.apply_to_ui);
    assert_eq!(
        round_trip.render_profiles.export,
        RenderProfileExportMode::Profile
    );
    assert_eq!(
        round_trip.render_profiles.export_profile.as_deref(),
        Some("export")
    );
    assert_eq!(round_trip.render_profiles.profiles.len(), 2);
    assert_eq!(
        round_trip.render_profiles.profiles[0].mappings[0].from,
        "#000000"
    );
}

#[test]
fn config_draft_round_trips_pdf_export() {
    let mut config = Config::default();
    config.export.pdf.filename_template = Some("board_%Y".to_string());
    config.export.pdf.all_boards_filename_template = Some("all_%Y".to_string());
    config.export.pdf.page_size = PdfPageSize::Letter;
    config.export.pdf.orientation = PdfOrientation::Landscape;
    config.export.pdf.fit = PdfFitMode::FitContentToPage;
    config.export.pdf.transparent_background = PdfTransparentBackground::Desktop;
    config.export.pdf.custom_width = 900.0;
    config.export.pdf.custom_height = 700.0;
    config.export.pdf.content_source_padding = 32.0;
    config.export.pdf.labels.enabled = true;
    config.export.pdf.labels.position = PdfLabelPosition::TopRight;
    config.export.pdf.labels.content = PdfLabelContentMode::BoardAndPage;
    config.export.pdf.labels.template = "{board_name} {page}/{pages}".to_string();
    config.export.pdf.labels.font_family = "Serif".to_string();
    config.export.pdf.labels.font_size = 12.0;
    config.export.pdf.labels.margin = 18.0;
    config.export.pdf.labels.padding_x = 7.0;
    config.export.pdf.labels.padding_y = 4.0;
    config.export.pdf.labels.text_color = [0.2, 0.3, 0.4, 0.9];
    config.export.pdf.labels.background_enabled = false;
    config.export.pdf.labels.background_color = [0.8, 0.7, 0.6, 0.5];

    let draft = ConfigDraft::from_config(&config);
    assert_eq!(draft.export_pdf_page_size, PdfPageSizeOption::Letter);
    assert_eq!(
        draft.export_pdf_orientation,
        PdfOrientationOption::Landscape
    );
    assert_eq!(draft.export_pdf_fit, PdfFitModeOption::FitContentToPage);
    assert_eq!(
        draft.export_pdf_transparent_background,
        PdfTransparentBackgroundOption::Desktop
    );

    let round_trip = draft
        .to_config(&Config::default())
        .expect("expected PDF export to round trip");

    assert_eq!(
        round_trip.export.pdf.filename_template.as_deref(),
        Some("board_%Y")
    );
    assert_eq!(
        round_trip
            .export
            .pdf
            .all_boards_filename_template
            .as_deref(),
        Some("all_%Y")
    );
    assert_eq!(round_trip.export.pdf.page_size, PdfPageSize::Letter);
    assert_eq!(round_trip.export.pdf.orientation, PdfOrientation::Landscape);
    assert_eq!(round_trip.export.pdf.fit, PdfFitMode::FitContentToPage);
    assert_eq!(
        round_trip.export.pdf.transparent_background,
        PdfTransparentBackground::Desktop
    );
    assert_eq!(round_trip.export.pdf.content_source_padding, 32.0);
    assert!(round_trip.export.pdf.labels.enabled);
    assert_eq!(
        round_trip.export.pdf.labels.position,
        PdfLabelPosition::TopRight
    );
    assert_eq!(
        round_trip.export.pdf.labels.content,
        PdfLabelContentMode::BoardAndPage
    );
    assert!(!round_trip.export.pdf.labels.background_enabled);
    assert_eq!(
        round_trip.export.pdf.labels.text_color,
        [0.2, 0.3, 0.4, 0.9]
    );
    assert_eq!(
        round_trip.export.pdf.labels.background_color,
        [0.8, 0.7, 0.6, 0.5]
    );
    assert_eq!(
        round_trip.export.pdf.labels.template,
        "{board_name} {page}/{pages}"
    );
}

#[test]
fn config_draft_blocks_invalid_pdf_export_values() {
    let mut draft = ConfigDraft::from_config(&Config::default());
    draft.export_pdf_label_template = "{missing}".to_string();
    draft.export_pdf_custom_width = "0".to_string();
    draft.export_pdf_content_source_padding = "-1".to_string();
    draft.export_pdf_label_font_size = "nan".to_string();
    draft
        .export_pdf_label_text_color
        .set_component(2, "nope".to_string());
    draft
        .export_pdf_label_background_color
        .set_component(0, "1.5".to_string());

    let errors = draft
        .to_config(&Config::default())
        .expect_err("expected PDF validation errors");
    let fields: Vec<&str> = errors.iter().map(|err| err.field.as_str()).collect();

    assert!(fields.contains(&"export.pdf.labels.template"));
    assert!(fields.contains(&"export.pdf.custom_width"));
    assert!(fields.contains(&"export.pdf.content_source_padding"));
    assert!(fields.contains(&"export.pdf.labels.font_size"));
    assert!(fields.contains(&"export.pdf.labels.text_color[2]"));
    assert!(fields.contains(&"export.pdf.labels.background_color[0]"));
}

#[test]
fn config_draft_blocks_empty_custom_pdf_template() {
    let mut draft = ConfigDraft::from_config(&Config::default());
    draft.export_pdf_label_content = PdfLabelContentModeOption::CustomTemplate;
    draft.export_pdf_label_template = "   ".to_string();

    let errors = draft
        .to_config(&Config::default())
        .expect_err("expected empty template validation error");

    assert!(errors.iter().any(|err| {
        err.field == "export.pdf.labels.template" && err.message.contains("non-empty")
    }));
}

#[test]
fn config_draft_ignores_invalid_pdf_template_for_non_custom_label_content() {
    let mut draft = ConfigDraft::from_config(&Config::default());
    draft.export_pdf_label_content = PdfLabelContentModeOption::DocumentPage;
    draft.export_pdf_label_template = "{missing}".to_string();

    let round_trip = draft
        .to_config(&Config::default())
        .expect("non-custom label content should ignore template errors");

    assert_eq!(
        round_trip.export.pdf.labels.content,
        PdfLabelContentMode::DocumentPage
    );
    assert_eq!(
        round_trip.export.pdf.labels.template,
        Config::default().export.pdf.labels.template
    );
}

#[test]
fn render_profile_selection_options_include_selectable_off() {
    let ids = vec!["print".to_string(), "projector".to_string()];

    assert_eq!(
        RenderProfileSelectionOption::list(&ids),
        vec![
            RenderProfileSelectionOption::Off,
            RenderProfileSelectionOption::Profile("print".to_string()),
            RenderProfileSelectionOption::Profile("projector".to_string()),
        ]
    );
    assert_eq!(
        RenderProfileSelectionOption::from_active("print", &ids),
        RenderProfileSelectionOption::Profile("print".to_string())
    );
    assert_eq!(
        RenderProfileSelectionOption::from_active("missing", &ids),
        RenderProfileSelectionOption::Off
    );
    assert_eq!(RenderProfileSelectionOption::Off.profile_id(), "");
}

#[test]
fn config_draft_reports_invalid_render_profile_hex() {
    let mut draft = ConfigDraft::from_config(&Config::default());
    let mut profile = draft.render_profiles.new_profile();
    profile.mappings[0].from = "#GGGGGG".to_string();
    draft.render_profiles.profiles.push(profile);

    let errors = draft
        .to_config(&Config::default())
        .expect_err("expected invalid hex");

    assert!(
        errors
            .iter()
            .any(|error| error.field.contains("mappings[0].from"))
    );
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

    draft.set_mouse_drag_tool(
        DragMouseButton::Left,
        DragToolField::Drag,
        DragToolOption::Default,
    );
    assert_eq!(draft.drawing_drag_tools.left.drag_tool, DragTool::Pen);

    draft.set_mouse_drag_tool(
        DragMouseButton::Right,
        DragToolField::Drag,
        DragToolOption::Pen,
    );
    assert_eq!(draft.drawing_drag_tools.right.drag_tool, DragTool::Pen);
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
    let mut tool_settings =
        PerToolDrawingSettings::new(ColorSpec::Name("black".to_string()).to_color(), 3.0);
    tool_settings.line.color = ColorSpec::Name("blue".to_string()).to_color();
    tool_settings.line.thickness = 9.0;
    let line_color = ColorSpec::from(tool_settings.line.color);
    let preset = ToolPresetConfig {
        name: Some("Primary".to_string()),
        tool: Tool::Line,
        color: line_color,
        size: 9.0,
        tool_settings: Some(PresetToolStatesConfig::from_runtime(&tool_settings, 18.0)),
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
        drag_tools: None,
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

#[test]
fn preset_tool_change_loads_selected_tool_profile_values() {
    let mut config = Config::default();
    config.presets.slot_count = 3;
    let pen_color = ColorSpec::Rgb([10, 20, 30]);
    let marker_color = ColorSpec::Rgb([200, 180, 20]);
    let mut tool_settings = PerToolDrawingSettings::new(pen_color.to_color(), 3.0);
    tool_settings.marker.color = marker_color.to_color();
    tool_settings.marker.thickness = 22.0;
    config.presets.set_slot(
        1,
        Some(ToolPresetConfig {
            name: Some("Profile".to_string()),
            tool: Tool::Pen,
            color: pen_color.clone(),
            size: 3.0,
            tool_settings: Some(PresetToolStatesConfig::from_runtime(&tool_settings, 18.0)),
            eraser_kind: None,
            eraser_mode: None,
            marker_opacity: None,
            fill_enabled: None,
            font_size: None,
            text_background_enabled: None,
            arrow_length: None,
            arrow_angle: None,
            arrow_head_at_end: None,
            show_status_bar: None,
            drag_tools: None,
        }),
    );

    let mut draft = ConfigDraft::from_config(&config);
    draft
        .presets
        .slot_mut(1)
        .expect("slot")
        .set_tool(ToolOption::Marker);

    let round_trip = draft
        .to_config(&config)
        .expect("expected config to round trip");
    let preset = round_trip.presets.get_slot(1).expect("preset");
    let settings = preset.tool_settings.as_ref().expect("tool settings");

    assert_eq!(preset.tool, Tool::Marker);
    assert_eq!(preset.color, marker_color);
    assert_eq!(preset.size, 22.0);
    assert_eq!(settings.pen.color, pen_color);
    assert_eq!(settings.pen.size, 3.0);
    assert_eq!(settings.marker.color, marker_color);
    assert_eq!(settings.marker.size, 22.0);
}

#[test]
fn preset_visible_edits_update_selected_tool_profile_only() {
    let mut config = Config::default();
    config.presets.slot_count = 3;
    let pen_color = ColorSpec::Rgb([10, 20, 30]);
    let line_color = ColorSpec::Rgb([40, 50, 60]);
    let marker_color = ColorSpec::Rgb([200, 180, 20]);
    let updated_marker_color = ColorSpec::Rgb([12, 34, 56]);
    let mut tool_settings = PerToolDrawingSettings::new(pen_color.to_color(), 3.0);
    tool_settings.line.color = line_color.to_color();
    tool_settings.line.thickness = 9.0;
    tool_settings.marker.color = marker_color.to_color();
    tool_settings.marker.thickness = 22.0;
    config.presets.set_slot(
        1,
        Some(ToolPresetConfig {
            name: Some("Profile".to_string()),
            tool: Tool::Marker,
            color: marker_color,
            size: 22.0,
            tool_settings: Some(PresetToolStatesConfig::from_runtime(&tool_settings, 18.0)),
            eraser_kind: None,
            eraser_mode: None,
            marker_opacity: None,
            fill_enabled: None,
            font_size: None,
            text_background_enabled: None,
            arrow_length: None,
            arrow_angle: None,
            arrow_head_at_end: None,
            show_status_bar: None,
            drag_tools: None,
        }),
    );

    let mut draft = ConfigDraft::from_config(&config);
    let slot = draft.presets.slot_mut(1).expect("slot");
    slot.color = ColorInput::from_color(&updated_marker_color);
    slot.size = "28".to_string();

    let round_trip = draft
        .to_config(&config)
        .expect("expected config to round trip");
    let preset = round_trip.presets.get_slot(1).expect("preset");
    let settings = preset.tool_settings.as_ref().expect("tool settings");

    assert_eq!(preset.tool, Tool::Marker);
    assert_eq!(preset.color, updated_marker_color);
    assert_eq!(preset.size, 28.0);
    assert_eq!(settings.pen.color, pen_color);
    assert_eq!(settings.pen.size, 3.0);
    assert_eq!(settings.line.color, line_color);
    assert_eq!(settings.line.size, 9.0);
    assert_eq!(settings.marker.color, updated_marker_color);
    assert_eq!(settings.marker.size, 28.0);
    assert_eq!(settings.eraser_size, 18.0);
}

#[test]
fn config_draft_round_trips_drag_tool_mapping() {
    let mut config = Config::default();
    config.drawing.drag_tool = Tool::Arrow;
    config.drawing.shift_drag_tool = Tool::Eraser;
    config.drawing.ctrl_drag_tool = Tool::Pen;
    config.drawing.ctrl_shift_drag_tool = Tool::Rect;
    config.drawing.tab_drag_tool = Tool::Ellipse;
    let mut drag_tools = config.drawing.effective_drag_tools();
    drag_tools.right.drag_tool = DragTool::Pen;
    drag_tools.right.drag_color = Some(ColorSpec::Name("blue".to_string()));
    drag_tools.middle.drag_tool = DragTool::Eraser;
    config.drawing.drag_tools = Some(drag_tools.clone());

    let draft = ConfigDraft::from_config(&config);
    assert_eq!(draft.drawing_drag_tool, ToolOption::Arrow);
    assert_eq!(draft.drawing_shift_drag_tool, ToolOption::Eraser);
    assert_eq!(draft.drawing_ctrl_drag_tool, ToolOption::Pen);
    assert_eq!(draft.drawing_ctrl_shift_drag_tool, ToolOption::Rect);
    assert_eq!(draft.drawing_tab_drag_tool, ToolOption::Ellipse);

    let round_trip = draft
        .to_config(&config)
        .expect("expected config to round trip");
    assert_eq!(round_trip.drawing.drag_tool, config.drawing.drag_tool);
    assert_eq!(
        round_trip.drawing.shift_drag_tool,
        config.drawing.shift_drag_tool
    );
    assert_eq!(
        round_trip.drawing.ctrl_drag_tool,
        config.drawing.ctrl_drag_tool
    );
    assert_eq!(
        round_trip.drawing.ctrl_shift_drag_tool,
        config.drawing.ctrl_shift_drag_tool
    );
    assert_eq!(
        round_trip.drawing.tab_drag_tool,
        config.drawing.tab_drag_tool
    );
    assert_eq!(round_trip.drawing.drag_tools, Some(drag_tools));
}

#[test]
fn config_draft_round_trips_xdg_focus_loss_behavior() {
    let mut config = Config::default();
    config.ui.xdg_focus_loss_behavior = XdgFocusLossBehavior::Stay;

    let draft = ConfigDraft::from_config(&config);
    assert!(draft.ui_xdg_keep_on_focus_loss);

    let round_trip = draft
        .to_config(&config)
        .expect("expected config to round trip");
    assert_eq!(
        round_trip.ui.xdg_focus_loss_behavior,
        XdgFocusLossBehavior::Stay
    );
}
