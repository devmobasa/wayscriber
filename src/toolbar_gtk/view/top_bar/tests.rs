//! GTK top-strip unit tests.

use std::collections::HashMap;
use std::ffi::{CStr, CString};
use std::time::Duration;

use super::*;
use crate::config::KeyBinding;
use crate::input::state::test_support::make_test_input_state;
use crate::ui::toolbar::ToolbarBindingHints;
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
        vec![KeyBinding::parse("9").expect("binding")],
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

/// `StructureKey.use_icons` is `use_icons || plan.compact`, so a compact
/// plan masks an icon-mode flip from the structural rebuild. The popover
/// content keys must therefore track `use_icons` themselves: the Settings
/// "Icon buttons" checkbox bakes `ToggleIconMode(!use_icons)` at build
/// time and would otherwise re-emit the stale pre-flip value, and both
/// popovers render icon-vs-text button bodies from it.
#[test]
fn popover_content_keys_track_icon_mode_even_when_the_compact_plan_masks_it() {
    let mut state = make_test_input_state();
    state.toolbar_use_icons = false;
    let text_mode = ToolbarSnapshot::from_input_with_bindings(
        &state,
        ToolbarBindingHints::from_input_state(&state),
    );
    state.toolbar_use_icons = true;
    let icon_mode = ToolbarSnapshot::from_input_with_bindings(
        &state,
        ToolbarBindingHints::from_input_state(&state),
    );

    // The masking scenario is real: under a compact plan the structure key
    // cannot tell the two snapshots apart.
    let mut compact_plan = TopStripPlan::unconstrained();
    compact_plan.compact = true;
    assert!(
        StructureKey::of(&text_mode, &compact_plan) == StructureKey::of(&icon_mode, &compact_plan),
        "compact plan masks the icon-mode flip from the structural rebuild"
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
fn canvas_popover_content_key_rebuilds_on_section_and_value_changes() {
    let state = make_test_input_state();
    let base = ToolbarSnapshot::from_input_with_bindings(
        &state,
        ToolbarBindingHints::from_input_state(&state),
    );

    // A section display toggle drives a content rebuild.
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
    state.toolbar_use_icons = true;
    let regular = ToolbarSnapshot::from_input_with_bindings(
        &state,
        ToolbarBindingHints::from_input_state(&state),
    );

    state.toolbar_layout_mode = ToolbarLayoutMode::Simple;
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
fn assert_gtk_style_widget(
    widget: &gtk4::Widget,
    control: model::StylePillControl,
    snapshot: &ToolbarSnapshot,
) {
    let id = widget.widget_name().to_string();
    match control.role() {
        model::StylePillRole::Swatch => {
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
            assert_accessible_label(widget, &control.label(snapshot), &id);
        }
        model::StylePillRole::Slider => {
            // SliderRow: a box hosting the hand-drawn track DrawingArea.
            assert!(widget.is::<gtk4::Box>(), "{id} slider row");
            assert!(
                find_control_surface(widget)
                    .is_some_and(|surface| surface.is::<gtk4::DrawingArea>()),
                "{id} slider track"
            );
        }
        model::StylePillRole::Value => {
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
        model::StylePillRole::Toggle => {
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
        model::StylePillRole::Button => {
            let button = widget
                .clone()
                .downcast::<gtk4::Button>()
                .unwrap_or_else(|_| panic!("{id} is a button"));
            // Docked selection cycle buttons show the live value; plain
            // buttons show their label.
            let expected_text = match control {
                model::StylePillControl::SelectionCycle(_) => {
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
        }
        model::StylePillRole::Stepper => {
            let steps = control.steps(snapshot).expect("stepper halves");
            assert!(widget.is::<gtk4::Box>(), "{id} stepper row");
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
        model::StylePillRole::Segmented => {
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
                // Contract-id parity with the builtin tree's HitArea halves.
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
    }
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
            assert_eq!(style.active, expected.active);
            assert_eq!(style.disabled, !expected.enabled);
            assert_eq!(
                style.destructive,
                expected.role == model::TopToolbarControlRole::Destructive
            );
        }
        W::TextButton { label, style } => {
            assert_eq!(label.text, expected.label);
            assert_eq!(style.active, expected.active);
            assert_eq!(style.disabled, !expected.enabled);
            assert_eq!(
                style.destructive,
                expected.role == model::TopToolbarControlRole::Destructive
            );
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
        if control == model::StylePillControl::OpacitySlider {
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
    use crate::backend::wayland::TopToolbarWidgetKind as W;

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
        let plan = plan_top_strip(&snapshot);
        let expected = expected_style_pill_nodes(&snapshot, &plan);
        let (width, height) = top_toolbar_size(&snapshot);
        let tree =
            crate::backend::wayland::build_top_toolbar_view(&snapshot, width as f64, height as f64);

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
            let interaction_event = node.interact.as_ref().map(|interaction| &interaction.event);
            let interaction_tooltip = node
                .interact
                .as_ref()
                .and_then(|interaction| interaction.tooltip.clone());
            match expectation {
                StylePillNodeExpectation::Control(control) => {
                    let expected_event = control
                        .enabled(&snapshot)
                        .then(|| control.event(&snapshot))
                        .flatten();
                    assert_eq!(
                        interaction_event,
                        expected_event.as_ref(),
                        "{name}: {id} event"
                    );
                    if node.interact.is_some() {
                        assert_eq!(
                            interaction_tooltip,
                            control.tooltip(&snapshot),
                            "{name}: {id} tooltip"
                        );
                    }
                    match (control.role(), &node.kind) {
                        (model::StylePillRole::Swatch, W::Swatch { color, selected }) => {
                            let expected_color = match control {
                                model::StylePillControl::QuickSwatch(index) => {
                                    snapshot.quick_colors.rendered_entries()[*index].color
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
                            assert_eq!(*selected, control.active(&snapshot), "{name}: {id}");
                        }
                        (model::StylePillRole::Slider, W::Slider { t }) => {
                            let (spec, value) = control.slider(&snapshot).expect("slider spec");
                            assert!(
                                (*t - spec.t_from_value(value)).abs() < 1e-9,
                                "{name}: {id} slider position"
                            );
                        }
                        (model::StylePillRole::Value, W::TextButton { label, .. }) => {
                            assert_eq!(
                                Some(label.text.clone()),
                                control.value_text(&snapshot),
                                "{name}: {id} live numeral"
                            );
                            // Covered by the generic event assertion above:
                            // the numeral opens the precise-entry popup.
                            assert!(
                                matches!(
                                    interaction_event,
                                    Some(crate::ui::toolbar::ToolbarEvent::OpenPrecisionEntry(_))
                                ),
                                "{name}: {id} opens the precise entry"
                            );
                        }
                        (model::StylePillRole::Toggle, W::MiniCheckbox { checked, label }) => {
                            assert_eq!(*checked, control.active(&snapshot), "{name}: {id}");
                            assert_eq!(
                                label.text,
                                control.label(&snapshot).as_ref(),
                                "{name}: {id} label"
                            );
                        }
                        (model::StylePillRole::Button, W::TextButton { label, style }) => {
                            // Docked selection cycle buttons show the live
                            // value; plain buttons show their label.
                            let expected_text = match control {
                                model::StylePillControl::SelectionCycle(_) => {
                                    control.value_text(&snapshot).expect("cycle value text")
                                }
                                _ => control.label(&snapshot).into_owned(),
                            };
                            assert_eq!(label.text, expected_text, "{name}: {id} text");
                            assert_eq!(
                                style.disabled,
                                !control.enabled(&snapshot),
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
                            let segments = control.segments(&snapshot).expect("segments");
                            assert_eq!(left.text, segments[0].label, "{name}: {id}");
                            assert_eq!(right.text, segments[1].label, "{name}: {id}");
                            assert_eq!(*active_right, segments[1].active, "{name}: {id}");
                            assert!(node.interact.is_none(), "halves carry the interactions");
                        }
                        (role, kind) => panic!("{name}: {id} role {role:?} painted as {kind:?}"),
                    }
                }
                StylePillNodeExpectation::Readout(control) => {
                    assert!(node.interact.is_none(), "{name}: {id} readout is decor");
                    match &node.kind {
                        W::Label(label) => assert_eq!(
                            Some(label.text.clone()),
                            control.value_text(&snapshot),
                            "{name}: {id} readout"
                        ),
                        other => panic!("{name}: {id} readout kind {other:?}"),
                    }
                }
                StylePillNodeExpectation::StepHalf(control, index) => {
                    let steps = control.steps(&snapshot).expect("stepper halves");
                    let step = &steps[*index];
                    let enabled = control.enabled(&snapshot);
                    match &node.kind {
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
                        interaction_tooltip.as_deref(),
                        enabled.then_some(step.tooltip.as_str()),
                        "{name}: {id} step tooltip"
                    );
                }
                StylePillNodeExpectation::StepValue(control) => {
                    assert!(node.interact.is_none(), "{name}: {id} readout is decor");
                    match &node.kind {
                        W::Label(label) => assert_eq!(
                            Some(label.text.clone()),
                            control.value_text(&snapshot),
                            "{name}: {id} stepper readout"
                        ),
                        other => panic!("{name}: {id} stepper readout kind {other:?}"),
                    }
                }
                StylePillNodeExpectation::SegmentHalf(control, index) => {
                    let segments = control.segments(&snapshot).expect("segments");
                    let segment = &segments[*index];
                    assert!(matches!(node.kind, W::HitArea), "{name}: {id}");
                    assert_eq!(
                        interaction_event,
                        Some(&segment.event),
                        "{name}: {id} segment event"
                    );
                    assert_eq!(
                        interaction_tooltip.as_deref(),
                        Some(segment.tooltip.as_str()),
                        "{name}: {id} segment tooltip"
                    );
                }
            }
        }
    }
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
    // One scenario per style-pill morph state, so every pill shape passes
    // the full order/island/semantics contract.
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
    scenarios.push(("text-mode", text_mode));
    scenarios.push(("selection", style_pill_selection_snapshot(&regular)));

    for (name, snapshot) in scenarios {
        let plan = plan_top_strip(&snapshot);
        let spec = super::strip::top_toolbar_spec(&snapshot, &plan);
        let expected = expected_semantic_records(&snapshot, &spec, &plan);
        let style_controls = style_pill_controls(&snapshot, &plan);
        let (tx, _rx) = std::sync::mpsc::channel();
        let mut top = TopBar::new_for_test(FeedbackSender::new(tx));
        if snapshot.top_minimized {
            top.build_minimized(&snapshot, &plan);
        } else if snapshot.top_micro_active() {
            top.build_micro(&snapshot, &plan);
        } else {
            top.build_strip(&snapshot, &plan);
        }
        for updater in top.updaters.borrow().iter() {
            updater(&snapshot);
        }

        let widgets = collect_semantic_widgets(top.root.upcast_ref());
        assert_eq!(
            widgets
                .iter()
                .map(|widget| widget.widget_name().to_string())
                .collect::<Vec<_>>(),
            expected_main_widget_ids(&spec, &snapshot, &plan),
            "{name} GTK widget order"
        );
        // Ordered ids alone cannot catch a control appended to the wrong
        // pill: also assert every widget's ancestor chain reaches the
        // island container the shared spec assigns it.
        let expected_islands = expected_island_keys(&snapshot, &spec, &plan);
        for widget in &widgets {
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
        for widget in widgets {
            let id = widget.widget_name();
            if let Some((_, control)) = style_controls
                .iter()
                .find(|(control_id, _)| *control_id == id)
            {
                assert_gtk_style_widget(&widget, *control, &snapshot);
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
        detach_test_popovers(&mut top);
    }

    // Compact plans normally drop quick colors before reaching the last
    // degradation step. Keep a direct adapter case so the presentation
    // contract cannot silently diverge if that planner policy changes.
    // Colors left the strip (M7-C1) and the presets island yields under the
    // compact plan (M7-C2): assert neither renders in a compact build.
    let mut compact_plan = TopStripPlan::unconstrained();
    compact_plan.compact = true;
    let (tx, _rx) = std::sync::mpsc::channel();
    let mut compact_top = TopBar::new_for_test(FeedbackSender::new(tx));
    compact_top.build_strip(&regular, &compact_plan);
    let compact_widgets = collect_semantic_widgets(compact_top.root.upcast_ref());
    let compact_ids = compact_widgets
        .iter()
        .map(|widget| widget.widget_name().to_string())
        .collect::<Vec<_>>();
    // Positive presence first, so the absence check below can never pass
    // vacuously on an empty strip: the compact build must materialize exactly
    // the shared spec's protected widget set (tools/history/chrome).
    let compact_spec = super::strip::top_toolbar_spec(&regular, &compact_plan);
    let expected_compact_ids = expected_main_widget_ids(&compact_spec, &regular, &compact_plan);
    assert!(
        !expected_compact_ids.is_empty(),
        "the compact strip still builds its protected core"
    );
    assert_eq!(
        compact_ids, expected_compact_ids,
        "the compact strip builds exactly the shared spec's widget set"
    );
    // Then the contract of this case: no colors or presets survive compaction.
    assert!(
        compact_ids.iter().all(|name| {
            !name.starts_with("top.quick-color.")
                && name.as_str() != "top.group.quick-colors"
                && !name.starts_with("top.preset.")
        }),
        "the compact strip carries no colors or presets: {compact_ids:?}"
    );
    detach_test_popovers(&mut compact_top);

    let mut shapes = regular.clone();
    shapes.shape_picker_open = true;
    shapes.active_tool = Tool::RegularPolygon;
    let (tx, shape_rx) = std::sync::mpsc::channel();
    let shape_top = TopBar::new_for_test(FeedbackSender::new(tx));
    let shape_content = shape_top.build_shapes_popover_content(
        &shapes,
        (ICON_BUTTON, ICON_BUTTON),
        ICON_SIZE,
        true,
        1.0,
    );
    let shape_tools = model::visible_shape_picker_rows(&shapes, false)
        .into_iter()
        .flatten()
        .filter(|tool| model::tool_visible(&shapes, *tool))
        .collect::<Vec<_>>();
    let mut expected_shape_ids = shape_tools
        .iter()
        .map(|tool| {
            format!(
                "top.picker.{}",
                model::toolbar_item_id_for_tool(*tool).as_str()
            )
        })
        .collect::<Vec<_>>();
    if model::top_fill_visible(&shapes) {
        expected_shape_ids.push(
            crate::config::toolbar_item_ids::TOP_UTILITY_FILL
                .as_str()
                .to_string(),
        );
    }
    expected_shape_ids.extend([
        "top.options.sides-minus".to_string(),
        "top.options.sides-plus".to_string(),
    ]);
    let shape_widgets = collect_semantic_widgets(shape_content.upcast_ref());
    assert_eq!(
        shape_widgets
            .iter()
            .map(|widget| widget.widget_name().to_string())
            .collect::<Vec<_>>(),
        expected_shape_ids,
        "GTK shapes-popover order"
    );
    for (widget, tool) in shape_widgets.iter().zip(&shape_tools) {
        let expected = control_record(
            &shapes,
            SemanticLane::Strip,
            model::TopToolbarControl::Tool(*tool),
            true,
        );
        assert_gtk_control_widget(widget, &expected);
    }
    let first_shape = first_control_surface(&shape_widgets[0])
        .downcast::<gtk4::Button>()
        .expect("shape-picker tool button");
    first_shape.emit_clicked();
    assert_eq!(
        shape_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("GTK shape event"),
        GtkToolbarFeedback::Event {
            event: ToolbarEvent::SelectTool(shape_tools[0]),
            rebind_requested: false,
        }
    );

    let mut line_shapes = shapes.clone();
    line_shapes.active_tool = Tool::Line;
    line_shapes.tool_override = None;
    let line_shape_content = shape_top.build_shapes_popover_content(
        &line_shapes,
        (ICON_BUTTON, ICON_BUTTON),
        ICON_SIZE,
        true,
        1.0,
    );
    assert!(
        collect_semantic_widgets(line_shape_content.upcast_ref())
            .iter()
            .any(|widget| {
                widget.widget_name() == crate::config::toolbar_item_ids::TOP_UTILITY_FILL.as_str()
            }),
        "GTK Shapes must expose Fill before a fill-capable shape is selected"
    );

    let mut overflow_plan = TopStripPlan::unconstrained();
    overflow_plan.dropped_tools = vec![Tool::Line, Tool::Arrow];
    overflow_plan.dropped_utilities = vec![
        model::TopUtilityButton::Screenshot,
        model::TopUtilityButton::Highlight,
    ];
    let overflow_spec = super::strip::top_toolbar_spec(&regular, &overflow_plan);
    let overflow_content = shape_top.build_overflow_popover_content(
        &regular,
        &overflow_spec,
        (ICON_BUTTON, ICON_BUTTON),
        ICON_SIZE,
        true,
        1.0,
    );
    let overflow_widgets = collect_semantic_widgets(overflow_content.upcast_ref());
    assert_eq!(
        overflow_widgets
            .iter()
            .map(|widget| widget.widget_name().to_string())
            .collect::<Vec<_>>(),
        overflow_spec
            .overflow()
            .iter()
            .map(|control| format!("top.overflow.{}", control.id().render_id()))
            .collect::<Vec<_>>(),
        "GTK overflow order"
    );
    for (widget, control) in overflow_widgets.iter().zip(overflow_spec.overflow()) {
        let expected = control_record(&regular, SemanticLane::Overflow, *control, true);
        assert_gtk_control_widget(widget, &expected);
    }
    let first_overflow = first_control_surface(&overflow_widgets[0])
        .downcast::<gtk4::Button>()
        .expect("overflow tool button");
    first_overflow.emit_clicked();
    assert_eq!(
        shape_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("GTK overflow event"),
        GtkToolbarFeedback::Event {
            event: overflow_spec.overflow()[0].event(&regular),
            rebind_requested: false,
        }
    );

    let (tx, rx) = std::sync::mpsc::channel();
    let mut event_top = TopBar::new_for_test(FeedbackSender::new(tx));
    let shape = event_top.shapes_picker_button(
        &regular,
        model::TopToolbarControl::ShapePicker,
        (ICON_BUTTON, ICON_BUTTON),
        ICON_SIZE,
        true,
    );
    let highlight = event_top.action_button(
        &regular,
        model::TopToolbarControl::Utility(model::TopToolbarUtility::Highlight),
        (ICON_BUTTON, ICON_BUTTON),
        ICON_SIZE,
        true,
        true,
    );
    let pin = event_top.pin_button(&regular, model::TopToolbarControl::Pin, PIN_BUTTON_SIZE);
    let overflow = event_top.overflow_button(
        &regular,
        model::TopToolbarControl::Overflow,
        (ICON_BUTTON, ICON_BUTTON),
        ICON_SIZE,
    );
    for (popover, capture_surface) in [
        (
            event_top.shapes_popover.as_ref().unwrap(),
            event_top.shapes_capture_surface.as_ref().unwrap(),
        ),
        (
            event_top.overflow_popover.as_ref().unwrap(),
            event_top.overflow_capture_surface.as_ref().unwrap(),
        ),
    ] {
        let content = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
        capture_surface.set_content(&content);
        super::popovers::set_popover_capture_transparent(popover, capture_surface, true, false);
        assert!(
            popover.has_css_class(crate::toolbar_gtk::css::CAPTURE_TRANSPARENT_CLASS),
            "capture suppression must clear native popover chrome"
        );
        assert!(
            !popover.can_target(),
            "capture suppression must stop invisible popovers from accepting input immediately"
        );
        assert_eq!(capture_surface.content_opacity(), Some(0.0));
        assert!(capture_surface.proof_visible());

        super::popovers::set_popover_capture_transparent(popover, capture_surface, false, true);
        assert!(!popover.has_css_class(crate::toolbar_gtk::css::CAPTURE_TRANSPARENT_CLASS));
        assert!(popover.can_target());
        assert_eq!(capture_surface.content_opacity(), Some(1.0));
        assert!(!capture_surface.proof_visible());
    }
    for (button, expected) in [
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
    ] {
        button.emit_clicked();
        assert_eq!(
            rx.recv_timeout(Duration::from_secs(1)).expect("GTK event"),
            GtkToolbarFeedback::Event {
                event: expected,
                rebind_requested: false,
            }
        );
    }

    let mut active = regular.clone();
    active.shape_picker_open = true;
    active.any_highlight_active = true;
    active.top_pinned = true;
    active.top_overflow_open = true;
    event_top.shapes_expected_open.set(true);
    event_top.overflow_expected_open.set(true);
    for updater in event_top.updaters.borrow().iter() {
        updater(&active);
    }
    for (button, expected) in [
        (&shape, ToolbarEvent::ToggleShapePicker(false)),
        (&highlight, ToolbarEvent::ToggleAllHighlight(false)),
        (&pin, ToolbarEvent::PinTopToolbar(false)),
        (&overflow, ToolbarEvent::ToggleTopOverflow(false)),
    ] {
        button.emit_clicked();
        assert_eq!(
            rx.recv_timeout(Duration::from_secs(1)).expect("GTK event"),
            GtkToolbarFeedback::Event {
                event: expected,
                rebind_requested: false,
            }
        );
    }
    event_top.shapes_expected_open.set(false);
    event_top.overflow_expected_open.set(false);

    // The direct factories parent popovers to their buttons. Detach those
    // before rebuilding the test bar, matching the production rebuild path.
    detach_test_popovers(&mut event_top);

    event_top.build_strip(&highlighted, &plan_top_strip(&highlighted));
    let ring = collect_semantic_widgets(event_top.root.upcast_ref())
        .into_iter()
        .find(|widget| {
            widget.widget_name()
                == crate::config::toolbar_item_ids::TOP_UTILITY_HIGHLIGHT_RING.as_str()
        })
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
    detach_test_popovers(&mut event_top);

    // --- Style pill interactions --------------------------------------------
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
    let pill_widget = |top: &TopBar, id: &str| {
        find_widget_named(top.root.upcast_ref(), id)
            .unwrap_or_else(|| panic!("style pill widget {id}"))
    };

    // Eraser morph: the Brush/Stroke segment emits SetEraserMode per half;
    // the size numeral opens the overlay precise-entry popup.
    let eraser = style_pill_tool_snapshot(&regular, Tool::Eraser);
    event_top.build_strip(&eraser, &plan_top_strip(&eraser));
    let segment_row = pill_widget(&event_top, "top.style.eraser-mode");
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
    let numeral = pill_widget(&event_top, "top.style.thickness-value")
        .downcast::<gtk4::Button>()
        .expect("numeral button");
    numeral.emit_clicked();
    assert_eq!(
        rx.recv_timeout(Duration::from_secs(1))
            .expect("GTK numeral event"),
        GtkToolbarFeedback::Event {
            event: ToolbarEvent::OpenPrecisionEntry(
                crate::ui::toolbar::PrecisionEntryTarget::Thickness
            ),
            rebind_requested: false,
        },
        "the numeral opens the overlay precise-entry popup"
    );
    detach_test_popovers(&mut event_top);

    // Stroke morph: the chip opens the one true picker popup, swatches set
    // quick colors, and the updaters drive the live numeral and idle fade.
    let pen = style_pill_tool_snapshot(&regular, Tool::Pen);
    event_top.build_strip(&pen, &plan_top_strip(&pen));
    pill_widget(&event_top, "top.style.color-chip")
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
    pill_widget(&event_top, "top.style.swatch.1")
        .downcast::<gtk4::Button>()
        .expect("swatch button")
        .emit_clicked();
    assert_eq!(
        rx.recv_timeout(Duration::from_secs(1))
            .expect("GTK swatch event"),
        GtkToolbarFeedback::Event {
            event: ToolbarEvent::SetQuickColor {
                color: pen.quick_colors.rendered_entries()[1].color,
                action: crate::config::QuickColorPalette::action_for_index(1),
            },
            rebind_requested: false,
        }
    );
    let mut churned = pen.clone();
    churned.thickness += 3.0;
    churned.top_fade = 0.4;
    for updater in event_top.updaters.borrow().iter() {
        updater(&churned);
    }
    let numeral = pill_widget(&event_top, "top.style.thickness-value")
        .downcast::<gtk4::Button>()
        .expect("numeral button");
    assert_eq!(
        numeral.label().as_deref(),
        Some(format!("{:.0}px", churned.thickness).as_str()),
        "the numeral tracks the live thickness"
    );
    let pill_box = find_widget_named(event_top.root.upcast_ref(), "island.style")
        .expect("style pill container");
    assert!(
        (pill_box.opacity() - 0.4).abs() < 1e-6,
        "the pill fades with the strip"
    );
    detach_test_popovers(&mut event_top);

    // Shape morph: the Fill mini-toggle emits the requested state.
    let shape = style_pill_tool_snapshot(&regular, Tool::Rect);
    event_top.build_strip(&shape, &plan_top_strip(&shape));
    let fill = pill_widget(&event_top, "top.style.fill")
        .downcast::<gtk4::CheckButton>()
        .expect("fill check button");
    fill.set_active(!shape.fill_enabled);
    assert_eq!(
        rx.recv_timeout(Duration::from_secs(1))
            .expect("GTK fill event"),
        GtkToolbarFeedback::Event {
            event: ToolbarEvent::ToggleFill(!shape.fill_enabled),
            rebind_requested: false,
        }
    );
    detach_test_popovers(&mut event_top);

    // --- Session/Settings popovers: the re-hosted pane content ---------------
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

    let session_model =
        model::ToolbarSessionModel::for_popover(&session_snapshot).expect("session model");
    let session_content = menu_top.build_session_popover_content(&session_snapshot, 1.0);
    let session_panel = find_widget_named(&session_content, "top.menu.session.panel")
        .expect("session popover panel box");
    let mut session_buttons: Vec<gtk4::Button> = Vec::new();
    collect_descendants(&session_panel, &mut session_buttons);
    assert_eq!(
        session_buttons.len(),
        session_model.buttons.len() + session_model.recents.len(),
        "the popover exposes exactly the pane's controls"
    );
    for (button, button_model) in session_buttons.iter().zip(session_model.buttons.iter()) {
        assert_eq!(
            button.tooltip_text().as_deref(),
            Some(button_model.label),
            "session button tooltip"
        );
        assert_eq!(button.is_sensitive(), button_model.enabled);
    }
    session_buttons[0].emit_clicked();
    assert_eq!(
        menu_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("GTK session open event"),
        GtkToolbarFeedback::Event {
            event: session_model.buttons[0].event.clone(),
            rebind_requested: false,
        }
    );
    session_buttons
        .last()
        .expect("recent row button")
        .emit_clicked();
    assert_eq!(
        menu_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("GTK recent event"),
        GtkToolbarFeedback::Event {
            event: session_model.recents[0].event(),
            rebind_requested: false,
        }
    );

    let mut settings_snapshot = regular.clone();
    settings_snapshot.settings_popover_open = true;
    let settings_model =
        model::ToolbarSettingsModel::for_popover(&settings_snapshot).expect("settings model");
    let settings_content = menu_top.build_settings_popover_content(&settings_snapshot, 1.0);
    let settings_panel = find_widget_named(&settings_content, "top.menu.settings.panel")
        .expect("settings popover panel box");

    // Toggle parity: one check button per pane toggle, same order/state.
    let mut checks: Vec<gtk4::CheckButton> = Vec::new();
    collect_descendants(&settings_panel, &mut checks);
    let toggles: Vec<_> = settings_model.toggle_rows().into_iter().flatten().collect();
    assert_eq!(checks.len(), toggles.len(), "settings toggle parity");
    for (check, toggle) in checks.iter().zip(&toggles) {
        assert_eq!(check.label().as_deref(), Some(toggle.label.as_ref()));
        assert_eq!(check.is_active(), toggle.checked, "{}", toggle.label);
    }
    checks[0].set_active(!toggles[0].checked);
    assert_eq!(
        menu_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("GTK settings toggle event"),
        GtkToolbarFeedback::Event {
            event: toggles[0].activation.compatibility_event(),
            rebind_requested: false,
        }
    );

    // Layout-mode segments and the settings button grid carry the pane's
    // events.
    let mut buttons: Vec<gtk4::Button> = Vec::new();
    collect_descendants(&settings_panel, &mut buttons);
    let tabs: Vec<_> = buttons
        .iter()
        .filter(|button| button.has_css_class("tab"))
        .collect();
    let control = model::layout_mode_control(settings_snapshot.layout_mode);
    let model::ToolbarControlKind::Segmented(segmented) = &control.kind else {
        panic!("layout mode control is segmented");
    };
    assert_eq!(tabs.len(), segmented.segments().len());
    tabs[0].emit_clicked();
    assert_eq!(
        menu_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("GTK layout mode event"),
        GtkToolbarFeedback::Event {
            event: segmented.segments()[0].activation.compatibility_event(),
            rebind_requested: false,
        }
    );
    let plain_buttons: Vec<_> = buttons
        .iter()
        .filter(|button| !button.has_css_class("tab"))
        .collect();
    assert_eq!(
        plain_buttons.len(),
        settings_model.buttons().len(),
        "settings button parity"
    );
    plain_buttons[0].emit_clicked();
    assert_eq!(
        menu_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("GTK settings button event"),
        GtkToolbarFeedback::Event {
            event: settings_model.buttons()[0].event.clone(),
            rebind_requested: false,
        }
    );

    // Canvas popover parity: the command sections render buttons, the Step
    // config renders its two toggles, and toggling every section off leaves
    // the popover empty.
    assert!(menu_top.canvas_popover.is_some(), "canvas popover exists");
    let mut canvas_snapshot = regular.clone();
    canvas_snapshot.canvas_popover_open = true;
    canvas_snapshot.show_boards_section = true;
    canvas_snapshot.show_pages_section = true;
    canvas_snapshot.show_zoom_actions = true;
    canvas_snapshot.show_actions_advanced = true;
    canvas_snapshot.show_step_section = true;
    let (canvas_content, _canvas_updaters) =
        menu_top.build_canvas_popover_content(&canvas_snapshot, 1.0);
    let canvas_panel = find_widget_named(&canvas_content, "top.menu.canvas.panel")
        .expect("canvas popover panel box");
    let mut canvas_buttons: Vec<gtk4::Button> = Vec::new();
    collect_descendants(&canvas_panel, &mut canvas_buttons);
    assert!(
        canvas_buttons.len() >= 4,
        "the canvas popover exposes the command-section buttons"
    );
    // The Step config exposes exactly its two toggles; toggling the first
    // emits ToggleCustomSection with the live state.
    let mut canvas_checks: Vec<gtk4::CheckButton> = Vec::new();
    collect_descendants(&canvas_panel, &mut canvas_checks);
    assert_eq!(
        canvas_checks.len(),
        2,
        "Step buttons + Delay sliders toggles"
    );
    canvas_checks[0].set_active(!canvas_snapshot.custom_section_enabled);
    assert_eq!(
        menu_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("GTK canvas step toggle event"),
        GtkToolbarFeedback::Event {
            event: ToolbarEvent::ToggleCustomSection(!canvas_snapshot.custom_section_enabled),
            rebind_requested: false,
        }
    );

    // Delay-slider live update WITHOUT a subtree rebuild: the Canvas popover's
    // delay sliders ride the persistent value-updaters this builder returns
    // (the content key omits their values), so an external delay change flows
    // through an updater, not a rebuild. Build with the delay sliders shown,
    // then run the returned updaters with a bumped delay and confirm the slider
    // reflects it in place.
    let mut delay_snapshot = regular.clone();
    delay_snapshot.canvas_popover_open = true;
    delay_snapshot.show_step_section = true;
    delay_snapshot.show_delay_sliders = true;
    delay_snapshot.custom_section_enabled = false;
    delay_snapshot.undo_all_delay_ms = 1000;
    let (delay_content, delay_updaters) =
        menu_top.build_canvas_popover_content(&delay_snapshot, 1.0);
    assert!(
        !delay_updaters.is_empty(),
        "the delay sliders register persistent value-updaters"
    );
    let delay_panel = find_widget_named(&delay_content, "top.menu.canvas.panel")
        .expect("delay canvas popover panel box");
    let mut delay_boxes: Vec<gtk4::Box> = Vec::new();
    collect_descendants(&delay_panel, &mut delay_boxes);
    let undo_all_slider_tooltip = |boxes: &[gtk4::Box]| -> String {
        boxes
            .iter()
            .find_map(|widget| {
                widget
                    .tooltip_text()
                    .filter(|tooltip| tooltip.contains("Undo-all delay"))
                    .map(|tooltip| tooltip.to_string())
            })
            .expect("undo-all delay slider tooltip")
    };
    assert!(
        undo_all_slider_tooltip(&delay_boxes).contains("1.0s"),
        "the delay slider starts at the built value"
    );
    // A backend echo of a new delay flows through the persistent updater —
    // never a rebuild — and the same slider widget updates in place.
    let mut bumped_delay = delay_snapshot.clone();
    bumped_delay.undo_all_delay_ms = 2500;
    for updater in &delay_updaters {
        updater(&bumped_delay);
    }
    assert!(
        undo_all_slider_tooltip(&delay_boxes).contains("2.5s"),
        "the persistent updater sets the new delay in place (no subtree rebuild)"
    );

    let mut empty_canvas = regular.clone();
    empty_canvas.canvas_popover_open = true;
    empty_canvas.show_boards_section = false;
    empty_canvas.show_pages_section = false;
    empty_canvas.show_zoom_actions = false;
    empty_canvas.show_actions_advanced = false;
    empty_canvas.show_step_section = false;
    let (empty_content, _empty_updaters) =
        menu_top.build_canvas_popover_content(&empty_canvas, 1.0);
    let empty_panel = find_widget_named(&empty_content, "top.menu.canvas.panel")
        .expect("empty canvas popover panel box");
    let mut empty_buttons: Vec<gtk4::Button> = Vec::new();
    collect_descendants(&empty_panel, &mut empty_buttons);
    assert!(
        empty_buttons.is_empty(),
        "every section toggled off leaves no canvas command buttons"
    );

    detach_test_popovers(&mut menu_top);
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
