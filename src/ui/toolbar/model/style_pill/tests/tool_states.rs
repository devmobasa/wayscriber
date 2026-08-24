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
fn text_state_is_swatches_size_and_font_segment() {
    let mut snapshot = snapshot();
    snapshot.text_active = true;
    let spec = StylePillSpec::build(&snapshot, &plan());
    assert_eq!(spec.state(), StylePillState::Text);
    let ids = control_ids(&spec);
    assert!(ids.contains(&"top.style.color-chip".to_string()));
    let tail: Vec<_> = ids.iter().rev().take(3).rev().cloned().collect();
    assert_eq!(
        tail,
        [
            "top.style.font-size",
            "top.style.font-size-value",
            "top.style.font-family",
        ]
    );
    assert!(!ids.contains(&"top.style.thickness".to_string()));

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

    let segments = StylePillControl::FontFamilySegment
        .segments(&snapshot)
        .expect("font segments");
    assert_eq!(segments[0].label, "Sans");
    assert_eq!(segments[1].label, "Mono");
    assert!(matches!(
        &segments[0].event,
        ToolbarEvent::SetFont(font) if font.family == "Sans"
    ));
    assert!(matches!(
        &segments[1].event,
        ToolbarEvent::SetFont(font) if font.family == "Monospace"
    ));
    assert_eq!(segments[0].active, snapshot.font.family == "Sans");
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
    assert!(ids.contains(&"top.style.font-family".to_string()));
}
