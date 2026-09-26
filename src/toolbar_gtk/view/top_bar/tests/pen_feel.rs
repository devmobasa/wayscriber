//! Pen feel panel and delayed-snapshot meter-scroll assertions.

use super::super::{
    FeedbackSender, GtkToolbarFeedback, STYLE_METER_W, STYLE_ROW_H, Tool, ToolbarEvent,
    ToolbarSnapshot, TopBar, model, plan_top_strip,
};
use super::expectations::style_pill_tool_snapshot;
use super::widget_support::{
    assert_accessible_label, detach_test_popovers, find_widget_named,
    has_capture_phase_click_gesture,
};
use crate::input::state::test_support::make_test_input_state;
use gtk4::prelude::*;
use std::time::Duration;

pub(super) fn assert_meter_scroll_survives_delayed_snapshots() {
    for setting in [
        model::StrokeSetting::Smoothing,
        model::StrokeSetting::ShapeDetection,
    ] {
        let mut state = make_test_input_state();
        state.set_pen_smoothing(1);
        state.set_shape_recognition_sensitivity(1);
        let snapshot = ToolbarSnapshot::from_input(&state);
        let (tx, rx) = std::sync::mpsc::channel();
        let row = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
        let refresh = super::super::meter::append_meter_bars(
            &FeedbackSender::new(tx),
            &row,
            super::super::meter::MeterBarsSpec {
                setting,
                id_prefix: "test.meter".to_string(),
                size: (STYLE_METER_W, STYLE_ROW_H),
            },
            |_| true,
            &snapshot,
            1.0,
        );
        let controllers = row.observe_controllers();
        let wheel = (0..controllers.n_items())
            .find_map(|index| {
                controllers
                    .item(index)?
                    .downcast::<gtk4::EventControllerScroll>()
                    .ok()
            })
            .expect("meter scroll controller");
        let scroll = |state: &mut crate::input::InputState, dy: f64, expected: u8| {
            assert!(wheel.emit_by_name::<bool>("scroll", &[&0.0f64, &dy]));
            let GtkToolbarFeedback::Event { event, .. } = rx
                .recv_timeout(Duration::from_secs(1))
                .expect("meter wheel event")
            else {
                panic!("wheel sends a toolbar event");
            };
            state.apply_toolbar_event(event);
            assert_eq!(
                setting.level(&ToolbarSnapshot::from_input(state)),
                expected,
                "{setting:?} dy={dy}"
            );
        };

        scroll(&mut state, -2.0, 3);
        let delayed = ToolbarSnapshot::from_input(&state);
        scroll(&mut state, -1.0, 4);
        refresh(&delayed);
        scroll(&mut state, 1.0, 3);
        scroll(&mut state, -20.0, setting.max());
        scroll(&mut state, -1.0, setting.max());
        scroll(&mut state, 20.0, 0);
        scroll(&mut state, 1.0, 0);
    }
}

fn named<W: IsA<gtk4::Widget>>(root: &impl IsA<gtk4::Widget>, id: &str) -> W {
    find_widget_named(root.as_ref(), id)
        .unwrap_or_else(|| panic!("{id} exists"))
        .downcast::<W>()
        .unwrap_or_else(|_| panic!("{id} widget type"))
}

/// The Pen feel chip toggles a panel popover built from the shared
/// sections: a meter per setting the tool uses (its bars send the shared
/// level events), the live preview, and the level's name and hint, which
/// the panel's updaters keep current while it stays open.
pub(super) fn assert_pen_feel_contract(regular: &ToolbarSnapshot) {
    let (tx, rx) = std::sync::mpsc::channel();
    let mut top = TopBar::new_for_test(FeedbackSender::new(tx));
    let mut shape_pen = style_pill_tool_snapshot(regular, Tool::LiveShape);
    shape_pen.stroke_controls = crate::config::ToolbarStrokeControls::Panel;
    shape_pen.pen_smoothing = 3;
    shape_pen.shape_recognition_sensitivity = 3;
    top.build_strip(
        &shape_pen,
        &plan_top_strip(&crate::ui_text::UiTextEngine::default(), &shape_pen),
    );
    assert!(top.feel.mounted.is_some(), "the chip carries its panel");
    assert_pen_feel_popover_relays_keys(&top, &rx);

    let chip: gtk4::Button = named(&top.root, "top.style.pen-feel");
    for (open, event) in [(false, true), (true, false)] {
        top.feel.expected_open.set(open);
        chip.emit_clicked();
        assert_eq!(
            rx.recv_timeout(Duration::from_secs(1)).expect("chip event"),
            GtkToolbarFeedback::Event {
                event: ToolbarEvent::TogglePenFeelPanel(event),
                rebind_requested: false,
            }
        );
    }
    top.feel.expected_open.set(false);

    let mut open = shape_pen.clone();
    open.pen_feel_open = true;
    let (content, updaters) = top.build_pen_feel_content(&open, 1.0);
    assert!(
        has_capture_phase_click_gesture(content.upcast_ref()),
        "the panel captures click modifiers"
    );
    assert_eq!(
        named::<gtk4::Label>(&content, "top.feel.title").text(),
        "Pen feel"
    );
    for section in model::pen_feel_sections(&open) {
        assert_pen_feel_section(&content, &section, &rx);
    }

    // Levels change while the panel stays open; nothing rebuilds.
    let mut changed = open.clone();
    changed.pen_smoothing = crate::draw::MAX_PEN_SMOOTHING;
    changed.shape_recognition_sensitivity = 0;
    for updater in &updaters {
        updater(&changed);
    }
    for (id, text) in [
        ("top.feel.smoothing.value", "Maximum"),
        ("top.feel.smoothing.hint", "Smoothest; tight corners soften"),
        ("top.feel.detection.value", "Precise"),
        (
            "top.feel.detection.hint",
            "Only near-perfect strokes become shapes",
        ),
    ] {
        assert_eq!(named::<gtk4::Label>(&content, id).text(), text, "{id}");
    }
    for bar in 1..=crate::draw::MAX_PEN_SMOOTHING {
        let id = format!("top.feel.smoothing.level-{bar}");
        assert!(named::<gtk4::Button>(&content, &id).has_css_class("filled"));
    }
    assert!(!named::<gtk4::Button>(&content, "top.feel.detection.level-1").has_css_class("filled"));

    // Pen has only the smoothing section.
    let mut pen = style_pill_tool_snapshot(regular, Tool::Pen);
    pen.pen_feel_open = true;
    let (pen_content, _) = top.build_pen_feel_content(&pen, 1.0);
    assert!(find_widget_named(pen_content.upcast_ref(), "top.feel.smoothing.preview").is_some());
    assert!(find_widget_named(pen_content.upcast_ref(), "top.feel.detection.label").is_none());

    detach_test_popovers(&mut top);
}

/// Keys that reach the open panel go to the overlay like the other menus':
/// Escape closes it, and a shortcut closes it and runs. Switching input on
/// every snapshot leaves the popover targetable.
fn assert_pen_feel_popover_relays_keys(
    top: &TopBar,
    rx: &std::sync::mpsc::Receiver<GtkToolbarFeedback>,
) {
    use crate::toolbar_gtk::widgets::key_relay_controller;
    use gtk4::glib::translate::IntoGlib;

    let resources = top.feel.mounted.as_ref().expect("Pen feel popover");
    for _ in 0..2 {
        resources.set_capture_transparent(false);
    }
    assert!(resources.popover.can_target(), "the panel takes input");

    let relay = key_relay_controller(&resources.popover).expect("the panel relays keys");
    for keyval in [gtk4::gdk::Key::Escape, gtk4::gdk::Key::s] {
        let handled = relay.emit_by_name::<bool>(
            "key-pressed",
            &[&keyval, &0u32, &gtk4::gdk::ModifierType::empty()],
        );
        assert!(handled, "{keyval:?} leaves the panel for the overlay");
        assert_eq!(
            rx.recv_timeout(Duration::from_secs(1))
                .expect("relayed key feedback"),
            GtkToolbarFeedback::Key {
                keyval: keyval.into_glib(),
                ctrl: false,
                shift: false,
                alt: false,
                logo: false,
            }
        );
    }
}

fn assert_pen_feel_section(
    content: &gtk4::Box,
    section: &model::PenFeelSection,
    rx: &std::sync::mpsc::Receiver<GtkToolbarFeedback>,
) {
    let key = section.setting.panel_key();
    assert_eq!(
        named::<gtk4::Label>(content, &section.id("label")).text(),
        section.setting.name(),
        "{key} name"
    );
    assert_eq!(
        named::<gtk4::Label>(content, &section.id("value")).text(),
        section.level_name,
        "{key} level"
    );
    assert_eq!(
        named::<gtk4::Label>(content, &section.id("hint")).text(),
        section.hint,
        "{key} hint"
    );

    let mut widths = 0;
    for segment in &section.meter.segments {
        let bar: gtk4::Button = named(content, &segment.id);
        assert!(bar.has_css_class("meter-bar"), "{}", segment.id);
        assert_eq!(
            bar.has_css_class("filled"),
            segment.filled,
            "{}",
            segment.id
        );
        assert_eq!(
            bar.tooltip_text().as_deref(),
            Some(segment.tooltip.as_str())
        );
        assert_accessible_label(bar.upcast_ref(), &segment.tooltip, &segment.id);
        widths += bar.width_request();
        bar.emit_clicked();
        assert_eq!(
            rx.recv_timeout(Duration::from_secs(1))
                .expect("panel bar event"),
            GtkToolbarFeedback::Event {
                event: segment.event.clone(),
                rebind_requested: false,
            },
            "{}",
            segment.id
        );
    }
    let column = model::PEN_FEEL_CONTENT_W.round() as i32;
    assert!(
        (widths - column).abs() <= section.meter.segments.len() as i32,
        "{key} bars span the column: {widths}"
    );

    let preview = find_widget_named(content.upcast_ref(), &section.id("preview"));
    assert_eq!(preview.is_some(), section.has_preview(), "{key} preview");
    if let Some(preview) = preview {
        let area = preview
            .downcast::<gtk4::DrawingArea>()
            .expect("the preview is a drawing area");
        assert_eq!(area.content_width(), column);
        assert_eq!(
            area.content_height(),
            model::PEN_FEEL_PREVIEW_H.round() as i32
        );
    }
}
