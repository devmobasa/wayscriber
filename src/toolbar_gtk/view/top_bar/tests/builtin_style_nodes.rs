//! Built-in style-pill node assertions.

use super::super::{ToolbarEvent, ToolbarSnapshot, model};
use super::builtin_contract::StylePillNodeExpectation;
use super::expectations::slider_opacity_paint;

#[allow(clippy::too_many_arguments)]
pub(super) fn assert_builtin_style_pill_node(
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
        StylePillNodeExpectation::Caption(control) => {
            assert_builtin_style_pill_caption(name, kind, has_interaction, id, *control)
        }
        StylePillNodeExpectation::MeterBar(control, index) => assert_builtin_style_pill_meter_bar(
            name,
            snapshot,
            kind,
            interaction_event,
            interaction_tooltip,
            id,
            *control,
            *index,
        ),
        StylePillNodeExpectation::ArrowChipGlyph => {
            use crate::backend::wayland::TopToolbarWidgetKind as W;
            assert!(!has_interaction, "{name}: {id} is decor");
            assert_eq!(
                kind,
                &W::ArrowStylePreview {
                    style: snapshot.arrow_style
                },
                "{name}: {id}"
            );
        }
        StylePillNodeExpectation::ArrowChipLabel => {
            use crate::backend::wayland::TopToolbarWidgetKind as W;
            assert!(!has_interaction, "{name}: {id} is decor");
            assert!(
                matches!(
                    kind,
                    W::Label(label)
                        if label.text == model::arrow_style_chip_label(snapshot.arrow_style)
                ),
                "{name}: {id} kind {kind:?}"
            );
        }
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
            assert_builtin_style_pill_slider(name, snapshot, *t, None, id, control);
        }
        (model::StylePillRole::Slider, W::OpacitySlider { t, paint }) => {
            assert_builtin_style_pill_slider(name, snapshot, *t, Some(*paint), id, control);
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
                model::StylePillControl::SelectionCycle(_) => {
                    control.value_text(snapshot).expect("cycle value text")
                }
                // The chip's glyph and name are decor nodes laid over it.
                model::StylePillControl::ArrowStyleChip => String::new(),
                _ => control.label(snapshot).into_owned(),
            };
            assert_eq!(label.text, expected_text, "{name}: {id} text");
            assert_eq!(
                style.disabled,
                !control.enabled(snapshot),
                "{name}: {id} disabled style"
            );
            assert_eq!(
                style.active,
                control.active(snapshot),
                "{name}: {id} active style"
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
        W::OpacitySwatch { paint } => assert_eq!(
            Some(*paint),
            slider_opacity_paint(control, snapshot),
            "{name}: {id} readout swatch"
        ),
        other => panic!("{name}: {id} readout kind {other:?}"),
    }
}

/// A builtin slider sits at the model's position and paints the model's
/// opacity track exactly when the model has one.
fn assert_builtin_style_pill_slider(
    name: &str,
    snapshot: &ToolbarSnapshot,
    t: f64,
    paint: Option<model::OpacityPaint>,
    id: &str,
    control: model::StylePillControl,
) {
    let (spec, value) = control.slider(snapshot).expect("slider spec");
    assert!(
        (t - spec.t_from_value(value)).abs() < 1e-9,
        "{name}: {id} slider position"
    );
    assert_eq!(
        paint,
        slider_opacity_paint(control, snapshot),
        "{name}: {id} track paint"
    );
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
fn assert_builtin_style_pill_meter_bar(
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

    let meter = control.meter(snapshot).expect("level meter");
    let segment = &meter.segments[index];
    let enabled = control.enabled(snapshot);
    match kind {
        W::MeterBar {
            filled,
            enabled: painted_enabled,
        } => {
            assert_eq!(*filled, segment.filled, "{name}: {id} bar fill");
            assert_eq!(*painted_enabled, enabled, "{name}: {id} bar enabled");
        }
        other => panic!("{name}: {id} meter bar kind {other:?}"),
    }
    assert_eq!(
        interaction_event,
        enabled.then_some(&segment.event),
        "{name}: {id} bar event"
    );
    assert_eq!(
        interaction_tooltip,
        enabled.then_some(segment.tooltip.as_str()),
        "{name}: {id} bar tooltip"
    );
}

fn assert_builtin_style_pill_caption(
    name: &str,
    kind: &crate::backend::wayland::TopToolbarWidgetKind,
    has_interaction: bool,
    id: &str,
    control: model::StylePillControl,
) {
    use crate::backend::wayland::TopToolbarWidgetKind as W;

    assert!(!has_interaction, "{name}: {id} caption is decor");
    match kind {
        W::Label(label) => {
            assert_eq!(
                Some(label.text.as_str()),
                control.caption(),
                "{name}: {id} caption text"
            );
            assert!(label.caption, "{name}: {id} caption tone");
        }
        other => panic!("{name}: {id} caption kind {other:?}"),
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
