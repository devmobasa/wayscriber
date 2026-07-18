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
    assert!(plan.compact || plan.show_overflow || plan.swatch_count < 8);
    assert!(top_default_width(&snapshot) <= 700);
}

#[test]
fn compact_adapter_keeps_quick_color_shortcut_badges() {
    let state = make_test_input_state();
    let snapshot = ToolbarSnapshot::from_input_with_bindings(
        &state,
        ToolbarBindingHints::from_input_state(&state),
    );
    let mut plan = TopStripPlan::unconstrained();
    plan.compact = true;
    plan.swatch_count = 2;
    let spec = super::strip::top_toolbar_spec(&snapshot, &plan);

    assert!(super::strip::quick_color_badge_row_visible(
        &snapshot, &spec
    ));
    assert!(spec.strip().iter().any(|node| {
        matches!(
            node,
            model::TopToolbarNode::Control(model::TopToolbarControl::QuickColor(index))
                if model::TopToolbarControl::QuickColor(*index)
                    .shortcut_badge(&snapshot)
                    .is_some()
        )
    }));
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
                let show_badge =
                    matches!(control, model::TopToolbarControl::QuickColor(_)) || !plan.compact;
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

fn expected_main_widget_ids(spec: &model::TopToolbarSpec) -> Vec<String> {
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
    } else if expected.role != model::TopToolbarControlRole::Swatch {
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

fn detach_test_popovers(top: &mut TopBar) {
    if let Some(popover) = top.shapes_popover.take() {
        popover.unparent();
    }
    top.shapes_capture_surface = None;
    if let Some(popover) = top.overflow_popover.take() {
        popover.unparent();
    }
    top.overflow_capture_surface = None;
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
    let mut text = regular.clone();
    text.use_icons = false;
    let mut highlighted = regular.clone();
    highlighted.highlight_tool_active = true;
    let mut narrow = regular.clone();
    narrow.top_viewport_max = Some(520.0);

    for (name, snapshot) in [
        ("regular", regular.clone()),
        ("simple", simple),
        ("minimized", minimized),
        ("text", text),
        ("highlighted", highlighted.clone()),
        ("narrow", narrow),
    ] {
        let plan = plan_top_strip(&snapshot);
        let spec = super::strip::top_toolbar_spec(&snapshot, &plan);
        let expected = expected_semantic_records(&snapshot, &spec, &plan);
        let (tx, _rx) = std::sync::mpsc::channel();
        let mut top = TopBar::new_for_test(FeedbackSender::new(tx));
        if snapshot.top_minimized {
            top.build_minimized(&snapshot, &plan);
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
            expected_main_widget_ids(&spec),
            "{name} GTK widget order"
        );
        for widget in widgets {
            let id = widget.widget_name();
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
    let mut compact_plan = TopStripPlan::unconstrained();
    compact_plan.compact = true;
    compact_plan.swatch_count = 2;
    let compact_spec = super::strip::top_toolbar_spec(&regular, &compact_plan);
    let compact_expected = expected_semantic_records(&regular, &compact_spec, &compact_plan);
    let (tx, _rx) = std::sync::mpsc::channel();
    let mut compact_top = TopBar::new_for_test(FeedbackSender::new(tx));
    compact_top.build_strip(&regular, &compact_plan);
    let compact_widgets = collect_semantic_widgets(compact_top.root.upcast_ref());
    for control in compact_expected.iter().filter_map(|record| match record {
        SemanticAdapterRecord::Control(control) if control.id.starts_with("top.quick-color.") => {
            Some(control)
        }
        _ => None,
    }) {
        let widget = compact_widgets
            .iter()
            .find(|widget| widget.widget_name() == control.id)
            .expect("compact quick-color widget");
        assert_gtk_control_widget(widget, control);
    }
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
    overflow_plan.show_overflow = true;
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
        PIN_BUTTON_SIZE,
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
        super::popovers::set_popover_capture_transparent(popover, capture_surface, true, true);
        assert!(
            popover.has_css_class(crate::toolbar_gtk::css::CAPTURE_TRANSPARENT_CLASS),
            "capture suppression must clear native popover chrome"
        );
        assert!(
            popover.can_target(),
            "input stays enabled until transparent presentation is confirmed"
        );
        assert_eq!(capture_surface.content_opacity(), Some(0.0));
        assert!(capture_surface.proof_visible());

        super::popovers::set_popover_capture_transparent(popover, capture_surface, true, false);
        assert!(!popover.can_target());

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
