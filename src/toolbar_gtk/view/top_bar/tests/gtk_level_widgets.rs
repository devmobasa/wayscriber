//! GTK meter, stepper, and segmented-widget assertions.

use super::super::{STYLE_CAPTION_W, STYLE_METER_W, ToolbarSnapshot, model};
use super::widget_support::assert_accessible_label;
use gtk4::prelude::*;

pub(super) fn assert_gtk_style_meter(
    widget: &gtk4::Widget,
    control: model::StylePillControl,
    snapshot: &ToolbarSnapshot,
    id: &str,
) {
    let meter = control.meter(snapshot).expect("level meter");
    let row = widget
        .clone()
        .downcast::<gtk4::Box>()
        .unwrap_or_else(|_| panic!("{id} is a meter row"));
    // The builtin lays the caption and bars out abutting and the width
    // planner budgets caption + bar row exactly.
    assert_eq!(row.spacing(), 0, "{id} meter spacing");
    assert_accessible_label(widget, &control.label(snapshot), id);

    let caption = control.caption().expect("meters carry a caption");
    let caption_label = widget
        .first_child()
        .expect("meter caption")
        .downcast::<gtk4::Label>()
        .unwrap_or_else(|_| panic!("{id} caption is a label"));
    assert_eq!(caption_label.text(), caption, "{id} caption text");
    assert_eq!(
        caption_label.widget_name().as_str(),
        format!("{}.caption", control.id()),
        "{id} caption id"
    );
    assert_eq!(
        caption_label.width_request(),
        STYLE_CAPTION_W.round() as i32,
        "{id} caption keeps the planned slot"
    );

    let mut bar = caption_label.next_sibling();
    let mut bar_widths = 0;
    for segment in &meter.segments {
        let button = bar
            .clone()
            .unwrap_or_else(|| panic!("{} exists", segment.id))
            .downcast::<gtk4::Button>()
            .unwrap_or_else(|_| panic!("{} is a button", segment.id));
        assert_eq!(button.widget_name().as_str(), segment.id, "{id} bar id");
        assert!(button.has_css_class("meter-bar"), "{} class", segment.id);
        assert_eq!(
            button.has_css_class("filled"),
            segment.filled,
            "{} fill",
            segment.id
        );
        assert_eq!(
            button.tooltip_text().as_deref(),
            Some(segment.tooltip.as_str()),
            "{} tooltip",
            segment.id
        );
        assert_accessible_label(button.upcast_ref(), &segment.tooltip, &segment.id);
        assert_eq!(
            button.is_sensitive(),
            control.enabled(snapshot),
            "{} enabled",
            segment.id
        );
        bar_widths += button.width_request();
        bar = button.next_sibling();
    }
    assert!(bar.is_none(), "{id} has one bar per level");
    assert!(
        (bar_widths - STYLE_METER_W.round() as i32).abs() <= meter.segments.len() as i32,
        "{id} bars fill the planned meter row: {bar_widths}"
    );
}

pub(super) fn assert_gtk_style_stepper(
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
    // The builtin lays the parts out abutting and the width planner budgets
    // caption + step + value + step exactly.
    assert_eq!(row.spacing(), 0, "{id} stepper spacing");
    assert_accessible_label(widget, &control.label(snapshot), id);
    let mut first = widget.first_child().expect("stepper first child");
    if let Some(caption) = control.caption() {
        let caption_label = first
            .clone()
            .downcast::<gtk4::Label>()
            .unwrap_or_else(|_| panic!("{id} caption is a label"));
        assert_eq!(caption_label.text(), caption, "{id} caption text");
        assert!(
            caption_label.has_css_class("stepper-caption"),
            "{id} caption tone"
        );
        assert_eq!(
            caption_label.widget_name().as_str(),
            format!("{}.caption", control.id()),
            "{id} caption id"
        );
        assert_eq!(
            caption_label.width_request(),
            STYLE_CAPTION_W.round() as i32,
            "{id} caption keeps the planned slot"
        );
        first = caption_label.next_sibling().expect("stepper minus half");
    }
    let minus = first;
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
    assert!(
        value_label.has_css_class("stepper-value"),
        "{id} readout uses the primary foreground"
    );
}

pub(super) fn assert_gtk_style_segmented(
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
