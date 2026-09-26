//! GTK style-pill widget assertions.

use super::super::{
    STYLE_PEN_FEEL_W, STYLE_PILL_GAP, STYLE_SLIDER_W, STYLE_VALUE_W, ToolbarSnapshot, model,
};
use super::expectations::slider_opacity_paint;
use super::gtk_level_widgets::{
    assert_gtk_style_meter, assert_gtk_style_segmented, assert_gtk_style_stepper,
};
use super::widget_support::assert_accessible_label;
use gtk4::prelude::*;

pub(super) fn assert_gtk_style_widget(
    widget: &gtk4::Widget,
    control: model::StylePillControl,
    snapshot: &ToolbarSnapshot,
) {
    let id = widget.widget_name().to_string();
    match control.role() {
        model::StylePillRole::Swatch => assert_gtk_style_swatch(widget, control, snapshot, &id),
        model::StylePillRole::Slider => assert_gtk_style_slider(widget, control, snapshot, &id),
        model::StylePillRole::Value => assert_gtk_style_value(widget, control, snapshot, &id),
        model::StylePillRole::Toggle => assert_gtk_style_toggle(widget, control, snapshot, &id),
        model::StylePillRole::Button => assert_gtk_style_button(widget, control, snapshot, &id),
        model::StylePillRole::Stepper => assert_gtk_style_stepper(widget, control, snapshot, &id),
        model::StylePillRole::Meter => assert_gtk_style_meter(widget, control, snapshot, &id),
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

fn assert_gtk_style_slider(
    widget: &gtk4::Widget,
    control: model::StylePillControl,
    snapshot: &ToolbarSnapshot,
    id: &str,
) {
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
    // The marker opacity reads out as a swatch in the label's slot.
    let opacity_paint = slider_opacity_paint(control, snapshot);
    assert_eq!(
        value.property::<bool>("visible"),
        carries_readout && opacity_paint.is_none(),
        "{id} readout visibility"
    );
    let swatch = value.next_sibling();
    assert_eq!(
        swatch
            .as_ref()
            .map(|swatch| swatch.is::<gtk4::DrawingArea>()),
        opacity_paint.map(|_| true),
        "{id} readout swatch"
    );
    if let Some(swatch) = swatch {
        assert_eq!(
            swatch.width_request(),
            -1,
            "{id} swatch sizes by its content"
        );
        assert_eq!(
            swatch
                .downcast::<gtk4::DrawingArea>()
                .expect("swatch area")
                .content_width(),
            STYLE_VALUE_W.round() as i32,
            "{id} swatch fills the readout slot"
        );
    }
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
    if carries_readout && opacity_paint.is_none() {
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
    // label. The arrow chip lays its glyph and name out as a child instead.
    let expected_text = match control {
        model::StylePillControl::SelectionCycle(_) => {
            Some(control.value_text(snapshot).expect("cycle value text"))
        }
        model::StylePillControl::ArrowStyleChip => None,
        _ => Some(control.label(snapshot).into_owned()),
    };
    assert_eq!(
        button.label().as_deref(),
        expected_text.as_deref(),
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
    if control == model::StylePillControl::PenFeelChip {
        assert_gtk_pen_feel_chip(&button, control, snapshot, id);
    }
    if control == model::StylePillControl::ArrowStyleChip {
        assert_gtk_arrow_style_chip(&button, control, snapshot, id);
    }
}

/// The chip keeps its planned slot with the glyph and the style's name
/// inside it, reads as pressed while its menu is open, and names the style
/// for assistive tech.
fn assert_gtk_arrow_style_chip(
    button: &gtk4::Button,
    control: model::StylePillControl,
    snapshot: &ToolbarSnapshot,
    id: &str,
) {
    let slot = (model::ARROW_STYLE_CHIP_W).round() as i32;
    assert_eq!(button.width_request(), slot, "{id} keeps the planned slot");
    let content = button.child().expect("chip content");
    let content_w = content.measure(gtk4::Orientation::Horizontal, -1).1 + content.margin_start();
    assert!(content_w <= slot, "{id} content is {content_w}px wide");
    let glyph = content.first_child().expect("chip glyph");
    assert!(glyph.is::<gtk4::DrawingArea>(), "{id} glyph");
    let label = glyph
        .next_sibling()
        .and_then(|label| label.downcast::<gtk4::Label>().ok())
        .expect("chip label");
    assert_eq!(
        label.label(),
        model::arrow_style_chip_label(snapshot.arrow_style),
        "{id} label"
    );
    assert_eq!(
        button.has_css_class("active"),
        control.active(snapshot),
        "{id} pressed while open"
    );
    assert_accessible_label(
        button.upcast_ref(),
        &format!("Arrow style: {}", snapshot.arrow_style.label()),
        id,
    );
}

/// The chip keeps the slot the planner budgets, reads as pressed while its
/// panel is open, and names the current levels for assistive tech.
fn assert_gtk_pen_feel_chip(
    button: &gtk4::Button,
    control: model::StylePillControl,
    snapshot: &ToolbarSnapshot,
    id: &str,
) {
    assert_eq!(
        button.width_request(),
        STYLE_PEN_FEEL_W.round() as i32,
        "{id} keeps the planned slot"
    );
    // A size request is a minimum: a label wider than the slot would grow the
    // button past the arrangement the planner declared fits.
    let label_w = button
        .child()
        .expect("chip label")
        .measure(gtk4::Orientation::Horizontal, -1)
        .1;
    assert!(
        label_w <= STYLE_PEN_FEEL_W.round() as i32,
        "{id} label is {label_w}px wide"
    );
    assert_eq!(
        button.has_css_class("active"),
        control.active(snapshot),
        "{id} pressed while open"
    );
    assert_accessible_label(
        button.upcast_ref(),
        &model::StylePillControl::pen_feel_accessible_label(snapshot),
        id,
    );
}
