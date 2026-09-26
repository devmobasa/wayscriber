//! GTK scenario semantics, islands, widths, and preset assertions.

use super::super::{
    FeedbackSender, Tool, ToolbarLayoutMode, ToolbarSnapshot, TopBar, TopStripPlan, model,
    plan_top_strip,
};
use super::expectations::{
    SemanticAdapterRecord, SemanticControlRecord, expected_semantic_records,
    stroke_controls_scenarios, style_pill_controls, style_pill_selection_snapshot,
    style_pill_tool_snapshot,
};
use super::gtk_style_widgets::assert_gtk_style_widget;
use super::widget_support::{
    assert_accessible_label, collect_descendants, collect_semantic_widgets, detach_test_popovers,
    find_widget_named, first_control_surface,
};
use crate::input::state::test_support::make_test_input_state;
use crate::ui::toolbar::ToolbarBindingHints;
use gtk4::prelude::*;
use std::collections::HashMap;

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

pub(super) fn assert_gtk_control_widget(widget: &gtk4::Widget, expected: &SemanticControlRecord) {
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

pub(super) fn install_gtk_contract_metrics() {
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

pub(super) fn gtk_widget_contract_scenarios() -> (
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
            ("shape-pen-tool", Tool::LiveShape),
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
    scenarios.extend(stroke_controls_scenarios(&regular));
    (regular, highlighted, scenarios)
}

pub(super) fn assert_gtk_widget_scenarios(
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
    let plan = plan_top_strip(&crate::ui_text::UiTextEngine::default(), snapshot);
    let spec = super::super::strip::top_toolbar_spec(snapshot, &plan);
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

pub(super) fn assert_font_button_width_stable(widths: &std::collections::BTreeMap<&str, i32>) {
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

/// A filled slot keeps its number as a corner caption over the drawn face; an
/// empty slot is a muted numbered button.
pub(super) fn assert_preset_slot_faces(regular: &ToolbarSnapshot) {
    let mut snapshot = regular.clone();
    snapshot.presets = vec![None; 5];
    snapshot.presets[0] = Some(crate::ui::toolbar::PresetSlotSnapshot {
        name: None,
        tool: Tool::Pen,
        color: crate::draw::Color::new(1.0, 0.0, 0.0, 1.0),
        size: 4.0,
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
    });
    let plan = TopStripPlan::unconstrained();
    let mut top = build_contract_top(&snapshot, &plan);
    let root: &gtk4::Widget = top.root.upcast_ref();

    let filled = find_widget_named(root, "top.preset.0").expect("filled preset slot");
    let mut labels: Vec<gtk4::Label> = Vec::new();
    collect_descendants(&filled, &mut labels);
    let number = labels
        .iter()
        .find(|label| label.has_css_class("preset-number"))
        .expect("a filled slot keeps its number");
    assert_eq!(number.text(), "1");
    assert!(!filled.has_css_class("empty"));

    let empty = find_widget_named(root, "top.preset.1")
        .expect("empty preset slot")
        .downcast::<gtk4::Button>()
        .expect("empty slot is a button");
    assert_eq!(empty.label().as_deref(), Some("2"));
    assert!(empty.has_css_class("empty"), "an empty slot reads muted");
    assert_eq!(
        empty.tooltip_text().as_deref(),
        Some(
            model::TopToolbarControl::Preset(1)
                .tooltip(&snapshot)
                .as_str()
        )
    );

    detach_test_popovers(&mut top);
}

pub(super) fn assert_compact_gtk_widget_contract(regular: &ToolbarSnapshot) {
    let mut compact_plan = TopStripPlan::unconstrained();
    compact_plan.compact = true;
    let (tx, _rx) = std::sync::mpsc::channel();
    let mut compact_top = TopBar::new_for_test(FeedbackSender::new(tx));
    compact_top.build_strip(regular, &compact_plan);
    let compact_ids = collect_semantic_widgets(compact_top.root.upcast_ref())
        .iter()
        .map(|widget| widget.widget_name().to_string())
        .collect::<Vec<_>>();
    let compact_spec = super::super::strip::top_toolbar_spec(regular, &compact_plan);
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
