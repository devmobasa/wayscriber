use super::*;

impl StylePillControl {
    pub(crate) fn id(self) -> Cow<'static, str> {
        match self {
            Self::ColorChip => Cow::Borrowed("top.style.color-chip"),
            Self::QuickSwatch(index) => Cow::Owned(format!("top.style.swatch.{index}")),
            Self::ThicknessSlider => Cow::Borrowed("top.style.thickness"),
            Self::ThicknessValue => Cow::Borrowed("top.style.thickness-value"),
            Self::OpacitySlider => Cow::Borrowed("top.style.opacity"),
            Self::FillToggle => Cow::Borrowed("top.style.fill"),
            Self::AutoNumberToggle => Cow::Borrowed("top.style.auto-number"),
            // Distinct per counter: classic mode (context_aware_ui = false)
            // can materialize both resets in one spec, and the frontends
            // key focus/updater resolution on unique ids.
            Self::CounterReset(StylePillCounter::Arrow) => {
                Cow::Borrowed("top.style.counter-reset.arrow")
            }
            Self::CounterReset(StylePillCounter::Step) => {
                Cow::Borrowed("top.style.counter-reset.step")
            }
            Self::FontSizeSlider => Cow::Borrowed("top.style.font-size"),
            Self::FontSizeValue => Cow::Borrowed("top.style.font-size-value"),
            Self::FontFamilySegment => Cow::Borrowed("top.style.font-family"),
            Self::EraserModeSegment => Cow::Borrowed("top.style.eraser-mode"),
            Self::SelectionCycle(kind) | Self::SelectionStepper(kind) => {
                Cow::Owned(format!("top.style.sel.{}", selection_kind_slug(kind)))
            }
        }
    }

    pub(crate) fn role(self) -> StylePillRole {
        match self {
            Self::ColorChip | Self::QuickSwatch(_) => StylePillRole::Swatch,
            Self::ThicknessSlider | Self::OpacitySlider | Self::FontSizeSlider => {
                StylePillRole::Slider
            }
            Self::ThicknessValue | Self::FontSizeValue => StylePillRole::Value,
            Self::FillToggle | Self::AutoNumberToggle => StylePillRole::Toggle,
            Self::CounterReset(_) => StylePillRole::Button,
            Self::FontFamilySegment | Self::EraserModeSegment => StylePillRole::Segmented,
            Self::SelectionCycle(_) => StylePillRole::Button,
            Self::SelectionStepper(_) => StylePillRole::Stepper,
        }
    }

    /// Primary click/drag event. `None` for segmented controls and
    /// selection steppers, whose events live on their halves.
    pub(crate) fn event(self, snapshot: &ToolbarSnapshot) -> Option<ToolbarEvent> {
        Some(match self {
            Self::ColorChip => ToolbarEvent::OpenColorPickerPopup,
            Self::QuickSwatch(index) => {
                let entry = &snapshot.quick_colors.rendered_entries()[index];
                ToolbarEvent::SetQuickColor {
                    color: entry.color,
                    action: QuickColorPalette::action_for_index(index),
                    index,
                }
            }
            Self::ThicknessSlider => ToolbarEvent::SetThickness(snapshot.thickness),
            Self::OpacitySlider => ToolbarEvent::SetMarkerOpacity(snapshot.marker_opacity),
            Self::FontSizeSlider => ToolbarEvent::SetFontSize(snapshot.font_size),
            Self::FillToggle => ToolbarEvent::ToggleFill(!snapshot.fill_enabled),
            Self::AutoNumberToggle => {
                ToolbarEvent::ToggleArrowLabels(!snapshot.arrow_label_enabled)
            }
            Self::CounterReset(StylePillCounter::Arrow) => ToolbarEvent::ResetArrowLabelCounter,
            Self::CounterReset(StylePillCounter::Step) => ToolbarEvent::ResetStepMarkerCounter,
            // The numerals open the precise-entry popup on the overlay.
            Self::ThicknessValue => ToolbarEvent::OpenPrecisionEntry(
                crate::ui::toolbar::PrecisionEntryTarget::Thickness,
            ),
            Self::FontSizeValue => {
                ToolbarEvent::OpenPrecisionEntry(crate::ui::toolbar::PrecisionEntryTarget::FontSize)
            }
            // A cycle click is a forward step through the same apply
            // machinery the properties popup uses.
            Self::SelectionCycle(kind) => {
                ToolbarEvent::AdjustSelectionProperty { kind, direction: 1 }
            }
            Self::FontFamilySegment | Self::EraserModeSegment | Self::SelectionStepper(_) => {
                return None;
            }
        })
    }

    /// Primary click for controls whose event lives on the control itself.
    /// Segmented controls and steppers keep events on their halves.
    pub(crate) fn click_event(self, snapshot: &ToolbarSnapshot) -> ToolbarEvent {
        self.event(snapshot)
            .expect("this style-pill control has a primary click event")
    }

    pub(crate) fn enabled(self, snapshot: &ToolbarSnapshot) -> bool {
        match self {
            // Locked/mixed-locked entries surface as disabled controls,
            // exactly like the greyed rows of the properties popup.
            Self::SelectionCycle(kind) | Self::SelectionStepper(kind) => {
                selection_entry(snapshot, kind).is_some_and(|entry| !entry.disabled)
            }
            _ => true,
        }
    }

    pub(crate) fn active(self, snapshot: &ToolbarSnapshot) -> bool {
        match self {
            Self::ColorChip => true,
            Self::QuickSwatch(index) => {
                snapshot.quick_colors.rendered_entries()[index].color == snapshot.color
            }
            Self::FillToggle => snapshot.fill_enabled,
            Self::AutoNumberToggle => snapshot.arrow_label_enabled,
            _ => false,
        }
    }

    /// Slider range plus current value for the slider controls.
    pub(crate) fn slider(self, snapshot: &ToolbarSnapshot) -> Option<(ToolbarSliderSpec, f64)> {
        match self {
            Self::ThicknessSlider => Some((ToolbarSliderSpec::THICKNESS, snapshot.thickness)),
            Self::OpacitySlider => {
                Some((ToolbarSliderSpec::MARKER_OPACITY, snapshot.marker_opacity))
            }
            Self::FontSizeSlider => Some((ToolbarSliderSpec::FONT_SIZE, snapshot.font_size)),
            _ => None,
        }
    }

    /// Slider range plus current value. Callers that already matched a slider
    /// variant use this instead of skipping the control.
    pub(crate) fn slider_value(self, snapshot: &ToolbarSnapshot) -> (ToolbarSliderSpec, f64) {
        self.slider(snapshot)
            .expect("this style-pill control is a slider")
    }

    /// Live readout for sliders and their numeral buttons. The unit follows
    /// the tool context: px for thickness/size targets (the snapshot
    /// already routes eraser/marker sizes through `thickness`), pt for
    /// text size, % for marker opacity.
    pub(crate) fn value_text(self, snapshot: &ToolbarSnapshot) -> Option<String> {
        match self {
            Self::ThicknessSlider | Self::ThicknessValue => {
                Some(format!("{:.0}px", snapshot.thickness))
            }
            Self::OpacitySlider => Some(format!("{:.0}%", snapshot.marker_opacity * 100.0)),
            Self::FontSizeSlider | Self::FontSizeValue => {
                Some(format!("{:.0}pt", snapshot.font_size))
            }
            Self::SelectionCycle(kind) | Self::SelectionStepper(kind) => {
                selection_entry(snapshot, kind).map(|entry| entry.value.clone())
            }
            _ => None,
        }
    }

    /// Live readout for a value-bearing control already matched by the
    /// renderer. Selection entries in the spec always have a snapshot row.
    pub(crate) fn required_value_text(self, snapshot: &ToolbarSnapshot) -> String {
        self.value_text(snapshot)
            .expect("this style-pill control has a live value")
    }

    pub(crate) fn label(self, snapshot: &ToolbarSnapshot) -> Cow<'static, str> {
        match self {
            Self::ColorChip => Cow::Borrowed("Color picker"),
            Self::QuickSwatch(index) => Cow::Owned(
                snapshot.quick_colors.rendered_entries()[index]
                    .label
                    .clone(),
            ),
            Self::ThicknessSlider => {
                Cow::Borrowed(ToolContext::from_snapshot(snapshot).thickness_label)
            }
            Self::OpacitySlider => Cow::Borrowed("Marker opacity"),
            Self::FontSizeSlider => Cow::Borrowed("Text size"),
            Self::ThicknessValue => Cow::Owned(format!("{:.0}px", snapshot.thickness)),
            Self::FontSizeValue => Cow::Owned(format!("{:.0}pt", snapshot.font_size)),
            Self::FillToggle => Cow::Borrowed(action_short_label(Action::ToggleFill)),
            Self::AutoNumberToggle => Cow::Borrowed("Auto-number"),
            Self::CounterReset(_) => Cow::Borrowed("Reset"),
            Self::FontFamilySegment => Cow::Borrowed("Font"),
            Self::EraserModeSegment => Cow::Borrowed("Eraser mode"),
            Self::SelectionCycle(kind) | Self::SelectionStepper(kind) => Cow::Owned(
                selection_entry(snapshot, kind)
                    .map(|entry| entry.label.clone())
                    .unwrap_or_default(),
            ),
        }
    }

    pub(crate) fn tooltip(self, snapshot: &ToolbarSnapshot) -> Option<String> {
        match self {
            Self::ColorChip => Some("Color picker".to_string()),
            Self::QuickSwatch(index) => {
                let entry = &snapshot.quick_colors.rendered_entries()[index];
                let binding = QuickColorPalette::action_for_index(index)
                    .and_then(|action| snapshot.binding_hints.binding_for_action(action));
                Some(format_quick_color_tooltip(&entry.label, binding))
            }
            Self::ThicknessValue => Some(
                ToolContext::from_snapshot(snapshot)
                    .thickness_label
                    .to_string(),
            ),
            Self::FontSizeValue => Some("Text size".to_string()),
            Self::FillToggle => Some(format_binding_label(
                action_label(Action::ToggleFill),
                snapshot
                    .binding_hints
                    .binding_for_action(Action::ToggleFill),
            )),
            Self::AutoNumberToggle => Some("Auto-number arrows 1, 2, 3.".to_string()),
            Self::CounterReset(StylePillCounter::Arrow) => Some(format!(
                "Reset numbering to 1 (next: {})",
                snapshot.arrow_label_next
            )),
            Self::CounterReset(StylePillCounter::Step) => Some(format!(
                "Reset numbering to 1 (next: {})",
                snapshot.step_marker_next
            )),
            Self::SelectionCycle(kind) => selection_entry(snapshot, kind)
                .map(|entry| format!("{}: {}", entry.label, entry.value)),
            Self::ThicknessSlider
            | Self::OpacitySlider
            | Self::FontSizeSlider
            | Self::FontFamilySegment
            | Self::EraserModeSegment
            | Self::SelectionStepper(_) => None,
        }
    }

    /// Segment halves of the segmented controls, in reading order.
    pub(crate) fn segments(self, snapshot: &ToolbarSnapshot) -> Option<[StylePillSegment; 2]> {
        match self {
            Self::FontFamilySegment => Some([
                StylePillSegment {
                    id: "top.style.font-family.sans",
                    label: "Sans",
                    event: ToolbarEvent::SetFont(FontDescriptor::new(
                        "Sans".to_string(),
                        "bold".to_string(),
                        "normal".to_string(),
                    )),
                    active: snapshot.font.family == "Sans",
                    tooltip: "Sans font".to_string(),
                },
                StylePillSegment {
                    id: "top.style.font-family.mono",
                    label: "Mono",
                    event: ToolbarEvent::SetFont(FontDescriptor::new(
                        "Monospace".to_string(),
                        "normal".to_string(),
                        "normal".to_string(),
                    )),
                    active: snapshot.font.family == "Monospace",
                    tooltip: "Monospace font".to_string(),
                },
            ]),
            Self::EraserModeSegment => Some([
                StylePillSegment {
                    id: "top.style.eraser-mode.brush",
                    label: "Brush",
                    event: ToolbarEvent::SetEraserMode(EraserMode::Brush),
                    active: snapshot.eraser_mode == EraserMode::Brush,
                    tooltip: "Erase with the brush".to_string(),
                },
                StylePillSegment {
                    id: "top.style.eraser-mode.stroke",
                    label: "Stroke",
                    event: ToolbarEvent::SetEraserMode(EraserMode::Stroke),
                    active: snapshot.eraser_mode == EraserMode::Stroke,
                    tooltip: format_binding_label(
                        "Erase whole strokes",
                        snapshot
                            .binding_hints
                            .binding_for_action(Action::ToggleEraserMode),
                    ),
                },
            ]),
            _ => None,
        }
    }

    /// The −/+ halves of a selection stepper, in reading order.
    pub(crate) fn steps(self, snapshot: &ToolbarSnapshot) -> Option<[StylePillStep; 2]> {
        let Self::SelectionStepper(kind) = self else {
            return None;
        };
        let entry = selection_entry(snapshot, kind)?;
        let (minus_id, plus_id) = match kind {
            SelectionPropertyKind::Thickness => (
                "top.style.sel.thickness.minus",
                "top.style.sel.thickness.plus",
            ),
            SelectionPropertyKind::FontSize => (
                "top.style.sel.font-size.minus",
                "top.style.sel.font-size.plus",
            ),
            SelectionPropertyKind::ArrowLength => (
                "top.style.sel.arrow-length.minus",
                "top.style.sel.arrow-length.plus",
            ),
            SelectionPropertyKind::ArrowAngle => (
                "top.style.sel.arrow-angle.minus",
                "top.style.sel.arrow-angle.plus",
            ),
            _ => return None,
        };
        Some([
            StylePillStep {
                id: minus_id,
                label: "\u{2212}",
                event: ToolbarEvent::AdjustSelectionProperty {
                    kind,
                    direction: -1,
                },
                tooltip: format!("Decrease {}", entry.label.to_lowercase()),
            },
            StylePillStep {
                id: plus_id,
                label: "+",
                event: ToolbarEvent::AdjustSelectionProperty { kind, direction: 1 },
                tooltip: format!("Increase {}", entry.label.to_lowercase()),
            },
        ])
    }

    /// Segment halves for a control already matched as segmented.
    pub(crate) fn required_segments(self, snapshot: &ToolbarSnapshot) -> [StylePillSegment; 2] {
        self.segments(snapshot)
            .expect("this style-pill control is segmented")
    }

    /// −/+ halves for a selection stepper already present in the spec.
    pub(crate) fn required_steps(self, snapshot: &ToolbarSnapshot) -> [StylePillStep; 2] {
        self.steps(snapshot)
            .expect("this style-pill stepper has minus/plus halves")
    }
}
