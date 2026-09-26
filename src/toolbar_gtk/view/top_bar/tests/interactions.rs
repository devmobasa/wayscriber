//! Toolbar feedback and style-pill interaction tests.

use super::super::{
    FeedbackSender, GtkToolbarFeedback, ICON_BUTTON, ICON_SIZE, PIN_BUTTON_SIZE, Tool,
    ToolbarEvent, ToolbarSnapshot, TopBar, model, plan_top_strip,
};
use super::expectations::style_pill_tool_snapshot;
use super::widget_support::{collect_semantic_widgets, detach_test_popovers, find_widget_named};
use crate::config::toolbar_item_ids as ids;
use crate::toolbar_gtk::widgets::{emit_secondary_press, secondary_click_gesture};
use gtk4::prelude::*;
use std::time::Duration;

pub(super) fn assert_gtk_toggle_events(regular: &ToolbarSnapshot, highlighted: &ToolbarSnapshot) {
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
    top.shapes.expected_open.set(true);
    top.overflow.expected_open.set(true);
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
    top.shapes.expected_open.set(false);
    top.overflow.expected_open.set(false);
    detach_test_popovers(&mut top);
    assert_highlight_ring_event(&mut top, highlighted, &rx);
}

fn assert_test_popover_capture_surfaces(top: &TopBar) {
    for (popover, capture_surface) in [
        (
            top.shapes
                .mounted
                .as_ref()
                .map(|resources| &resources.popover)
                .unwrap(),
            top.shapes
                .mounted
                .as_ref()
                .map(|resources| &resources.capture_surface)
                .unwrap(),
        ),
        (
            top.overflow
                .mounted
                .as_ref()
                .map(|resources| &resources.popover)
                .unwrap(),
            top.overflow
                .mounted
                .as_ref()
                .map(|resources| &resources.capture_surface)
                .unwrap(),
        ),
    ] {
        let content = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
        capture_surface.set_content(&content);
        super::super::popovers::set_popover_capture_transparent(
            popover,
            capture_surface,
            true,
            false,
        );
        assert!(popover.has_css_class(crate::toolbar_gtk::css::CAPTURE_TRANSPARENT_CLASS));
        assert!(!popover.can_target());
        assert_eq!(capture_surface.content_opacity(), Some(0.0));
        assert!(capture_surface.proof_visible());
        super::super::popovers::set_popover_capture_transparent(
            popover,
            capture_surface,
            false,
            true,
        );
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
    top.build_strip(
        highlighted,
        &plan_top_strip(&crate::ui_text::UiTextEngine::default(), highlighted),
    );
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

pub(super) fn assert_style_pill_interactions(regular: &ToolbarSnapshot) {
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
    top.build_strip(
        &eraser,
        &plan_top_strip(&crate::ui_text::UiTextEngine::default(), &eraser),
    );
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
    top.build_strip(
        &pen,
        &plan_top_strip(&crate::ui_text::UiTextEngine::default(), &pen),
    );
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
    top.build_strip(
        &shape,
        &plan_top_strip(&crate::ui_text::UiTextEngine::default(), &shape),
    );
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
fn gtk_stateful_toggle_adapter_emits_the_requested_live_state() {
    use super::super::controls::event_for_toggle_state;

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
