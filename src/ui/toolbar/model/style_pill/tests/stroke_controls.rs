//! `[ui.toolbar] stroke_controls`: how the pill shows pen smoothing and Shape
//! Pen detection. The Pen feel chip is the default; meters and steppers put
//! one inline control per setting in the pill.

use super::*;
use crate::config::ToolbarStrokeControls;

const STYLES: [ToolbarStrokeControls; 3] = ToolbarStrokeControls::ALL;

fn styled(tool: Tool, style: ToolbarStrokeControls) -> ToolbarSnapshot {
    let mut snapshot = snapshot_for_tool(tool);
    snapshot.stroke_controls = style;
    snapshot
}

/// The pill's stroke-feel controls, in pill order.
fn stroke_feel_controls(snapshot: &ToolbarSnapshot, plan: &TopStripPlan) -> Vec<StylePillControl> {
    StylePillSpec::build(snapshot, plan)
        .controls()
        .iter()
        .copied()
        .filter(|control| {
            matches!(
                control,
                StylePillControl::PenFeelChip
                    | StylePillControl::PenSmoothingMeter
                    | StylePillControl::ShapeSensitivityMeter
                    | StylePillControl::PenSmoothingStepper
                    | StylePillControl::ShapeSensitivityStepper
            )
        })
        .collect()
}

#[test]
fn the_configured_style_reaches_the_snapshot_and_defaults_to_the_panel() {
    let mut state = make_test_input_state();
    assert_eq!(
        ToolbarSnapshot::from_input(&state).stroke_controls,
        ToolbarStrokeControls::Panel
    );

    for style in STYLES {
        state.test_set_toolbar_stroke_controls(style);
        assert_eq!(ToolbarSnapshot::from_input(&state).stroke_controls, style);
    }
}

#[test]
fn each_style_lists_its_controls_for_each_tool() {
    use StylePillControl as C;

    let cases: [(Tool, [Vec<StylePillControl>; 3]); 3] = [
        (
            Tool::Pen,
            [
                vec![C::PenFeelChip],
                vec![C::PenSmoothingMeter],
                vec![C::PenSmoothingStepper],
            ],
        ),
        (
            Tool::Marker,
            [
                vec![C::PenFeelChip],
                vec![C::PenSmoothingMeter],
                vec![C::PenSmoothingStepper],
            ],
        ),
        (
            Tool::LiveShape,
            [
                // One chip stands in for both settings.
                vec![C::PenFeelChip],
                vec![C::PenSmoothingMeter, C::ShapeSensitivityMeter],
                vec![C::PenSmoothingStepper, C::ShapeSensitivityStepper],
            ],
        ),
    ];
    for (tool, expected) in cases {
        for (style, expected) in STYLES.into_iter().zip(expected) {
            assert_eq!(
                stroke_feel_controls(&styled(tool, style), &plan()),
                expected,
                "{tool:?} {style:?}"
            );
        }
    }

    // Tools that draw no smoothed path and recognize nothing get none.
    for tool in [
        Tool::Line,
        Tool::Rect,
        Tool::Arrow,
        Tool::Eraser,
        Tool::StepMarker,
        Tool::Spotlight,
        Tool::Blur,
    ] {
        for style in STYLES {
            assert!(
                stroke_feel_controls(&styled(tool, style), &plan()).is_empty(),
                "{tool:?} {style:?}"
            );
        }
    }
}

/// Under width pressure the chip leaves exactly where the meters and
/// steppers would, before the pill's core.
#[test]
fn width_pressure_sheds_the_stroke_feel_controls_in_every_style() {
    let mut squeezed = plan();
    squeezed.drop_style_extras = true;

    for style in STYLES {
        for tool in [Tool::Pen, Tool::Marker, Tool::LiveShape] {
            let snapshot = styled(tool, style);
            assert!(
                stroke_feel_controls(&snapshot, &squeezed).is_empty(),
                "{tool:?} {style:?}"
            );
            assert!(
                StylePillSpec::build(&snapshot, &squeezed)
                    .controls()
                    .contains(&StylePillControl::ColorChip),
                "{tool:?} {style:?} keeps its core"
            );
        }
    }
}

#[test]
fn the_pen_feel_chip_opens_its_panel_and_summarizes_the_tools_levels() {
    let chip = StylePillControl::PenFeelChip;
    let mut shape_pen = styled(Tool::LiveShape, ToolbarStrokeControls::Panel);
    shape_pen.pen_smoothing = 3;
    shape_pen.shape_recognition_sensitivity = 3;

    assert_eq!(chip.id(), "top.style.pen-feel");
    assert_eq!(chip.role(), StylePillRole::Button);
    assert_eq!(chip.label(&shape_pen), "Pen feel \u{25BE}");
    assert_eq!(chip.caption(), None, "the chip's label names it");
    assert!(chip.enabled(&shape_pen));
    assert!(!chip.active(&shape_pen));
    assert_eq!(
        chip.event(&shape_pen),
        Some(ToolbarEvent::TogglePenFeelPanel(true))
    );
    assert_eq!(
        chip.tooltip(&shape_pen).as_deref(),
        Some("Smoothing: Medium \u{b7} Shape detection: Forgiving \u{2014} click to adjust")
    );

    // Pen and Marker have no detection to summarize.
    let mut pen = styled(Tool::Pen, ToolbarStrokeControls::Panel);
    pen.pen_smoothing = 0;
    assert_eq!(
        chip.tooltip(&pen).as_deref(),
        Some("Smoothing: Off \u{2014} click to adjust")
    );

    // Open, it reads as pressed and its click closes the panel.
    shape_pen.pen_feel_open = true;
    assert!(chip.active(&shape_pen));
    assert_eq!(
        chip.event(&shape_pen),
        Some(ToolbarEvent::TogglePenFeelPanel(false))
    );
}

/// The original stepper presentation, restored as the "stepper" style:
/// captions, halves, readouts, and tooltips as they were.
#[test]
fn the_tool_steppers_keep_their_original_captions_and_names() {
    let spec = StylePillSpec::build(
        &styled(Tool::LiveShape, ToolbarStrokeControls::Stepper),
        &plan(),
    );
    let captions: Vec<_> = spec
        .controls()
        .iter()
        .filter(|control| {
            control.role() == StylePillRole::Stepper
                && !matches!(control, StylePillControl::SelectionStepper(_))
        })
        .map(|control| control.caption())
        .collect();
    assert_eq!(captions, [Some("Smooth"), Some("Detect")]);

    // The caption is the short on-screen word; the accessible name stays
    // the full one.
    assert_eq!(
        StylePillControl::ShapeSensitivityStepper.label(&snapshot()),
        "Sensitivity"
    );
    assert_eq!(
        StylePillControl::PenSmoothingStepper.label(&snapshot()),
        "Smoothing"
    );
    for control in [
        StylePillControl::PenSmoothingStepper,
        StylePillControl::ShapeSensitivityStepper,
    ] {
        assert_eq!(control.event(&snapshot()), None, "{control:?}");
        assert!(control.meter(&snapshot()).is_none(), "{control:?}");
    }
}

#[test]
fn the_smoothing_stepper_moves_one_whole_pass_at_a_time() {
    let mut snapshot = styled(Tool::Pen, ToolbarStrokeControls::Stepper);
    snapshot.pen_smoothing = 3;

    let stepper = StylePillControl::PenSmoothingStepper;
    assert_eq!(stepper.role(), StylePillRole::Stepper);
    assert_eq!(stepper.value_text(&snapshot).as_deref(), Some("3"));

    let steps = stepper.required_steps(&snapshot);
    assert_eq!(steps[0].id, "top.style.pen-smoothing.minus");
    assert_eq!(steps[0].event, ToolbarEvent::SetPenSmoothing(2));
    assert_eq!(steps[0].tooltip, "Less smoothing");
    assert_eq!(steps[1].id, "top.style.pen-smoothing.plus");
    assert_eq!(steps[1].event, ToolbarEvent::SetPenSmoothing(4));
    assert_eq!(steps[1].tooltip, "More smoothing");

    // Zero passes is a state, not a quantity.
    snapshot.pen_smoothing = 0;
    assert_eq!(stepper.value_text(&snapshot).as_deref(), Some("Off"));
    assert_eq!(
        stepper.required_steps(&snapshot)[0].event,
        ToolbarEvent::SetPenSmoothing(0),
        "there is nothing below off"
    );

    snapshot.pen_smoothing = crate::draw::MAX_PEN_SMOOTHING;
    assert_eq!(
        stepper.required_steps(&snapshot)[1].event,
        ToolbarEvent::SetPenSmoothing(crate::draw::MAX_PEN_SMOOTHING)
    );
}

#[test]
fn the_sensitivity_stepper_stays_in_range() {
    let stepper = StylePillControl::ShapeSensitivityStepper;
    let mut snapshot = styled(Tool::LiveShape, ToolbarStrokeControls::Stepper);
    snapshot.shape_recognition_sensitivity = 2;

    assert_eq!(stepper.value_text(&snapshot).as_deref(), Some("2"));
    let steps = stepper.required_steps(&snapshot);
    assert_eq!(
        steps[0].event,
        ToolbarEvent::SetShapeRecognitionSensitivity(1)
    );
    assert_eq!(steps[0].tooltip, "Keep more strokes as ink");
    assert_eq!(
        steps[1].event,
        ToolbarEvent::SetShapeRecognitionSensitivity(3)
    );
    assert_eq!(steps[1].tooltip, "Recognize rougher strokes");

    let max = crate::config::MAX_SHAPE_RECOGNITION_SENSITIVITY;
    snapshot.shape_recognition_sensitivity = 0;
    assert_eq!(
        stepper.required_steps(&snapshot)[0].event,
        ToolbarEvent::SetShapeRecognitionSensitivity(0)
    );
    snapshot.shape_recognition_sensitivity = max;
    assert_eq!(
        stepper.required_steps(&snapshot)[1].event,
        ToolbarEvent::SetShapeRecognitionSensitivity(max)
    );
}

#[test]
fn the_pen_feel_chip_drops_its_tooltip_while_the_panel_is_open() {
    let mut snapshot = snapshot_for_tool(Tool::Pen);
    assert!(StylePillControl::PenFeelChip.tooltip(&snapshot).is_some());

    snapshot.pen_feel_open = true;
    assert_eq!(StylePillControl::PenFeelChip.tooltip(&snapshot), None);
}
