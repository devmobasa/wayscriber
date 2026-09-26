//! Shapes, overflow, layout, arrow-style, and key-relay assertions.

use super::super::{
    FeedbackSender, GtkToolbarFeedback, ICON_BUTTON, ICON_SIZE, Tool, ToolbarEvent,
    ToolbarSnapshot, TopBar, TopStripPlan, model, plan_top_strip,
};
use super::expectations::{SemanticLane, control_record, style_pill_tool_snapshot};
use super::gtk_contract::assert_gtk_control_widget;
use super::widget_support::{
    collect_semantic_widgets, detach_test_popovers, find_widget_named, first_control_surface,
    has_capture_phase_click_gesture,
};
use crate::config::toolbar_item_ids as ids;
use gtk4::prelude::*;
use std::time::Duration;

pub(super) fn assert_shapes_and_overflow_contract(regular: &ToolbarSnapshot) {
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
    let tools = model::visible_shape_picker_rows(&shapes, shapes.layout_mode)
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
    let spec = super::super::strip::top_toolbar_spec(regular, &plan);
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

/// The layout button opens a preset menu built from the shared entries; a
/// row applies its preset. No click cycles presets any more.
pub(super) fn assert_layout_menu_contract(regular: &ToolbarSnapshot) {
    let (tx, rx) = std::sync::mpsc::channel();
    let mut top = TopBar::new_for_test(FeedbackSender::new(tx));
    top.build_strip(
        regular,
        &plan_top_strip(&crate::ui_text::UiTextEngine::default(), regular),
    );
    assert!(
        top.layout.mounted.is_some(),
        "the layout menu popover exists"
    );

    let button = find_widget_named(top.root.upcast_ref(), ids::TOP_CHROME_LAYOUT.as_str())
        .and_then(|widget| widget.downcast::<gtk4::Button>().ok())
        .expect("layout chrome button");
    button.emit_clicked();
    assert_eq!(
        rx.recv_timeout(Duration::from_secs(1))
            .expect("layout button event"),
        GtkToolbarFeedback::Event {
            event: ToolbarEvent::ToggleLayoutMenu(true),
            rebind_requested: false,
        }
    );

    let mut open = regular.clone();
    open.layout_menu_open = true;
    let content = top.build_layout_menu_content(&open, 1.0);
    for entry in model::layout_menu_entries(open.layout_mode) {
        let key = model::layout_mode_label(entry.mode).to_ascii_lowercase();
        let row = find_widget_named(content.upcast_ref(), &format!("top.layout.{key}"))
            .and_then(|widget| widget.downcast::<gtk4::Button>().ok())
            .unwrap_or_else(|| panic!("{key} row"));
        assert_eq!(row.has_css_class("active"), entry.current, "{key} mark");
        row.emit_clicked();
        assert_eq!(
            rx.recv_timeout(Duration::from_secs(1))
                .expect("layout row event"),
            GtkToolbarFeedback::Event {
                event: ToolbarEvent::SetToolbarLayoutMode(entry.mode),
                rebind_requested: false,
            }
        );
    }

    detach_test_popovers(&mut top);
}

/// The arrow chip opens a menu built from the shared entries; a row sets its
/// style. The chip no longer cycles.
pub(super) fn assert_arrow_style_menu_contract(regular: &ToolbarSnapshot) {
    let arrow = style_pill_tool_snapshot(regular, Tool::Arrow);
    let (tx, rx) = std::sync::mpsc::channel();
    let mut top = TopBar::new_for_test(FeedbackSender::new(tx));
    top.build_strip(
        &arrow,
        &plan_top_strip(&crate::ui_text::UiTextEngine::default(), &arrow),
    );
    assert!(
        top.arrow_style.mounted.is_some(),
        "the arrow style menu popover exists"
    );

    let chip = find_widget_named(top.root.upcast_ref(), "top.style.arrow-style")
        .and_then(|widget| widget.downcast::<gtk4::Button>().ok())
        .expect("arrow style chip");
    chip.emit_clicked();
    assert_eq!(
        rx.recv_timeout(Duration::from_secs(1))
            .expect("arrow chip event"),
        GtkToolbarFeedback::Event {
            event: ToolbarEvent::ToggleArrowStyleMenu(true),
            rebind_requested: false,
        }
    );

    let mut open = arrow.clone();
    open.arrow_style = crate::draw::ArrowStyle::Double;
    open.arrow_style_menu_open = true;
    let content = top.build_arrow_style_menu_content(&open, 1.0);
    for entry in model::arrow_style_menu_entries(open.arrow_style) {
        let id = entry.id();
        let row = find_widget_named(content.upcast_ref(), &id)
            .and_then(|widget| widget.downcast::<gtk4::Button>().ok())
            .unwrap_or_else(|| panic!("{id} row"));
        assert_eq!(row.has_css_class("active"), entry.current, "{id} mark");
        assert_eq!(
            row.tooltip_text().as_deref(),
            Some(entry.tooltip().as_str()),
            "{id} tooltip"
        );
        assert!(
            find_widget_named(row.upcast_ref(), &format!("{id}.preview"))
                .is_some_and(|preview| preview.is::<gtk4::DrawingArea>()),
            "{id} draws its preview"
        );
        row.emit_clicked();
        assert_eq!(
            rx.recv_timeout(Duration::from_secs(1))
                .expect("arrow row event"),
            GtkToolbarFeedback::Event {
                event: ToolbarEvent::SetArrowStyle(entry.style),
                rebind_requested: false,
            }
        );
    }

    detach_test_popovers(&mut top);
}

/// Keys typed while the toolbar holds keyboard focus used to vanish, and
/// Escape could not close a popover. The window and every mounted popover
/// relay presses to the overlay, which routes them like its own keys.
pub(super) fn assert_key_relay_contract(regular: &ToolbarSnapshot) {
    use crate::toolbar_gtk::widgets::key_relay_controller;
    use gtk4::glib::translate::IntoGlib;

    let (tx, rx) = std::sync::mpsc::channel();
    let mut top = TopBar::new_for_test(FeedbackSender::new(tx));
    top.build_strip(
        regular,
        &plan_top_strip(&crate::ui_text::UiTextEngine::default(), regular),
    );

    let window_relay = key_relay_controller(&top.window).expect("toolbar window relays keys");
    for (name, resources) in top.popover_resources() {
        assert!(
            key_relay_controller(&resources.popover).is_some(),
            "{name} relays keys"
        );
    }

    for (keyval, state, expected_shift) in [
        (
            gtk4::gdk::Key::Escape,
            gtk4::gdk::ModifierType::empty(),
            false,
        ),
        (gtk4::gdk::Key::S, gtk4::gdk::ModifierType::SHIFT_MASK, true),
    ] {
        let handled = window_relay.emit_by_name::<bool>("key-pressed", &[&keyval, &0u32, &state]);
        assert!(handled, "{keyval:?} is consumed by the relay");
        assert_eq!(
            rx.recv_timeout(Duration::from_secs(1))
                .expect("relayed key feedback"),
            GtkToolbarFeedback::Key {
                keyval: keyval.into_glib(),
                ctrl: false,
                shift: expected_shift,
                alt: false,
                logo: false,
            }
        );
    }

    let handled = window_relay.emit_by_name::<bool>(
        "key-pressed",
        &[
            &gtk4::gdk::Key::Tab,
            &0u32,
            &gtk4::gdk::ModifierType::empty(),
        ],
    );
    assert!(!handled, "Tab stays with GTK focus navigation");
    assert!(rx.try_recv().is_err(), "Tab is not relayed");

    detach_test_popovers(&mut top);
}
