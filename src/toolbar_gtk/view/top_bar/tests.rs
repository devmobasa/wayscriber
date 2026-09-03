//! GTK top-strip unit tests.

use std::collections::HashMap;
use std::ffi::{CStr, CString};
use std::time::Duration;

use super::*;
use crate::config::{Shortcut, ToolbarSectionFlag, toolbar_item_ids as ids};
use crate::input::state::test_support::make_test_input_state;
use crate::toolbar_gtk::widgets::{emit_secondary_press, secondary_click_gesture};
use crate::ui::toolbar::{
    RuntimeUiPersistenceMode, RuntimeUiPersistenceSnapshot, ToolbarBindingHints,
};
use gtk4::prelude::*;

#[test]
fn top_structure_rebuilds_when_current_shortcuts_change() {
    let mut state = make_test_input_state();
    let initial = ToolbarSnapshot::from_input_with_bindings(
        &state,
        ToolbarBindingHints::from_input_state(&state),
    );
    let initial_plan = plan_top_strip(&initial);
    let initial_key = StructureKey::of(&initial, &initial_plan);

    state.set_action_bindings(HashMap::from([(
        Action::SelectPenTool,
        vec![Shortcut::parse("9").expect("binding")],
    )]));
    let changed = ToolbarSnapshot::from_input_with_bindings(
        &state,
        ToolbarBindingHints::from_input_state(&state),
    );
    let changed_plan = plan_top_strip(&changed);
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
    let base_key = StructureKey::of(&base, &plan_top_strip(&base));

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
        let changed_key = StructureKey::of(&changed, &plan_top_strip(&changed));
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
        StructureKey::of(&base, &plan_top_strip(&base))
            != StructureKey::of(&changed, &plan_top_strip(&changed)),
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
/// persistent `canvas_updaters` instead (set in place, a no-op mid-drag).
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

    let regular_width = top_default_width(&regular);
    let simple_width = top_default_width(&simple);
    assert!(simple_width < regular_width);
    assert_eq!(simple_width, top_toolbar_size(&simple).0 as i32);
}

#[test]
fn degraded_layout_requests_the_selected_plan_width() {
    let state = make_test_input_state();
    let mut snapshot = ToolbarSnapshot::from_input_with_bindings(
        &state,
        ToolbarBindingHints::from_input_state(&state),
    );
    snapshot.top_viewport_max = Some(700.0);

    let plan = plan_top_strip(&snapshot);
    let degraded = plan.compact
        || plan.drop_presets
        || !plan.dropped_tools.is_empty()
        || !plan.dropped_utilities.is_empty()
        || plan.swatch_count < 8;
    assert!(degraded, "the 700px budget must degrade the plan: {plan:?}");
    assert!(top_default_width(&snapshot) <= 700);
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
    let full = super::strip::top_toolbar_spec(&snapshot, &TopStripPlan::unconstrained());
    assert!(has_preset(&full));

    // A compact plan drops the whole non-essential presets island.
    let mut compact = TopStripPlan::unconstrained();
    compact.compact = true;
    assert!(!has_preset(&super::strip::top_toolbar_spec(
        &snapshot, &compact
    )));
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SemanticLane {
    Strip,
    Contextual,
    Chrome,
    Overflow,
}

#[derive(Debug, Clone, PartialEq)]
struct SemanticControlRecord {
    lane: SemanticLane,
    id: String,
    event: ToolbarEvent,
    label: String,
    accessible_label: String,
    tooltip: String,
    shortcut_badge: Option<String>,
    enabled: bool,
    active: bool,
    role: model::TopToolbarControlRole,
    icon: Option<model::TopToolbarIcon>,
}

#[derive(Debug, Clone, PartialEq)]
enum SemanticAdapterRecord {
    Divider(&'static str),
    Control(SemanticControlRecord),
}

fn control_record(
    snapshot: &ToolbarSnapshot,
    lane: SemanticLane,
    control: model::TopToolbarControl,
    show_badge: bool,
) -> SemanticControlRecord {
    SemanticControlRecord {
        lane,
        id: control.id().render_id().into_owned(),
        event: control.event(snapshot),
        label: control.label(snapshot).into_owned(),
        accessible_label: control.accessible_label(snapshot).into_owned(),
        tooltip: if lane == SemanticLane::Overflow {
            control.overflow_tooltip(snapshot)
        } else {
            control.tooltip(snapshot)
        },
        shortcut_badge: show_badge
            .then(|| control.shortcut_badge(snapshot))
            .flatten(),
        enabled: control.enabled(snapshot),
        active: control.active(snapshot),
        role: control.role(),
        icon: control.icon(snapshot),
    }
}

fn expected_semantic_records(
    snapshot: &ToolbarSnapshot,
    spec: &model::TopToolbarSpec,
    plan: &TopStripPlan,
) -> Vec<SemanticAdapterRecord> {
    let mut records = Vec::new();
    for node in spec.strip() {
        match *node {
            model::TopToolbarNode::Divider(divider) => {
                records.push(SemanticAdapterRecord::Divider(divider.id()));
            }
            model::TopToolbarNode::Control(control) => {
                // Colors left the strip (M7-C1); badges now ride tool and
                // utility buttons only, which drop to icons under compact.
                let show_badge = !plan.compact;
                records.push(SemanticAdapterRecord::Control(control_record(
                    snapshot,
                    SemanticLane::Strip,
                    control,
                    show_badge,
                )));
                if matches!(
                    control,
                    model::TopToolbarControl::Utility(model::TopToolbarUtility::Highlight)
                ) {
                    records.extend(spec.contextual().iter().copied().map(|contextual| {
                        SemanticAdapterRecord::Control(control_record(
                            snapshot,
                            SemanticLane::Contextual,
                            contextual,
                            false,
                        ))
                    }));
                }
            }
        }
    }
    records.extend(spec.chrome().iter().copied().map(|control| {
        SemanticAdapterRecord::Control(control_record(
            snapshot,
            SemanticLane::Chrome,
            control,
            false,
        ))
    }));
    if snapshot.top_overflow_open {
        records.extend(spec.overflow().iter().copied().map(|control| {
            SemanticAdapterRecord::Control(control_record(
                snapshot,
                SemanticLane::Overflow,
                control,
                !plan.compact,
            ))
        }));
    }
    records
}

/// Variant of `base` with one tool active and the settings overrides pinned
/// off, so each scenario exercises exactly one pure style-pill morph state.
fn style_pill_tool_snapshot(base: &ToolbarSnapshot, tool: Tool) -> ToolbarSnapshot {
    let mut snapshot = base.clone();
    snapshot.active_tool = tool;
    snapshot.tool_override = None;
    snapshot.thickness_targets_eraser = tool == Tool::Eraser;
    snapshot.thickness_targets_marker = tool == Tool::Marker;
    snapshot.show_text_controls = false;
    snapshot.show_marker_opacity_section = false;
    snapshot
}

fn selection_property_entry(
    label: &str,
    value: &str,
    kind: crate::input::SelectionPropertyKind,
    disabled: bool,
) -> crate::input::SelectionPropertyEntry {
    crate::input::SelectionPropertyEntry {
        label: label.to_string(),
        value: value.to_string(),
        kind,
        disabled,
    }
}

/// Select tool with a docked selection: a cycle entry, a stepper entry,
/// and a locked (disabled) cycle entry.
fn style_pill_selection_snapshot(base: &ToolbarSnapshot) -> ToolbarSnapshot {
    use crate::input::SelectionPropertyKind as K;
    let mut snapshot = style_pill_tool_snapshot(base, Tool::Select);
    snapshot.selection_properties = vec![
        selection_property_entry("Color", "Red", K::Color, false),
        selection_property_entry("Thickness", "3.0px", K::Thickness, false),
        selection_property_entry("Fill", "Locked", K::Fill, true),
    ];
    snapshot.selection_has_text = true;
    snapshot.selected_text_bold = Some(false);
    snapshot
}

fn record_id(record: &SemanticAdapterRecord) -> &str {
    match record {
        SemanticAdapterRecord::Divider(id) => id,
        SemanticAdapterRecord::Control(control) => &control.id,
    }
}

fn record_lane(record: &SemanticAdapterRecord) -> SemanticLane {
    match record {
        SemanticAdapterRecord::Divider(_) => SemanticLane::Strip,
        SemanticAdapterRecord::Control(control) => control.lane,
    }
}

fn collect_semantic_widgets(root: &gtk4::Widget) -> Vec<gtk4::Widget> {
    fn visit(widget: &gtk4::Widget, widgets: &mut Vec<gtk4::Widget>) {
        if widget.widget_name().starts_with("top.") {
            widgets.push(widget.clone());
            return;
        }
        let mut child = widget.first_child();
        while let Some(current) = child {
            child = current.next_sibling();
            visit(&current, widgets);
        }
    }

    let mut widgets = Vec::new();
    let mut child = root.first_child();
    while let Some(current) = child {
        child = current.next_sibling();
        visit(&current, &mut widgets);
    }
    widgets
}

/// Key of the nearest `island.<key>`-named ancestor container, or None when
/// the widget lives outside every pill island (the contextual ring row and
/// the minimized/micro single-control surfaces).
fn nearest_island_key(widget: &gtk4::Widget) -> Option<String> {
    let mut ancestor = widget.parent();
    while let Some(current) = ancestor {
        let name = current.widget_name();
        if let Some(key) = name.as_str().strip_prefix("island.") {
            return Some(key.to_string());
        }
        ancestor = current.parent();
    }
    None
}

/// Expected island container per semantic widget id, derived from the shared
/// spec's `node.island()`/`control.island()` accessors. `None` marks widgets
/// the GTK adapter intentionally hosts outside the pill islands: the
/// contextual ring row (its own detached pill below the strip) and the
/// minimized tab / micro chip (the whole surface is the control).
fn expected_island_keys(
    snapshot: &ToolbarSnapshot,
    spec: &model::TopToolbarSpec,
    plan: &TopStripPlan,
) -> HashMap<String, Option<&'static str>> {
    let mut expected = HashMap::new();
    let islands_built = !snapshot.top_minimized && !snapshot.top_micro_active();
    for node in spec.strip() {
        let id = match node {
            model::TopToolbarNode::Divider(divider) => divider.id().to_string(),
            model::TopToolbarNode::Control(control) => control.id().render_id().into_owned(),
        };
        expected.insert(id, islands_built.then(|| node.island().key()));
    }
    for control in spec.chrome() {
        expected.insert(
            control.id().render_id().into_owned(),
            islands_built.then(|| control.island().key()),
        );
    }
    for control in spec.contextual() {
        expected.insert(control.id().render_id().into_owned(), None);
    }
    // Every style-pill control must sit inside the detached `island.style`
    // pill box under the band.
    for (id, _) in style_pill_controls(snapshot, plan) {
        expected.insert(id, Some("style"));
    }
    expected
}

/// Ordered `(id, control)` list of the style pill for one snapshot, from
/// the shared morph spec both frontends render.
fn style_pill_controls(
    snapshot: &ToolbarSnapshot,
    plan: &TopStripPlan,
) -> Vec<(String, model::StylePillControl)> {
    model::StylePillSpec::build(snapshot, plan)
        .controls()
        .iter()
        .map(|control| (control.id().into_owned(), *control))
        .collect()
}

fn expected_main_widget_ids(
    spec: &model::TopToolbarSpec,
    snapshot: &ToolbarSnapshot,
    plan: &TopStripPlan,
) -> Vec<String> {
    let mut ids = spec
        .strip()
        .iter()
        .map(|node| match node {
            model::TopToolbarNode::Divider(divider) => divider.id().to_string(),
            model::TopToolbarNode::Control(control) => control.id().render_id().into_owned(),
        })
        .collect::<Vec<_>>();
    ids.extend(
        spec.chrome()
            .iter()
            .chain(spec.contextual())
            .map(|control| control.id().render_id().into_owned()),
    );
    // The style pill renders under the islands, after every band widget.
    ids.extend(
        style_pill_controls(snapshot, plan)
            .into_iter()
            .map(|(id, _)| id),
    );
    ids
}

fn find_control_surface(root: &gtk4::Widget) -> Option<gtk4::Widget> {
    if root.is::<gtk4::Button>() || root.is::<gtk4::CheckButton>() || root.is::<gtk4::DrawingArea>()
    {
        return Some(root.clone());
    }
    let mut child = root.first_child();
    while let Some(current) = child {
        child = current.next_sibling();
        if let Some(surface) = find_control_surface(&current) {
            return Some(surface);
        }
    }
    None
}

fn first_control_surface(root: &gtk4::Widget) -> gtk4::Widget {
    find_control_surface(root).unwrap_or_else(|| {
        panic!(
            "semantic widget has no control surface: {}",
            root.widget_name()
        )
    })
}

fn shortcut_badge_text(root: &gtk4::Widget) -> Option<String> {
    if let Ok(label) = root.clone().downcast::<gtk4::Label>()
        && label.has_css_class("shortcut-badge")
        && !label.text().is_empty()
    {
        return Some(label.text().to_string());
    }
    let mut child = root.first_child();
    while let Some(current) = child {
        child = current.next_sibling();
        if let Some(text) = shortcut_badge_text(&current) {
            return Some(text);
        }
    }
    None
}

fn assert_accessible_label(widget: &gtk4::Widget, expected: &str, id: &str) {
    let expected = CString::new(expected).expect("accessible label contains no NUL");
    // GTK returns a newly allocated diagnostic string on mismatch and null
    // when the live accessible property has the requested value.
    let mismatch = unsafe {
        gtk4::ffi::gtk_test_accessible_check_property(
            widget.as_ptr().cast(),
            gtk4::ffi::GTK_ACCESSIBLE_PROPERTY_LABEL,
            expected.as_ptr(),
        )
    };
    if mismatch.is_null() {
        return;
    }
    let message = unsafe { CStr::from_ptr(mismatch) }
        .to_string_lossy()
        .into_owned();
    unsafe { gtk4::glib::ffi::g_free(mismatch.cast()) };
    panic!("{id} accessible label: {message}");
}

fn assert_gtk_control_widget(widget: &gtk4::Widget, expected: &SemanticControlRecord) {
    let surface = first_control_surface(widget);
    assert_accessible_label(&surface, &expected.accessible_label, &expected.id);
    assert_eq!(
        surface.tooltip_text().as_deref(),
        Some(expected.tooltip.as_str()),
        "{} tooltip",
        expected.id
    );
    assert_eq!(
        surface.is_sensitive(),
        expected.enabled,
        "{} enabled state",
        expected.id
    );
    assert_eq!(
        shortcut_badge_text(widget),
        expected.shortcut_badge,
        "{} shortcut badge",
        expected.id
    );

    if expected.role == model::TopToolbarControlRole::Destructive {
        assert!(surface.has_css_class("destructive"), "{}", expected.id);
    }
    if expected.id == crate::config::toolbar_item_ids::TOP_CHROME_PIN.as_str() {
        assert_eq!(
            surface.has_css_class("pinned"),
            expected.active,
            "{} pinned state",
            expected.id
        );
    } else if let Ok(check) = surface.clone().downcast::<gtk4::CheckButton>() {
        assert_eq!(check.is_active(), expected.active, "{} state", expected.id);
    } else {
        assert_eq!(
            surface.has_css_class("active"),
            expected.active,
            "{} active class",
            expected.id
        );
    }

    if let Ok(button) = surface.clone().downcast::<gtk4::Button>()
        && let Some(label) = button.label()
    {
        assert_eq!(label, expected.label, "{} text label", expected.id);
    }
}

/// Assert one GTK style-pill widget against its shared-spec control: widget
/// class per role, live label/value text, tooltip, active state, and the
/// segment halves' labels/actives for segmented controls.
/// Width the font button asks for, or `None` when this widget is not it.
///
/// The only pill label the system supplies rather than this program, so it is
/// the only one whose width is not known in advance. `set_size_request` is a
/// *minimum* in GTK: an unbounded label grows the button past the slot the
/// layout planned and pushes the rest of the pill off the arrangement the
/// builtin toolbar drew from the same plan.
fn font_button_natural_width(
    widget: &gtk4::Widget,
    control: model::StylePillControl,
) -> Option<i32> {
    if control != model::StylePillControl::FontFamilyPicker {
        return None;
    }
    Some(widget.measure(gtk4::Orientation::Horizontal, -1).1)
}

fn assert_gtk_style_widget(
    widget: &gtk4::Widget,
    control: model::StylePillControl,
    snapshot: &ToolbarSnapshot,
) {
    let id = widget.widget_name().to_string();
    match control.role() {
        model::StylePillRole::Swatch => assert_gtk_style_swatch(widget, control, snapshot, &id),
        model::StylePillRole::Slider => assert_gtk_style_slider(widget, control, &id),
        model::StylePillRole::Value => assert_gtk_style_value(widget, control, snapshot, &id),
        model::StylePillRole::Toggle => assert_gtk_style_toggle(widget, control, snapshot, &id),
        model::StylePillRole::Button => assert_gtk_style_button(widget, control, snapshot, &id),
        model::StylePillRole::Stepper => assert_gtk_style_stepper(widget, control, snapshot, &id),
        model::StylePillRole::Segmented => {
            assert_gtk_style_segmented(widget, control, snapshot, &id)
        }
    }
}

fn assert_gtk_style_swatch(
    widget: &gtk4::Widget,
    control: model::StylePillControl,
    snapshot: &ToolbarSnapshot,
    id: &str,
) {
    let button = widget
        .clone()
        .downcast::<gtk4::Button>()
        .unwrap_or_else(|_| panic!("{id} is a swatch button"));
    assert!(button.has_css_class("swatch"), "{id} swatch class");
    assert_eq!(
        button.tooltip_text().as_deref(),
        control.tooltip(snapshot).as_deref(),
        "{id} tooltip"
    );
    assert_accessible_label(widget, &control.label(snapshot), id);
}

fn assert_gtk_style_slider(widget: &gtk4::Widget, control: model::StylePillControl, id: &str) {
    // SliderRow: a box hosting the hand-drawn track DrawingArea.
    let row = widget
        .clone()
        .downcast::<gtk4::Box>()
        .unwrap_or_else(|_| panic!("{id} is a slider row"));
    let track = row.first_child().expect("slider track");
    assert!(track.is::<gtk4::DrawingArea>(), "{id} slider track");
    let value = track.next_sibling().expect("slider value readout");
    let value = value
        .downcast::<gtk4::Label>()
        .unwrap_or_else(|_| panic!("{id} value readout is a label"));
    let carries_readout = control.carries_inline_readout();
    assert_eq!(
        value.property::<bool>("visible"),
        carries_readout,
        "{id} readout visibility"
    );
    let expected_width = if carries_readout {
        STYLE_SLIDER_W + STYLE_PILL_GAP + STYLE_VALUE_W
    } else {
        STYLE_SLIDER_W
    };
    assert_eq!(
        row.width_request(),
        expected_width.round() as i32,
        "{id} keeps the shared track width when its readout is visible"
    );
    if carries_readout {
        assert_eq!(
            value.xalign(),
            0.0,
            "{id} places its readout next to the track"
        );
    }
}

fn assert_gtk_style_value(
    widget: &gtk4::Widget,
    control: model::StylePillControl,
    snapshot: &ToolbarSnapshot,
    id: &str,
) {
    let button = widget
        .clone()
        .downcast::<gtk4::Button>()
        .unwrap_or_else(|_| panic!("{id} is a numeral button"));
    assert_eq!(
        button.label().as_deref(),
        control.value_text(snapshot).as_deref(),
        "{id} live numeral"
    );
    assert_eq!(
        button.tooltip_text().as_deref(),
        control.tooltip(snapshot).as_deref(),
        "{id} tooltip"
    );
}

fn assert_gtk_style_toggle(
    widget: &gtk4::Widget,
    control: model::StylePillControl,
    snapshot: &ToolbarSnapshot,
    id: &str,
) {
    let check = widget
        .clone()
        .downcast::<gtk4::CheckButton>()
        .unwrap_or_else(|_| panic!("{id} is a check button"));
    assert_eq!(check.is_active(), control.active(snapshot), "{id} state");
    assert_eq!(
        check.label().as_deref(),
        Some(control.label(snapshot).as_ref()),
        "{id} label"
    );
    assert_eq!(
        check.tooltip_text().as_deref(),
        control.tooltip(snapshot).as_deref(),
        "{id} tooltip"
    );
}

fn assert_gtk_style_button(
    widget: &gtk4::Widget,
    control: model::StylePillControl,
    snapshot: &ToolbarSnapshot,
    id: &str,
) {
    let button = widget
        .clone()
        .downcast::<gtk4::Button>()
        .unwrap_or_else(|_| panic!("{id} is a button"));
    // Cycle buttons show the live value they step; plain buttons show their
    // label.
    let expected_text = match control {
        model::StylePillControl::SelectionCycle(_) | model::StylePillControl::ArrowStyleCycle => {
            control.value_text(snapshot).expect("cycle value text")
        }
        _ => control.label(snapshot).into_owned(),
    };
    assert_eq!(
        button.label().as_deref(),
        Some(expected_text.as_str()),
        "{id} text"
    );
    assert_eq!(
        button.is_sensitive(),
        control.enabled(snapshot),
        "{id} enabled"
    );
    assert_eq!(
        button.tooltip_text().as_deref(),
        control.tooltip(snapshot).as_deref(),
        "{id} tooltip"
    );
    if control == model::StylePillControl::FontFamilyPicker {
        // The builtin leaves a gap before the family picker so it does not
        // crowd the point-size numeral.
        assert!(
            button.margin_start() > 0,
            "{id} lost the leading gap the builtin gives it"
        );
    }
}

fn assert_gtk_style_stepper(
    widget: &gtk4::Widget,
    control: model::StylePillControl,
    snapshot: &ToolbarSnapshot,
    id: &str,
) {
    let steps = control.steps(snapshot).expect("stepper halves");
    let row = widget
        .clone()
        .downcast::<gtk4::Box>()
        .unwrap_or_else(|_| panic!("{id} is a stepper row"));
    // The builtin lays the three parts out abutting and the width planner
    // budgets step + value + step exactly.
    assert_eq!(row.spacing(), 0, "{id} stepper spacing");
    assert_accessible_label(widget, &control.label(snapshot), id);
    let minus = widget.first_child().expect("stepper minus half");
    let value = minus.next_sibling().expect("stepper value readout");
    let plus = value.next_sibling().expect("stepper plus half");
    assert!(plus.next_sibling().is_none(), "{id} has three children");
    for (half, step) in [(&minus, &steps[0]), (&plus, &steps[1])] {
        let button = half
            .clone()
            .downcast::<gtk4::Button>()
            .unwrap_or_else(|_| panic!("{} is a button", step.id));
        assert_eq!(half.widget_name().as_str(), step.id, "{id} half id");
        assert_eq!(
            button.label().as_deref(),
            Some(step.label),
            "{} label",
            step.id
        );
        assert_eq!(
            button.tooltip_text().as_deref(),
            Some(step.tooltip.as_str()),
            "{} tooltip",
            step.id
        );
        // A tooltip is not an accessible name: a screen reader on these halves
        // would otherwise announce only "−" and "+".
        assert_accessible_label(button.upcast_ref(), &step.tooltip, step.id);
        assert_eq!(
            button.is_sensitive(),
            control.enabled(snapshot),
            "{} enabled",
            step.id
        );
    }
    let value_label = value
        .downcast::<gtk4::Label>()
        .unwrap_or_else(|_| panic!("{id} value readout is a label"));
    assert_eq!(
        value_label.widget_name().as_str(),
        format!("{}.value", control.id()),
        "{id} value id"
    );
    assert_eq!(
        Some(value_label.text().to_string()),
        control.value_text(snapshot),
        "{id} live value"
    );
}

fn assert_gtk_style_segmented(
    widget: &gtk4::Widget,
    control: model::StylePillControl,
    snapshot: &ToolbarSnapshot,
    id: &str,
) {
    let segments = control.segments(snapshot).expect("segment halves");
    let mut buttons = Vec::new();
    let mut child = widget.first_child();
    while let Some(current) = child {
        child = current.next_sibling();
        buttons.push(
            current
                .downcast::<gtk4::Button>()
                .unwrap_or_else(|_| panic!("{id} segment half is a button")),
        );
    }
    assert_eq!(buttons.len(), segments.len(), "{id} segment count");
    for (button, segment) in buttons.iter().zip(&segments) {
        assert_eq!(button.widget_name().as_str(), segment.id, "{id} half id");
        assert!(button.has_css_class("tab"), "{id} tab class");
        assert_eq!(
            button.label().as_deref(),
            Some(segment.label),
            "{} label",
            segment.id
        );
        assert_eq!(
            button.has_css_class("active"),
            segment.active,
            "{} active state",
            segment.id
        );
        assert_eq!(
            button.tooltip_text().as_deref(),
            Some(segment.tooltip.as_str()),
            "{} tooltip",
            segment.id
        );
    }
}

/// True when `widget` carries a capture-phase click gesture, i.e. the controller
/// `install_click_modifier_capture` adds. A popover is its own GTK native, so
/// without one the toolbar window never sees the rebind chord for clicks inside.
fn has_capture_phase_click_gesture(widget: &gtk4::Widget) -> bool {
    let controllers = widget.observe_controllers();
    (0..controllers.n_items()).any(|index| {
        controllers
            .item(index)
            .and_then(|object| object.downcast::<gtk4::GestureClick>().ok())
            .is_some_and(|gesture| gesture.propagation_phase() == gtk4::PropagationPhase::Capture)
    })
}

fn detach_test_popovers(top: &mut TopBar) {
    if let Some(popover) = top.shapes_popover.take() {
        popover.unparent();
    }
    top.shapes_capture_surface = None;
    if let Some(popover) = top.overflow_popover.take() {
        popover.unparent();
    }
    top.overflow_capture_surface = None;
    if let Some(popover) = top.canvas_popover.take() {
        popover.unparent();
    }
    top.canvas_capture_surface = None;
    if let Some(popover) = top.session_popover.take() {
        popover.unparent();
    }
    top.session_capture_surface = None;
    if let Some(popover) = top.settings_popover.take() {
        popover.unparent();
    }
    top.settings_capture_surface = None;
}

fn assert_builtin_node(
    node: &crate::backend::wayland::TopToolbarWidgetKind,
    interaction: Option<&crate::ui::toolbar::ToolbarEvent>,
    tooltip: Option<&str>,
    shortcut_badge: Option<&str>,
    expected: &SemanticAdapterRecord,
) {
    use crate::backend::wayland::TopToolbarWidgetKind as W;

    let SemanticAdapterRecord::Control(expected) = expected else {
        assert!(matches!(node, W::Divider { vertical: true }));
        return;
    };

    assert_eq!(interaction, expected.enabled.then_some(&expected.event));
    assert_eq!(
        tooltip,
        expected.enabled.then_some(expected.tooltip.as_str())
    );
    assert_eq!(shortcut_badge, expected.shortcut_badge.as_deref());

    assert_builtin_node_kind(node, expected);
}

fn assert_builtin_node_kind(
    node: &crate::backend::wayland::TopToolbarWidgetKind,
    expected: &SemanticControlRecord,
) {
    use crate::backend::wayland::TopToolbarWidgetKind as W;

    match node {
        W::IconButton {
            glyph,
            icon_size: _,
            style,
        } => {
            let icon = expected.icon.expect("semantic icon for icon button");
            assert!(std::ptr::fn_addr_eq(
                glyph.0,
                crate::toolbar_icons::top_toolbar_icon_painter(icon)
            ));
            assert_builtin_button_style(style.active, style.disabled, style.destructive, expected);
        }
        W::TextButton { label, style } => {
            assert_eq!(label.text, expected.label);
            assert_builtin_button_style(style.active, style.disabled, style.destructive, expected);
        }
        W::Swatch { selected, .. } => assert_eq!(*selected, expected.active),
        W::PresetSlot {
            glyph,
            label,
            active,
            ..
        } => {
            assert_eq!(*active, expected.active);
            match expected.icon {
                // A filled slot carries the saved tool glyph; an empty slot
                // has no glyph and shows its 1-based number label.
                Some(icon) => {
                    let glyph = glyph.as_ref().expect("filled preset slot has a glyph");
                    assert!(std::ptr::fn_addr_eq(
                        glyph.0,
                        crate::toolbar_icons::top_toolbar_icon_painter(icon)
                    ));
                }
                None => {
                    assert!(glyph.is_none(), "empty preset slot has no glyph");
                    assert_eq!(*label, expected.label);
                }
            }
        }
        W::MicroChip { glyph, .. } => {
            let icon = expected.icon.expect("semantic icon for micro chip");
            assert!(std::ptr::fn_addr_eq(
                glyph.0,
                crate::toolbar_icons::top_toolbar_icon_painter(icon)
            ));
            assert_eq!(expected.role, model::TopToolbarControlRole::Restore);
        }
        W::PinButton { pinned } => assert_eq!(*pinned, expected.active),
        W::MiniCheckbox { checked, label } => {
            assert_eq!(*checked, expected.active);
            assert_eq!(label.text, expected.label);
        }
        W::DragHandle | W::MinimizeButton => {}
        other => panic!("unexpected semantic control kind: {other:?}"),
    }
}

fn assert_builtin_button_style(
    active: bool,
    disabled: bool,
    destructive: bool,
    expected: &SemanticControlRecord,
) {
    assert_eq!(active, expected.active);
    assert_eq!(disabled, !expected.enabled);
    assert_eq!(
        destructive,
        expected.role == model::TopToolbarControlRole::Destructive
    );
}

fn builtin_semantic_records(
    snapshot: &ToolbarSnapshot,
    expected: &[SemanticAdapterRecord],
) -> Vec<SemanticAdapterRecord> {
    let (width, height) = top_toolbar_size(snapshot);
    let tree =
        crate::backend::wayland::build_top_toolbar_view(snapshot, width as f64, height as f64);
    let mut records = Vec::new();
    for node in tree.nodes() {
        let raw_id = node.id.as_str();
        let (lane, id) = if let Some(id) = raw_id.strip_prefix("top.overflow.") {
            (SemanticLane::Overflow, id)
        } else if raw_id == "top.utility.highlight-ring" {
            (SemanticLane::Contextual, raw_id)
        } else {
            let lane = expected
                .iter()
                .find(|record| record_id(record) == raw_id)
                .map(record_lane)
                .unwrap_or(SemanticLane::Strip);
            (lane, raw_id)
        };
        let Some(record) = expected
            .iter()
            .find(|record| record_id(record) == id && record_lane(record) == lane)
        else {
            continue;
        };
        assert_builtin_node(
            &node.kind,
            node.interact.as_ref().map(|interaction| &interaction.event),
            node.interact
                .as_ref()
                .and_then(|interaction| interaction.tooltip.as_deref()),
            node.shortcut_badge
                .as_ref()
                .map(|badge| badge.label.as_str()),
            record,
        );
        records.push(record.clone());
    }
    records
}

#[test]
fn shared_spec_matches_builtin_order_and_full_semantics_without_starting_a_gui() {
    let state = make_test_input_state();
    let regular = ToolbarSnapshot::from_input_with_bindings(
        &state,
        ToolbarBindingHints::from_input_state(&state),
    );
    let mut simple = regular.clone();
    simple.layout_mode = ToolbarLayoutMode::Simple;
    let mut minimized = regular.clone();
    minimized.top_minimized = true;
    let mut micro = regular.clone();
    micro.top_display_mode = crate::config::TopDisplayMode::Micro;
    let mut narrow = regular.clone();
    narrow.top_viewport_max = Some(520.0);
    narrow.top_overflow_open = true;
    let mut text = regular.clone();
    text.use_icons = false;
    let mut shapes = regular.clone();
    shapes.shape_picker_open = true;
    shapes.active_tool = Tool::RegularPolygon;
    let mut highlighted = regular.clone();
    highlighted.highlight_tool_active = true;

    for (name, snapshot) in [
        ("regular", regular),
        ("simple", simple),
        ("minimized", minimized),
        ("micro", micro),
        ("narrow", narrow),
        ("text", text),
        ("shapes", shapes),
        ("highlighted", highlighted),
    ] {
        let plan = plan_top_strip(&snapshot);
        let spec = super::strip::top_toolbar_spec(&snapshot, &plan);
        let expected = expected_semantic_records(&snapshot, &spec, &plan);
        for record in &expected {
            if let SemanticAdapterRecord::Control(control) = record {
                assert!(!control.accessible_label.is_empty(), "{name}: {control:?}");
            }
        }
        assert_eq!(
            expected,
            builtin_semantic_records(&snapshot, &expected),
            "{name} adapter semantics"
        );
    }
}

/// Expected builtin node kinds per style-pill control, in tree order.
enum StylePillNodeExpectation {
    Control(model::StylePillControl),
    /// Inline readout decoration (the opacity slider's percent label).
    Readout(model::StylePillControl),
    /// Interactive half of a segmented control.
    SegmentHalf(model::StylePillControl, usize),
    /// Interactive −/+ half of a selection stepper.
    StepHalf(model::StylePillControl, usize),
    /// The value readout between the stepper halves (decor).
    StepValue(model::StylePillControl),
}

fn expected_style_pill_nodes(
    snapshot: &ToolbarSnapshot,
    plan: &TopStripPlan,
) -> Vec<(String, StylePillNodeExpectation)> {
    let mut nodes = Vec::new();
    for (id, control) in style_pill_controls(snapshot, plan) {
        // Steppers render as three nodes (−, readout, +) without a node
        // carrying the control id itself.
        if let Some(steps) = control.steps(snapshot) {
            nodes.push((
                steps[0].id.to_string(),
                StylePillNodeExpectation::StepHalf(control, 0),
            ));
            nodes.push((
                format!("{id}.value"),
                StylePillNodeExpectation::StepValue(control),
            ));
            nodes.push((
                steps[1].id.to_string(),
                StylePillNodeExpectation::StepHalf(control, 1),
            ));
            continue;
        }
        nodes.push((id.clone(), StylePillNodeExpectation::Control(control)));
        if control.carries_inline_readout() {
            nodes.push((
                format!("{id}.readout"),
                StylePillNodeExpectation::Readout(control),
            ));
        }
        if let Some(segments) = control.segments(snapshot) {
            for (index, segment) in segments.iter().enumerate() {
                nodes.push((
                    segment.id.to_string(),
                    StylePillNodeExpectation::SegmentHalf(control, index),
                ));
            }
        }
    }
    nodes
}

#[test]
fn style_pill_spec_matches_builtin_tree_across_morph_states() {
    let state = make_test_input_state();
    let regular = ToolbarSnapshot::from_input_with_bindings(
        &state,
        ToolbarBindingHints::from_input_state(&state),
    );
    let mut arrow = style_pill_tool_snapshot(&regular, Tool::Arrow);
    arrow.arrow_label_enabled = true;
    arrow.arrow_label_next = 7;
    let mut text_mode = style_pill_tool_snapshot(&regular, Tool::Pen);
    text_mode.text_active = true;
    let mut minimized = regular.clone();
    minimized.top_minimized = true;
    let mut micro = regular.clone();
    micro.top_display_mode = crate::config::TopDisplayMode::Micro;

    for (name, snapshot) in [
        ("regular", regular.clone()),
        ("pen", style_pill_tool_snapshot(&regular, Tool::Pen)),
        ("marker", style_pill_tool_snapshot(&regular, Tool::Marker)),
        ("eraser", style_pill_tool_snapshot(&regular, Tool::Eraser)),
        ("shape", style_pill_tool_snapshot(&regular, Tool::Rect)),
        ("arrow", arrow),
        (
            "step-marker",
            style_pill_tool_snapshot(&regular, Tool::StepMarker),
        ),
        ("text-mode", text_mode),
        ("select", style_pill_tool_snapshot(&regular, Tool::Select)),
        ("selection", style_pill_selection_snapshot(&regular)),
        ("minimized", minimized),
        ("micro", micro),
    ] {
        assert_builtin_style_pill_scenario(name, &snapshot);
    }
}

fn assert_builtin_style_pill_scenario(name: &str, snapshot: &ToolbarSnapshot) {
    let plan = plan_top_strip(snapshot);
    let expected = expected_style_pill_nodes(snapshot, &plan);
    let (width, height) = top_toolbar_size(snapshot);
    let tree =
        crate::backend::wayland::build_top_toolbar_view(snapshot, width as f64, height as f64);

    assert_eq!(
        tree.node_by_id(&"top.island.style".into()).is_some(),
        !expected.is_empty(),
        "{name}: the pill card exists exactly when the spec has controls"
    );

    let actual: Vec<_> = tree
        .nodes()
        .iter()
        .filter(|node| node.id.as_str().starts_with("top.style."))
        .collect();
    assert_eq!(
        actual
            .iter()
            .map(|node| node.id.as_str().to_string())
            .collect::<Vec<_>>(),
        expected
            .iter()
            .map(|(id, _)| id.clone())
            .collect::<Vec<_>>(),
        "{name}: builtin pill node order"
    );

    for (node, (id, expectation)) in actual.iter().zip(&expected) {
        assert_builtin_style_pill_node(
            name,
            snapshot,
            &node.kind,
            node.interact.as_ref().map(|interaction| &interaction.event),
            node.interact
                .as_ref()
                .and_then(|interaction| interaction.tooltip.as_deref()),
            node.interact.is_some(),
            id,
            expectation,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn assert_builtin_style_pill_node(
    name: &str,
    snapshot: &ToolbarSnapshot,
    kind: &crate::backend::wayland::TopToolbarWidgetKind,
    interaction_event: Option<&ToolbarEvent>,
    interaction_tooltip: Option<&str>,
    has_interaction: bool,
    id: &str,
    expectation: &StylePillNodeExpectation,
) {
    match expectation {
        StylePillNodeExpectation::Control(control) => assert_builtin_style_pill_control(
            name,
            snapshot,
            kind,
            interaction_event,
            interaction_tooltip,
            has_interaction,
            id,
            *control,
        ),
        StylePillNodeExpectation::Readout(control) => {
            assert_builtin_style_pill_readout(name, snapshot, kind, has_interaction, id, *control)
        }
        StylePillNodeExpectation::StepHalf(control, index) => assert_builtin_style_pill_step_half(
            name,
            snapshot,
            kind,
            interaction_event,
            interaction_tooltip,
            id,
            *control,
            *index,
        ),
        StylePillNodeExpectation::StepValue(control) => assert_builtin_style_pill_step_value(
            name,
            snapshot,
            kind,
            has_interaction,
            id,
            *control,
        ),
        StylePillNodeExpectation::SegmentHalf(control, index) => {
            assert_builtin_style_pill_segment_half(
                name,
                snapshot,
                kind,
                interaction_event,
                interaction_tooltip,
                id,
                *control,
                *index,
            )
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn assert_builtin_style_pill_control(
    name: &str,
    snapshot: &ToolbarSnapshot,
    kind: &crate::backend::wayland::TopToolbarWidgetKind,
    interaction_event: Option<&ToolbarEvent>,
    interaction_tooltip: Option<&str>,
    has_interaction: bool,
    id: &str,
    control: model::StylePillControl,
) {
    let expected_event = control
        .enabled(snapshot)
        .then(|| control.event(snapshot))
        .flatten();
    assert_eq!(
        interaction_event,
        expected_event.as_ref(),
        "{name}: {id} event"
    );
    if has_interaction {
        assert_eq!(
            interaction_tooltip,
            control.tooltip(snapshot).as_deref(),
            "{name}: {id} tooltip"
        );
    }
    assert_builtin_style_pill_control_kind(
        name,
        snapshot,
        kind,
        interaction_event,
        has_interaction,
        id,
        control,
    );
}

#[allow(clippy::too_many_arguments)]
fn assert_builtin_style_pill_control_kind(
    name: &str,
    snapshot: &ToolbarSnapshot,
    kind: &crate::backend::wayland::TopToolbarWidgetKind,
    interaction_event: Option<&ToolbarEvent>,
    has_interaction: bool,
    id: &str,
    control: model::StylePillControl,
) {
    use crate::backend::wayland::TopToolbarWidgetKind as W;

    match (control.role(), kind) {
        (model::StylePillRole::Swatch, W::Swatch { color, selected }) => {
            let expected_color = match control {
                model::StylePillControl::QuickSwatch(index) => {
                    snapshot.quick_colors.rendered_entries()[index].color
                }
                _ => snapshot.color,
            };
            assert_eq!(
                *color,
                (
                    expected_color.r,
                    expected_color.g,
                    expected_color.b,
                    expected_color.a
                ),
                "{name}: {id} color"
            );
            assert_eq!(*selected, control.active(snapshot), "{name}: {id}");
        }
        (model::StylePillRole::Slider, W::Slider { t }) => {
            let (spec, value) = control.slider(snapshot).expect("slider spec");
            assert!(
                (*t - spec.t_from_value(value)).abs() < 1e-9,
                "{name}: {id} slider position"
            );
        }
        (model::StylePillRole::Value, W::TextButton { label, .. }) => {
            assert_eq!(
                Some(label.text.clone()),
                control.value_text(snapshot),
                "{name}: {id} live numeral"
            );
            assert!(
                matches!(interaction_event, Some(ToolbarEvent::OpenPrecisionEntry(_))),
                "{name}: {id} opens the precise entry"
            );
        }
        (model::StylePillRole::Toggle, W::MiniCheckbox { checked, label }) => {
            assert_eq!(*checked, control.active(snapshot), "{name}: {id}");
            assert_eq!(
                label.text,
                control.label(snapshot).as_ref(),
                "{name}: {id} label"
            );
        }
        (model::StylePillRole::Button, W::TextButton { label, style }) => {
            let expected_text = match control {
                model::StylePillControl::SelectionCycle(_)
                | model::StylePillControl::ArrowStyleCycle => {
                    control.value_text(snapshot).expect("cycle value text")
                }
                _ => control.label(snapshot).into_owned(),
            };
            assert_eq!(label.text, expected_text, "{name}: {id} text");
            assert_eq!(
                style.disabled,
                !control.enabled(snapshot),
                "{name}: {id} disabled style"
            );
        }
        (
            model::StylePillRole::Segmented,
            W::SegmentedControl {
                left,
                right,
                active_right,
            },
        ) => {
            let segments = control.segments(snapshot).expect("segments");
            assert_eq!(left.text, segments[0].label, "{name}: {id}");
            assert_eq!(right.text, segments[1].label, "{name}: {id}");
            assert_eq!(*active_right, segments[1].active, "{name}: {id}");
            assert!(!has_interaction, "halves carry the interactions");
        }
        (role, kind) => panic!("{name}: {id} role {role:?} painted as {kind:?}"),
    }
}

fn assert_builtin_style_pill_readout(
    name: &str,
    snapshot: &ToolbarSnapshot,
    kind: &crate::backend::wayland::TopToolbarWidgetKind,
    has_interaction: bool,
    id: &str,
    control: model::StylePillControl,
) {
    use crate::backend::wayland::TopToolbarWidgetKind as W;

    assert!(!has_interaction, "{name}: {id} readout is decor");
    match kind {
        W::Label(label) => assert_eq!(
            Some(label.text.clone()),
            control.value_text(snapshot),
            "{name}: {id} readout"
        ),
        other => panic!("{name}: {id} readout kind {other:?}"),
    }
}

#[allow(clippy::too_many_arguments)]
fn assert_builtin_style_pill_step_half(
    name: &str,
    snapshot: &ToolbarSnapshot,
    kind: &crate::backend::wayland::TopToolbarWidgetKind,
    interaction_event: Option<&ToolbarEvent>,
    interaction_tooltip: Option<&str>,
    id: &str,
    control: model::StylePillControl,
    index: usize,
) {
    use crate::backend::wayland::TopToolbarWidgetKind as W;

    let steps = control.steps(snapshot).expect("stepper halves");
    let step = &steps[index];
    let enabled = control.enabled(snapshot);
    match kind {
        W::TextButton { label, style } => {
            assert_eq!(label.text, step.label, "{name}: {id} step label");
            assert_eq!(style.disabled, !enabled, "{name}: {id} step style");
        }
        other => panic!("{name}: {id} step kind {other:?}"),
    }
    assert_eq!(
        interaction_event,
        enabled.then_some(&step.event),
        "{name}: {id} step event"
    );
    assert_eq!(
        interaction_tooltip,
        enabled.then_some(step.tooltip.as_str()),
        "{name}: {id} step tooltip"
    );
}

fn assert_builtin_style_pill_step_value(
    name: &str,
    snapshot: &ToolbarSnapshot,
    kind: &crate::backend::wayland::TopToolbarWidgetKind,
    has_interaction: bool,
    id: &str,
    control: model::StylePillControl,
) {
    use crate::backend::wayland::TopToolbarWidgetKind as W;

    assert!(!has_interaction, "{name}: {id} readout is decor");
    match kind {
        W::Label(label) => assert_eq!(
            Some(label.text.clone()),
            control.value_text(snapshot),
            "{name}: {id} stepper readout"
        ),
        other => panic!("{name}: {id} stepper readout kind {other:?}"),
    }
}

#[allow(clippy::too_many_arguments)]
fn assert_builtin_style_pill_segment_half(
    name: &str,
    snapshot: &ToolbarSnapshot,
    kind: &crate::backend::wayland::TopToolbarWidgetKind,
    interaction_event: Option<&ToolbarEvent>,
    interaction_tooltip: Option<&str>,
    id: &str,
    control: model::StylePillControl,
    index: usize,
) {
    use crate::backend::wayland::TopToolbarWidgetKind as W;

    let segments = control.segments(snapshot).expect("segments");
    let segment = &segments[index];
    assert!(matches!(kind, W::HitArea), "{name}: {id}");
    assert_eq!(
        interaction_event,
        Some(&segment.event),
        "{name}: {id} segment event"
    );
    assert_eq!(
        interaction_tooltip,
        Some(segment.tooltip.as_str()),
        "{name}: {id} segment tooltip"
    );
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

    let pen_key = StructureKey::of(&pen, &plan_top_strip(&pen));
    let eraser_key = StructureKey::of(&eraser, &plan_top_strip(&eraser));
    assert!(
        pen_key != eraser_key,
        "a pill morph change must rebuild the GTK bar structure"
    );

    // Pure value churn (thickness) keeps the structure stable: live values
    // run through updaters, not rebuilds.
    let mut thicker = pen.clone();
    thicker.thickness += 3.0;
    let thicker_key = StructureKey::of(&thicker, &plan_top_strip(&thicker));
    assert!(pen_key == thicker_key, "value churn must not rebuild");
}

fn find_widget_named(root: &gtk4::Widget, name: &str) -> Option<gtk4::Widget> {
    if root.widget_name() == name {
        return Some(root.clone());
    }
    let mut child = root.first_child();
    while let Some(current) = child {
        child = current.next_sibling();
        if let Some(found) = find_widget_named(&current, name) {
            return Some(found);
        }
    }
    None
}

fn collect_descendants<W: IsA<gtk4::Widget>>(root: &gtk4::Widget, out: &mut Vec<W>) {
    if let Ok(widget) = root.clone().downcast::<W>() {
        out.push(widget);
    }
    let mut child = root.first_child();
    while let Some(current) = child {
        child = current.next_sibling();
        collect_descendants(&current, out);
    }
}

fn install_gtk_contract_metrics() {
    let css_provider = gtk4::CssProvider::new();
    css_provider.load_from_string(&crate::toolbar_gtk::css::stylesheet(1.0));
    if let Some(display) = gtk4::gdk::Display::default() {
        gtk4::style_context_add_provider_for_display(
            &display,
            &css_provider,
            gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
    if let Some(settings) = gtk4::Settings::default() {
        settings.set_gtk_font_name(Some("Sans 11"));
        settings.set_gtk_xft_dpi(96 * 1024);
    }
}

fn gtk_widget_contract_scenarios() -> (
    ToolbarSnapshot,
    ToolbarSnapshot,
    Vec<(&'static str, ToolbarSnapshot)>,
) {
    let state = make_test_input_state();
    let regular = ToolbarSnapshot::from_input_with_bindings(
        &state,
        ToolbarBindingHints::from_input_state(&state),
    );
    let mut simple = regular.clone();
    simple.layout_mode = ToolbarLayoutMode::Simple;
    let mut minimized = regular.clone();
    minimized.top_minimized = true;
    let mut micro = regular.clone();
    micro.top_display_mode = crate::config::TopDisplayMode::Micro;
    let mut text = regular.clone();
    text.use_icons = false;
    let mut highlighted = regular.clone();
    highlighted.highlight_tool_active = true;
    let mut narrow = regular.clone();
    narrow.top_viewport_max = Some(520.0);
    let mut scenarios = vec![
        ("regular", regular.clone()),
        ("simple", simple),
        ("minimized", minimized),
        ("micro", micro),
        ("text", text),
        ("highlighted", highlighted.clone()),
        ("narrow", narrow),
    ];
    scenarios.extend(
        [
            ("marker-tool", Tool::Marker),
            ("eraser-tool", Tool::Eraser),
            ("shape-tool", Tool::Rect),
            ("arrow-tool", Tool::Arrow),
            ("step-marker-tool", Tool::StepMarker),
            ("select-tool", Tool::Select),
        ]
        .map(|(name, tool)| (name, style_pill_tool_snapshot(&regular, tool))),
    );
    let mut text_mode = style_pill_tool_snapshot(&regular, Tool::Pen);
    text_mode.text_active = true;
    scenarios.push(("text-mode", text_mode.clone()));
    let mut long_font = text_mode;
    long_font.font = crate::draw::FontDescriptor::new(
        "Noto Sans Mono CJK JP ExtraCondensed Black".to_string(),
        "normal".to_string(),
        "normal".to_string(),
    );
    scenarios.push(("long-font-name", long_font));
    scenarios.push(("selection", style_pill_selection_snapshot(&regular)));
    (regular, highlighted, scenarios)
}

fn assert_gtk_widget_scenarios(
    scenarios: Vec<(&'static str, ToolbarSnapshot)>,
) -> std::collections::BTreeMap<&'static str, i32> {
    let mut widths = std::collections::BTreeMap::new();
    for (name, snapshot) in scenarios {
        assert_gtk_widget_scenario(name, &snapshot, &mut widths);
    }
    widths
}

fn build_contract_top(snapshot: &ToolbarSnapshot, plan: &TopStripPlan) -> TopBar {
    let (tx, _rx) = std::sync::mpsc::channel();
    let mut top = TopBar::new_for_test(FeedbackSender::new(tx));
    if snapshot.top_minimized {
        top.build_minimized(snapshot, plan);
    } else if snapshot.top_micro_active() {
        top.build_micro(snapshot, plan);
    } else {
        top.build_strip(snapshot, plan);
    }
    for updater in top.updaters.borrow().iter() {
        updater(snapshot);
    }
    top
}

fn assert_gtk_widget_scenario(
    name: &'static str,
    snapshot: &ToolbarSnapshot,
    widths: &mut std::collections::BTreeMap<&'static str, i32>,
) {
    let plan = plan_top_strip(snapshot);
    let spec = super::strip::top_toolbar_spec(snapshot, &plan);
    let expected = expected_semantic_records(snapshot, &spec, &plan);
    let style_controls = style_pill_controls(snapshot, &plan);
    let mut top = build_contract_top(snapshot, &plan);
    let widgets = collect_semantic_widgets(top.root.upcast_ref());
    assert_eq!(
        widgets
            .iter()
            .map(|widget| widget.widget_name().to_string())
            .collect::<Vec<_>>(),
        expected_main_widget_ids(&spec, snapshot, &plan),
        "{name} GTK widget order"
    );
    assert_scenario_widget_islands(name, snapshot, &spec, &plan, &widgets);
    assert_scenario_widget_semantics(name, snapshot, &expected, &style_controls, widgets, widths);
    detach_test_popovers(&mut top);
}

fn assert_scenario_widget_islands(
    name: &str,
    snapshot: &ToolbarSnapshot,
    spec: &model::TopToolbarSpec,
    plan: &TopStripPlan,
    widgets: &[gtk4::Widget],
) {
    let expected_islands = expected_island_keys(snapshot, spec, plan);
    for widget in widgets {
        let id = widget.widget_name().to_string();
        let expected_island = expected_islands
            .get(&id)
            .unwrap_or_else(|| panic!("{name}: no island expectation for {id}"));
        assert_eq!(
            nearest_island_key(widget).as_deref(),
            *expected_island,
            "{name}: {id} island membership"
        );
    }
}

fn assert_scenario_widget_semantics(
    name: &'static str,
    snapshot: &ToolbarSnapshot,
    expected: &[SemanticAdapterRecord],
    style_controls: &[(String, model::StylePillControl)],
    widgets: Vec<gtk4::Widget>,
    widths: &mut std::collections::BTreeMap<&'static str, i32>,
) {
    for widget in widgets {
        let id = widget.widget_name();
        if let Some((_, control)) = style_controls
            .iter()
            .find(|(control_id, _)| *control_id == id)
        {
            assert_gtk_style_widget(&widget, *control, snapshot);
            if let Some(width) = font_button_natural_width(&widget, *control) {
                widths.insert(name, width);
            }
            continue;
        }
        let Some(control) = expected.iter().find_map(|record| match record {
            SemanticAdapterRecord::Control(control) if control.id == id => Some(control),
            _ => None,
        }) else {
            assert!(
                expected.iter().any(
                    |record| matches!(record, SemanticAdapterRecord::Divider(divider) if *divider == id)
                ),
                "{name}: unexpected GTK widget {id}"
            );
            continue;
        };
        assert_gtk_control_widget(&widget, control);
    }
}

fn assert_font_button_width_stable(widths: &std::collections::BTreeMap<&str, i32>) {
    let short = widths
        .get("text-mode")
        .copied()
        .expect("the text-mode pill has a font button");
    let long = widths
        .get("long-font-name")
        .copied()
        .expect("the long-font-name pill has a font button");
    assert_eq!(
        long, short,
        "the font button grew from {short}px to {long}px for a longer family name"
    );
}

fn assert_compact_gtk_widget_contract(regular: &ToolbarSnapshot) {
    let mut compact_plan = TopStripPlan::unconstrained();
    compact_plan.compact = true;
    let (tx, _rx) = std::sync::mpsc::channel();
    let mut compact_top = TopBar::new_for_test(FeedbackSender::new(tx));
    compact_top.build_strip(regular, &compact_plan);
    let compact_ids = collect_semantic_widgets(compact_top.root.upcast_ref())
        .iter()
        .map(|widget| widget.widget_name().to_string())
        .collect::<Vec<_>>();
    let compact_spec = super::strip::top_toolbar_spec(regular, &compact_plan);
    let expected_compact_ids = expected_main_widget_ids(&compact_spec, regular, &compact_plan);
    assert!(
        !expected_compact_ids.is_empty(),
        "the compact strip still builds its protected core"
    );
    assert_eq!(
        compact_ids, expected_compact_ids,
        "the compact strip builds exactly the shared spec's widget set"
    );
    assert!(
        compact_ids.iter().all(|name| {
            !name.starts_with("top.quick-color.")
                && name.as_str() != "top.group.quick-colors"
                && !name.starts_with("top.preset.")
        }),
        "the compact strip carries no colors or presets: {compact_ids:?}"
    );
    detach_test_popovers(&mut compact_top);
}

fn assert_shapes_and_overflow_contract(regular: &ToolbarSnapshot) {
    assert_shapes_popover_contract(regular);
    assert_overflow_popover_contract(regular);
}

fn assert_shapes_popover_contract(regular: &ToolbarSnapshot) {
    let mut shapes = regular.clone();
    shapes.shape_picker_open = true;
    shapes.active_tool = Tool::RegularPolygon;
    let (tx, rx) = std::sync::mpsc::channel();
    let top = TopBar::new_for_test(FeedbackSender::new(tx));
    let content =
        top.build_shapes_popover_content(&shapes, (ICON_BUTTON, ICON_BUTTON), ICON_SIZE, true, 1.0);
    assert!(
        has_capture_phase_click_gesture(content.upcast_ref()),
        "the shapes popover must capture click modifiers"
    );
    let tools = model::visible_shape_picker_rows(&shapes, false)
        .into_iter()
        .flatten()
        .filter(|tool| model::tool_visible(&shapes, *tool))
        .collect::<Vec<_>>();
    let mut expected_ids = tools
        .iter()
        .map(|tool| {
            format!(
                "top.picker.{}",
                model::toolbar_item_id_for_tool(*tool).as_str()
            )
        })
        .collect::<Vec<_>>();
    if model::top_fill_visible(&shapes) {
        expected_ids.push(ids::TOP_UTILITY_FILL.as_str().to_string());
    }
    expected_ids.extend([
        "top.options.sides-minus".to_string(),
        "top.options.sides-plus".to_string(),
    ]);
    let widgets = collect_semantic_widgets(content.upcast_ref());
    assert_eq!(
        widgets
            .iter()
            .map(|widget| widget.widget_name().to_string())
            .collect::<Vec<_>>(),
        expected_ids,
        "GTK shapes-popover order"
    );
    for (widget, tool) in widgets.iter().zip(&tools) {
        let expected = control_record(
            &shapes,
            SemanticLane::Strip,
            model::TopToolbarControl::Tool(*tool),
            true,
        );
        assert_gtk_control_widget(widget, &expected);
    }
    first_control_surface(&widgets[0])
        .downcast::<gtk4::Button>()
        .expect("shape-picker tool button")
        .emit_clicked();
    assert_eq!(
        rx.recv_timeout(Duration::from_secs(1))
            .expect("GTK shape event"),
        GtkToolbarFeedback::Event {
            event: ToolbarEvent::SelectTool(tools[0]),
            rebind_requested: false,
        }
    );

    let mut line_shapes = shapes;
    line_shapes.active_tool = Tool::Line;
    line_shapes.tool_override = None;
    let line_content = top.build_shapes_popover_content(
        &line_shapes,
        (ICON_BUTTON, ICON_BUTTON),
        ICON_SIZE,
        true,
        1.0,
    );
    assert!(
        collect_semantic_widgets(line_content.upcast_ref())
            .iter()
            .any(|widget| widget.widget_name() == ids::TOP_UTILITY_FILL.as_str()),
        "GTK Shapes must expose Fill before a fill-capable shape is selected"
    );
}

fn assert_overflow_popover_contract(regular: &ToolbarSnapshot) {
    let (tx, rx) = std::sync::mpsc::channel();
    let top = TopBar::new_for_test(FeedbackSender::new(tx));
    let mut plan = TopStripPlan::unconstrained();
    plan.dropped_tools = vec![Tool::Line, Tool::Arrow];
    plan.dropped_utilities = vec![
        model::TopUtilityButton::Screenshot,
        model::TopUtilityButton::Highlight,
    ];
    let spec = super::strip::top_toolbar_spec(regular, &plan);
    let content = top.build_overflow_popover_content(
        regular,
        &spec,
        (ICON_BUTTON, ICON_BUTTON),
        ICON_SIZE,
        true,
        1.0,
    );
    let widgets = collect_semantic_widgets(content.upcast_ref());
    assert_eq!(
        widgets
            .iter()
            .map(|widget| widget.widget_name().to_string())
            .collect::<Vec<_>>(),
        spec.overflow()
            .iter()
            .map(|control| format!("top.overflow.{}", control.id().render_id()))
            .collect::<Vec<_>>(),
        "GTK overflow order"
    );
    for (widget, control) in widgets.iter().zip(spec.overflow()) {
        let expected = control_record(regular, SemanticLane::Overflow, *control, true);
        assert_gtk_control_widget(widget, &expected);
    }
    first_control_surface(&widgets[0])
        .downcast::<gtk4::Button>()
        .expect("overflow tool button")
        .emit_clicked();
    assert_eq!(
        rx.recv_timeout(Duration::from_secs(1))
            .expect("GTK overflow event"),
        GtkToolbarFeedback::Event {
            event: spec.overflow()[0].event(regular),
            rebind_requested: false,
        }
    );
}

fn assert_gtk_toggle_events(regular: &ToolbarSnapshot, highlighted: &ToolbarSnapshot) {
    let (tx, rx) = std::sync::mpsc::channel();
    let mut top = TopBar::new_for_test(FeedbackSender::new(tx));
    let shape = top.shapes_picker_button(
        regular,
        model::TopToolbarControl::ShapePicker,
        (ICON_BUTTON, ICON_BUTTON),
        ICON_SIZE,
        true,
    );
    let highlight = top.action_button(
        regular,
        model::TopToolbarControl::Utility(model::TopToolbarUtility::Highlight),
        (ICON_BUTTON, ICON_BUTTON),
        ICON_SIZE,
        true,
        true,
    );
    let pin = top.pin_button(regular, model::TopToolbarControl::Pin, PIN_BUTTON_SIZE);
    let overflow = top.overflow_button(
        regular,
        model::TopToolbarControl::Overflow,
        (ICON_BUTTON, ICON_BUTTON),
        ICON_SIZE,
    );
    assert_test_popover_capture_surfaces(&top);
    assert_gtk_button_events(
        &rx,
        [
            (
                &shape,
                ToolbarEvent::ToggleShapePicker(!regular.shape_picker_open),
            ),
            (
                &highlight,
                ToolbarEvent::ToggleAllHighlight(!regular.any_highlight_active),
            ),
            (&pin, ToolbarEvent::PinTopToolbar(!regular.top_pinned)),
            (
                &overflow,
                ToolbarEvent::ToggleTopOverflow(!regular.top_overflow_open),
            ),
        ],
    );

    let mut active = regular.clone();
    active.shape_picker_open = true;
    active.any_highlight_active = true;
    active.top_pinned = true;
    active.top_overflow_open = true;
    top.shapes_expected_open.set(true);
    top.overflow_expected_open.set(true);
    for updater in top.updaters.borrow().iter() {
        updater(&active);
    }
    assert_gtk_button_events(
        &rx,
        [
            (&shape, ToolbarEvent::ToggleShapePicker(false)),
            (&highlight, ToolbarEvent::ToggleAllHighlight(false)),
            (&pin, ToolbarEvent::PinTopToolbar(false)),
            (&overflow, ToolbarEvent::ToggleTopOverflow(false)),
        ],
    );
    top.shapes_expected_open.set(false);
    top.overflow_expected_open.set(false);
    detach_test_popovers(&mut top);
    assert_highlight_ring_event(&mut top, highlighted, &rx);
}

fn assert_test_popover_capture_surfaces(top: &TopBar) {
    for (popover, capture_surface) in [
        (
            top.shapes_popover.as_ref().unwrap(),
            top.shapes_capture_surface.as_ref().unwrap(),
        ),
        (
            top.overflow_popover.as_ref().unwrap(),
            top.overflow_capture_surface.as_ref().unwrap(),
        ),
    ] {
        let content = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
        capture_surface.set_content(&content);
        super::popovers::set_popover_capture_transparent(popover, capture_surface, true, false);
        assert!(popover.has_css_class(crate::toolbar_gtk::css::CAPTURE_TRANSPARENT_CLASS));
        assert!(!popover.can_target());
        assert_eq!(capture_surface.content_opacity(), Some(0.0));
        assert!(capture_surface.proof_visible());
        super::popovers::set_popover_capture_transparent(popover, capture_surface, false, true);
        assert!(!popover.has_css_class(crate::toolbar_gtk::css::CAPTURE_TRANSPARENT_CLASS));
        assert!(popover.can_target());
        assert_eq!(capture_surface.content_opacity(), Some(1.0));
        assert!(!capture_surface.proof_visible());
    }
}

fn assert_gtk_button_events(
    rx: &std::sync::mpsc::Receiver<GtkToolbarFeedback>,
    cases: [(&gtk4::Button, ToolbarEvent); 4],
) {
    for (button, event) in cases {
        button.emit_clicked();
        assert_eq!(
            rx.recv_timeout(Duration::from_secs(1)).expect("GTK event"),
            GtkToolbarFeedback::Event {
                event,
                rebind_requested: false,
            }
        );
    }
}

fn assert_highlight_ring_event(
    top: &mut TopBar,
    highlighted: &ToolbarSnapshot,
    rx: &std::sync::mpsc::Receiver<GtkToolbarFeedback>,
) {
    top.build_strip(highlighted, &plan_top_strip(highlighted));
    let ring = collect_semantic_widgets(top.root.upcast_ref())
        .into_iter()
        .find(|widget| widget.widget_name() == ids::TOP_UTILITY_HIGHLIGHT_RING.as_str())
        .expect("GTK highlight-ring widget")
        .downcast::<gtk4::CheckButton>()
        .expect("highlight ring check button");
    ring.set_active(!highlighted.highlight_tool_ring_enabled);
    assert_eq!(
        rx.recv_timeout(Duration::from_secs(1))
            .expect("GTK ring event"),
        GtkToolbarFeedback::Event {
            event: ToolbarEvent::ToggleHighlightToolRing(!highlighted.highlight_tool_ring_enabled),
            rebind_requested: false,
        }
    );
    detach_test_popovers(top);
}

fn pill_widget(top: &TopBar, id: &str) -> gtk4::Widget {
    find_widget_named(top.root.upcast_ref(), id).unwrap_or_else(|| panic!("style pill widget {id}"))
}

fn assert_style_pill_interactions(regular: &ToolbarSnapshot) {
    let (tx, rx) = std::sync::mpsc::channel();
    let mut top = TopBar::new_for_test(FeedbackSender::new(tx));
    assert_eraser_pill_interactions(&mut top, regular, &rx);
    assert_pen_pill_interactions(&mut top, regular, &rx);
    assert_shape_pill_interaction(&mut top, regular, &rx);
}

fn assert_eraser_pill_interactions(
    top: &mut TopBar,
    regular: &ToolbarSnapshot,
    rx: &std::sync::mpsc::Receiver<GtkToolbarFeedback>,
) {
    let eraser = style_pill_tool_snapshot(regular, Tool::Eraser);
    top.build_strip(&eraser, &plan_top_strip(&eraser));
    let segment_row = pill_widget(top, "top.style.eraser-mode");
    let mut halves = Vec::new();
    let mut child = segment_row.first_child();
    while let Some(current) = child {
        child = current.next_sibling();
        halves.push(
            current
                .downcast::<gtk4::Button>()
                .expect("segment half button"),
        );
    }
    assert_eq!(halves.len(), 2);
    for (half, mode) in halves.iter().zip([
        crate::input::EraserMode::Brush,
        crate::input::EraserMode::Stroke,
    ]) {
        half.emit_clicked();
        assert_eq!(
            rx.recv_timeout(Duration::from_secs(1))
                .expect("GTK eraser segment event"),
            GtkToolbarFeedback::Event {
                event: ToolbarEvent::SetEraserMode(mode),
                rebind_requested: false,
            }
        );
    }
    pill_widget(top, "top.style.thickness-value")
        .downcast::<gtk4::Button>()
        .expect("numeral button")
        .emit_clicked();
    assert_eq!(
        rx.recv_timeout(Duration::from_secs(1))
            .expect("GTK numeral event"),
        GtkToolbarFeedback::Event {
            event: ToolbarEvent::OpenPrecisionEntry(
                crate::ui::toolbar::PrecisionEntryTarget::Thickness
            ),
            rebind_requested: false,
        }
    );
    detach_test_popovers(top);
}

fn assert_pen_pill_interactions(
    top: &mut TopBar,
    regular: &ToolbarSnapshot,
    rx: &std::sync::mpsc::Receiver<GtkToolbarFeedback>,
) {
    let pen = style_pill_tool_snapshot(regular, Tool::Pen);
    top.build_strip(&pen, &plan_top_strip(&pen));
    pill_widget(top, "top.style.color-chip")
        .downcast::<gtk4::Button>()
        .expect("chip button")
        .emit_clicked();
    assert_eq!(
        rx.recv_timeout(Duration::from_secs(1))
            .expect("GTK chip event"),
        GtkToolbarFeedback::Event {
            event: ToolbarEvent::OpenColorPickerPopup,
            rebind_requested: false,
        }
    );
    let swatch = pill_widget(top, "top.style.swatch.1")
        .downcast::<gtk4::Button>()
        .expect("swatch button");
    swatch.emit_clicked();
    assert_eq!(
        rx.recv_timeout(Duration::from_secs(1))
            .expect("GTK swatch event"),
        GtkToolbarFeedback::Event {
            event: ToolbarEvent::SetQuickColor {
                color: pen.quick_colors.rendered_entries()[1].color,
                action: crate::config::QuickColorPalette::action_for_index(1),
                index: 1,
            },
            rebind_requested: false,
        }
    );
    emit_secondary_press(swatch.upcast_ref());
    assert_eq!(
        rx.recv_timeout(Duration::from_secs(1))
            .expect("GTK swatch recolor event"),
        GtkToolbarFeedback::Event {
            event: ToolbarEvent::EditQuickColor { index: 1 },
            rebind_requested: false,
        }
    );
    assert!(
        secondary_click_gesture(pill_widget(top, "top.style.color-chip").upcast_ref()).is_none()
    );
    let mut churned = pen;
    churned.thickness += 3.0;
    churned.top_fade = 0.4;
    for updater in top.updaters.borrow().iter() {
        updater(&churned);
    }
    let numeral = pill_widget(top, "top.style.thickness-value")
        .downcast::<gtk4::Button>()
        .expect("numeral button");
    assert_eq!(
        numeral.label().as_deref(),
        Some(format!("{:.0}px", churned.thickness).as_str()),
        "the numeral tracks the live thickness"
    );
    let pill_box =
        find_widget_named(top.root.upcast_ref(), "island.style").expect("style pill container");
    assert!((pill_box.opacity() - 0.4).abs() < 1e-6);
    detach_test_popovers(top);
}

fn assert_shape_pill_interaction(
    top: &mut TopBar,
    regular: &ToolbarSnapshot,
    rx: &std::sync::mpsc::Receiver<GtkToolbarFeedback>,
) {
    let shape = style_pill_tool_snapshot(regular, Tool::Rect);
    top.build_strip(&shape, &plan_top_strip(&shape));
    pill_widget(top, "top.style.fill")
        .downcast::<gtk4::CheckButton>()
        .expect("fill check button")
        .set_active(!shape.fill_enabled);
    assert_eq!(
        rx.recv_timeout(Duration::from_secs(1))
            .expect("GTK fill event"),
        GtkToolbarFeedback::Event {
            event: ToolbarEvent::ToggleFill(!shape.fill_enabled),
            rebind_requested: false,
        }
    );
    detach_test_popovers(top);
}

#[test]
fn actual_gtk_widgets_match_the_shared_contract_without_presenting_a_window() {
    const CHILD_ENV: &str = "WAYSCRIBER_GTK_WIDGET_CONTRACT_CHILD";
    const TEST_NAME: &str = "toolbar_gtk::view::top_bar::tests::actual_gtk_widgets_match_the_shared_contract_without_presenting_a_window";

    if std::env::var_os(CHILD_ENV).is_none() {
        let status = std::process::Command::new(std::env::current_exe().expect("test binary"))
            .arg(TEST_NAME)
            .arg("--exact")
            .arg("--test-threads=1")
            .env(CHILD_ENV, "1")
            .status()
            .expect("run isolated GTK widget contract test");
        assert!(status.success(), "isolated GTK widget contract test failed");
        return;
    }

    if let Err(error) = gtk4::init() {
        eprintln!("skipping GTK widget contract test: {error}");
        return;
    }

    install_gtk_contract_metrics();
    let (regular, highlighted, scenarios) = gtk_widget_contract_scenarios();
    let font_button_widths = assert_gtk_widget_scenarios(scenarios);

    assert_font_button_width_stable(&font_button_widths);

    // Compact plans normally drop quick colors before reaching the last
    // degradation step. Keep a direct adapter case so the presentation
    // contract cannot silently diverge if that planner policy changes.
    // Colors left the strip (M7-C1) and the presets island yields under the
    // compact plan (M7-C2): assert neither renders in a compact build.
    assert_compact_gtk_widget_contract(&regular);

    assert_shapes_and_overflow_contract(&regular);

    assert_gtk_toggle_events(&regular, &highlighted);

    assert_style_pill_interactions(&regular);

    assert_menu_popover_contracts(&regular);
}

fn assert_menu_popover_contracts(regular: &ToolbarSnapshot) {
    // --- Session/Settings popovers: the re-hosted pane content ---------------
    let mut session_snapshot = regular.clone();
    session_snapshot.session_popover_open = true;
    session_snapshot.active_session_name = Some("lecture.wayscriber-session".to_string());
    session_snapshot.active_session_path =
        Some(std::path::PathBuf::from("/tmp/lecture.wayscriber-session"));
    session_snapshot.recent_sessions = vec![crate::ui::toolbar::SessionRecentSnapshot {
        display_name: "recent-0.wayscriber-session".to_string(),
        path: std::path::PathBuf::from("/tmp/recent-0.wayscriber-session"),
    }];
    let (tx, menu_rx) = std::sync::mpsc::channel();
    let mut menu_top = TopBar::new_for_test(FeedbackSender::new(tx));
    // Building the strip creates the two overflow-anchored native popovers.
    menu_top.build_strip(&session_snapshot, &plan_top_strip(&session_snapshot));
    assert!(menu_top.session_popover.is_some(), "session popover exists");
    assert!(
        menu_top.settings_popover.is_some(),
        "settings popover exists"
    );

    assert_session_popover_contract(&menu_top, &session_snapshot, &menu_rx);
    assert_settings_popover_contract(&menu_top, regular, &menu_rx);
    assert_canvas_popover_contract(&menu_top, regular, &menu_rx);

    detach_test_popovers(&mut menu_top);
}

fn assert_session_popover_contract(
    top: &TopBar,
    snapshot: &ToolbarSnapshot,
    rx: &std::sync::mpsc::Receiver<GtkToolbarFeedback>,
) {
    let model = model::ToolbarSessionModel::for_popover(snapshot).expect("session model");
    let content = top.build_session_popover_content(snapshot, 1.0);
    let panel =
        find_widget_named(&content, "top.menu.session.panel").expect("session popover panel box");
    let mut buttons: Vec<gtk4::Button> = Vec::new();
    collect_descendants(&panel, &mut buttons);
    assert_eq!(
        buttons.len(),
        model.buttons.len() + model.recents.len(),
        "the popover exposes exactly the pane's controls"
    );
    for (button, button_model) in buttons.iter().zip(model.buttons.iter()) {
        assert_eq!(button.tooltip_text().as_deref(), Some(button_model.label));
        assert_eq!(button.is_sensitive(), button_model.enabled);
    }
    buttons[0].emit_clicked();
    assert_eq!(
        rx.recv_timeout(Duration::from_secs(1))
            .expect("GTK session open event"),
        GtkToolbarFeedback::Event {
            event: model.buttons[0].event.clone(),
            rebind_requested: false,
        }
    );
    buttons.last().expect("recent row button").emit_clicked();
    assert_eq!(
        rx.recv_timeout(Duration::from_secs(1))
            .expect("GTK recent event"),
        GtkToolbarFeedback::Event {
            event: model.recents[0].event(),
            rebind_requested: false,
        }
    );
}

fn assert_settings_popover_contract(
    top: &TopBar,
    regular: &ToolbarSnapshot,
    rx: &std::sync::mpsc::Receiver<GtkToolbarFeedback>,
) {
    let mut snapshot = regular.clone();
    snapshot.layout_mode = ToolbarLayoutMode::Advanced;
    snapshot.settings_popover_open = true;
    snapshot.runtime_ui_persistence = Some(RuntimeUiPersistenceSnapshot {
        path: "/home/user/.local/share/wayscriber/runtime-ui.toml".into(),
        mode: RuntimeUiPersistenceMode::Supported,
        detail: None,
        recovery_artifacts: Vec::new(),
    });
    let model = model::ToolbarSettingsModel::for_popover(&snapshot).expect("settings model");
    let (content, updaters) = top.build_settings_popover_content(&snapshot, 1.0);
    content.add_css_class("wayscriber-toolbar");
    let scroller = content
        .clone()
        .downcast::<gtk4::ScrolledWindow>()
        .expect("settings popover scroll viewport");
    let panel =
        find_widget_named(&content, "top.menu.settings.panel").expect("settings popover panel box");
    let (_, natural_height, _, _) = panel.measure(gtk4::Orientation::Vertical, -1);
    assert!(natural_height <= scroller.max_content_height());
    assert_settings_toggle_contract(&mut snapshot, &model, &panel, &updaters, rx);
    assert_settings_button_contract(&snapshot, &model, &panel, rx);
}

fn assert_settings_toggle_contract(
    snapshot: &mut ToolbarSnapshot,
    model: &model::ToolbarSettingsModel,
    panel: &gtk4::Widget,
    updaters: &[Updater],
    rx: &std::sync::mpsc::Receiver<GtkToolbarFeedback>,
) {
    let mut checks: Vec<gtk4::CheckButton> = Vec::new();
    collect_descendants(panel, &mut checks);
    let toggles: Vec<_> = model.toggle_rows().into_iter().flatten().collect();
    assert_eq!(checks.len(), toggles.len(), "settings toggle parity");
    for (check, toggle) in checks.iter().zip(&toggles) {
        assert_eq!(check.label().as_deref(), Some(toggle.label.as_ref()));
        assert_eq!(check.is_active(), toggle.checked, "{}", toggle.label);
    }
    checks[0].set_active(!toggles[0].checked);
    assert_eq!(
        rx.recv_timeout(Duration::from_secs(1))
            .expect("GTK settings toggle event"),
        GtkToolbarFeedback::Event {
            event: toggles[0].activation.clone(),
            rebind_requested: false,
        }
    );
    let updated = !toggles[0].checked;
    snapshot.context_aware_ui = updated;
    for updater in updaters {
        updater(snapshot);
    }
    assert_eq!(checks[0].is_active(), updated);
    checks[0].set_active(!updated);
    assert_eq!(
        rx.recv_timeout(Duration::from_secs(1))
            .expect("updated GTK settings toggle event"),
        GtkToolbarFeedback::Event {
            event: ToolbarEvent::ToggleContextAwareUi(!updated),
            rebind_requested: false,
        }
    );
}

fn assert_settings_button_contract(
    snapshot: &ToolbarSnapshot,
    model: &model::ToolbarSettingsModel,
    panel: &gtk4::Widget,
    rx: &std::sync::mpsc::Receiver<GtkToolbarFeedback>,
) {
    let mut buttons: Vec<gtk4::Button> = Vec::new();
    collect_descendants(panel, &mut buttons);
    let tabs: Vec<_> = buttons
        .iter()
        .filter(|button| button.has_css_class("tab"))
        .collect();
    let control = model::layout_mode_control(snapshot.layout_mode);
    let model::ToolbarControlKind::Segmented(segmented) = &control.kind else {
        panic!("layout mode control is segmented");
    };
    assert_eq!(tabs.len(), segmented.segments().len());
    tabs[0].emit_clicked();
    assert_eq!(
        rx.recv_timeout(Duration::from_secs(1))
            .expect("GTK layout mode event"),
        GtkToolbarFeedback::Event {
            event: segmented.segments()[0].activation.clone(),
            rebind_requested: false,
        }
    );
    let plain: Vec<_> = buttons
        .iter()
        .filter(|button| !button.has_css_class("tab"))
        .collect();
    assert_eq!(plain.len(), model.buttons().len(), "settings button parity");
    plain[0].emit_clicked();
    assert_eq!(
        rx.recv_timeout(Duration::from_secs(1))
            .expect("GTK settings button event"),
        GtkToolbarFeedback::Event {
            event: model.buttons()[0].event.clone(),
            rebind_requested: false,
        }
    );
}

fn assert_canvas_popover_contract(
    top: &TopBar,
    regular: &ToolbarSnapshot,
    rx: &std::sync::mpsc::Receiver<GtkToolbarFeedback>,
) {
    assert!(top.canvas_popover.is_some(), "canvas popover exists");
    assert_canvas_command_sections(top, regular, rx);
    assert_canvas_delay_updates(top, regular);
    assert_empty_canvas_popover(top, regular);
}

fn assert_canvas_command_sections(
    top: &TopBar,
    regular: &ToolbarSnapshot,
    rx: &std::sync::mpsc::Receiver<GtkToolbarFeedback>,
) {
    let mut snapshot = regular.clone();
    snapshot.canvas_popover_open = true;
    snapshot.show_actions_section = true;
    snapshot.show_boards_section = true;
    snapshot.show_pages_section = true;
    snapshot.show_zoom_actions = true;
    snapshot.show_actions_advanced = true;
    snapshot.show_step_section = true;
    let (content, _) = top.build_canvas_popover_content(&snapshot, 1.0);
    let panel =
        find_widget_named(&content, "top.menu.canvas.panel").expect("canvas popover panel box");
    assert_eq!(
        panel.width_request(),
        crate::ui::theme::toolbar::CANVAS_MENU_CONTENT_W as i32
    );
    assert_eq!(panel.margin_start(), 10);
    assert_eq!(panel.margin_end(), 10);
    let mut buttons: Vec<gtk4::Button> = Vec::new();
    collect_descendants(&panel, &mut buttons);
    assert!(buttons.len() >= 4);
    for noun in ["Board", "Page"] {
        assert_canvas_command_button_layout(&buttons, noun);
    }
    let mut checks: Vec<gtk4::CheckButton> = Vec::new();
    collect_descendants(&panel, &mut checks);
    assert_eq!(checks.len(), 2, "Step buttons + Delay sliders toggles");
    checks[0].set_active(!snapshot.custom_section_enabled);
    assert_eq!(
        rx.recv_timeout(Duration::from_secs(1))
            .expect("GTK canvas step toggle event"),
        GtkToolbarFeedback::Event {
            event: ToolbarEvent::ToggleCustomSection(!snapshot.custom_section_enabled),
            rebind_requested: false,
        }
    );
}

fn assert_canvas_command_button_layout(buttons: &[gtk4::Button], noun: &str) {
    let button_with_tooltip = |prefix: &str| {
        buttons
            .iter()
            .find(|button| {
                button
                    .tooltip_text()
                    .is_some_and(|tooltip| tooltip.starts_with(prefix))
            })
            .unwrap_or_else(|| panic!("{prefix} Canvas button"))
    };
    let duplicate = button_with_tooltip(&format!("Duplicate {noun}"));
    let delete = button_with_tooltip(&format!("Delete {noun}"));
    for button in [duplicate, delete] {
        assert_eq!(button.width_request(), 32);
        assert_eq!(button.halign(), gtk4::Align::Center);
        assert!(!button.hexpands());
    }
    assert_eq!(delete.margin_end(), 6);
    let safe_parent = duplicate.parent().expect("safe-action group");
    let destructive_parent = delete.parent().expect("destructive-action row");
    assert_ne!(safe_parent, destructive_parent);
    assert!(
        safe_parent
            .downcast::<gtk4::Box>()
            .expect("homogeneous safe-action box")
            .is_homogeneous()
    );
    assert_eq!(
        destructive_parent
            .downcast::<gtk4::Box>()
            .expect("guarded command row")
            .spacing(),
        12
    );
}

fn undo_all_slider_tooltip(boxes: &[gtk4::Box]) -> String {
    boxes
        .iter()
        .find_map(|widget| {
            widget
                .tooltip_text()
                .filter(|tooltip| tooltip.contains("Undo-all delay"))
                .map(|tooltip| tooltip.to_string())
        })
        .expect("undo-all delay slider tooltip")
}

fn assert_canvas_delay_updates(top: &TopBar, regular: &ToolbarSnapshot) {
    let mut snapshot = regular.clone();
    snapshot.canvas_popover_open = true;
    snapshot.show_step_section = true;
    snapshot.show_delay_sliders = true;
    snapshot.custom_section_enabled = false;
    snapshot.undo_all_delay_ms = 1000;
    let (content, updaters) = top.build_canvas_popover_content(&snapshot, 1.0);
    assert!(!updaters.is_empty());
    let panel = find_widget_named(&content, "top.menu.canvas.panel")
        .expect("delay canvas popover panel box");
    let mut boxes: Vec<gtk4::Box> = Vec::new();
    collect_descendants(&panel, &mut boxes);
    assert!(undo_all_slider_tooltip(&boxes).contains("1.0s"));
    let mut bumped = snapshot;
    bumped.undo_all_delay_ms = 2500;
    for updater in &updaters {
        updater(&bumped);
    }
    assert!(undo_all_slider_tooltip(&boxes).contains("2.5s"));
}

fn assert_empty_canvas_popover(top: &TopBar, regular: &ToolbarSnapshot) {
    let mut snapshot = regular.clone();
    snapshot.canvas_popover_open = true;
    snapshot.show_actions_section = false;
    snapshot.show_boards_section = false;
    snapshot.show_pages_section = false;
    snapshot.show_zoom_actions = false;
    snapshot.show_actions_advanced = false;
    snapshot.show_step_section = false;
    let (content, _) = top.build_canvas_popover_content(&snapshot, 1.0);
    let panel = find_widget_named(&content, "top.menu.canvas.panel")
        .expect("empty canvas popover panel box");
    let mut buttons: Vec<gtk4::Button> = Vec::new();
    collect_descendants(&panel, &mut buttons);
    assert!(buttons.is_empty());
}

#[test]
fn gtk_stateful_toggle_adapter_emits_the_requested_live_state() {
    use super::controls::event_for_toggle_state;

    let cases = [
        (
            model::TopToolbarControl::ShapePicker,
            ToolbarEvent::ToggleShapePicker(false),
            ToolbarEvent::ToggleShapePicker(true),
        ),
        (
            model::TopToolbarControl::Utility(model::TopToolbarUtility::Highlight),
            ToolbarEvent::ToggleAllHighlight(false),
            ToolbarEvent::ToggleAllHighlight(true),
        ),
        (
            model::TopToolbarControl::Pin,
            ToolbarEvent::PinTopToolbar(false),
            ToolbarEvent::PinTopToolbar(true),
        ),
        (
            model::TopToolbarControl::Overflow,
            ToolbarEvent::ToggleTopOverflow(false),
            ToolbarEvent::ToggleTopOverflow(true),
        ),
        (
            model::TopToolbarControl::HighlightRing,
            ToolbarEvent::ToggleHighlightToolRing(false),
            ToolbarEvent::ToggleHighlightToolRing(true),
        ),
    ];

    for (control, inactive, active) in cases {
        assert_eq!(event_for_toggle_state(control, false), inactive);
        assert_eq!(event_for_toggle_state(control, true), active);
    }
}
