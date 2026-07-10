mod actions;
pub(crate) mod activation;
pub(crate) mod control;
pub(crate) mod event_policy;
pub(crate) mod header;
pub(crate) mod session;
pub(crate) mod settings;
pub(crate) mod tools;

#[allow(unused_imports)]
pub(crate) use actions::{
    ToolbarActionsModel, ToolbarButtonModel, ToolbarCommandGroup, ToolbarCommandGroupKind,
    toolbar_boards_model, toolbar_pages_model,
};
#[allow(unused_imports)]
pub(crate) use activation::{
    ToolbarActivation, ToolbarColorPicker, ToolbarControlId, ToolbarDragTarget, ToolbarSlider,
    ToolbarSliderSpec, ToolbarSliderTarget, delay_secs_from_t, delay_t_from_ms,
};
#[allow(unused_imports)]
pub(crate) use control::{
    ToolbarBoardChipPresentation, ToolbarControl, ToolbarControlKind, ToolbarControlPresentation,
    ToolbarControlRole, ToolbarIcon, ToolbarModelError, ToolbarPresentationPayload, ToolbarSegment,
    ToolbarSegmentedControl, ToolbarSingleControl, ToolbarTooltip,
};
#[allow(unused_imports)]
pub(crate) use event_policy::{
    ToolbarBackendRoute, ToolbarEventPolicy, ToolbarPersistence, ToolbarPersistenceTarget,
    ToolbarPreApplyEffect, ToolbarUiPersistenceTarget, action_for_apply_preset,
    action_for_clear_preset, action_for_event, action_for_save_preset, action_for_tool,
    short_label_for_event, tooltip_label_for_event,
};
#[allow(unused_imports)]
pub(crate) use header::{SideHeaderModel, board_chip_label, layout_mode_control};
#[allow(unused_imports)]
pub(crate) use session::{ToolbarSessionButton, ToolbarSessionModel, ToolbarSessionRecent};
#[allow(unused_imports)]
pub(crate) use settings::{ToolbarSettingsButton, ToolbarSettingsModel, ToolbarSettingsToggle};
#[allow(unused_imports)]
pub(crate) use tools::{
    SemanticToolIcon, TopToolGroup, TopUtilityButton, current_shape_tool, default_drag_hint,
    default_polygon_tool, default_shape_tool, fill_tool_active, is_fill_tool, is_polygon_tool,
    ordered_pane_sections, ordered_side_sections, pane_for_section, polygon_tools,
    semantic_icon_for_tool, shape_tools, tool_visible, toolbar_item_id_for_tool,
    toolbar_item_visible, top_clear_canvas_visible, top_fill_visible, top_highlight_ring_visible,
    top_highlight_visible, top_screenshot_visible, top_shape_picker_visible,
    top_sticky_note_visible, top_text_visible, top_tool_buttons, top_tool_group,
    visible_shape_picker_max_row_len, visible_shape_picker_row_count, visible_shape_picker_rows,
    visible_tool_count, visible_top_tool_buttons, visible_top_utility_buttons,
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{
        ToolbarGroupId, ToolbarLayoutMode, toolbar_item_definitions, toolbar_item_ids as ids,
    };
    use crate::input::state::test_support::make_test_input_state;
    use crate::ui::toolbar::{SidePane, ToolbarBindingHints, ToolbarEvent, ToolbarSnapshot};

    fn snapshot() -> ToolbarSnapshot {
        let mut state = make_test_input_state();
        state.toolbar_side_pane = SidePane::Canvas;
        state.show_actions_section = true;
        state.show_actions_advanced = false;
        state.show_zoom_actions = true;
        state.show_pages_section = true;
        state.show_boards_section = true;
        ToolbarSnapshot::from_input_with_bindings(&state, ToolbarBindingHints::default())
    }

    #[test]
    fn actions_model_keeps_advanced_actions_in_canvas_pane() {
        let mut snapshot = snapshot();
        snapshot.show_actions_section = false;
        snapshot.show_actions_advanced = true;
        snapshot.delay_actions_enabled = true;

        let model = ToolbarActionsModel::from_snapshot(&snapshot).expect("actions model");
        assert_eq!(model.groups().len(), 2);
        assert_eq!(model.groups()[0].kind, ToolbarCommandGroupKind::Zoom);
        assert_eq!(
            model.groups()[1].kind,
            ToolbarCommandGroupKind::AdvancedActions
        );
        assert_eq!(model.groups()[1].buttons.len(), 5);

        snapshot.active_side_pane = SidePane::Draw;
        assert!(ToolbarActionsModel::from_snapshot(&snapshot).is_none());
    }

    #[test]
    fn page_and_board_models_report_disabled_navigation() {
        let mut snapshot = snapshot();
        snapshot.page_count = 2;
        snapshot.board_count = 1;
        snapshot.is_transparent = true;

        let pages = toolbar_pages_model(&snapshot).expect("pages model");
        assert!(!pages.buttons[0].enabled);
        assert!(pages.buttons[1].enabled);

        let boards = toolbar_boards_model(&snapshot).expect("boards model");
        assert!(!boards.buttons[0].enabled);
        assert!(!boards.buttons[1].enabled);
        assert!(!boards.buttons[3].enabled);
    }

    #[test]
    fn dynamic_toolbar_labels_live_with_the_event_model() {
        let mut snapshot = snapshot();
        snapshot.frozen_active = true;
        snapshot.zoom_locked = true;

        let freeze = ToolbarButtonModel::new(ToolbarEvent::ToggleFreeze, true);
        let zoom_lock = ToolbarButtonModel::new(ToolbarEvent::ToggleZoomLock, true);

        assert_eq!(freeze.short_label(&snapshot, "Action"), "Unfreeze");
        assert_eq!(zoom_lock.tooltip_label(&snapshot, "Action"), "Unlock Zoom");
        assert_eq!(zoom_lock.binding_hint(&snapshot), None);
    }

    #[test]
    fn segmented_controls_validate_invariants() {
        let segment = ToolbarSegment {
            id: ToolbarControlId::IconModeIcons,
            label: "Ico".into(),
            activation: ToolbarActivation::Click(ToolbarEvent::ToggleIconMode(true)),
            action: None,
            tooltip: ToolbarTooltip::text("Icons mode"),
            enabled: true,
        };

        assert_eq!(
            ToolbarSegmentedControl::try_new(None, Vec::new()).unwrap_err(),
            ToolbarModelError::EmptySegments
        );
        assert_eq!(
            ToolbarSegmentedControl::try_new(None, vec![segment.clone(), segment.clone()])
                .unwrap_err(),
            ToolbarModelError::DuplicateSegmentId(ToolbarControlId::IconModeIcons)
        );
        assert_eq!(
            ToolbarSegmentedControl::try_new(
                Some(ToolbarControlId::IconModeText),
                vec![segment.clone()]
            )
            .unwrap_err(),
            ToolbarModelError::MissingActiveSegment(ToolbarControlId::IconModeText)
        );
        assert!(
            ToolbarSegmentedControl::try_new(Some(ToolbarControlId::IconModeIcons), vec![segment])
                .is_ok()
        );
    }

    #[test]
    fn side_header_model_contains_dynamic_board_chip() {
        let mut snapshot = snapshot();
        snapshot.board_index = 1;
        snapshot.board_count = 3;
        snapshot.board_name = "Sprint".to_string();
        snapshot.page_count = 2;

        let header = SideHeaderModel::from_snapshot(&snapshot);
        let ToolbarPresentationPayload::BoardChip(chip) = &header.board_chip.presentation.payload
        else {
            panic!("board chip payload");
        };

        assert_eq!(chip.label, "Board 2/3 · Sprint · p.1/2");
        assert_eq!(chip.board_index, 1);
        assert_eq!(chip.board_count, 3);
        assert_eq!(chip.page_count, 2);
    }

    #[test]
    fn settings_model_includes_context_ui_and_simple_mode_hides_advanced_toggles() {
        let mut snapshot = snapshot();
        snapshot.active_side_pane = SidePane::Settings;
        snapshot.layout_mode = ToolbarLayoutMode::Simple;
        snapshot.show_settings_section = true;

        let model = ToolbarSettingsModel::from_snapshot(&snapshot).expect("settings");
        assert_eq!(
            model.toggles()[0].id,
            ToolbarControlId::SettingsContextAwareUi
        );
        assert!(
            !model
                .toggles()
                .iter()
                .any(|toggle| toggle.id == ToolbarControlId::SettingsAdvancedActions)
        );

        snapshot.layout_mode = ToolbarLayoutMode::Regular;
        let model = ToolbarSettingsModel::from_snapshot(&snapshot).expect("settings");
        assert!(
            model
                .toggles()
                .iter()
                .any(|toggle| toggle.id == ToolbarControlId::SettingsAdvancedActions)
        );

        snapshot.active_side_pane = SidePane::Draw;
        assert!(ToolbarSettingsModel::from_snapshot(&snapshot).is_none());
    }

    #[test]
    fn settings_model_moves_hidden_item_overrides_into_customization_panel() {
        let mut snapshot = snapshot();
        snapshot.active_side_pane = SidePane::Settings;
        snapshot.show_settings_section = true;
        snapshot.resolved_toolbar_items = crate::config::ToolbarItemsConfig {
            hidden: vec![ids::TOP_TOOL_PEN.as_str().to_string()],
            shown: Vec::new(),
            order: crate::config::ToolbarItemOrderConfig::default(),
        }
        .resolved();

        let model = ToolbarSettingsModel::from_snapshot(&snapshot).expect("settings");
        assert!(model.item_overrides().is_empty());
        assert!(model.buttons().iter().any(|button| {
            matches!(
                &button.event,
                ToolbarEvent::SetToolbarItemCustomizationOpen(true)
            )
        }));
        assert!(
            model.buttons().iter().any(|button| matches!(
                &button.event,
                ToolbarEvent::ResetToolbarItemHiddenOverrides
            ))
        );

        snapshot.customize_items_open = true;
        let model = ToolbarSettingsModel::from_snapshot(&snapshot).expect("settings");
        assert!(model.item_overrides().is_empty());
        assert!(
            model
                .groups()
                .iter()
                .any(|group| group.label.as_ref() == "Top tools")
        );

        snapshot.customize_items_group =
            Some(crate::ui::toolbar::ToolbarItemCustomizeGroup::TopTools);
        let model = ToolbarSettingsModel::from_snapshot(&snapshot).expect("settings");
        assert!(model.groups().is_empty());
        assert!(
            model
                .item_overrides()
                .iter()
                .any(|item| item.id == ids::TOP_TOOL_PEN && !item.shown)
        );
        assert!(!model.item_overrides().iter().any(|item| {
            toolbar_item_definitions().iter().any(|definition| {
                definition.id == item.id && definition.group == Some(ToolbarGroupId::Settings)
            })
        }));
        assert!(model.buttons().iter().any(|button| matches!(
            &button.event,
            ToolbarEvent::SetToolbarItemCustomizationGroup(None)
        )));
    }

    #[test]
    fn event_policy_classifies_persistence_and_pre_apply_effects() {
        assert_eq!(
            ToolbarEventPolicy::for_event(&ToolbarEvent::ToggleStatusBar(false)).persistence,
            ToolbarPersistence::Persist(ToolbarPersistenceTarget::Ui(
                ToolbarUiPersistenceTarget::StatusBar
            ))
        );
        assert_eq!(
            ToolbarEventPolicy::for_event(&ToolbarEvent::ToggleCustomSection(true)).persistence,
            ToolbarPersistence::Persist(ToolbarPersistenceTarget::History)
        );
        assert_eq!(
            ToolbarEventPolicy::for_event(&ToolbarEvent::SetThickness(2.0)).persistence,
            ToolbarPersistence::RuntimeOnly
        );
        assert_eq!(
            ToolbarEventPolicy::for_event(&ToolbarEvent::SetSidePane(SidePane::Canvas)).persistence,
            ToolbarPersistence::Persist(ToolbarPersistenceTarget::Toolbar)
        );
        assert_eq!(
            ToolbarEventPolicy::for_event(&ToolbarEvent::ToggleSideSectionCollapsed(
                crate::ui::toolbar::ToolbarSideSection::Colors,
                true,
            ))
            .persistence,
            ToolbarPersistence::Persist(ToolbarPersistenceTarget::Toolbar)
        );
        assert_eq!(
            ToolbarEventPolicy::for_event(&ToolbarEvent::ScrollSidePane(12.0)).persistence,
            ToolbarPersistence::RuntimeOnly
        );
        assert_eq!(
            ToolbarEventPolicy::for_event(&ToolbarEvent::SetSidePane(SidePane::Canvas))
                .pre_apply_effects,
            vec![ToolbarPreApplyEffect::RecordDrawerHintShown]
        );
        assert!(
            ToolbarEventPolicy::for_event(&ToolbarEvent::SetSidePane(SidePane::Draw))
                .pre_apply_effects
                .is_empty()
        );
    }
}
