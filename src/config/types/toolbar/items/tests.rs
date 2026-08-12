use super::*;

#[test]
fn known_hidden_ids_resolve_and_unknown_ids_round_trip() {
    let config = ToolbarItemsConfig {
        hidden: vec![
            ids::SIDE_ACTIONS_UNDO_ALL.as_str().to_string(),
            "future.toolbar.item".to_string(),
        ],
        shown: Vec::new(),
        order: ToolbarItemOrderConfig::default(),
    };

    let resolved = config.resolved();

    assert!(resolved.is_hidden(ids::SIDE_ACTIONS_UNDO_ALL));
    assert_eq!(resolved.unknown_hidden, vec!["future.toolbar.item"]);
}

#[test]
fn default_hidden_items_hide_screenshot_tool() {
    let resolved = ToolbarItemsConfig::default().resolved();

    assert!(resolved.is_hidden(ids::TOP_UTILITY_SCREENSHOT));
    assert_eq!(
        item_visibility_setting(&resolved, ids::TOP_UTILITY_SCREENSHOT),
        ToolbarItemVisibilitySetting::Hidden
    );
}

#[test]
fn the_ocr_utility_is_defined_once_ordered_and_hidden_by_default() {
    let definitions: Vec<_> = toolbar_item_definitions()
        .iter()
        .filter(|definition| definition.id == ids::TOP_UTILITY_OCR)
        .collect();
    assert_eq!(definitions.len(), 1);
    assert_eq!(definitions[0].label, "Copy text");

    let order = ToolbarItemOrderConfig::default().resolved();
    let top_controls = order.ordered_ids(ToolbarItemOrderGroup::TopControls);
    assert_eq!(
        top_controls
            .iter()
            .filter(|id| **id == ids::TOP_UTILITY_OCR)
            .count(),
        1
    );

    // Hidden by the code-level baseline, which reads as no explicit setting:
    // nothing is written to `hidden`, so a config that predates the item still
    // resolves as hiding it.
    let resolved = ToolbarItemsConfig::default().resolved();
    assert!(resolved.is_hidden(ids::TOP_UTILITY_OCR));
    assert_eq!(
        item_visibility_setting(&resolved, ids::TOP_UTILITY_OCR),
        ToolbarItemVisibilitySetting::Default
    );
    assert!(
        !ToolbarItemsConfig::default()
            .hidden
            .iter()
            .any(|raw| raw == ids::TOP_UTILITY_OCR.as_str())
    );
}

/// The regression the baseline exists for: an install predating OCR carries its
/// own `hidden` list, and serde applies the struct default only to a *missing*
/// field — so a shipped default-hidden entry would never reach it.
#[test]
fn a_config_written_before_ocr_existed_still_hides_it() {
    let upgraded: ToolbarItemsConfig =
        toml::from_str("hidden = [\"top.utility.screenshot\"]\nshown = []\n")
            .expect("previous-release toolbar items parse");

    let resolved = upgraded.resolved();
    assert!(resolved.is_hidden(ids::TOP_UTILITY_SCREENSHOT));
    assert!(
        resolved.is_hidden(ids::TOP_UTILITY_OCR),
        "upgrading exposed the opt-in OCR button"
    );
}

/// Turning it on has to stick, which needs an explicit `shown` entry: merely
/// dropping it from `hidden` would leave the baseline hiding it again.
#[test]
fn showing_the_ocr_utility_records_it_as_explicitly_shown() {
    let mut config = ToolbarItemsConfig::default();
    config.set_hidden(ids::TOP_UTILITY_OCR, false);

    assert!(
        config
            .shown
            .iter()
            .any(|raw| raw == ids::TOP_UTILITY_OCR.as_str())
    );
    assert!(!config.resolved().is_hidden(ids::TOP_UTILITY_OCR));

    config.set_hidden(ids::TOP_UTILITY_OCR, true);
    assert!(config.resolved().is_hidden(ids::TOP_UTILITY_OCR));
}

#[test]
fn showing_the_ocr_utility_is_an_explicit_customization_that_survives_reads() {
    let mut config = ToolbarItemsConfig::default();

    assert!(
        config.set_visibility_setting(ids::TOP_UTILITY_OCR, ToolbarItemVisibilitySetting::Shown)
    );
    assert!(!config.resolved().is_hidden(ids::TOP_UTILITY_OCR));
    // Re-applying the same setting is not a change; only a different one is.
    assert!(
        !config.set_visibility_setting(ids::TOP_UTILITY_OCR, ToolbarItemVisibilitySetting::Shown)
    );
    assert!(config.reset_known_hidden_to_defaults());
    assert!(config.resolved().is_hidden(ids::TOP_UTILITY_OCR));
}

#[test]
fn visibility_setting_is_hidden_first_and_known_mutation_canonicalizes_conflicts() {
    let mut config = ToolbarItemsConfig {
        hidden: vec![ids::TOP_TOOL_PEN.as_str().to_string()],
        shown: vec![ids::TOP_TOOL_PEN.as_str().to_string()],
        order: ToolbarItemOrderConfig::default(),
    };

    assert_eq!(
        item_visibility_setting(&config.resolved(), ids::TOP_TOOL_PEN),
        ToolbarItemVisibilitySetting::Hidden
    );
    assert!(config.set_visibility_setting(ids::TOP_TOOL_PEN, ToolbarItemVisibilitySetting::Shown));
    let resolved = config.resolved();
    assert_eq!(
        item_visibility_setting(&resolved, ids::TOP_TOOL_PEN),
        ToolbarItemVisibilitySetting::Shown
    );
    assert!(!resolved.hidden.contains(&ids::TOP_TOOL_PEN));
}

#[test]
fn resettable_visibility_ids_are_exactly_customizable_individual_items() {
    let actual = resettable_individual_toolbar_item_ids().collect::<BTreeSet<_>>();
    let expected = toolbar_item_definitions()
        .iter()
        .filter(|definition| toolbar_item_visibility_override_allowed(definition))
        .filter(|definition| {
            super::super::visibility::section_flag_for_item(definition.id).is_none()
        })
        .map(|definition| definition.id)
        .collect::<BTreeSet<_>>();

    assert_eq!(actual, expected);
    assert!(actual.contains(&ids::TOP_TOOL_PEN));
    assert!(actual.contains(&ids::TOP_UTILITY_SCREENSHOT));
    assert!(!actual.contains(&ids::TOP_CHROME_OVERFLOW));
    assert!(!actual.contains(&ids::SIDE_SETTINGS_ABOUT));
}

#[test]
fn set_hidden_preserves_unknown_ids_while_mutating_known_ids() {
    let mut config = ToolbarItemsConfig {
        hidden: vec![
            "future.toolbar.item".to_string(),
            ids::SIDE_ACTIONS_UNDO_ALL.as_str().to_string(),
            ids::SIDE_ACTIONS_UNDO_ALL.as_str().to_string(),
            ids::SIDE_PAGES_DUPLICATE.as_str().to_string(),
        ],
        shown: Vec::new(),
        order: ToolbarItemOrderConfig::default(),
    };

    config.set_hidden(ids::SIDE_ACTIONS_UNDO_ALL, false);
    config.set_hidden(ids::TOP_TOOL_PEN, true);

    assert_eq!(
        config.hidden,
        vec![
            "future.toolbar.item".to_string(),
            ids::SIDE_PAGES_DUPLICATE.as_str().to_string(),
            ids::TOP_TOOL_PEN.as_str().to_string()
        ]
    );
}

#[test]
fn reset_known_hidden_restores_defaults_and_preserves_unknown_ids() {
    let mut config = ToolbarItemsConfig {
        hidden: vec![
            "future.toolbar.item".to_string(),
            ids::SIDE_ACTIONS_UNDO_ALL.as_str().to_string(),
        ],
        shown: Vec::new(),
        order: ToolbarItemOrderConfig::default(),
    };

    assert!(config.reset_known_hidden_to_defaults());
    assert_eq!(
        config.hidden,
        vec![
            ids::TOP_UTILITY_SCREENSHOT.as_str().to_string(),
            "future.toolbar.item".to_string()
        ]
    );
    assert!(!config.reset_known_hidden_to_defaults());
}

#[test]
fn default_order_matches_visual_toolbar_defaults() {
    let resolved = ToolbarItemsConfig::default().resolved();

    assert_eq!(
        resolved.order.ordered_ids(ToolbarItemOrderGroup::TopTools),
        DEFAULT_TOP_TOOLS_ORDER
    );
    assert_eq!(
        resolved
            .order
            .ordered_ids(ToolbarItemOrderGroup::TopControls),
        DEFAULT_TOP_CONTROLS_ORDER
    );
}

#[test]
fn item_order_moves_known_ids_and_preserves_unknown_ids() {
    let mut config = ToolbarItemsConfig {
        hidden: Vec::new(),
        shown: Vec::new(),
        order: ToolbarItemOrderConfig {
            top_tools: vec![
                "future.toolbar.item".to_string(),
                ids::TOP_TOOL_PEN.as_str().to_string(),
                ids::TOP_TOOL_SELECT.as_str().to_string(),
            ],
            ..ToolbarItemOrderConfig::default()
        },
    };

    assert!(config.move_item_by(ToolbarItemOrderGroup::TopTools, ids::TOP_TOOL_PEN, 1,));

    assert_eq!(
        config.order.top_tools.last(),
        Some(&"future.toolbar.item".to_string())
    );
    assert_eq!(
        config
            .resolved()
            .order
            .ordered_ids(ToolbarItemOrderGroup::TopTools)[1],
        ids::TOP_TOOL_PEN
    );
}

#[test]
fn top_control_order_excludes_visibility_only_utilities() {
    let config = ToolbarItemsConfig {
        hidden: Vec::new(),
        shown: Vec::new(),
        order: ToolbarItemOrderConfig {
            top_controls: vec![
                ids::TOP_UTILITY_SHAPE_PICKER.as_str().to_string(),
                ids::TOP_UTILITY_TEXT.as_str().to_string(),
                ids::TOP_UTILITY_FILL.as_str().to_string(),
            ],
            ..ToolbarItemOrderConfig::default()
        },
    };

    let resolved = config.resolved();
    let ordered = resolved
        .order
        .ordered_ids(ToolbarItemOrderGroup::TopControls);
    assert_eq!(ordered[0], ids::TOP_UTILITY_TEXT);
    assert!(!ordered.contains(&ids::TOP_UTILITY_SHAPE_PICKER));
    assert!(!ordered.contains(&ids::TOP_UTILITY_FILL));
}

#[test]
fn toolbar_group_ids_cover_live_popover_groups_only() {
    assert_eq!(
        "step-undo".parse::<ToolbarGroupId>(),
        Ok(ToolbarGroupId::StepUndo)
    );
    assert_eq!(
        "actions".parse::<ToolbarGroupId>(),
        Ok(ToolbarGroupId::Actions)
    );
    assert!("step-markers".parse::<ToolbarGroupId>().is_err());
    assert!("colors".parse::<ToolbarGroupId>().is_err());
}

#[test]
fn every_retired_panel_item_id_is_unknown_to_the_active_model() {
    for id in [
        "side.group.colors",
        "side.group.thickness",
        "side.group.eraser-mode",
        "side.group.polygon-sides",
        "side.group.arrow-labels",
        "side.group.step-markers",
        "side.group.marker-opacity",
        "side.group.text-size",
        "side.group.font",
        "side.group.settings",
        "side.group.session",
        "side.actions.undo",
        "side.actions.redo",
        "side.actions.clear-canvas",
        "side.boards.rename",
        "side.tool-options.color",
        "side.tool-options.thickness",
        "side.tool-options.marker-opacity",
        "side.tool-options.eraser-mode",
        "side.tool-options.font-size",
        "side.tool-options.font-family",
        "side.tool-options.polygon-sides",
        "side.tool-options.arrow-labels",
        "side.tool-options.step-marker-reset",
    ] {
        assert!(
            id.parse::<ToolbarItemId>().is_err(),
            "retired panel id must not enter the active model: {id}"
        );
    }
}

#[test]
fn toolbar_item_definitions_are_unique_parseable_and_labeled() {
    let mut seen = BTreeSet::new();

    for definition in toolbar_item_definitions() {
        assert!(
            seen.insert(definition.id.as_str()),
            "duplicate toolbar item id: {}",
            definition.id
        );
        assert_eq!(
            definition.id.as_str().parse::<ToolbarItemId>(),
            Ok(definition.id)
        );
        assert!(
            !definition.label.is_empty(),
            "missing toolbar item label: {}",
            definition.id
        );
    }
}
