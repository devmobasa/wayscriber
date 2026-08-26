use super::*;

#[test]
fn state_derives_from_the_tool_options_kind() {
    let cases = [
        (Tool::Select, StylePillState::Hidden),
        (Tool::Pen, StylePillState::Stroke),
        (Tool::Line, StylePillState::Stroke),
        (Tool::Marker, StylePillState::Marker),
        (Tool::Eraser, StylePillState::Eraser),
        (Tool::Rect, StylePillState::Shape),
        (Tool::Ellipse, StylePillState::Shape),
        (Tool::Arrow, StylePillState::Arrow),
        (Tool::StepMarker, StylePillState::StepMarker),
        (Tool::Spotlight, StylePillState::Spotlight),
    ];
    for (tool, expected) in cases {
        let snapshot = snapshot_for_tool(tool);
        assert_eq!(
            StylePillSpec::state_of(&snapshot, &plan()),
            expected,
            "{tool:?} maps to {}",
            expected.key()
        );
        assert_eq!(StylePillSpec::build(&snapshot, &plan()).state(), expected);
    }

    let mut text = snapshot();
    text.text_active = true;
    assert_eq!(
        StylePillSpec::state_of(&text, &plan()),
        StylePillState::Text
    );
}

#[test]
fn spotlight_state_is_a_magnification_slider_without_stroke_controls() {
    let mut snapshot = snapshot_for_tool(Tool::Spotlight);
    snapshot.spotlight_magnification = 2.25;

    let spec = StylePillSpec::build(&snapshot, &plan());
    assert_eq!(spec.state(), StylePillState::Spotlight);
    assert_eq!(control_ids(&spec), ["top.style.spotlight-magnification"]);

    let slider = StylePillControl::SpotlightMagnificationSlider;
    assert_eq!(
        slider.event(&snapshot),
        Some(ToolbarEvent::SetSpotlightMagnification(2.25))
    );
    assert_eq!(
        slider.slider(&snapshot),
        Some((ToolbarSliderSpec::SPOTLIGHT_MAGNIFICATION, 2.25))
    );
    assert_eq!(slider.value_text(&snapshot).as_deref(), Some("2.25x"));
}

#[test]
fn the_smoothing_stepper_moves_one_whole_pass_at_a_time() {
    // A stepper rather than a slider: seven whole passes across a 110px track
    // is 18px per step, and the row of near-identical bars was the thing that
    // made the pill hard to read.
    let mut snapshot = snapshot_for_tool(Tool::Pen);
    snapshot.pen_smoothing = 3;

    let stepper = StylePillControl::PenSmoothingStepper;
    assert_eq!(stepper.role(), StylePillRole::Stepper);
    assert_eq!(
        stepper.event(&snapshot),
        None,
        "a stepper keeps its events on its halves"
    );
    assert_eq!(stepper.value_text(&snapshot).as_deref(), Some("3"));

    let steps = stepper.required_steps(&snapshot);
    assert_eq!(steps[0].event, ToolbarEvent::SetPenSmoothing(2));
    assert_eq!(steps[1].event, ToolbarEvent::SetPenSmoothing(4));

    // Zero passes is a state, not a quantity.
    snapshot.pen_smoothing = 0;
    assert_eq!(stepper.value_text(&snapshot).as_deref(), Some("Off"));
}

#[test]
fn every_slider_says_what_it_does_and_carries_a_name() {
    // The pill draws sliders as bare tracks with a numeral beside them, so a
    // hover is the only place they can say which is which. `label` is what both
    // frontends hand to the accessibility layer; a slider with neither is three
    // anonymous bars to anyone not looking at the numerals.
    let mut snapshot = snapshot_for_tool(Tool::Pen);
    snapshot.show_marker_opacity_section = true;
    snapshot.show_text_controls = true;

    for control in [
        StylePillControl::ThicknessSlider,
        StylePillControl::OpacitySlider,
        StylePillControl::FontSizeSlider,
        StylePillControl::SpotlightMagnificationSlider,
    ] {
        assert_eq!(control.role(), StylePillRole::Slider);
        assert!(
            control
                .tooltip(&snapshot)
                .is_some_and(|text| !text.is_empty()),
            "{control:?} has nothing to say on hover"
        );
        assert!(
            !control.label(&snapshot).is_empty(),
            "{control:?} has no accessible name"
        );
    }
}

#[test]
fn the_thickness_tooltip_follows_what_the_slider_is_actually_sizing() {
    // One slider targets the pen, the marker, or the eraser depending on what
    // is active, so a fixed wording would be wrong two thirds of the time.
    let mut snapshot = snapshot_for_tool(Tool::Eraser);
    snapshot.thickness_targets_eraser = true;
    let eraser = StylePillControl::ThicknessSlider
        .tooltip(&snapshot)
        .expect("a tooltip");
    assert!(eraser.to_lowercase().contains("eraser"), "got {eraser:?}");

    let pen = StylePillControl::ThicknessSlider
        .tooltip(&snapshot_for_tool(Tool::Pen))
        .expect("a tooltip");
    assert!(!pen.to_lowercase().contains("eraser"), "got {pen:?}");
}

#[test]
fn bold_has_a_control_of_its_own() {
    // Family selection must not decide weight, so Bold has an independent
    // control with an independent event.
    let mut snapshot = snapshot();
    snapshot.text_active = true;

    let toggle = StylePillControl::FontWeightToggle;
    assert_eq!(toggle.role(), StylePillRole::Toggle);
    assert_eq!(toggle.label(&snapshot), "Bold");

    snapshot.font = crate::draw::FontDescriptor::new(
        "Sans".to_string(),
        "normal".to_string(),
        "normal".to_string(),
    );
    assert!(!toggle.active(&snapshot));
    assert_eq!(
        toggle.event(&snapshot),
        Some(ToolbarEvent::SetFontBold(true))
    );

    snapshot.font = crate::draw::FontDescriptor::new(
        "Sans".to_string(),
        "Bold".to_string(),
        "normal".to_string(),
    );
    assert!(
        toggle.active(&snapshot),
        "the weight is compared without case, like every other font identity"
    );
    assert_eq!(
        toggle.event(&snapshot),
        Some(ToolbarEvent::SetFontBold(false))
    );

    let ids = control_ids(&StylePillSpec::build(&snapshot, &plan()));
    assert!(ids.contains(&"top.style.font-bold".to_string()));
}

#[test]
fn a_numeric_weight_does_not_read_as_bold() {
    // The toggle writes words. A config asking for 700 is asking for something
    // a two-state control cannot say, so it must not claim to be showing it.
    let mut snapshot = snapshot();
    snapshot.font = crate::draw::FontDescriptor::new(
        "Sans".to_string(),
        "700".to_string(),
        "normal".to_string(),
    );

    assert!(!StylePillControl::FontWeightToggle.active(&snapshot));
}

#[test]
fn the_smoothing_stepper_stops_at_both_ends_of_the_range() {
    let mut snapshot = snapshot_for_tool(Tool::Pen);

    snapshot.pen_smoothing = 0;
    let steps = StylePillControl::PenSmoothingStepper.required_steps(&snapshot);
    assert_eq!(
        steps[0].event,
        ToolbarEvent::SetPenSmoothing(0),
        "there is nothing below off"
    );

    snapshot.pen_smoothing = crate::draw::MAX_PEN_SMOOTHING;
    let steps = StylePillControl::PenSmoothingStepper.required_steps(&snapshot);
    assert_eq!(
        steps[1].event,
        ToolbarEvent::SetPenSmoothing(crate::draw::MAX_PEN_SMOOTHING)
    );
}

#[test]
fn the_smoothing_stepper_follows_the_tool_it_can_change() {
    // Pen and Marker accumulate the paths smoothing runs on. Line and Blur
    // share the Stroke control group but draw no path, so a stepper there
    // would be a control that does nothing to what is about to be drawn.
    for tool in [Tool::Pen, Tool::Marker] {
        let spec = StylePillSpec::build(&snapshot_for_tool(tool), &plan());
        assert!(
            control_ids(&spec).contains(&"top.style.pen-smoothing".to_string()),
            "{tool:?} draws a smoothed stroke"
        );
    }
    for tool in [Tool::Line, Tool::Blur, Tool::Rect, Tool::Eraser] {
        let spec = StylePillSpec::build(&snapshot_for_tool(tool), &plan());
        assert!(
            !control_ids(&spec).contains(&"top.style.pen-smoothing".to_string()),
            "{tool:?} draws nothing smoothing reaches"
        );
    }
}

#[test]
fn the_font_button_shows_the_family_in_use_and_opens_the_picker() {
    let mut snapshot = snapshot();
    snapshot.text_active = true;
    snapshot.font = crate::draw::FontDescriptor::new(
        "Noto Sans CJK JP Black".to_string(),
        "normal".to_string(),
        "normal".to_string(),
    );

    let button = StylePillControl::FontFamilyPicker;
    assert_eq!(button.event(&snapshot), Some(ToolbarEvent::OpenFontPicker));
    assert_eq!(button.role(), StylePillRole::Button);
    assert!(
        button.label(&snapshot).chars().count() <= 13,
        "the pill is width-planned; got {:?}",
        button.label(&snapshot)
    );
    assert_eq!(
        button.tooltip(&snapshot).as_deref(),
        Some("Noto Sans CJK JP Black - choose from every installed font"),
        "the full name has to be readable somewhere"
    );
}

#[test]
fn a_squeezed_pill_sheds_its_extras_before_it_sheds_the_color_chip() {
    let mut snapshot = snapshot_for_tool(Tool::Pen);
    snapshot.show_text_controls = true;
    let mut squeezed = plan();
    squeezed.drop_style_extras = true;

    let ids = control_ids(&StylePillSpec::build(&snapshot, &squeezed));

    assert!(!ids.contains(&"top.style.pen-smoothing".to_string()));
    assert!(!ids.contains(&"top.style.font-bold".to_string()));
    assert!(
        ids.contains(&"top.style.color-chip".to_string())
            && ids.contains(&"top.style.thickness".to_string())
            // The only font control there is now, so it stays: dropping it
            // would leave no way to change the family from the toolbar at all.
            && ids.contains(&"top.style.font-family-picker".to_string()),
        "the pill's core stays: {ids:?}"
    );
}

#[test]
fn spotlight_state_exposes_an_inline_missing_source_hint() {
    let mut snapshot = snapshot_for_tool(Tool::Spotlight);
    snapshot.spotlight_magnification = 2.25;
    snapshot.spotlight_magnifier_source =
        Some(crate::draw::SpotlightMagnifierSource::IncompleteTransparent);

    let slider = StylePillControl::SpotlightMagnificationSlider;
    assert_eq!(
        slider.status_text(&snapshot),
        Some("Freeze screen to preview")
    );

    snapshot.spotlight_magnifier_source =
        Some(crate::draw::SpotlightMagnifierSource::CompleteSolid);
    assert_eq!(slider.status_text(&snapshot), None);

    // No backend has answered, so the control says nothing rather than
    // guessing that the canvas is or is not magnifiable.
    snapshot.spotlight_magnifier_source = None;
    assert_eq!(slider.status_text(&snapshot), None);
}

#[test]
fn a_compact_strip_drops_the_pill_and_with_it_the_unavailable_status() {
    // The status label has no compact presentation of its own, and needs none:
    // under that width pressure the whole pill yields, so nothing downstream
    // has to decide what to do with a hint it cannot fit.
    let mut snapshot = snapshot_for_tool(Tool::Spotlight);
    snapshot.spotlight_magnification = 2.25;
    snapshot.spotlight_magnifier_source =
        Some(crate::draw::SpotlightMagnifierSource::IncompleteTransparent);

    let mut compact = plan();
    compact.compact = true;

    assert_eq!(
        StylePillSpec::state_of(&snapshot, &compact),
        StylePillState::Hidden
    );
    assert!(
        StylePillSpec::build(&snapshot, &compact)
            .controls()
            .is_empty(),
        "a hidden pill materializes no control that could carry a status"
    );
}

#[test]
fn minimized_and_micro_strips_hide_the_pill() {
    let mut minimized = snapshot();
    minimized.top_minimized = true;
    assert_eq!(
        StylePillSpec::state_of(&minimized, &plan()),
        StylePillState::Hidden
    );
    assert!(
        StylePillSpec::build(&minimized, &plan())
            .controls()
            .is_empty()
    );

    let mut micro = snapshot();
    micro.top_display_mode = crate::config::TopDisplayMode::Micro;
    assert_eq!(
        StylePillSpec::state_of(&micro, &plan()),
        StylePillState::Hidden
    );
    assert!(StylePillSpec::build(&micro, &plan()).controls().is_empty());
}

#[test]
fn stroke_state_orders_chip_swatches_slider_and_numeral() {
    let snapshot = snapshot_for_tool(Tool::Pen);
    let spec = StylePillSpec::build(&snapshot, &plan());
    assert_eq!(spec.state(), StylePillState::Stroke);

    let swatch_count = snapshot
        .quick_colors
        .rendered_entries()
        .len()
        .min(StylePillSpec::MAX_SWATCHES);
    assert!(swatch_count > 0);
    let mut expected = vec!["top.style.color-chip".to_string()];
    expected.extend((0..swatch_count).map(|index| format!("top.style.swatch.{index}")));
    expected.push("top.style.thickness".to_string());
    expected.push("top.style.thickness-value".to_string());
    // The pen draws the strokes smoothing applies to, so the pill offers it.
    expected.push("top.style.pen-smoothing".to_string());
    assert_eq!(control_ids(&spec), expected);

    let chip = spec.controls()[0];
    assert_eq!(
        chip.event(&snapshot),
        Some(ToolbarEvent::OpenColorPickerPopup)
    );
    assert_eq!(chip.role(), StylePillRole::Swatch);

    let swatch = spec.controls()[1];
    let entry = &snapshot.quick_colors.rendered_entries()[0];
    assert_eq!(
        swatch.event(&snapshot),
        Some(ToolbarEvent::SetQuickColor {
            color: entry.color,
            action: QuickColorPalette::action_for_index(0),
            index: 0,
        })
    );

    let slider = StylePillControl::ThicknessSlider;
    assert_eq!(
        slider.event(&snapshot),
        Some(ToolbarEvent::SetThickness(snapshot.thickness))
    );
    assert_eq!(
        slider.slider(&snapshot),
        Some((ToolbarSliderSpec::THICKNESS, snapshot.thickness))
    );

    // The numeral is a distinct button control opening the overlay
    // precise-entry popup, with a px readout.
    let numeral = StylePillControl::ThicknessValue;
    assert_eq!(numeral.role(), StylePillRole::Value);
    assert_eq!(
        numeral.event(&snapshot),
        Some(ToolbarEvent::OpenPrecisionEntry(
            crate::ui::toolbar::PrecisionEntryTarget::Thickness
        ))
    );
    assert_eq!(
        numeral.value_text(&snapshot),
        Some(format!("{:.0}px", snapshot.thickness))
    );
    assert_eq!(numeral.tooltip(&snapshot).as_deref(), Some("Thickness"));
}

#[test]
fn hiding_quick_colors_hides_the_pill_swatches_but_keeps_the_chip() {
    use crate::config::ToolbarItemsConfig;

    let mut snapshot = snapshot_for_tool(Tool::Pen);
    // Default (item visible): the swatch row renders after the color chip.
    let visible = control_ids(&StylePillSpec::build(&snapshot, &plan()));
    assert!(visible.contains(&"top.style.color-chip".to_string()));
    assert!(visible.contains(&"top.style.swatch.0".to_string()));

    // The "Quick colors" customization toggle is repointed to gate this
    // swatch row (M7-C1): hiding the item drops the swatches while the
    // color chip stays reachable.
    let mut items = ToolbarItemsConfig::default();
    items.set_hidden(ids::TOP_GROUP_QUICK_COLORS, true);
    snapshot.resolved_toolbar_items = items.resolved();
    let hidden = control_ids(&StylePillSpec::build(&snapshot, &plan()));
    assert!(
        hidden.contains(&"top.style.color-chip".to_string()),
        "the color chip survives hiding Quick colors: {hidden:?}"
    );
    assert!(
        !hidden.iter().any(|id| id.starts_with("top.style.swatch.")),
        "hiding Quick colors hides the pill swatches: {hidden:?}"
    );

    // A Text-state pill (also color-bearing) honours the same gate.
    let mut text = snapshot.clone();
    text.text_active = true;
    let text_ids = control_ids(&StylePillSpec::build(&text, &plan()));
    assert_eq!(
        StylePillSpec::build(&text, &plan()).state(),
        StylePillState::Text
    );
    assert!(text_ids.contains(&"top.style.color-chip".to_string()));
    assert!(
        !text_ids
            .iter()
            .any(|id| id.starts_with("top.style.swatch.")),
        "hiding Quick colors also hides the Text-state swatches: {text_ids:?}"
    );
}

#[test]
fn shape_state_appends_the_fill_toggle() {
    let snapshot = snapshot_for_tool(Tool::Rect);
    let spec = StylePillSpec::build(&snapshot, &plan());
    assert_eq!(spec.state(), StylePillState::Shape);
    let ids = control_ids(&spec);
    assert_eq!(ids.last().map(String::as_str), Some("top.style.fill"));

    let fill = *spec.controls().last().expect("fill control");
    assert_eq!(
        fill.event(&snapshot),
        Some(ToolbarEvent::ToggleFill(!snapshot.fill_enabled))
    );
    assert_eq!(fill.active(&snapshot), snapshot.fill_enabled);
    assert_eq!(fill.role(), StylePillRole::Toggle);
}

#[test]
fn arrow_state_gates_the_reset_button_on_the_toggle() {
    let mut snapshot = snapshot_for_tool(Tool::Arrow);
    snapshot.arrow_label_enabled = false;
    let spec = StylePillSpec::build(&snapshot, &plan());
    assert_eq!(spec.state(), StylePillState::Arrow);
    let ids = control_ids(&spec);
    assert!(ids.contains(&"top.style.auto-number".to_string()));
    assert!(!ids.contains(&"top.style.counter-reset.arrow".to_string()));

    snapshot.arrow_label_enabled = true;
    snapshot.arrow_label_next = 7;
    let spec = StylePillSpec::build(&snapshot, &plan());
    let ids = control_ids(&spec);
    let toggle_pos = ids
        .iter()
        .position(|id| id == "top.style.auto-number")
        .expect("toggle");
    assert_eq!(
        ids.get(toggle_pos + 1).map(String::as_str),
        Some("top.style.counter-reset.arrow")
    );

    let toggle = StylePillControl::AutoNumberToggle;
    assert_eq!(
        toggle.event(&snapshot),
        Some(ToolbarEvent::ToggleArrowLabels(false))
    );
    assert!(toggle.active(&snapshot));

    let reset = StylePillControl::CounterReset(StylePillCounter::Arrow);
    assert_eq!(
        reset.event(&snapshot),
        Some(ToolbarEvent::ResetArrowLabelCounter)
    );
    assert_eq!(
        reset.tooltip(&snapshot).as_deref(),
        Some("Reset numbering to 1 (next: 7)")
    );
}

#[test]
fn next_arrow_style_control_does_not_advertise_the_selection_aware_shortcut() {
    use crate::config::{Action, Shortcut};
    use crate::input::state::test_support::make_test_input_state_with_action_bindings;
    use std::collections::HashMap;

    let state = make_test_input_state_with_action_bindings(HashMap::from([(
        Action::CycleArrowStyle,
        vec![Shortcut::parse("Ctrl+J").expect("binding")],
    )]));
    let hints = ToolbarBindingHints::from_input_state(&state);
    assert_eq!(
        hints.binding_for_action(Action::CycleArrowStyle),
        Some("Ctrl+J"),
        "the regression needs a real selection-aware shortcut to suppress"
    );
    let mut snapshot = ToolbarSnapshot::from_input_with_bindings(&state, hints);
    snapshot.active_tool = Tool::Arrow;
    snapshot.tool_override = None;
    snapshot.show_text_controls = false;
    snapshot.show_marker_opacity_section = false;

    assert_eq!(
        StylePillControl::ArrowStyleCycle
            .tooltip(&snapshot)
            .as_deref(),
        Some("Next arrow style: Standard")
    );
}

#[test]
fn step_marker_state_carries_the_step_reset() {
    let mut snapshot = snapshot_for_tool(Tool::StepMarker);
    snapshot.step_marker_next = 4;
    let spec = StylePillSpec::build(&snapshot, &plan());
    assert_eq!(spec.state(), StylePillState::StepMarker);
    assert!(
        spec.controls()
            .contains(&StylePillControl::CounterReset(StylePillCounter::Step))
    );
    let reset = StylePillControl::CounterReset(StylePillCounter::Step);
    assert_eq!(
        reset.event(&snapshot),
        Some(ToolbarEvent::ResetStepMarkerCounter)
    );
    assert_eq!(
        reset.tooltip(&snapshot).as_deref(),
        Some("Reset numbering to 1 (next: 4)")
    );
}

#[test]
fn marker_state_adds_the_opacity_slider() {
    let snapshot = snapshot_for_tool(Tool::Marker);
    let spec = StylePillSpec::build(&snapshot, &plan());
    assert_eq!(spec.state(), StylePillState::Marker);
    let ids = control_ids(&spec);
    let thickness_pos = ids
        .iter()
        .position(|id| id == "top.style.thickness")
        .expect("thickness slider");
    let opacity_pos = ids
        .iter()
        .position(|id| id == "top.style.opacity")
        .expect("opacity slider");
    assert!(
        thickness_pos < opacity_pos,
        "thickness before opacity: {ids:?}"
    );

    let opacity = StylePillControl::OpacitySlider;
    assert_eq!(
        opacity.event(&snapshot),
        Some(ToolbarEvent::SetMarkerOpacity(snapshot.marker_opacity))
    );
    assert_eq!(
        opacity.slider(&snapshot),
        Some((ToolbarSliderSpec::MARKER_OPACITY, snapshot.marker_opacity))
    );
    assert_eq!(
        opacity.value_text(&snapshot),
        Some(format!("{:.0}%", snapshot.marker_opacity * 100.0))
    );
}

#[test]
fn eraser_state_is_size_slider_plus_mode_segment_without_color() {
    let snapshot = snapshot_for_tool(Tool::Eraser);
    let spec = StylePillSpec::build(&snapshot, &plan());
    assert_eq!(spec.state(), StylePillState::Eraser);
    assert_eq!(
        control_ids(&spec),
        [
            "top.style.thickness",
            "top.style.thickness-value",
            "top.style.eraser-mode",
        ]
    );

    // The numeral respects the context label for the eraser target.
    assert_eq!(
        StylePillControl::ThicknessValue
            .tooltip(&snapshot)
            .as_deref(),
        Some("Eraser size")
    );

    let segment = StylePillControl::EraserModeSegment;
    assert_eq!(segment.role(), StylePillRole::Segmented);
    assert_eq!(segment.event(&snapshot), None);
    let segments = segment.segments(&snapshot).expect("segments");
    assert_eq!(segments[0].label, "Brush");
    assert_eq!(
        segments[0].event,
        ToolbarEvent::SetEraserMode(EraserMode::Brush)
    );
    assert_eq!(segments[1].label, "Stroke");
    assert_eq!(
        segments[1].event,
        ToolbarEvent::SetEraserMode(EraserMode::Stroke)
    );
    assert_eq!(
        segments[0].active,
        snapshot.eraser_mode == EraserMode::Brush
    );
    assert_ne!(segments[0].active, segments[1].active);
}

#[test]
fn text_state_is_swatches_size_and_one_font_control() {
    let mut snapshot = snapshot();
    snapshot.text_active = true;
    let spec = StylePillSpec::build(&snapshot, &plan());
    assert_eq!(spec.state(), StylePillState::Text);
    let ids = control_ids(&spec);
    assert!(ids.contains(&"top.style.color-chip".to_string()));
    let tail: Vec<_> = ids.iter().rev().take(4).rev().cloned().collect();
    assert_eq!(
        tail,
        [
            "top.style.font-size",
            "top.style.font-size-value",
            // Weight and family are independent controls.
            "top.style.font-bold",
            "top.style.font-family-picker",
        ]
    );
    assert!(!ids.contains(&"top.style.thickness".to_string()));
    assert!(
        !ids.contains(&"top.style.pen-smoothing".to_string()),
        "typing text draws no stroke for smoothing to reach"
    );

    let slider = StylePillControl::FontSizeSlider;
    assert_eq!(
        slider.event(&snapshot),
        Some(ToolbarEvent::SetFontSize(snapshot.font_size))
    );
    let numeral = StylePillControl::FontSizeValue;
    assert_eq!(
        numeral.event(&snapshot),
        Some(ToolbarEvent::OpenPrecisionEntry(
            crate::ui::toolbar::PrecisionEntryTarget::FontSize
        ))
    );
    assert_eq!(
        numeral.value_text(&snapshot),
        Some(format!("{:.0}pt", snapshot.font_size))
    );

    // There is one family control: the current-family picker.
    let ids = control_ids(&StylePillSpec::build(&snapshot, &plan()));
    assert!(
        !ids.contains(&"top.style.font-family".to_string()),
        "no duplicate family control: {ids:?}"
    );
    assert!(ids.contains(&"top.style.font-family-picker".to_string()));
}

#[test]
fn width_degradation_narrows_then_hides_the_pill() {
    let snapshot = snapshot_for_tool(Tool::Pen);

    // Swatches follow the plan's degradation ladder.
    let mut narrowed = plan();
    narrowed.swatch_count = 4;
    let ids = control_ids(&StylePillSpec::build(&snapshot, &narrowed));
    assert!(ids.contains(&"top.style.swatch.3".to_string()));
    assert!(!ids.contains(&"top.style.swatch.4".to_string()));

    narrowed.swatch_count = 0;
    let ids = control_ids(&StylePillSpec::build(&snapshot, &narrowed));
    assert!(ids.contains(&"top.style.color-chip".to_string()));
    assert!(!ids.contains(&"top.style.swatch.0".to_string()));

    // The last-resort compact presentation hides the pill entirely.
    let mut compact = plan();
    compact.compact = true;
    assert_eq!(
        StylePillSpec::state_of(&snapshot, &compact),
        StylePillState::Hidden
    );
    assert!(
        StylePillSpec::build(&snapshot, &compact)
            .controls()
            .is_empty()
    );
    assert!(!StylePillSpec::visible(&snapshot, &compact));
}

#[test]
fn settings_overrides_extend_the_stroke_state() {
    let mut snapshot = snapshot_for_tool(Tool::Pen);
    snapshot.show_text_controls = true;
    snapshot.show_marker_opacity_section = true;
    let ids = control_ids(&StylePillSpec::build(&snapshot, &plan()));
    assert!(ids.contains(&"top.style.opacity".to_string()));
    assert!(ids.contains(&"top.style.font-size".to_string()));
    assert!(ids.contains(&"top.style.font-family-picker".to_string()));
}

#[test]
fn the_docked_selection_control_reports_on_the_selected_shape_not_the_tool_default() {
    let mut snapshot = snapshot_for_tool(Tool::Select);
    // The next Spotlight the user draws would be unmagnified...
    snapshot.spotlight_magnification = crate::draw::DEFAULT_SPOTLIGHT_MAGNIFICATION;
    // ...but the shape they have selected is not.
    snapshot.selection_spotlight_magnification = Some(3.0);
    snapshot.spotlight_magnifier_source =
        Some(crate::draw::SpotlightMagnifierSource::IncompleteTransparent);
    snapshot
        .selection_properties
        .push(crate::input::SelectionPropertyEntry {
            label: "Magnification".to_string(),
            value: "3x".to_string(),
            kind: crate::input::SelectionPropertyKind::SpotlightMagnification,
            disabled: false,
        });

    let stepper = StylePillControl::SelectionStepper(
        crate::input::SelectionPropertyKind::SpotlightMagnification,
    );
    assert!(stepper.has_status_slot());
    assert_eq!(
        stepper.status_text(&snapshot),
        Some("Freeze screen to preview"),
        "a magnified selection needs the hint even when the tool default is 1x"
    );

    // The slider tracks the tool default, which is not magnified, so it stays quiet.
    assert_eq!(
        StylePillControl::SpotlightMagnificationSlider.status_text(&snapshot),
        None
    );

    // An unmagnified selection has nothing to warn about either.
    snapshot.selection_spotlight_magnification = Some(crate::draw::DEFAULT_SPOTLIGHT_MAGNIFICATION);
    assert_eq!(stepper.status_text(&snapshot), None);
}

#[test]
fn controls_without_a_magnification_readout_have_no_status_slot() {
    for control in [
        StylePillControl::ThicknessSlider,
        StylePillControl::OpacitySlider,
        StylePillControl::FontSizeSlider,
        StylePillControl::ColorChip,
    ] {
        assert!(!control.has_status_slot(), "{control:?}");
        assert_eq!(control.status_text(&snapshot()), None, "{control:?}");
    }
}
