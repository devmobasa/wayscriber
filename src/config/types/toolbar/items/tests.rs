use super::*;

#[test]
fn known_hidden_ids_resolve_and_unknown_ids_round_trip() {
    let config = ToolbarItemsConfig {
        hidden: vec![
            ids::SIDE_ACTIONS_UNDO_ALL.as_str().to_string(),
            "future.toolbar.item".to_string(),
        ],
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
    assert_eq!(
        resolved
            .order
            .ordered_ids(ToolbarItemOrderGroup::SideSections),
        DEFAULT_SIDE_SECTIONS_ORDER
    );
}

#[test]
fn item_order_moves_known_ids_and_preserves_unknown_ids() {
    let mut config = ToolbarItemsConfig {
        hidden: Vec::new(),
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
fn side_section_order_uses_runtime_representable_blocks() {
    let config = ToolbarItemsConfig {
        hidden: Vec::new(),
        order: ToolbarItemOrderConfig {
            side_sections: vec![
                ids::SIDE_GROUP_FONT.as_str().to_string(),
                ids::SIDE_GROUP_THICKNESS.as_str().to_string(),
                ids::SIDE_GROUP_POLYGON_SIDES.as_str().to_string(),
            ],
            ..ToolbarItemOrderConfig::default()
        },
    };

    let resolved = config.resolved();
    let ordered = resolved
        .order
        .ordered_ids(ToolbarItemOrderGroup::SideSections);
    assert_eq!(ordered[0], ids::SIDE_GROUP_THICKNESS);
    assert!(!ordered.contains(&ids::SIDE_GROUP_FONT));
    assert!(!ordered.contains(&ids::SIDE_GROUP_POLYGON_SIDES));
}

#[test]
fn toolbar_group_ids_include_step_markers_and_step_undo() {
    assert_eq!(
        "step-markers".parse::<ToolbarGroupId>(),
        Ok(ToolbarGroupId::StepMarkers)
    );
    assert_eq!(
        "step-undo".parse::<ToolbarGroupId>(),
        Ok(ToolbarGroupId::StepUndo)
    );
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
