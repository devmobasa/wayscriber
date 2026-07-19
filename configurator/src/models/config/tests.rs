use super::super::color::ColorInput;
use super::super::fields::{
    DragMouseButton, DragToolField, DragToolOption, FontWeightOption, OverrideOption,
    PdfFitModeOption, PdfLabelContentModeOption, PdfOrientationOption, PdfPageSizeOption,
    PdfTransparentBackgroundOption, QuadField, ReducedMotionOption, SessionStorageModeOption,
    TextField, ToggleField, ToolOption, ToolbarLayoutModeOption, ToolbarOverrideField,
    ToolbarRebindModifierOption, TripletField, UiThemeOption,
};

#[test]
fn config_draft_round_trips_toolbar_rebind_modifier() {
    let mut config = Config::default();
    config.ui.toolbar.rebind_modifier = wayscriber::config::ToolbarRebindModifier::CtrlAlt;
    let mut draft = ConfigDraft::from_config(&config);
    assert_eq!(
        draft.ui_toolbar_rebind_modifier,
        ToolbarRebindModifierOption::CtrlAlt
    );

    draft.ui_toolbar_rebind_modifier = ToolbarRebindModifierOption::ShiftAlt;
    let round_trip = draft
        .to_config(&config)
        .expect("toolbar modifier round trip");
    assert_eq!(
        round_trip.ui.toolbar.rebind_modifier,
        wayscriber::config::ToolbarRebindModifier::ShiftAlt
    );
}
use super::super::{ColorMode, NamedColorOption};
use super::{ConfigDraft, RenderProfileSelectionOption};
use wayscriber::config::{
    ColorSpec, Config, ConfigDocument, PdfFitMode, PdfLabelContentMode, PdfLabelPosition,
    PdfOrientation, PdfPageSize, PdfTransparentBackground, PresetToolStatesConfig,
    QuickColorConfig, ReducedMotion, RenderColorMappingConfig, RenderProfileConfig,
    RenderProfileExportMode, ToolPresetConfig, ToolbarItemOrderConfig, ToolbarItemOrderGroup,
    ToolbarItemsConfig, ToolbarSectionFlag, UiTheme, XdgFocusLossBehavior, toolbar_item_ids as ids,
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
fn sparse_configurator_no_op_save_remains_byte_for_byte_sparse() {
    let temp = crate::test_temp::tempdir().expect("create temp directory");
    let path = temp.path().join("config.toml");
    let original = "config_revision = 1\n# intentionally sparse\n";
    std::fs::write(&path, original).expect("write sparse config");
    let document = ConfigDocument::load_from_path(&path).expect("load sparse config");
    let draft = ConfigDraft::from_config(document.config());
    let updated = draft
        .to_config(document.config())
        .expect("convert untouched sparse draft");

    document
        .save_with_backup(updated)
        .expect("save untouched sparse draft");

    assert_eq!(
        std::fs::read_to_string(path).expect("read sparse config"),
        original
    );
}

#[test]
fn config_draft_round_trips_precise_floats_without_truncation() {
    let mut config = Config::default();
    config.drawing.default_thickness = 1.234_567_890_123_45;
    config.ui.toolbar.top_offset = -9.876_543_210_987_65;
    config.export.pdf.custom_width = 812.345_678_901_234;

    let draft = ConfigDraft::from_config(&config);
    let round_trip = draft
        .to_config(&config)
        .expect("precise floats should round trip");

    assert_eq!(
        round_trip.drawing.default_thickness.to_bits(),
        config.drawing.default_thickness.to_bits()
    );
    assert_eq!(
        round_trip.ui.toolbar.top_offset.to_bits(),
        config.ui.toolbar.top_offset.to_bits()
    );
    assert_eq!(
        round_trip.export.pdf.custom_width.to_bits(),
        config.export.pdf.custom_width.to_bits()
    );
}

#[test]
fn sparse_configurator_edit_adds_only_the_edited_field_path() {
    let temp = crate::test_temp::tempdir().expect("create temp directory");
    let path = temp.path().join("config.toml");
    std::fs::write(&path, "# intentionally sparse\n").expect("write sparse config");
    let document = ConfigDocument::load_from_path(&path).expect("load sparse config");
    let mut draft = ConfigDraft::from_config(document.config());
    draft.performance_max_fps_no_vsync = "144".to_string();
    let updated = draft
        .to_config(document.config())
        .expect("convert sparse draft edit");

    document
        .save_with_backup(updated)
        .expect("save sparse draft edit");

    let saved = std::fs::read_to_string(path).expect("read sparse config");
    assert!(saved.contains("# intentionally sparse"));
    assert!(saved.contains("[performance]"));
    assert!(saved.contains("max_fps_no_vsync = 144"));
    assert!(saved.find("# intentionally sparse").unwrap() < saved.find("[performance]").unwrap());
    assert!(!saved.contains("[drawing]"));
    assert!(!saved.contains("[drawing.drag_tools"));
    assert!(!saved.contains("[boards]"));
    assert!(!saved.contains("[[boards.items]]"));
}

#[test]
fn config_draft_round_trips_quick_colors() {
    let mut config = Config::default();
    config.drawing.quick_colors.entries = vec![
        QuickColorConfig {
            label: "Blush".to_string(),
            color: ColorSpec::Name("#FFB3BA".to_string()),
        },
        QuickColorConfig {
            label: "Ink".to_string(),
            color: ColorSpec::Rgb([1, 2, 3]),
        },
    ];

    let draft = ConfigDraft::from_config(&config);
    assert_eq!(draft.drawing_quick_colors.entries[0].label, "Blush");
    assert_eq!(
        draft.drawing_quick_colors.entries[0].color.summary(),
        "#FFB3BA"
    );
    assert_eq!(draft.drawing_quick_colors.entries[1].label, "Ink");
    assert_eq!(
        draft.drawing_quick_colors.entries[1].color.rgb,
        ["1", "2", "3"]
    );

    let round_trip = draft
        .to_config(&Config::default())
        .expect("expected quick colors to round trip");

    assert_eq!(
        round_trip.drawing.quick_colors.entries[0],
        QuickColorConfig {
            label: "Blush".to_string(),
            color: ColorSpec::Name("#FFB3BA".to_string())
        }
    );
    assert_eq!(
        round_trip.drawing.quick_colors.entries[1],
        QuickColorConfig {
            label: "Ink".to_string(),
            color: ColorSpec::Rgb([1, 2, 3])
        }
    );
}

#[test]
fn config_draft_preserves_implicit_quick_color_defaults() {
    let config = Config::default();
    let draft = ConfigDraft::from_config(&config);

    let saved = draft
        .to_config(&config)
        .expect("expected implicit quick colors to save");

    assert_eq!(saved.drawing.quick_colors.configured_entry_count(), None);
    assert!(saved.drawing.quick_colors.is_implicit_default());
}

#[test]
fn config_draft_preserves_sparse_explicit_quick_colors_without_padding_the_file() {
    let temp = crate::test_temp::tempdir().expect("create temp directory");
    let path = temp.path().join("config.toml");
    let original = r#"config_revision = 1
[[drawing.quick_colors]]
label = "Only configured color"
color = "blue"
"#;
    std::fs::write(&path, original).expect("write sparse quick colors");
    let document = ConfigDocument::load_from_path(&path).expect("load sparse quick colors");
    assert_eq!(
        document
            .config()
            .drawing
            .quick_colors
            .configured_entry_count(),
        Some(1)
    );

    let draft = ConfigDraft::from_config(document.config());
    assert_eq!(draft.drawing_quick_colors.entries.len(), 8);
    let updated = draft
        .to_config(document.config())
        .expect("untouched sparse quick colors should round trip");
    assert_eq!(
        updated.drawing.quick_colors.configured_entry_count(),
        Some(1)
    );
    assert_eq!(updated.drawing.quick_colors.entries.len(), 1);

    document
        .save_with_backup(updated)
        .expect("save sparse quick colors");
    assert_eq!(
        std::fs::read_to_string(path).expect("read sparse quick colors"),
        original
    );
}

#[test]
fn config_draft_marks_changed_quick_colors_explicit() {
    let config = Config::default();
    let mut draft = ConfigDraft::from_config(&config);
    draft.drawing_quick_colors.entries[8].label = "Pool cyan".to_string();

    let saved = draft
        .to_config(&config)
        .expect("expected changed quick colors to save");

    assert_eq!(
        saved.drawing.quick_colors.configured_entry_count(),
        Some(draft.drawing_quick_colors.entries.len())
    );
}

#[test]
fn config_draft_reorders_and_removes_quick_colors() {
    let mut draft = ConfigDraft::from_config(&Config::default());
    assert_eq!(draft.drawing_quick_colors.entries.len(), 11);
    draft.drawing_quick_colors.entries[0].label = "First".to_string();
    draft.drawing_quick_colors.entries[1].label = "Second".to_string();

    assert!(draft.drawing_quick_colors.move_entry(1, -1));
    assert_eq!(draft.drawing_quick_colors.entries[0].label, "Second");

    assert!(draft.drawing_quick_colors.remove_entry(10));
    assert_eq!(draft.drawing_quick_colors.entries.len(), 10);
    assert!(draft.drawing_quick_colors.remove_entry(9));
    assert_eq!(draft.drawing_quick_colors.entries.len(), 9);
    assert!(draft.drawing_quick_colors.remove_entry(8));
    assert_eq!(draft.drawing_quick_colors.entries.len(), 8);
    assert!(!draft.drawing_quick_colors.remove_entry(1));

    draft.drawing_quick_colors.add_entry();
    assert_eq!(draft.drawing_quick_colors.entries.len(), 9);
    assert!(draft.drawing_quick_colors.remove_entry(8));
    assert_eq!(draft.drawing_quick_colors.entries.len(), 8);
}

#[test]
fn config_draft_switches_quick_color_named_hex_to_rgb_without_stale_rgb() {
    let mut draft = ConfigDraft::from_config(&Config::default());
    {
        let entry = &mut draft.drawing_quick_colors.entries[1];
        entry.color.mode = ColorMode::Named;
        entry.color.selected_named = NamedColorOption::Custom;
        entry.color.name = "#123456".to_string();
        entry.color.sync_rgb_from_preview();
        entry.color.mode = ColorMode::Rgb;
    }

    let round_trip = draft
        .to_config(&Config::default())
        .expect("expected quick color RGB to save");

    assert_eq!(
        round_trip.drawing.quick_colors.entries[1].color,
        ColorSpec::Rgb([18, 52, 86])
    );
}

#[test]
fn config_draft_rejects_invalid_quick_colors() {
    let mut draft = ConfigDraft::from_config(&Config::default());
    {
        let entry = &mut draft.drawing_quick_colors.entries[1];
        entry.color.mode = ColorMode::Named;
        entry.color.selected_named = NamedColorOption::Custom;
        entry.color.name = "#GG0000".to_string();
    }
    {
        let entry = &mut draft.drawing_quick_colors.entries[5];
        entry.color.mode = ColorMode::Named;
        entry.color.selected_named = NamedColorOption::Custom;
        entry.color.name = "chartreuse".to_string();
    }
    let errors = draft
        .to_config(&Config::default())
        .expect_err("expected invalid quick color errors");
    let fields: Vec<&str> = errors.iter().map(|err| err.field.as_str()).collect();

    assert!(fields.contains(&"drawing.quick_colors[1].color"));
    assert!(fields.contains(&"drawing.quick_colors[5].color"));
}

#[test]
fn config_draft_materializes_missing_quick_color_slots_and_labels() {
    let mut config = Config::default();
    config.drawing.quick_colors.entries = vec![QuickColorConfig {
        label: String::new(),
        color: ColorSpec::Name("blue".to_string()),
    }];

    let mut draft = ConfigDraft::from_config(&config);
    assert_eq!(draft.drawing_quick_colors.entries.len(), 8);
    assert_eq!(draft.drawing_quick_colors.entries[0].label, "Red");
    assert_eq!(draft.drawing_quick_colors.entries[1].label, "Green");

    draft.drawing_quick_colors.entries[0].label = " ".to_string();
    let round_trip = draft
        .to_config(&Config::default())
        .expect("blank labels should save with fallback labels");

    assert_eq!(round_trip.drawing.quick_colors.entries[0].label, "Red");
}

#[test]
fn config_draft_accepts_quick_color_hex_string() {
    let mut draft = ConfigDraft::from_config(&Config::default());
    let entry = &mut draft.drawing_quick_colors.entries[0];
    entry.color.mode = ColorMode::Named;
    entry.color.selected_named = NamedColorOption::Custom;
    entry.color.name = "#FFB3BA".to_string();

    let round_trip = draft
        .to_config(&Config::default())
        .expect("expected quick color hex to save");

    assert_eq!(
        round_trip.drawing.quick_colors.entries[0].color,
        ColorSpec::Name("#FFB3BA".to_string())
    );
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
fn config_draft_round_trips_toolbar_item_visibility_preserving_unknown_ids() {
    let mut config = Config::default();
    config.ui.toolbar.items = ToolbarItemsConfig {
        hidden: vec![
            "future.toolbar.item".to_string(),
            ids::SIDE_ACTIONS_UNDO_ALL.as_str().to_string(),
            ids::SIDE_ACTIONS_UNDO_ALL.as_str().to_string(),
        ],
        shown: Vec::new(),
        order: ToolbarItemOrderConfig::default(),
    };

    let mut draft = ConfigDraft::from_config(&config);
    draft.set_toolbar_item_visible(ids::SIDE_ACTIONS_UNDO_ALL, true);
    draft.set_toolbar_item_visible(ids::TOP_TOOL_PEN, false);

    let round_trip = draft
        .to_config(&config)
        .expect("expected config to round trip");

    assert_eq!(
        round_trip.ui.toolbar.items.hidden,
        vec![
            "future.toolbar.item".to_string(),
            ids::TOP_TOOL_PEN.as_str().to_string()
        ]
    );
}

#[test]
fn config_draft_preserves_legacy_toolbar_section_visibility_on_unrelated_save() {
    let mut config = Config::default();
    config.ui.toolbar.show_zoom_actions = false;

    let mut draft = ConfigDraft::from_config(&config);
    assert!(!draft.ui_toolbar_show_zoom_actions);
    assert!(
        draft
            .ui_toolbar_items
            .resolved()
            .hidden
            .contains(&ToolbarSectionFlag::ZoomActions.item_id())
    );

    draft.ui_toolbar_use_icons = !draft.ui_toolbar_use_icons;
    let round_trip = draft
        .to_config(&config)
        .expect("expected legacy toolbar visibility to round trip");

    assert!(!round_trip.ui.toolbar.show_zoom_actions);
    assert!(
        round_trip
            .ui
            .toolbar
            .items
            .resolved()
            .hidden
            .contains(&ToolbarSectionFlag::ZoomActions.item_id())
    );
}

#[test]
fn config_draft_applies_active_mode_override_before_folding_legacy_visibility() {
    for (legacy_value, override_value) in [(true, false), (false, true)] {
        let mut config = Config::default();
        config.ui.toolbar.show_presets = legacy_value;
        config.ui.toolbar.mode_overrides.regular.show_presets = Some(override_value);

        let mut draft = ConfigDraft::from_config(&config);
        assert_eq!(draft.ui_toolbar_show_presets, override_value);
        let resolved = draft.ui_toolbar_items.resolved();
        let presets_id = ToolbarSectionFlag::Presets.item_id();
        assert!(!resolved.shown.contains(&presets_id));
        assert!(!resolved.hidden.contains(&presets_id));

        draft.ui_toolbar_use_icons = !draft.ui_toolbar_use_icons;
        let round_trip = draft
            .to_config(&config)
            .expect("expected active mode override to survive an unrelated save");

        assert_eq!(round_trip.ui.toolbar.show_presets, override_value);
        let resolved = round_trip.ui.toolbar.items.resolved();
        assert!(!resolved.shown.contains(&presets_id));
        assert!(!resolved.hidden.contains(&presets_id));
    }
}

#[test]
fn active_toolbar_mode_override_refreshes_effective_section_visibility() {
    let mut draft = ConfigDraft::from_config(&Config::default());
    assert!(draft.ui_toolbar_show_presets);

    draft.set_toolbar_override(
        ToolbarLayoutModeOption::Regular,
        ToolbarOverrideField::ShowPresets,
        OverrideOption::Off,
    );

    assert!(!draft.ui_toolbar_show_presets);
}

#[test]
fn config_draft_round_trips_toolbar_item_order_preserving_unknown_ids() {
    let mut config = Config::default();
    config.ui.toolbar.items.order.top_tools = vec![
        "future.toolbar.item".to_string(),
        ids::TOP_TOOL_PEN.as_str().to_string(),
        ids::TOP_TOOL_SELECT.as_str().to_string(),
    ];

    let mut draft = ConfigDraft::from_config(&config);
    draft.move_toolbar_item(ToolbarItemOrderGroup::TopTools, ids::TOP_TOOL_PEN, 1);

    let round_trip = draft
        .to_config(&config)
        .expect("expected config to round trip");

    assert!(
        round_trip
            .ui
            .toolbar
            .items
            .order
            .top_tools
            .contains(&"future.toolbar.item".to_string())
    );
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
fn section_toggle_replaces_overlay_item_override_on_save() {
    let mut config = Config::default();
    config
        .ui
        .toolbar
        .items
        .set_hidden(ToolbarSectionFlag::Presets.item_id(), true);

    let mut draft = ConfigDraft::from_config(&config);
    assert!(!draft.ui_toolbar_show_presets);
    draft.set_toggle(ToggleField::UiToolbarShowPresets, true);

    let saved = draft
        .to_config(&config)
        .expect("toolbar config should save");
    let resolved = saved.ui.toolbar.items.resolved();
    assert!(
        !resolved
            .hidden
            .contains(&ToolbarSectionFlag::Presets.item_id())
    );
    assert!(
        resolved
            .shown
            .contains(&ToolbarSectionFlag::Presets.item_id())
    );
    assert!(
        wayscriber::config::resolve_section_visibility(
            saved.ui.toolbar.layout_mode,
            &saved.ui.toolbar.mode_overrides,
            &resolved,
        )
        .show_presets
    );
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
        polygon_sides: Some(7),
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
            polygon_sides: None,
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
            polygon_sides: None,
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
    config.drawing.drag_tool = wayscriber::input::DragBindableTool::Arrow;
    config.drawing.shift_drag_tool = wayscriber::input::DragBindableTool::Eraser;
    config.drawing.ctrl_drag_tool = wayscriber::input::DragBindableTool::Pen;
    config.drawing.ctrl_shift_drag_tool = wayscriber::input::DragBindableTool::Rect;
    config.drawing.tab_drag_tool = wayscriber::input::DragBindableTool::Ellipse;
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
fn config_draft_round_trips_polygon_sides() {
    let mut config = Config::default();
    config.drawing.polygon_sides = 8;

    let draft = ConfigDraft::from_config(&config);
    assert_eq!(draft.drawing_polygon_sides, "8");

    let round_trip = draft
        .to_config(&config)
        .expect("expected config to round trip");
    assert_eq!(round_trip.drawing.polygon_sides, 8);
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

#[test]
fn config_draft_round_trips_ui_theme() {
    let mut config = Config::default();
    config.ui.theme = UiTheme::Light;

    let draft = ConfigDraft::from_config(&config);
    assert_eq!(draft.ui_theme, UiThemeOption::Light);

    let round_trip = draft
        .to_config(&config)
        .expect("expected config to round trip");
    assert_eq!(round_trip.ui.theme, UiTheme::Light);
}

#[test]
fn config_draft_round_trips_ui_reduced_motion() {
    let mut config = Config::default();
    config.ui.reduced_motion = ReducedMotion::On;

    let draft = ConfigDraft::from_config(&config);
    assert_eq!(draft.ui_reduced_motion, ReducedMotionOption::On);

    let round_trip = draft
        .to_config(&config)
        .expect("expected config to round trip");
    assert_eq!(round_trip.ui.reduced_motion, ReducedMotion::On);
}

#[test]
fn config_draft_preserves_tray_icon_style() {
    let mut config = Config::default();
    config.tray.icon_style = wayscriber::config::TrayIconStyle::Colored;

    let draft = ConfigDraft::from_config(&config);
    let round_trip = draft
        .to_config(&config)
        .expect("expected config to round trip");

    assert_eq!(
        round_trip.tray.icon_style,
        wayscriber::config::TrayIconStyle::Colored
    );
}
