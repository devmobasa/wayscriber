//! Renderer-neutral specification of the contextual style pill (island D).
//!
//! The pill is a fourth detached island rendered under the top-strip
//! islands. It morphs with the active tool: the specification derives one
//! [`StylePillState`] from [`ToolContext`]/[`ToolOptionsKind`] and
//! enumerates the ordered control list for that state. Both frontends and
//! the contract tests consume this one structure, mirroring the
//! `TopToolbarSpec` pattern.
//!
//! Select (`ToolOptionsKind::None`) docks the current selection's
//! properties into the pill (`StylePillState::Selection`) while a
//! selection exists, and hides the pill otherwise. The docked controls
//! route through the same apply machinery as the overlay properties
//! popup, which stays available from the context menu.

use std::borrow::Cow;

use crate::config::{Action, QuickColorPalette, action_label, action_short_label};
use crate::draw::FontDescriptor;
use crate::input::{EraserMode, SelectionPropertyEntry, SelectionPropertyKind};
use crate::label_format::format_binding_label;
use crate::ui::toolbar::{ToolContext, ToolOptionsKind, ToolbarEvent, ToolbarSnapshot};

use super::{ToolbarSliderSpec, TopStripPlan};

/// Morph state of the style pill, derived from the active tool's options
/// kind. `Hidden` covers Select without a selection plus the
/// minimized/micro strip forms, where no contextual rows exist.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StylePillState {
    Hidden,
    /// Select tool with an active selection: the pill docks the selection
    /// properties (the same entry list as the overlay properties popup).
    Selection,
    Stroke,
    Marker,
    Eraser,
    Shape,
    Arrow,
    StepMarker,
    Text,
}

impl StylePillState {
    #[cfg(test)]
    pub(crate) const fn key(self) -> &'static str {
        match self {
            Self::Hidden => "hidden",
            Self::Selection => "selection",
            Self::Stroke => "stroke",
            Self::Marker => "marker",
            Self::Eraser => "eraser",
            Self::Shape => "shape",
            Self::Arrow => "arrow",
            Self::StepMarker => "step-marker",
            Self::Text => "text",
        }
    }
}

/// Which auto-number counter a reset button targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StylePillCounter {
    Arrow,
    Step,
}

/// One control in the pill's ordered list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StylePillControl {
    /// Current-color chip; opens the big overlay gradient picker popup.
    ColorChip,
    /// Quick-color swatch by palette index (up to [`StylePillSpec::MAX_SWATCHES`]).
    QuickSwatch(usize),
    /// Thickness/size slider. The snapshot's `thickness` already targets
    /// the eraser or marker size when those are active.
    ThicknessSlider,
    /// Live thickness numeral; clicking opens the precise-entry popup.
    ThicknessValue,
    /// Marker opacity slider.
    OpacitySlider,
    /// Shape fill toggle.
    FillToggle,
    /// Arrow auto-number toggle.
    AutoNumberToggle,
    /// Reset the arrow/step counter; tooltip carries the next number.
    CounterReset(StylePillCounter),
    /// Text size slider.
    FontSizeSlider,
    /// Live text-size numeral; clicking opens the precise-entry popup.
    FontSizeValue,
    /// Sans/Mono font family segmented control.
    FontFamilySegment,
    /// Brush/Stroke eraser mode segmented control (the old checkbox
    /// semantics as a two-segment control emitting `SetEraserMode`).
    EraserModeSegment,
    /// Docked selection property rendered as a compact cycle button that
    /// shows the entry's value; clicking steps the property forward
    /// through the properties apply machinery (Color, Fill, ArrowHead,
    /// TextBackground).
    SelectionCycle(SelectionPropertyKind),
    /// Docked numeric selection property rendered as a −/value/+ stepper;
    /// the halves step the property through the properties apply
    /// machinery (Thickness, FontSize, ArrowLength, ArrowAngle). The
    /// machinery is relative (direction steps), so the pill uses steppers
    /// where the tool states use sliders.
    SelectionStepper(SelectionPropertyKind),
}

/// Presentation role of one pill control.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StylePillRole {
    Swatch,
    Slider,
    /// Live numeral button (opens the precise-entry popover).
    Value,
    Toggle,
    Button,
    Segmented,
    /// −/value/+ stepper for docked numeric selection properties.
    Stepper,
}

/// One half of a pill segmented control.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct StylePillSegment {
    pub(crate) id: &'static str,
    pub(crate) label: &'static str,
    pub(crate) event: ToolbarEvent,
    pub(crate) active: bool,
    pub(crate) tooltip: String,
}

/// One half of a selection stepper (− or +).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct StylePillStep {
    pub(crate) id: &'static str,
    pub(crate) label: &'static str,
    pub(crate) event: ToolbarEvent,
    pub(crate) tooltip: String,
}

/// Stable id fragment for one selection property kind.
pub(crate) const fn selection_kind_slug(kind: SelectionPropertyKind) -> &'static str {
    match kind {
        SelectionPropertyKind::Color => "color",
        SelectionPropertyKind::Thickness => "thickness",
        SelectionPropertyKind::Fill => "fill",
        SelectionPropertyKind::FontSize => "font-size",
        SelectionPropertyKind::ArrowHead => "arrow-head",
        SelectionPropertyKind::ArrowLength => "arrow-length",
        SelectionPropertyKind::ArrowAngle => "arrow-angle",
        SelectionPropertyKind::TextBackground => "text-background",
    }
}

/// The pill control a docked selection entry maps to: relative numeric
/// properties become steppers, everything else a cycle button.
pub(crate) const fn selection_control_for_kind(kind: SelectionPropertyKind) -> StylePillControl {
    match kind {
        SelectionPropertyKind::Color
        | SelectionPropertyKind::Fill
        | SelectionPropertyKind::ArrowHead
        | SelectionPropertyKind::TextBackground => StylePillControl::SelectionCycle(kind),
        SelectionPropertyKind::Thickness
        | SelectionPropertyKind::FontSize
        | SelectionPropertyKind::ArrowLength
        | SelectionPropertyKind::ArrowAngle => StylePillControl::SelectionStepper(kind),
    }
}

fn selection_entry(
    snapshot: &ToolbarSnapshot,
    kind: SelectionPropertyKind,
) -> Option<&SelectionPropertyEntry> {
    snapshot
        .selection_properties
        .iter()
        .find(|entry| entry.kind == kind)
}

/// The style pill for one snapshot: a morph state plus its ordered controls.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct StylePillSpec {
    state: StylePillState,
    controls: Vec<StylePillControl>,
}

impl StylePillSpec {
    /// Maximum quick-color swatches shown in the pill: always the strip's
    /// quick-color cap, so widening one ladder can never silently leave
    /// the other behind.
    pub(crate) const MAX_SWATCHES: usize = TopStripPlan::MAX_QUICK_COLORS;

    pub(crate) fn build(snapshot: &ToolbarSnapshot, plan: &TopStripPlan) -> Self {
        let state = Self::state_of(snapshot, plan);
        if state == StylePillState::Hidden {
            return Self {
                state,
                controls: Vec::new(),
            };
        }

        if state == StylePillState::Selection {
            let controls = snapshot
                .selection_properties
                .iter()
                .map(|entry| selection_control_for_kind(entry.kind))
                .collect();
            return Self { state, controls };
        }

        let context = ToolContext::from_snapshot(snapshot);
        let mut controls = Vec::new();
        if context.needs_color {
            controls.push(StylePillControl::ColorChip);
            // Swatches follow the strip's width-degradation plan (8→6→4→0)
            // so the pill narrows with the islands above it.
            let count = snapshot
                .quick_colors
                .rendered_entries()
                .len()
                .min(Self::MAX_SWATCHES)
                .min(plan.swatch_count);
            controls.extend((0..count).map(StylePillControl::QuickSwatch));
        }
        if context.needs_thickness {
            controls.push(StylePillControl::ThicknessSlider);
            controls.push(StylePillControl::ThicknessValue);
        }
        if context.show_marker_opacity {
            controls.push(StylePillControl::OpacitySlider);
        }
        if context.show_fill_toggle {
            controls.push(StylePillControl::FillToggle);
        }
        if context.show_arrow_labels {
            controls.push(StylePillControl::AutoNumberToggle);
            if snapshot.arrow_label_enabled {
                controls.push(StylePillControl::CounterReset(StylePillCounter::Arrow));
            }
        }
        if context.show_step_counter {
            controls.push(StylePillControl::CounterReset(StylePillCounter::Step));
        }
        if context.show_font_controls {
            controls.push(StylePillControl::FontSizeSlider);
            controls.push(StylePillControl::FontSizeValue);
            controls.push(StylePillControl::FontFamilySegment);
        }
        if context.show_eraser_mode {
            controls.push(StylePillControl::EraserModeSegment);
        }

        Self { state, controls }
    }

    pub(crate) fn state(&self) -> StylePillState {
        self.state
    }

    pub(crate) fn controls(&self) -> &[StylePillControl] {
        &self.controls
    }

    /// Allocation-free visibility query for the sizing/planning paths. Every
    /// non-hidden state materializes at least one control (pinned by test),
    /// so this equals `!build(snapshot, plan).controls().is_empty()`.
    pub(crate) fn visible(snapshot: &ToolbarSnapshot, plan: &TopStripPlan) -> bool {
        Self::state_of(snapshot, plan) != StylePillState::Hidden
    }

    /// Allocation-free morph-state query matching [`Self::build`].
    pub(crate) fn state_of(snapshot: &ToolbarSnapshot, plan: &TopStripPlan) -> StylePillState {
        if snapshot.top_minimized || snapshot.top_micro_active() {
            return StylePillState::Hidden;
        }
        // The last-resort compact presentation keeps only the protected
        // strip core: the pill yields entirely under that width pressure.
        if plan.compact {
            return StylePillState::Hidden;
        }
        match ToolContext::from_snapshot(snapshot).tool_options_kind {
            // Select: docks the selection properties while a selection
            // exists; hidden otherwise.
            ToolOptionsKind::None => {
                if snapshot.selection_properties.is_empty() {
                    StylePillState::Hidden
                } else {
                    StylePillState::Selection
                }
            }
            ToolOptionsKind::Stroke => StylePillState::Stroke,
            ToolOptionsKind::Marker => StylePillState::Marker,
            ToolOptionsKind::Eraser => StylePillState::Eraser,
            ToolOptionsKind::Shape => StylePillState::Shape,
            ToolOptionsKind::Arrow => StylePillState::Arrow,
            ToolOptionsKind::StepMarker => StylePillState::StepMarker,
            ToolOptionsKind::Text => StylePillState::Text,
        }
    }
}

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
            Self::ThicknessValue | Self::FontSizeValue => Cow::Owned(
                self.value_text(snapshot)
                    .expect("numeral controls have a value text"),
            ),
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
                Some(format_binding_label(&entry.label, binding))
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::Tool;
    use crate::input::state::test_support::make_test_input_state;
    use crate::ui::toolbar::ToolbarBindingHints;

    fn snapshot() -> ToolbarSnapshot {
        let state = make_test_input_state();
        ToolbarSnapshot::from_input_with_bindings(&state, ToolbarBindingHints::default())
    }

    fn plan() -> TopStripPlan {
        TopStripPlan::unconstrained()
    }

    fn snapshot_for_tool(tool: Tool) -> ToolbarSnapshot {
        let mut snapshot = snapshot();
        snapshot.active_tool = tool;
        snapshot.tool_override = None;
        snapshot.thickness_targets_eraser = tool == Tool::Eraser;
        snapshot.thickness_targets_marker = tool == Tool::Marker;
        // Pin the pure per-tool morphs: these two settings are overrides
        // that extend any state (covered by a dedicated test below).
        snapshot.show_text_controls = false;
        snapshot.show_marker_opacity_section = false;
        snapshot
    }

    fn control_ids(spec: &StylePillSpec) -> Vec<String> {
        spec.controls()
            .iter()
            .map(|control| control.id().into_owned())
            .collect()
    }

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

    fn selection_entry(
        label: &str,
        value: &str,
        kind: SelectionPropertyKind,
        disabled: bool,
    ) -> SelectionPropertyEntry {
        SelectionPropertyEntry {
            label: label.to_string(),
            value: value.to_string(),
            kind,
            disabled,
        }
    }

    fn selection_snapshot() -> ToolbarSnapshot {
        let mut snapshot = snapshot_for_tool(Tool::Select);
        snapshot.selection_properties = vec![
            selection_entry("Color", "Red", SelectionPropertyKind::Color, false),
            selection_entry(
                "Thickness",
                "3.0px",
                SelectionPropertyKind::Thickness,
                false,
            ),
            selection_entry("Fill", "Off", SelectionPropertyKind::Fill, false),
            selection_entry(
                "Arrow angle",
                "Locked",
                SelectionPropertyKind::ArrowAngle,
                true,
            ),
        ];
        snapshot
    }

    #[test]
    fn select_with_a_selection_docks_the_property_entries_in_order() {
        let snapshot = selection_snapshot();
        assert_eq!(
            StylePillSpec::state_of(&snapshot, &plan()),
            StylePillState::Selection
        );
        let spec = StylePillSpec::build(&snapshot, &plan());
        assert_eq!(spec.state(), StylePillState::Selection);
        assert_eq!(
            control_ids(&spec),
            [
                "top.style.sel.color",
                "top.style.sel.thickness",
                "top.style.sel.fill",
                "top.style.sel.arrow-angle",
            ]
        );
        assert_eq!(
            spec.controls()[0],
            StylePillControl::SelectionCycle(SelectionPropertyKind::Color)
        );
        assert_eq!(
            spec.controls()[1],
            StylePillControl::SelectionStepper(SelectionPropertyKind::Thickness)
        );

        // Select without a selection stays hidden.
        let empty = snapshot_for_tool(Tool::Select);
        assert_eq!(
            StylePillSpec::state_of(&empty, &plan()),
            StylePillState::Hidden
        );
        assert!(StylePillSpec::build(&empty, &plan()).controls().is_empty());
    }

    #[test]
    fn selection_cycles_step_forward_through_the_apply_machinery() {
        let snapshot = selection_snapshot();
        let cycle = StylePillControl::SelectionCycle(SelectionPropertyKind::Color);
        assert_eq!(cycle.role(), StylePillRole::Button);
        assert!(cycle.enabled(&snapshot));
        assert_eq!(
            cycle.event(&snapshot),
            Some(ToolbarEvent::AdjustSelectionProperty {
                kind: SelectionPropertyKind::Color,
                direction: 1,
            })
        );
        assert_eq!(cycle.value_text(&snapshot).as_deref(), Some("Red"));
        assert_eq!(cycle.label(&snapshot).as_ref(), "Color");
        assert_eq!(cycle.tooltip(&snapshot).as_deref(), Some("Color: Red"));
        assert_eq!(cycle.steps(&snapshot), None);
    }

    #[test]
    fn selection_steppers_carry_minus_plus_halves() {
        let snapshot = selection_snapshot();
        let stepper = StylePillControl::SelectionStepper(SelectionPropertyKind::Thickness);
        assert_eq!(stepper.role(), StylePillRole::Stepper);
        assert!(stepper.enabled(&snapshot));
        assert_eq!(stepper.event(&snapshot), None, "halves carry the events");
        assert_eq!(stepper.value_text(&snapshot).as_deref(), Some("3.0px"));

        let steps = stepper.steps(&snapshot).expect("stepper halves");
        assert_eq!(steps[0].id, "top.style.sel.thickness.minus");
        assert_eq!(steps[1].id, "top.style.sel.thickness.plus");
        assert_eq!(
            steps[0].event,
            ToolbarEvent::AdjustSelectionProperty {
                kind: SelectionPropertyKind::Thickness,
                direction: -1,
            }
        );
        assert_eq!(
            steps[1].event,
            ToolbarEvent::AdjustSelectionProperty {
                kind: SelectionPropertyKind::Thickness,
                direction: 1,
            }
        );
        assert_eq!(steps[0].tooltip, "Decrease thickness");
        assert_eq!(steps[1].tooltip, "Increase thickness");
    }

    #[test]
    fn locked_selection_entries_disable_their_controls() {
        let snapshot = selection_snapshot();
        let locked = StylePillControl::SelectionStepper(SelectionPropertyKind::ArrowAngle);
        assert!(!locked.enabled(&snapshot));
        assert_eq!(locked.value_text(&snapshot).as_deref(), Some("Locked"));
        // An entry the selection does not expose is disabled too.
        let missing = StylePillControl::SelectionCycle(SelectionPropertyKind::TextBackground);
        assert!(!missing.enabled(&snapshot));
        assert_eq!(missing.value_text(&snapshot), None);
    }

    #[test]
    fn ids_are_stable_and_unique_per_spec() {
        for tool in [
            Tool::Pen,
            Tool::Marker,
            Tool::Eraser,
            Tool::Rect,
            Tool::Arrow,
            Tool::StepMarker,
        ] {
            let snapshot = snapshot_for_tool(tool);
            let ids = control_ids(&StylePillSpec::build(&snapshot, &plan()));
            let mut sorted = ids.clone();
            sorted.sort();
            sorted.dedup();
            assert_eq!(sorted.len(), ids.len(), "{tool:?} ids unique: {ids:?}");
            for id in &ids {
                assert!(id.starts_with("top.style."), "{id} uses the pill prefix");
            }
        }

        // Classic mode (context_aware_ui = false) can materialize BOTH
        // counter resets in one spec: the step marker's plus the arrow
        // counter's (arrow auto-numbering enabled). Their ids must stay
        // distinct so focus/updater resolution by id is unambiguous.
        let mut classic = snapshot_for_tool(Tool::StepMarker);
        classic.context_aware_ui = false;
        classic.arrow_label_enabled = true;
        let ids = control_ids(&StylePillSpec::build(&classic, &plan()));
        assert!(
            ids.contains(&"top.style.counter-reset.arrow".to_string()),
            "classic ids: {ids:?}"
        );
        assert!(
            ids.contains(&"top.style.counter-reset.step".to_string()),
            "classic ids: {ids:?}"
        );
        let mut sorted = ids.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(sorted.len(), ids.len(), "classic-mode ids unique: {ids:?}");
    }

    #[test]
    fn allocation_free_queries_match_the_materialized_spec() {
        let mut minimized = snapshot();
        minimized.top_minimized = true;
        let mut micro = snapshot();
        micro.top_display_mode = crate::config::TopDisplayMode::Micro;
        let mut text = snapshot();
        text.text_active = true;

        let mut cases = vec![minimized, micro, text];
        for tool in [
            Tool::Select,
            Tool::Pen,
            Tool::Marker,
            Tool::Eraser,
            Tool::Rect,
            Tool::Arrow,
            Tool::StepMarker,
        ] {
            cases.push(snapshot_for_tool(tool));
        }

        for snapshot in cases {
            let spec = StylePillSpec::build(&snapshot, &plan());
            assert_eq!(StylePillSpec::state_of(&snapshot, &plan()), spec.state());
            assert_eq!(
                StylePillSpec::visible(&snapshot, &plan()),
                !spec.controls().is_empty(),
                "visible() must equal a non-empty control list ({:?})",
                spec.state()
            );
        }
    }
}
