//! Structure, content-key, and width-plan tests.

use super::super::{
    Action, CanvasMenuContentKey, SessionMenuContentKey, SettingsMenuContentKey, StructureKey,
    Tool, ToolbarLayoutMode, ToolbarSnapshot, TopStripPlan, model, plan_top_strip,
    top_default_width, top_toolbar_size,
};
use super::expectations::style_pill_tool_snapshot;
use crate::config::Shortcut;
use crate::config::ToolbarSectionFlag;
use crate::config::toolbar_item_ids as ids;
use crate::input::state::test_support::make_test_input_state;
use crate::ui::toolbar::RuntimeUiPersistenceMode;
use crate::ui::toolbar::RuntimeUiPersistenceSnapshot;
use crate::ui::toolbar::ToolbarBindingHints;
use std::collections::HashMap;

#[test]
fn top_structure_rebuilds_when_current_shortcuts_change() {
    let mut state = make_test_input_state();
    let initial = ToolbarSnapshot::from_input_with_bindings(
        &state,
        ToolbarBindingHints::from_input_state(&state),
    );
    let initial_plan = plan_top_strip(&crate::ui_text::UiTextEngine::default(), &initial);
    let initial_key = StructureKey::of(&initial, &initial_plan);

    state.set_action_bindings(HashMap::from([(
        Action::SelectPenTool,
        vec![Shortcut::parse("9").expect("binding")],
    )]));
    let changed = ToolbarSnapshot::from_input_with_bindings(
        &state,
        ToolbarBindingHints::from_input_state(&state),
    );
    let changed_plan = plan_top_strip(&crate::ui_text::UiTextEngine::default(), &changed);
    let changed_key = StructureKey::of(&changed, &changed_plan);

    assert!(initial_key != changed_key);
    assert_eq!(changed.binding_hints.badge_for_tool(Tool::Pen), Some("9"));
}

/// Popover content keys track `use_icons` directly because their action
/// buttons render differently in icon and text modes.
#[test]
fn popover_content_keys_track_icon_mode() {
    let mut state = make_test_input_state();
    state.set_toolbar_use_icons(false);
    let text_mode = ToolbarSnapshot::from_input_with_bindings(
        &state,
        ToolbarBindingHints::from_input_state(&state),
    );
    state.set_toolbar_use_icons(true);
    let icon_mode = ToolbarSnapshot::from_input_with_bindings(
        &state,
        ToolbarBindingHints::from_input_state(&state),
    );

    assert!(
        SettingsMenuContentKey::of(&text_mode) != SettingsMenuContentKey::of(&icon_mode),
        "settings popover content key tracks icon mode"
    );
    assert!(
        SessionMenuContentKey::of(&text_mode) != SessionMenuContentKey::of(&icon_mode),
        "session popover content key tracks icon mode"
    );
    assert!(
        CanvasMenuContentKey::of(&text_mode) != CanvasMenuContentKey::of(&icon_mode),
        "canvas popover content key tracks icon mode"
    );
}

#[test]
fn settings_popover_rebuilds_when_runtime_persistence_controls_change() {
    let state = make_test_input_state();
    let mut unhealthy = ToolbarSnapshot::from_input_with_bindings(
        &state,
        ToolbarBindingHints::from_input_state(&state),
    );
    unhealthy.runtime_ui_persistence = Some(RuntimeUiPersistenceSnapshot {
        path: "/tmp/runtime-ui.toml".into(),
        mode: RuntimeUiPersistenceMode::Unhealthy,
        detail: None,
        recovery_artifacts: Vec::new(),
    });
    let mut confirmation = unhealthy.clone();
    confirmation.runtime_ui_persistence.as_mut().unwrap().mode =
        RuntimeUiPersistenceMode::AwaitingInvalidResetConfirmation;

    assert!(
        SettingsMenuContentKey::of(&unhealthy) != SettingsMenuContentKey::of(&confirmation),
        "the top settings popover must replace recovery actions with confirm/cancel controls"
    );
}

#[test]
fn settings_popover_rebuilds_for_status_bar_contents_subpanel() {
    let state = make_test_input_state();
    let closed = ToolbarSnapshot::from_input_with_bindings(
        &state,
        ToolbarBindingHints::from_input_state(&state),
    );
    let mut open = closed.clone();
    open.status_bar_contents_open = true;

    assert!(
        SettingsMenuContentKey::of(&closed) != SettingsMenuContentKey::of(&open),
        "opening status-bar contents must rebuild the GTK Settings popover"
    );
}

#[test]
fn settings_popover_keeps_content_when_status_bar_interactivity_changes() {
    let state = make_test_input_state();
    let base = ToolbarSnapshot::from_input_with_bindings(
        &state,
        ToolbarBindingHints::from_input_state(&state),
    );
    let mut changed = base.clone();
    changed.status_bar_interactive = !base.status_bar_interactive;

    assert!(
        SettingsMenuContentKey::of(&base) == SettingsMenuContentKey::of(&changed),
        "changing status-bar interactivity must update the existing GTK controls"
    );
}

#[test]
fn settings_popover_keeps_content_when_any_status_bar_item_visibility_changes() {
    let state = make_test_input_state();
    let base = ToolbarSnapshot::from_input_with_bindings(
        &state,
        ToolbarBindingHints::from_input_state(&state),
    );
    let mutations: [fn(&mut ToolbarSnapshot); 11] = [
        |s| {
            s.show_active_output_badge = !s.show_active_output_badge;
        },
        |s| {
            s.show_status_selection_info = !s.show_status_selection_info;
        },
        |s| {
            s.show_status_board_badge = !s.show_status_board_badge;
        },
        |s| {
            s.show_status_page_badge = !s.show_status_page_badge;
        },
        |s| s.show_status_color = !s.show_status_color,
        |s| s.show_status_tool = !s.show_status_tool,
        |s| s.show_status_size = !s.show_status_size,
        |s| {
            s.show_status_context_indicators = !s.show_status_context_indicators;
        },
        |s| {
            s.show_toolbar_hint = !s.show_toolbar_hint;
        },
        |s| s.show_status_help = !s.show_status_help,
        |s| s.show_status_about = !s.show_status_about,
    ];

    for mutate in mutations {
        let mut changed = base.clone();
        mutate(&mut changed);
        assert!(
            SettingsMenuContentKey::of(&base) == SettingsMenuContentKey::of(&changed),
            "changing any status item visibility must update the existing GTK controls"
        );
    }
}

/// The main Settings page filters its toggle grid and offers "Restore
/// built-in visibility" from the resolved item store, and that button keeps
/// the popover open. Item-visibility changes must therefore rebuild the
/// popover content even while the customization sub-panel is closed, or the
/// restored controls stay missing and the Restore button stays stale.
#[test]
fn settings_popover_rebuilds_when_item_visibility_changes_outside_customization() {
    let state = make_test_input_state();
    let base = ToolbarSnapshot::from_input_with_bindings(
        &state,
        ToolbarBindingHints::from_input_state(&state),
    );
    assert!(!base.customize_items_open);
    let mut changed = base.clone();
    changed
        .resolved_toolbar_items
        .hidden
        .insert(ids::SIDE_SETTINGS_PRESET_TOASTS);
    changed
        .resolved_toolbar_items
        .shown
        .remove(&ids::SIDE_SETTINGS_PRESET_TOASTS);

    assert!(
        SettingsMenuContentKey::of(&base) != SettingsMenuContentKey::of(&changed),
        "hiding or restoring a settings item must rebuild the GTK Settings popover"
    );
}

#[test]
fn top_structure_ignores_popover_only_section_visibility_changes() {
    let state = make_test_input_state();
    let base = ToolbarSnapshot::from_input_with_bindings(
        &state,
        ToolbarBindingHints::from_input_state(&state),
    );
    let base_key = StructureKey::of(
        &base,
        &plan_top_strip(&crate::ui_text::UiTextEngine::default(), &base),
    );

    for flag in [
        ToolbarSectionFlag::Actions,
        ToolbarSectionFlag::ActionsAdvanced,
        ToolbarSectionFlag::ZoomActions,
        ToolbarSectionFlag::Pages,
        ToolbarSectionFlag::Boards,
        ToolbarSectionFlag::StepSection,
    ] {
        let mut changed = base.clone();
        match flag {
            ToolbarSectionFlag::Actions => {
                changed.show_actions_section = !changed.show_actions_section;
            }
            ToolbarSectionFlag::ActionsAdvanced => {
                changed.show_actions_advanced = !changed.show_actions_advanced;
            }
            ToolbarSectionFlag::ZoomActions => {
                changed.show_zoom_actions = !changed.show_zoom_actions;
            }
            ToolbarSectionFlag::Pages => {
                changed.show_pages_section = !changed.show_pages_section;
            }
            ToolbarSectionFlag::Boards => {
                changed.show_boards_section = !changed.show_boards_section;
            }
            ToolbarSectionFlag::StepSection => {
                changed.show_step_section = !changed.show_step_section;
            }
            ToolbarSectionFlag::Presets | ToolbarSectionFlag::TextControls => continue,
        }
        changed.resolved_toolbar_items.hidden.insert(flag.item_id());
        changed.resolved_toolbar_items.shown.remove(&flag.item_id());
        let changed_key = StructureKey::of(
            &changed,
            &plan_top_strip(&crate::ui_text::UiTextEngine::default(), &changed),
        );
        assert!(
            base_key == changed_key,
            "popover-only {flag:?} visibility must not rebuild the top bar"
        );
    }
}

#[test]
fn top_structure_still_tracks_top_item_visibility() {
    let state = make_test_input_state();
    let base = ToolbarSnapshot::from_input_with_bindings(
        &state,
        ToolbarBindingHints::from_input_state(&state),
    );
    let mut changed = base.clone();
    changed
        .resolved_toolbar_items
        .hidden
        .insert(ids::TOP_TOOL_PEN);
    changed
        .resolved_toolbar_items
        .shown
        .remove(&ids::TOP_TOOL_PEN);

    assert!(
        StructureKey::of(
            &base,
            &plan_top_strip(&crate::ui_text::UiTextEngine::default(), &base)
        ) != StructureKey::of(
            &changed,
            &plan_top_strip(&crate::ui_text::UiTextEngine::default(), &changed)
        ),
        "top-item visibility must still rebuild the top bar"
    );
}

#[test]
fn canvas_popover_content_key_rebuilds_on_section_and_value_changes() {
    let state = make_test_input_state();
    let base = ToolbarSnapshot::from_input_with_bindings(
        &state,
        ToolbarBindingHints::from_input_state(&state),
    );

    // Every section display toggle drives a content rebuild.
    let mut actions_toggled = base.clone();
    actions_toggled.show_actions_section = !base.show_actions_section;
    assert!(
        CanvasMenuContentKey::of(&base) != CanvasMenuContentKey::of(&actions_toggled),
        "toggling Actions rebuilds the canvas popover content"
    );

    let mut toggled = base.clone();
    toggled.show_boards_section = !base.show_boards_section;
    assert!(
        CanvasMenuContentKey::of(&base) != CanvasMenuContentKey::of(&toggled),
        "toggling a section rebuilds the canvas popover content"
    );

    // A step-count change (no structural change) still rebuilds the content.
    let mut stepped = base.clone();
    stepped.custom_undo_steps = base.custom_undo_steps + 1;
    assert!(
        CanvasMenuContentKey::of(&base) != CanvasMenuContentKey::of(&stepped),
        "a step-count change rebuilds the canvas popover content"
    );

    // A no-op change leaves the key stable, so hover/press survive.
    assert!(
        CanvasMenuContentKey::of(&base) == CanvasMenuContentKey::of(&base.clone()),
        "an unchanged snapshot keeps the content key stable"
    );
}

/// Each delay slider emits continuously during a drag: if its value were part
/// of the content key, the first backend echo would rebuild the whole popover
/// subtree, destroying the live gesture and resetting the scroll. So a
/// delay-value change must leave the content key stable — the values ride the
/// persistent Canvas popover value updaters instead (set in place, a no-op mid-drag).
#[test]
fn canvas_popover_content_key_ignores_delay_slider_values() {
    let state = make_test_input_state();
    let base = ToolbarSnapshot::from_input_with_bindings(
        &state,
        ToolbarBindingHints::from_input_state(&state),
    );

    let mutations: [fn(&mut ToolbarSnapshot); 4] = [
        |s| s.custom_undo_delay_ms += 250,
        |s| s.custom_redo_delay_ms += 250,
        |s| s.undo_all_delay_ms += 250,
        |s| s.redo_all_delay_ms += 250,
    ];
    for mutate in mutations {
        let mut changed = base.clone();
        mutate(&mut changed);
        assert!(
            CanvasMenuContentKey::of(&base) == CanvasMenuContentKey::of(&changed),
            "a delay-slider value change must not rebuild the canvas popover content"
        );
    }

    // Guard: the step counts (changed by discrete −/+ clicks, never a drag)
    // stay in the key, so they still rebuild — no drag hazard there.
    let mut stepped = base.clone();
    stepped.custom_undo_steps += 1;
    assert!(
        CanvasMenuContentKey::of(&base) != CanvasMenuContentKey::of(&stepped),
        "a step-count change still rebuilds the canvas popover content"
    );
}

#[test]
fn simple_layout_requests_its_smaller_natural_width() {
    let mut state = make_test_input_state();
    state.set_toolbar_use_icons(true);
    let regular = ToolbarSnapshot::from_input_with_bindings(
        &state,
        ToolbarBindingHints::from_input_state(&state),
    );

    state.test_set_toolbar_layout(
        ToolbarLayoutMode::Simple,
        state.toolbar_mode_overrides().clone(),
    );
    let simple = ToolbarSnapshot::from_input_with_bindings(
        &state,
        ToolbarBindingHints::from_input_state(&state),
    );

    let regular_width = top_default_width(&crate::ui_text::UiTextEngine::default(), &regular);
    let simple_width = top_default_width(&crate::ui_text::UiTextEngine::default(), &simple);
    assert!(simple_width < regular_width);
    assert_eq!(
        simple_width,
        top_toolbar_size(&crate::ui_text::UiTextEngine::default(), &simple).0 as i32
    );
}

#[test]
fn degraded_layout_requests_the_selected_plan_width() {
    let state = make_test_input_state();
    let mut snapshot = ToolbarSnapshot::from_input_with_bindings(
        &state,
        ToolbarBindingHints::from_input_state(&state),
    );
    snapshot.top_viewport_max = Some(700.0);

    let plan = plan_top_strip(&crate::ui_text::UiTextEngine::default(), &snapshot);
    let degraded = plan.compact
        || plan.drop_presets
        || !plan.dropped_tools.is_empty()
        || !plan.dropped_utilities.is_empty()
        || plan.swatch_count < 8;
    assert!(degraded, "the 700px budget must degrade the plan: {plan:?}");
    assert!(top_default_width(&crate::ui_text::UiTextEngine::default(), &snapshot) <= 700);
}

/// Colors left the strip for the pill (M7-C1); the presets island is the new
/// non-essential island there, and it is the first thing to yield under the
/// compact plan (M7-C2).
#[test]
fn compact_plan_drops_the_presets_island() {
    let state = make_test_input_state();
    let snapshot = ToolbarSnapshot::from_input_with_bindings(
        &state,
        ToolbarBindingHints::from_input_state(&state),
    );
    let has_preset = |spec: &model::TopToolbarSpec| {
        spec.strip().iter().any(|node| {
            matches!(
                node,
                model::TopToolbarNode::Control(model::TopToolbarControl::Preset(_))
            )
        })
    };

    // The default state shows presets, so the unconstrained plan lists them.
    let full = super::super::strip::top_toolbar_spec(&snapshot, &TopStripPlan::unconstrained());
    assert!(has_preset(&full));

    // A compact plan drops the whole non-essential presets island.
    let mut compact = TopStripPlan::unconstrained();
    compact.compact = true;
    assert!(!has_preset(&super::super::strip::top_toolbar_spec(
        &snapshot, &compact
    )));
}

#[test]
fn top_structure_rebuilds_when_the_style_pill_morphs() {
    let state = make_test_input_state();
    let regular = ToolbarSnapshot::from_input_with_bindings(
        &state,
        ToolbarBindingHints::from_input_state(&state),
    );
    let pen = style_pill_tool_snapshot(&regular, Tool::Pen);
    let eraser = style_pill_tool_snapshot(&regular, Tool::Eraser);

    let pen_key = StructureKey::of(
        &pen,
        &plan_top_strip(&crate::ui_text::UiTextEngine::default(), &pen),
    );
    let eraser_key = StructureKey::of(
        &eraser,
        &plan_top_strip(&crate::ui_text::UiTextEngine::default(), &eraser),
    );
    assert!(
        pen_key != eraser_key,
        "a pill morph change must rebuild the GTK bar structure"
    );

    // Pure value churn (thickness) keeps the structure stable: live values
    // run through updaters, not rebuilds.
    let mut thicker = pen.clone();
    thicker.thickness += 3.0;
    let thicker_key = StructureKey::of(
        &thicker,
        &plan_top_strip(&crate::ui_text::UiTextEngine::default(), &thicker),
    );
    assert!(pen_key == thicker_key, "value churn must not rebuild");

    // Every stroke-controls style is its own structure, even the meter and
    // stepper whose control ids match; the open panel and the levels are not.
    let key_of = |snapshot: &ToolbarSnapshot| {
        StructureKey::of(
            snapshot,
            &plan_top_strip(&crate::ui_text::UiTextEngine::default(), snapshot),
        )
    };
    let styled = |style| {
        let mut snapshot = pen.clone();
        snapshot.stroke_controls = style;
        key_of(&snapshot)
    };
    let [panel, meter, stepper] = crate::config::ToolbarStrokeControls::ALL.map(styled);
    assert!(panel != meter && meter != stepper && panel != stepper);
    let mut open = pen.clone();
    open.pen_feel_open = true;
    open.pen_smoothing = 6;
    assert!(key_of(&open) == panel, "opening the panel must not rebuild");
}
