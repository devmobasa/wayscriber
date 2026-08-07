mod actions;
pub(crate) mod activation;
pub(crate) mod control;
pub(crate) mod event_policy;
pub(crate) mod header;
pub(crate) mod session;
pub(crate) mod settings;
mod style_pill;
pub(crate) mod tools;
mod top_spec;

#[allow(unused_imports)]
pub(crate) use actions::{
    ToolbarButtonModel, ToolbarCommandGroup, toolbar_actions_model_for_popover,
    toolbar_advanced_group_for_popover, toolbar_boards_model_for_popover,
    toolbar_pages_model_for_popover, toolbar_zoom_group_for_popover,
};
#[allow(unused_imports)]
pub(crate) use activation::{
    ToolbarControlId, ToolbarSlider, ToolbarSliderSpec, ToolbarSliderTarget, delay_t_from_ms,
};
#[allow(unused_imports)]
pub(crate) use control::{
    ToolbarBoardChipPresentation, ToolbarControl, ToolbarControlKind, ToolbarControlPresentation,
    ToolbarControlRole, ToolbarIcon, ToolbarModelError, ToolbarPresentationPayload, ToolbarSegment,
    ToolbarSegmentedControl, ToolbarSingleControl, ToolbarTooltip,
};
#[allow(unused_imports)]
pub(crate) use event_policy::{
    ToolbarBackendRoute, ToolbarEventPolicy, ToolbarPersistence, ToolbarPopover,
    ToolbarRuntimeUiPersistenceTarget, action_for_apply_preset, action_for_clear_preset,
    action_for_event, action_for_save_preset, action_for_tool, popovers_for_event,
    short_label_for_event, tooltip_label_for_event,
};
#[allow(unused_imports)]
pub(crate) use header::layout_mode_control;
#[allow(unused_imports)]
pub(crate) use session::{ToolbarSessionButton, ToolbarSessionModel, ToolbarSessionRecent};
#[allow(unused_imports)]
pub(crate) use settings::{
    ToolbarSettingsButton, ToolbarSettingsModel, ToolbarSettingsNotice,
    ToolbarSettingsNoticeSeverity, ToolbarSettingsToggle,
};
#[allow(unused_imports)]
pub(crate) use style_pill::{
    StylePillControl, StylePillCounter, StylePillRole, StylePillSegment, StylePillSpec,
    StylePillState,
};
#[allow(unused_imports)]
pub(crate) use tools::{
    SemanticToolIcon, TopToolGroup, TopUtilityButton, current_shape_tool, default_drag_hint,
    default_polygon_tool, default_shape_tool, is_fill_tool, is_polygon_tool, polygon_tools,
    semantic_icon_for_tool, shape_tools, tool_visible, toolbar_item_id_for_tool,
    toolbar_item_visible, top_clear_canvas_visible, top_fill_visible, top_highlight_ring_visible,
    top_highlight_visible, top_screenshot_visible, top_shape_picker_visible,
    top_sticky_note_visible, top_text_visible, top_tool_buttons, top_tool_group,
    visible_shape_picker_max_row_len, visible_shape_picker_row_count, visible_shape_picker_rows,
    visible_tool_count, visible_top_tool_buttons, visible_top_utility_buttons,
};
#[allow(unused_imports)]
pub(crate) use top_spec::{
    TopStripPlan, TopToolbarControl, TopToolbarControlId, TopToolbarControlRole, TopToolbarDivider,
    TopToolbarIcon, TopToolbarIsland, TopToolbarNode, TopToolbarSpec, TopToolbarUtility,
    action_tooltip, micro_ring_width, preset_slot,
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{
        ToolbarGroupId, ToolbarItemOrderConfig, ToolbarItemsConfig, ToolbarLayoutMode,
        toolbar_item_definitions, toolbar_item_ids as ids,
    };
    use crate::input::Tool;
    use crate::input::state::test_support::make_test_input_state;
    use crate::ui::toolbar::{
        RuntimeUiPersistenceMode, RuntimeUiPersistenceSnapshot, ToolbarBindingHints, ToolbarEvent,
        ToolbarSnapshot,
    };

    fn snapshot() -> ToolbarSnapshot {
        let mut state = make_test_input_state();
        state.show_actions_section = true;
        state.show_actions_advanced = false;
        state.show_zoom_actions = true;
        state.show_pages_section = true;
        state.show_boards_section = true;
        ToolbarSnapshot::from_input_with_bindings(&state, ToolbarBindingHints::default())
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
            id: ToolbarControlId::LayoutModeSimple,
            label: "Simple".into(),
            activation: ToolbarEvent::SetToolbarLayoutMode(ToolbarLayoutMode::Simple),
            action: None,
            tooltip: ToolbarTooltip::text("Simple layout"),
            enabled: true,
        };

        assert_eq!(
            ToolbarSegmentedControl::try_new(None, Vec::new()).unwrap_err(),
            ToolbarModelError::EmptySegments
        );
        assert_eq!(
            ToolbarSegmentedControl::try_new(None, vec![segment.clone(), segment.clone()])
                .unwrap_err(),
            ToolbarModelError::DuplicateSegmentId(ToolbarControlId::LayoutModeSimple)
        );
        assert_eq!(
            ToolbarSegmentedControl::try_new(
                Some(ToolbarControlId::LayoutModeRegular),
                vec![segment.clone()]
            )
            .unwrap_err(),
            ToolbarModelError::MissingActiveSegment(ToolbarControlId::LayoutModeRegular)
        );
        assert!(
            ToolbarSegmentedControl::try_new(
                Some(ToolbarControlId::LayoutModeSimple),
                vec![segment]
            )
            .is_ok()
        );
    }

    #[test]
    fn settings_model_includes_context_ui_and_simple_mode_hides_advanced_toggles() {
        let mut snapshot = snapshot();
        snapshot.layout_mode = ToolbarLayoutMode::Simple;

        let model = ToolbarSettingsModel::for_popover(&snapshot).expect("settings");
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
        let model = ToolbarSettingsModel::for_popover(&snapshot).expect("settings");
        assert!(
            model
                .toggles()
                .iter()
                .any(|toggle| toggle.id == ToolbarControlId::SettingsAdvancedActions)
        );
    }

    #[test]
    fn long_settings_toggles_take_a_full_row() {
        let mut snapshot = snapshot();
        snapshot.layout_mode = ToolbarLayoutMode::Regular;

        let model = ToolbarSettingsModel::for_popover(&snapshot).expect("settings");
        let rows = model.toggle_rows();
        assert_eq!(
            rows.iter().map(|row| row.len()).sum::<usize>(),
            model.toggles().len(),
            "packing covers every toggle exactly once"
        );
        for row in &rows {
            match row.as_slice() {
                [only] if only.wide => {}
                [only] => assert!(!only.wide, "narrow leftover row"),
                [a, b] => assert!(!a.wide && !b.wide, "wide toggles never share a row"),
                other => panic!("rows hold one or two toggles, got {}", other.len()),
            }
        }
        assert!(
            rows.iter().any(|row| row.len() == 1 && row[0].wide),
            "the long labels (advanced actions, multi-step) are wide rows"
        );
    }

    #[test]
    fn settings_model_moves_hidden_item_overrides_into_customization_panel() {
        let mut snapshot = snapshot();
        snapshot.resolved_toolbar_items = crate::config::ToolbarItemsConfig {
            hidden: vec![ids::TOP_TOOL_PEN.as_str().to_string()],
            shown: Vec::new(),
            order: crate::config::ToolbarItemOrderConfig::default(),
        }
        .resolved();

        let model = ToolbarSettingsModel::for_popover(&snapshot).expect("settings");
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
        let model = ToolbarSettingsModel::for_popover(&snapshot).expect("settings");
        assert!(model.item_overrides().is_empty());
        assert!(
            model
                .groups()
                .iter()
                .any(|group| group.label.as_ref() == "Top tools")
        );

        snapshot.customize_items_group =
            Some(crate::ui::toolbar::ToolbarItemCustomizeGroup::TopTools);
        let model = ToolbarSettingsModel::for_popover(&snapshot).expect("settings");
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

    fn has_visibility_reset_button(snapshot: &ToolbarSnapshot) -> bool {
        ToolbarSettingsModel::for_popover(snapshot).is_some_and(|model| {
            model
                .buttons()
                .iter()
                .any(|button| matches!(button.event, ToolbarEvent::ResetToolbarItemHiddenOverrides))
        })
    }

    #[test]
    fn factory_visibility_reset_button_tracks_only_eligible_tri_state_differences() {
        let mut snapshot = snapshot();
        snapshot.resolved_toolbar_items = ToolbarItemsConfig::default().resolved();
        assert!(snapshot.toolbar_item_hidden(ids::TOP_UTILITY_SCREENSHOT));
        assert!(!has_visibility_reset_button(&snapshot));

        let mut showing_screenshot = ToolbarItemsConfig::default();
        showing_screenshot.set_hidden(ids::TOP_UTILITY_SCREENSHOT, false);
        snapshot.resolved_toolbar_items = showing_screenshot.resolved();
        assert!(has_visibility_reset_button(&snapshot));

        let mut hidden_pen = ToolbarItemsConfig::default();
        hidden_pen.set_hidden(ids::TOP_TOOL_PEN, true);
        snapshot.resolved_toolbar_items = hidden_pen.resolved();
        assert!(has_visibility_reset_button(&snapshot));

        let mut shown_pen = ToolbarItemsConfig::default();
        shown_pen.shown.push(ids::TOP_TOOL_PEN.as_str().to_string());
        snapshot.resolved_toolbar_items = shown_pen.resolved();
        assert!(has_visibility_reset_button(&snapshot));

        let mut section_only = ToolbarItemsConfig::default();
        section_only.set_hidden(crate::config::ToolbarSectionFlag::Actions.item_id(), false);
        snapshot.resolved_toolbar_items = section_only.resolved();
        assert!(!has_visibility_reset_button(&snapshot));

        let mut chrome_only = ToolbarItemsConfig::default();
        chrome_only.set_hidden(ids::TOP_CHROME_OVERFLOW, true);
        chrome_only.set_hidden(ids::SIDE_SETTINGS_ABOUT, true);
        snapshot.resolved_toolbar_items = chrome_only.resolved();
        assert!(!has_visibility_reset_button(&snapshot));

        let mut unknown_only = ToolbarItemsConfig::default();
        unknown_only.hidden.push("future.toolbar.item".to_string());
        snapshot.resolved_toolbar_items = unknown_only.resolved();
        assert!(!has_visibility_reset_button(&snapshot));

        let mut mixed = unknown_only;
        mixed.set_hidden(ids::TOP_TOOL_PEN, true);
        snapshot.resolved_toolbar_items = mixed.resolved();
        assert!(has_visibility_reset_button(&snapshot));
    }

    #[test]
    fn active_order_groups_control_shared_top_models() {
        let mut snapshot = snapshot();
        snapshot.resolved_toolbar_items = ToolbarItemsConfig {
            hidden: Vec::new(),
            shown: Vec::new(),
            order: ToolbarItemOrderConfig {
                top_tools: vec![
                    ids::TOP_TOOL_MARKER.as_str().to_string(),
                    ids::TOP_TOOL_PEN.as_str().to_string(),
                ],
                top_controls: vec![
                    ids::TOP_UTILITY_CLEAR_CANVAS.as_str().to_string(),
                    ids::TOP_UTILITY_TEXT.as_str().to_string(),
                ],
            },
        }
        .resolved();

        assert_eq!(
            visible_top_tool_buttons(false, &snapshot)
                .take(2)
                .collect::<Vec<_>>(),
            vec![Tool::Marker, Tool::Pen]
        );
        assert_eq!(
            &visible_top_utility_buttons(&snapshot, false, true)[..2],
            &[TopUtilityButton::ClearCanvas, TopUtilityButton::Text]
        );
    }

    #[test]
    fn factory_visibility_reset_button_uses_unambiguous_wording() {
        let mut snapshot = snapshot();
        let mut items = crate::config::ToolbarItemsConfig::default();
        items.set_hidden(ids::TOP_TOOL_PEN, true);
        snapshot.resolved_toolbar_items = items.resolved();

        let model = ToolbarSettingsModel::for_popover(&snapshot).expect("settings");
        let button = model
            .buttons()
            .iter()
            .find(|button| matches!(button.event, ToolbarEvent::ResetToolbarItemHiddenOverrides))
            .expect("factory visibility reset");
        assert_eq!(button.label.as_ref(), "Restore built-in visibility");
        assert_eq!(
            button.tooltip.as_string().as_deref(),
            Some(
                "Restore built-in visibility for individual toolbar items; section preferences are unchanged"
            )
        );
    }

    #[test]
    fn factory_order_reset_button_uses_unambiguous_wording() {
        let mut snapshot = snapshot();
        let mut items = crate::config::ToolbarItemsConfig::default();
        assert!(items.move_item_to_index(
            crate::config::ToolbarItemOrderGroup::TopTools,
            ids::TOP_TOOL_PEN,
            5,
        ));
        snapshot.resolved_toolbar_items = items.resolved();
        snapshot.customize_items_open = true;
        snapshot.customize_items_group =
            Some(crate::ui::toolbar::ToolbarItemCustomizeGroup::TopTools);

        let model = ToolbarSettingsModel::for_popover(&snapshot).expect("settings");
        let button = model
            .buttons()
            .iter()
            .find(|button| matches!(button.event, ToolbarEvent::ResetToolbarItemOrder(_)))
            .expect("factory order reset");
        assert_eq!(button.label.as_ref(), "Restore built-in order");
        assert_eq!(
            button.tooltip.as_string().as_deref(),
            Some("Restore built-in order for this group; configured order is unchanged")
        );
    }

    #[test]
    fn runtime_persistence_controls_follow_status_and_preserve_complete_paths() {
        let mut snapshot = snapshot();
        let runtime_path = std::path::PathBuf::from(
            "/a/very/long/runtime/state/location/whose/complete/path/must/remain/visible/runtime-ui.toml",
        );
        let artifact_path = std::path::PathBuf::from(
            "/another/very/long/recovery/location/whose/complete/path/must/remain/visible/wayscriber-recovery.toml",
        );
        snapshot.runtime_ui_persistence = Some(RuntimeUiPersistenceSnapshot {
            path: runtime_path.clone(),
            mode: RuntimeUiPersistenceMode::Unhealthy,
            detail: Some("disk outcome is uncertain".to_string()),
            recovery_artifacts: vec![artifact_path.clone()],
        });

        let model = ToolbarSettingsModel::for_popover(&snapshot).expect("settings");
        assert!(
            model
                .buttons()
                .iter()
                .any(|button| { matches!(button.event, ToolbarEvent::RetryRuntimeUiPersistence) })
        );
        assert!(model.buttons().iter().any(|button| {
            matches!(
                button.event,
                ToolbarEvent::DiscardPendingRuntimeUiAndAdoptDisk
            )
        }));
        assert!(model.buttons().iter().any(|button| {
            matches!(
                button.event,
                ToolbarEvent::RequestPreserveInvalidRuntimeUiReset
            )
        }));
        let rendered = model
            .notices()
            .iter()
            .map(|notice| notice.text.as_ref())
            .collect::<String>();
        assert!(rendered.contains(runtime_path.to_string_lossy().as_ref()));
        assert!(rendered.contains(artifact_path.to_string_lossy().as_ref()));

        snapshot.runtime_ui_persistence = Some(RuntimeUiPersistenceSnapshot {
            path: runtime_path,
            mode: RuntimeUiPersistenceMode::AwaitingUnsupportedResetConfirmation {
                version: Some(9),
            },
            detail: None,
            recovery_artifacts: Vec::new(),
        });
        let model = ToolbarSettingsModel::for_popover(&snapshot).expect("settings");
        assert!(
            model.buttons().iter().any(|button| matches!(
                button.event,
                ToolbarEvent::ConfirmUnsupportedRuntimeUiReset
            ))
        );
        assert!(
            model.buttons().iter().any(|button| matches!(
                button.event,
                ToolbarEvent::CancelUnsupportedRuntimeUiReset
            ))
        );

        snapshot.runtime_ui_persistence = Some(RuntimeUiPersistenceSnapshot {
            path: std::path::PathBuf::from("/isolated/runtime-ui.toml"),
            mode: RuntimeUiPersistenceMode::Unavailable,
            detail: Some(
                "writer startup failed; runtime-only toolbar and board changes are process-only"
                    .to_string(),
            ),
            recovery_artifacts: Vec::new(),
        });
        let model = ToolbarSettingsModel::for_popover(&snapshot).expect("settings");
        assert!(!model.buttons().iter().any(|button| {
            matches!(
                button.event,
                ToolbarEvent::RequestRuntimeUiReset
                    | ToolbarEvent::RetryRuntimeUiPersistence
                    | ToolbarEvent::DiscardPendingRuntimeUiAndAdoptDisk
                    | ToolbarEvent::RequestPreserveInvalidRuntimeUiReset
            )
        }));
        let rendered = model
            .notices()
            .iter()
            .map(|notice| notice.text.as_ref())
            .collect::<String>();
        assert!(rendered.contains("persistence is unavailable"));
        assert!(rendered.contains("runtime-only toolbar and board changes are process-only"));
        assert!(rendered.contains("/isolated/runtime-ui.toml"));
    }

    #[test]
    fn event_policy_classifies_persistence() {
        // Chrome the user arranges from the overlay survives a restart as a
        // runtime override.
        assert_eq!(
            ToolbarEventPolicy::for_event(&ToolbarEvent::ToggleStatusBar(false)).persistence,
            ToolbarPersistence::RuntimeUi(ToolbarRuntimeUiPersistenceTarget::StatusBar)
        );
        assert_eq!(
            ToolbarEventPolicy::for_event(&ToolbarEvent::ToggleCustomSection(true)).persistence,
            ToolbarPersistence::RuntimeUi(ToolbarRuntimeUiPersistenceTarget::HistoryCustomSection)
        );
        // Drawing state is not chrome: a thickness change is this run's.
        assert_eq!(
            ToolbarEventPolicy::for_event(&ToolbarEvent::SetThickness(2.0)).persistence,
            ToolbarPersistence::Ephemeral
        );
        assert_eq!(
            ToolbarEventPolicy::for_event(&ToolbarEvent::RetryRuntimeUiPersistence).persistence,
            ToolbarPersistence::Ephemeral
        );
    }

    #[test]
    fn board_picker_button_is_popover_only_and_hideable() {
        let mut snapshot = snapshot();

        // The Canvas popover boards group leads with the board picker — the
        // top bar's only route to the picker.
        let popover = toolbar_boards_model_for_popover(&snapshot).expect("boards popover group");
        assert_eq!(
            popover.buttons.first().map(|button| &button.event),
            Some(&ToolbarEvent::ToggleBoardPicker),
            "the picker leads the popover boards row"
        );
        let picker = popover.buttons.first().expect("board picker button");
        assert_eq!(picker.short_label(&snapshot, "Board"), "Picker");
        assert_eq!(picker.tooltip_label(&snapshot, "Board"), "Board Picker");

        // Hiding side.boards.picker drops the button (config plumbing), while
        // the rest of the boards row survives.
        snapshot.resolved_toolbar_items = crate::config::ToolbarItemsConfig {
            hidden: vec![ids::SIDE_BOARDS_PICKER.as_str().to_string()],
            shown: Vec::new(),
            order: crate::config::ToolbarItemOrderConfig::default(),
        }
        .resolved();
        let hidden =
            toolbar_boards_model_for_popover(&snapshot).expect("boards popover still present");
        assert!(
            !hidden
                .buttons
                .iter()
                .any(|button| button.event == ToolbarEvent::ToggleBoardPicker),
            "hiding side.boards.picker removes the picker button"
        );
        assert!(
            hidden
                .buttons
                .iter()
                .any(|button| button.event == ToolbarEvent::BoardNew),
            "hiding the picker leaves the rest of the boards row intact"
        );
    }

    #[test]
    fn basic_actions_are_canvas_popover_controls_with_top_item_ids() {
        let mut snapshot = snapshot();
        snapshot.show_actions_section = true;
        snapshot.undo_available = true;
        snapshot.redo_available = true;

        let events = toolbar_actions_model_for_popover(&snapshot)
            .expect("actions")
            .buttons
            .into_iter()
            .map(|button| button.event)
            .collect::<Vec<_>>();
        assert_eq!(
            events,
            [
                ToolbarEvent::Undo,
                ToolbarEvent::Redo,
                ToolbarEvent::ClearCanvas { instant: false },
            ]
        );

        snapshot
            .resolved_toolbar_items
            .hidden
            .insert(ids::TOP_UTILITY_UNDO);
        let hidden = toolbar_actions_model_for_popover(&snapshot).expect("remaining actions");
        assert!(
            !hidden
                .buttons
                .iter()
                .any(|button| button.event == ToolbarEvent::Undo)
        );
        assert!(
            hidden
                .buttons
                .iter()
                .any(|button| button.event == ToolbarEvent::Redo)
        );

        snapshot.show_actions_section = false;
        assert!(toolbar_actions_model_for_popover(&snapshot).is_none());
    }

    #[test]
    fn retained_side_ids_still_control_unified_top_models() {
        fn hide(snapshot: &mut ToolbarSnapshot, id: crate::config::ToolbarItemId) {
            snapshot.resolved_toolbar_items = crate::config::ToolbarItemsConfig {
                hidden: vec![id.as_str().to_string()],
                shown: Vec::new(),
                order: crate::config::ToolbarItemOrderConfig::default(),
            }
            .resolved();
        }

        let mut snapshot = snapshot();
        snapshot.show_actions_advanced = true;
        snapshot.delay_actions_enabled = true;

        assert!(
            toolbar_zoom_group_for_popover(&snapshot)
                .expect("zoom")
                .buttons
                .iter()
                .any(|button| button.event == ToolbarEvent::ZoomIn)
        );
        hide(&mut snapshot, ids::SIDE_ACTIONS_ZOOM_IN);
        assert!(
            !toolbar_zoom_group_for_popover(&snapshot)
                .expect("zoom")
                .buttons
                .iter()
                .any(|button| button.event == ToolbarEvent::ZoomIn),
            "hiding side.actions.zoom-in removes the Canvas zoom-in control"
        );

        snapshot = self::snapshot();
        assert!(
            toolbar_pages_model_for_popover(&snapshot)
                .expect("pages")
                .buttons
                .iter()
                .any(|button| button.event == ToolbarEvent::PageDelete)
        );
        hide(&mut snapshot, ids::SIDE_PAGES_DELETE);
        assert!(
            !toolbar_pages_model_for_popover(&snapshot)
                .expect("pages")
                .buttons
                .iter()
                .any(|button| button.event == ToolbarEvent::PageDelete),
            "hiding side.pages.delete removes the Canvas page-delete control"
        );

        snapshot = self::snapshot();
        snapshot.show_actions_advanced = true;
        assert!(
            toolbar_advanced_group_for_popover(&snapshot)
                .expect("advanced")
                .buttons
                .iter()
                .any(|button| button.event == ToolbarEvent::ToggleFreeze)
        );
        hide(&mut snapshot, ids::SIDE_ACTIONS_FREEZE);
        assert!(
            !toolbar_advanced_group_for_popover(&snapshot)
                .expect("advanced")
                .buttons
                .iter()
                .any(|button| button.event == ToolbarEvent::ToggleFreeze),
            "hiding side.actions.freeze removes the Canvas freeze control"
        );

        snapshot = self::snapshot();
        assert!(
            ToolbarSessionModel::for_popover(&snapshot)
                .expect("session")
                .buttons
                .iter()
                .any(|button| button.event == ToolbarEvent::SessionInfo)
        );
        hide(&mut snapshot, ids::SIDE_SESSION_INFO);
        assert!(
            !ToolbarSessionModel::for_popover(&snapshot)
                .expect("session")
                .buttons
                .iter()
                .any(|button| button.event == ToolbarEvent::SessionInfo),
            "hiding side.session.info removes the Session Info control"
        );

        snapshot = self::snapshot();
        let about_visible = |snapshot: &ToolbarSnapshot| {
            ToolbarSettingsModel::for_popover(snapshot)
                .expect("settings")
                .buttons()
                .iter()
                .any(|button| matches!(button.event, ToolbarEvent::OpenAbout))
        };
        assert!(about_visible(&snapshot));
        hide(&mut snapshot, ids::SIDE_SETTINGS_ABOUT);
        assert!(
            !about_visible(&snapshot),
            "hiding side.settings.about removes the Settings About control"
        );
    }
}
